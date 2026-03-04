use eth2077_bridge::engine_api::{Address, HexQuantity, WithdrawalV1};
use eth2077_bridge::payload_converter::Eth2077Block;

#[test]
fn execution_payload_round_trip_preserves_eth2077_block() {
    let block = Eth2077Block {
        parent_hash: [0x11; 32],
        fee_recipient: [0x22; 20],
        state_root: [0x33; 32],
        receipts_root: [0x44; 32],
        block_number: 42,
        gas_limit: 30_000_000,
        gas_used: 21_000,
        timestamp: 1_731_234_567,
        base_fee_per_gas: 1_000_000_000,
        transactions: vec![vec![0x02, 0xaa, 0xbb, 0xcc], vec![0x03, 0x01, 0x02]],
        withdrawals: vec![
            WithdrawalV1 {
                index: HexQuantity("0x1".to_string()),
                validator_index: HexQuantity("0x2".to_string()),
                address: Address("0x1111111111111111111111111111111111111111".to_string()),
                amount: HexQuantity("0x3e8".to_string()),
            },
            WithdrawalV1 {
                index: HexQuantity("0x2".to_string()),
                validator_index: HexQuantity("0x4".to_string()),
                address: Address("0x2222222222222222222222222222222222222222".to_string()),
                amount: HexQuantity("0x7d0".to_string()),
            },
        ],
    };

    let payload = block.to_execution_payload();
    let recovered = Eth2077Block::from(payload);

    assert_eq!(recovered, block);
}
