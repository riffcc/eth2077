//! Gate 0 verification: execute a simple ETH transfer and verify state changes.

use alloy_primitives::{address, Address, B256, U256};
use eth2077_execution::executor::BlockExecutor;
use eth2077_execution::state::{AccountInfo, InMemoryStateDB};
use eth2077_types::canonical::Transaction;

#[test]
fn test_simple_eth_transfer() {
    // Setup: create a sender with 10 ETH and a receiver with 0
    let sender: Address = address!("0x1000000000000000000000000000000000000001");
    let receiver: Address = address!("0x2000000000000000000000000000000000000002");

    let mut state = InMemoryStateDB::new();
    state.insert_account(
        sender,
        AccountInfo {
            balance: U256::from(10_000_000_000_000_000_000u128), // 10 ETH in wei
            nonce: 0,
            code_hash: B256::ZERO,
            code: None,
        },
    );

    let mut executor = BlockExecutor::new(state);

    // Create a simple ETH transfer: send 1 ETH from sender to receiver
    let tx = Transaction {
        hash: B256::ZERO,
        nonce: 0,
        from: sender,
        to: Some(receiver),
        value: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
        gas_limit: 21_000,                                 // standard ETH transfer gas
        max_fee_per_gas: 1_000_000_000,                   // 1 gwei
        max_priority_fee_per_gas: 1_000_000_000,
        input: Default::default(),
        chain_id: 1,
        signature_r: U256::ZERO,
        signature_s: U256::ZERO,
        signature_v: 0,
    };

    let receipt = executor
        .execute_tx(&tx)
        .expect("ETH transfer should succeed");

    // Verify receipt
    assert!(receipt.status, "transfer should succeed");
    assert_eq!(
        receipt.cumulative_gas_used, 21_000,
        "standard transfer uses 21k gas"
    );
    assert!(receipt.logs.is_empty(), "ETH transfer produces no logs");

    // Verify state changes
    let state = executor.state();
    let sender_info = state.get_account(&sender);
    let receiver_info = state.get_account(&receiver);

    // Receiver should have 1 ETH
    assert_eq!(
        receiver_info.balance,
        U256::from(1_000_000_000_000_000_000u128)
    );

    // Sender should have less than 9 ETH (10 - 1 ETH transfer - gas costs)
    let nine_eth = U256::from(9_000_000_000_000_000_000u128);
    assert!(
        sender_info.balance < nine_eth,
        "sender should have paid for gas too"
    );
    assert!(sender_info.balance > U256::ZERO, "sender should still have funds");

    // Sender nonce should have incremented
    assert_eq!(sender_info.nonce, 1);

    println!("Gate 0 PASS: ETH transfer executed successfully!");
    println!("  Sender balance: {} wei", sender_info.balance);
    println!("  Receiver balance: {} wei", receiver_info.balance);
    println!("  Gas used: {}", receipt.cumulative_gas_used);
}
