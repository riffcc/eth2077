use std::{collections::HashMap, error::Error, net::SocketAddr, str::FromStr, sync::Arc};

use alloy_primitives::{Address, B256};
use eth2077_execution::state::InMemoryStateDB;
use eth2077_types::canonical::{Block, Log, Receipt, Transaction};
use jsonrpsee::{
    server::{ServerBuilder, ServerHandle},
    types::ErrorObjectOwned,
    RpcModule,
};
use serde_json::{json, Value};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct EthRpcState {
    blocks: Vec<Block>,
    receipts: HashMap<B256, Receipt>,
    state_db: InMemoryStateDB,
    pending_txs: Vec<Transaction>,
    chain_id: u64,
}

impl EthRpcState {
    pub fn new(chain_id: u64, genesis_block: Block, genesis_state: InMemoryStateDB) -> Self {
        Self {
            blocks: vec![genesis_block],
            receipts: HashMap::new(),
            state_db: genesis_state,
            pending_txs: Vec::new(),
            chain_id,
        }
    }

    pub fn append_block(
        &mut self,
        block: Block,
        receipts: Vec<Receipt>,
        new_state: InMemoryStateDB,
    ) {
        for receipt in receipts {
            self.receipts.insert(receipt.tx_hash, receipt);
        }
        self.blocks.push(block);
        self.state_db = new_state;
    }

    pub fn latest_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    pub fn latest_block_number(&self) -> u64 {
        self.latest_block().map(Block::number).unwrap_or(0)
    }

    pub fn take_pending_txs(&mut self) -> Vec<Transaction> {
        std::mem::take(&mut self.pending_txs)
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn get_block_by_number(&self, num: u64) -> Option<&Block> {
        self.blocks.iter().find(|block| block.number() == num)
    }
}

fn hex_u64(value: u64) -> String {
    format!("0x{value:x}")
}

fn invalid_params(message: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(-32602, message.into(), None::<()>)
}

fn parse_address(value: &str) -> Result<Address, ErrorObjectOwned> {
    Address::from_str(value).map_err(|_| invalid_params(format!("invalid address: {value}")))
}

fn parse_block_tag(tag: &str, latest: Option<u64>) -> Result<Option<u64>, ErrorObjectOwned> {
    if tag == "latest" {
        return Ok(latest);
    }

    let raw = tag
        .strip_prefix("0x")
        .ok_or_else(|| invalid_params(format!("unsupported block tag: {tag}")))?;

    if raw.is_empty() {
        return Err(invalid_params("invalid hex block number"));
    }

    u64::from_str_radix(raw, 16)
        .map(Some)
        .map_err(|_| invalid_params(format!("invalid block number: {tag}")))
}

fn log_to_json(log: &Log) -> Value {
    json!({
        "address": format!("{:#x}", log.address),
        "topics": log.topics.iter().map(|topic| format!("{topic:#x}")).collect::<Vec<_>>(),
        "data": format!("{:#x}", log.data),
    })
}

fn receipt_to_json(receipt: &Receipt) -> Value {
    json!({
        "transactionHash": format!("{:#x}", receipt.tx_hash),
        "status": if receipt.status { "0x1" } else { "0x0" },
        "cumulativeGasUsed": hex_u64(receipt.cumulative_gas_used),
        "logsBloom": format!("{:#x}", receipt.logs_bloom),
        "logs": receipt.logs.iter().map(log_to_json).collect::<Vec<_>>(),
    })
}

fn tx_to_json(tx: &Transaction) -> Value {
    json!({
        "hash": format!("{:#x}", tx.hash),
        "nonce": hex_u64(tx.nonce),
        "from": format!("{:#x}", tx.from),
        "to": tx.to.map(|addr| format!("{addr:#x}")),
        "value": format!("0x{:x}", tx.value),
        "gas": hex_u64(tx.gas_limit),
        "maxFeePerGas": hex_u64(tx.max_fee_per_gas),
        "maxPriorityFeePerGas": hex_u64(tx.max_priority_fee_per_gas),
        "input": format!("{:#x}", tx.input),
    })
}

fn block_to_json(block: &Block, full_txs: bool) -> Value {
    let transactions = if full_txs {
        block
            .transactions
            .iter()
            .map(tx_to_json)
            .collect::<Vec<_>>()
    } else {
        block
            .transactions
            .iter()
            .map(|tx| json!(format!("{:#x}", tx.hash)))
            .collect::<Vec<_>>()
    };

    json!({
        "number": hex_u64(block.header.number),
        "hash": format!("{:#x}", block.hash()),
        "parentHash": format!("{:#x}", block.header.parent_hash),
        "timestamp": hex_u64(block.header.timestamp),
        "gasLimit": hex_u64(block.header.gas_limit),
        "gasUsed": hex_u64(block.header.gas_used),
        "miner": format!("{:#x}", block.header.beneficiary),
        "baseFeePerGas": block.header.base_fee_per_gas.map(hex_u64),
        "transactions": transactions,
    })
}

pub fn build_eth_rpc_module(
    state: Arc<RwLock<EthRpcState>>,
) -> RpcModule<Arc<RwLock<EthRpcState>>> {
    let mut module = RpcModule::new(state);

    module
        .register_async_method("eth_chainId", |_, state, _| async move {
            let state = state.read().await;
            Ok::<String, ErrorObjectOwned>(hex_u64(state.chain_id()))
        })
        .expect("method names are static and unique");

    module
        .register_async_method("eth_blockNumber", |_, state, _| async move {
            let state = state.read().await;
            Ok::<String, ErrorObjectOwned>(hex_u64(state.latest_block_number()))
        })
        .expect("method names are static and unique");

    module
        .register_async_method("eth_getBalance", |params, state, _| async move {
            let (address, _block_tag): (String, Option<String>) = params.parse()?;
            let address = parse_address(&address)?;
            let state = state.read().await;
            let account = state.state_db.get_account(&address);
            Ok::<String, ErrorObjectOwned>(format!("0x{:x}", account.balance))
        })
        .expect("method names are static and unique");

    module
        .register_async_method("eth_getBlockByNumber", |params, state, _| async move {
            let (block_tag, full_txs): (String, bool) = params.parse()?;
            let state = state.read().await;
            let number = parse_block_tag(&block_tag, state.latest_block().map(Block::number))?;
            Ok::<Value, ErrorObjectOwned>(
                match number.and_then(|num| state.get_block_by_number(num)) {
                    Some(block) => block_to_json(block, full_txs),
                    None => Value::Null,
                },
            )
        })
        .expect("method names are static and unique");

    module
        .register_async_method("eth_getTransactionReceipt", |params, state, _| async move {
            let tx_hash: String = params.one()?;
            let tx_hash = B256::from_str(&tx_hash)
                .map_err(|_| invalid_params(format!("invalid tx hash: {tx_hash}")))?;

            let state = state.read().await;
            Ok::<Value, ErrorObjectOwned>(match state.receipts.get(&tx_hash) {
                Some(receipt) => receipt_to_json(receipt),
                None => Value::Null,
            })
        })
        .expect("method names are static and unique");

    module
        .register_async_method("eth_getTransactionCount", |params, state, _| async move {
            let (address, _block_tag): (String, Option<String>) = params.parse()?;
            let address = parse_address(&address)?;
            let state = state.read().await;
            let account = state.state_db.get_account(&address);
            Ok::<String, ErrorObjectOwned>(hex_u64(account.nonce))
        })
        .expect("method names are static and unique");

    module
        .register_async_method("eth_gasPrice", |_, state, _| async move {
            let state = state.read().await;
            let gas_price = state
                .latest_block()
                .and_then(|block| block.header.base_fee_per_gas)
                .unwrap_or(0);
            Ok::<String, ErrorObjectOwned>(hex_u64(gas_price))
        })
        .expect("method names are static and unique");

    module
}

pub async fn spawn_eth_rpc_server(
    bind_addr: SocketAddr,
    state: Arc<RwLock<EthRpcState>>,
) -> Result<(SocketAddr, ServerHandle), Box<dyn Error + Send + Sync>> {
    let server = ServerBuilder::default().build(bind_addr).await?;
    let local_addr = server.local_addr()?;
    let module = build_eth_rpc_module(state);
    let handle = server.start(module);
    Ok((local_addr, handle))
}
