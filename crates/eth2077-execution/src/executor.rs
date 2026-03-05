//! revm-backed transaction execution over the in-memory state DB.

use std::convert::Infallible;

use alloy_primitives::logs_bloom;
use eth2077_types::canonical::{Log as CanonicalLog, Receipt, Transaction};
use revm::{
    context::{result::ExecutionResult, Context, TxEnv},
    primitives::{Address, Bytes, TxKind, B256, U256},
    state::{Account, AccountInfo as RevmAccountInfo, Bytecode, EvmState},
    Database, ExecuteEvm, MainBuilder, MainContext,
};

use crate::state::{AccountInfo, InMemoryStateDB};

/// Errors returned by EVM execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    EvmError(String),
    InvalidTransaction(String),
}

impl core::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EvmError(msg) => write!(f, "evm execution error: {msg}"),
            Self::InvalidTransaction(msg) => write!(f, "invalid transaction: {msg}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Database adapter that exposes [`InMemoryStateDB`] through revm's [`Database`] trait.
pub struct RevmDatabase<'a> {
    state: &'a mut InMemoryStateDB,
}

impl<'a> RevmDatabase<'a> {
    pub fn new(state: &'a mut InMemoryStateDB) -> Self {
        Self { state }
    }
}

impl Database for RevmDatabase<'_> {
    type Error = Infallible;

    fn basic(&mut self, address: Address) -> Result<Option<RevmAccountInfo>, Self::Error> {
        if !self.state.account_exists(&address) {
            return Ok(None);
        }

        let account = self.state.get_account(&address);
        let code = account.code.map(|raw| Bytecode::new_raw(Bytes::from(raw)));
        Ok(Some(RevmAccountInfo {
            balance: account.balance,
            nonce: account.nonce,
            code_hash: account.code_hash,
            account_id: None,
            code,
        }))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        for address in self.state.accounts() {
            let account = self.state.get_account(address);
            if account.code_hash == code_hash {
                if let Some(code) = account.code {
                    return Ok(Bytecode::new_raw(Bytes::from(code)));
                }
            }
        }

        Ok(Bytecode::default())
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self.state.get_storage(&address, &index))
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        Ok(self.state.get_block_hash(number))
    }
}

/// Executes transactions against in-memory state and commits revm state transitions.
#[derive(Debug, Clone, Default)]
pub struct BlockExecutor {
    state: InMemoryStateDB,
}

impl BlockExecutor {
    pub fn new(state: InMemoryStateDB) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &InMemoryStateDB {
        &self.state
    }

    pub fn into_state(self) -> InMemoryStateDB {
        self.state
    }

    /// Execute a single transaction and commit resulting state changes.
    pub fn execute_tx(&mut self, tx: &Transaction) -> Result<Receipt, ExecutionError> {
        if tx.max_priority_fee_per_gas > tx.max_fee_per_gas {
            return Err(ExecutionError::InvalidTransaction(
                "max_priority_fee_per_gas exceeds max_fee_per_gas".to_string(),
            ));
        }
        if tx.gas_limit == 0 {
            return Err(ExecutionError::InvalidTransaction(
                "gas_limit must be greater than zero".to_string(),
            ));
        }

        let tx_kind = match tx.to {
            Some(to) => TxKind::Call(to),
            None => TxKind::Create,
        };
        let tx_env = TxEnv::builder()
            .caller(tx.from)
            .gas_limit(tx.gas_limit)
            .max_fee_per_gas(u128::from(tx.max_fee_per_gas))
            .gas_priority_fee(Some(u128::from(tx.max_priority_fee_per_gas)))
            .kind(tx_kind)
            .value(tx.value)
            .data(tx.input.clone())
            .nonce(tx.nonce)
            .chain_id(Some(tx.chain_id))
            .build()
            .map_err(|err| ExecutionError::InvalidTransaction(format!("{err:?}")))?;

        let db = RevmDatabase::new(&mut self.state);
        let mut evm = Context::mainnet().with_db(db).build_mainnet();
        let result_and_state = evm
            .transact(tx_env)
            .map_err(|err| ExecutionError::EvmError(err.to_string()))?;

        let result = result_and_state.result;
        if let ExecutionResult::Halt { reason, .. } = &result {
            return Err(ExecutionError::EvmError(format!(
                "transaction halted: {reason:?}"
            )));
        }

        let revm_logs = result.logs();
        let receipt_logs = revm_logs
            .iter()
            .map(|log| CanonicalLog {
                address: log.address,
                topics: log.data.topics().to_vec(),
                data: log.data.data.clone(),
            })
            .collect::<Vec<_>>();
        let bloom = logs_bloom(revm_logs.iter());

        self.apply_state_changes(result_and_state.state);

        Ok(Receipt {
            tx_hash: tx.hash,
            status: result.is_success(),
            cumulative_gas_used: result.gas_used(),
            logs_bloom: bloom,
            logs: receipt_logs,
        })
    }

    fn apply_state_changes(&mut self, state: EvmState) {
        for (address, account) in state {
            self.apply_account_change(address, &account);
        }
    }

    fn apply_account_change(&mut self, address: Address, account: &Account) {
        if account.is_selfdestructed() {
            self.state.remove_account(&address);
            return;
        }

        let info = &account.info;
        let code = info
            .code
            .as_ref()
            .map(|bytecode| bytecode.original_bytes().to_vec());
        self.state.insert_account(
            address,
            AccountInfo {
                balance: info.balance,
                nonce: info.nonce,
                code_hash: info.code_hash,
                code,
            },
        );

        if account.is_created() {
            self.state.clear_storage(&address);
        }

        for (slot, value) in account.changed_storage_slots() {
            self.state
                .set_storage(address, *slot, value.present_value());
        }
    }
}
