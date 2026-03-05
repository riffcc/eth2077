//! Gate 1 verification: build and validate a two-block pipeline.

use alloy_primitives::{address, keccak256, Address, U256};
use eth2077_execution::{
    block_builder::{BlockBuilder, BlockBuilderConfig},
    block_validator::BlockValidator,
    genesis::create_genesis_block,
};
use eth2077_types::canonical::Transaction;

fn transfer_tx(seed: &str, nonce: u64, from: Address, to: Address, value_wei: u128) -> Transaction {
    Transaction {
        hash: keccak256(seed.as_bytes()),
        nonce,
        from,
        to: Some(to),
        value: U256::from(value_wei),
        gas_limit: 21_000,
        max_fee_per_gas: 1_000_000_000,
        max_priority_fee_per_gas: 1_000_000_000,
        input: Default::default(),
        chain_id: 1,
        signature_r: U256::ZERO,
        signature_s: U256::ZERO,
        signature_v: 0,
    }
}

#[test]
fn test_gate1_block_pipeline() {
    let alice: Address = address!("0x1000000000000000000000000000000000000001");
    let bob: Address = address!("0x2000000000000000000000000000000000000002");
    let miner: Address = address!("0x3000000000000000000000000000000000000003");

    let initial_funds = U256::from(100_000_000_000_000_000_000u128); // 100 ETH
    let (genesis_block, genesis_state) =
        create_genesis_block(&[(alice, initial_funds), (bob, initial_funds)]);

    let builder = BlockBuilder::new(BlockBuilderConfig {
        beneficiary: miner,
        timestamp: None,
        gas_limit: None,
    });

    let block1_pre_state = genesis_state.clone();
    let block1_txs = vec![
        transfer_tx(
            "block1-tx1",
            0,
            alice,
            bob,
            1_000_000_000_000_000_000u128, // 1.0 ETH
        ),
        transfer_tx(
            "block1-tx2",
            0,
            bob,
            alice,
            400_000_000_000_000_000u128, // 0.4 ETH
        ),
        transfer_tx(
            "block1-tx3",
            1,
            alice,
            bob,
            200_000_000_000_000_000u128, // 0.2 ETH
        ),
    ];
    let built1 = builder
        .build_block(&genesis_block.header, block1_txs, genesis_state)
        .expect("block 1 should build");

    BlockValidator::validate_block(&built1.block, &block1_pre_state)
        .expect("block 1 should validate");

    let block2_pre_state = built1.state.clone();
    let block2_txs = vec![
        transfer_tx(
            "block2-tx1",
            1,
            bob,
            alice,
            300_000_000_000_000_000u128, // 0.3 ETH
        ),
        transfer_tx(
            "block2-tx2",
            2,
            alice,
            bob,
            100_000_000_000_000_000u128, // 0.1 ETH
        ),
    ];
    let built2 = builder
        .build_block(&built1.block.header, block2_txs, built1.state)
        .expect("block 2 should build");

    BlockValidator::validate_block(&built2.block, &block2_pre_state)
        .expect("block 2 should validate");

    assert_eq!(
        built2.block.header.parent_hash,
        built1.block.hash(),
        "block 2 parent_hash must reference block 1 hash"
    );

    let final_state = built2.state;
    let alice_info = final_state.get_account(&alice);
    let bob_info = final_state.get_account(&bob);

    println!("Gate 1 PASS: block pipeline executed and validated.");
    println!("  Genesis hash:   {:#x}", genesis_block.hash());
    println!("  Block 1 hash:   {:#x}", built1.block.hash());
    println!("  Block 2 hash:   {:#x}", built2.block.hash());
    println!("  Block 1 txs:    {}", built1.block.transactions.len());
    println!("  Block 2 txs:    {}", built2.block.transactions.len());
    println!("  Block 1 receipts: {}", built1.receipts.len());
    println!("  Block 2 receipts: {}", built2.receipts.len());
    println!("  Alice nonce:    {}", alice_info.nonce);
    println!("  Bob nonce:      {}", bob_info.nonce);
    println!("  Alice balance:  {} wei", alice_info.balance);
    println!("  Bob balance:    {} wei", bob_info.balance);
}
