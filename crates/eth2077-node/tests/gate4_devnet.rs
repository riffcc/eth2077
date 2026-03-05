use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use alloy_primitives::{address, Address, U256};
use eth2077_bridge::eth_rpc::{spawn_eth_rpc_server, EthRpcState};
use eth2077_execution::block_builder::{BlockBuilder, BlockBuilderConfig, BuiltBlock};
use eth2077_execution::genesis::create_genesis_block;
use eth2077_execution::state::InMemoryStateDB;
use eth2077_oob_consensus::consensus::{ConsensusEngine, ConsensusEvent, ConsensusMessage};
use eth2077_oob_consensus::fast_path::FastPathConfig;
use eth2077_oob_consensus::validator::{Validator, ValidatorSet};
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::HttpClientBuilder;
use serde_json::Value;
use tokio::sync::RwLock;

const LOCAL_VALIDATOR: u64 = 0;

#[tokio::test]
async fn test_devnet_produces_blocks_and_serves_rpc() {
    let funded_address = address!("0x1111111111111111111111111111111111111111");
    let funded_balance = U256::from(1_000_000_000_000_000_000_000u128);
    let allocs = vec![(funded_address, funded_balance)];
    let (genesis_block, genesis_state) = create_genesis_block(&allocs);

    let chain_id = 2_077u64;
    let rpc_state = Arc::new(RwLock::new(EthRpcState::new(
        chain_id,
        genesis_block.clone(),
        genesis_state.clone(),
    )));

    // Start a real JSON-RPC server on a random port
    let bind = SocketAddr::from(([127, 0, 0, 1], 0));
    let (rpc_addr, _rpc_handle) = spawn_eth_rpc_server(bind, rpc_state.clone())
        .await
        .expect("RPC server must start");

    let client = HttpClientBuilder::default()
        .build(format!("http://{rpc_addr}"))
        .expect("http client must build");

    // Verify genesis state via RPC
    let chain_id_hex: String = client
        .request("eth_chainId", jsonrpsee::rpc_params![])
        .await
        .expect("eth_chainId should work");
    assert_eq!(chain_id_hex, "0x81d", "chain ID should be 2077 = 0x81d");

    let block_number: String = client
        .request("eth_blockNumber", jsonrpsee::rpc_params![])
        .await
        .expect("eth_blockNumber should work");
    assert_eq!(block_number, "0x0", "genesis block number should be 0");

    // Set up single-validator consensus
    let validator_set = ValidatorSet::new(vec![Validator {
        index: LOCAL_VALIDATOR,
        weight: 1,
        public_key: [0; 32],
    }]);
    let fast_path = FastPathConfig {
        quorum_threshold: 1,
        timeout_ms: 5_000,
        optimistic_threshold: 1,
    };
    let mut consensus = ConsensusEngine::new(validator_set, fast_path);

    let mut execution_state = genesis_state;
    let mut proposed_blocks: HashMap<[u8; 32], BuiltBlock> = HashMap::new();
    let mut current_height = genesis_block.number() + 1;
    let blocks_to_produce = 5u64;

    // Produce 5 blocks through the full consensus + execution pipeline
    for _ in 0..blocks_to_produce {
        let mut events = consensus.start_height(current_height);

        let parent_header = {
            let state = rpc_state.read().await;
            state
                .latest_block()
                .expect("latest block should always exist")
                .header
                .clone()
        };

        let builder = BlockBuilder::new(BlockBuilderConfig {
            beneficiary: Address::ZERO,
            timestamp: None,
            gas_limit: Some(parent_header.gas_limit),
        });

        let built = builder
            .build_block(&parent_header, Vec::new(), execution_state.clone())
            .expect("block build should succeed");
        let block_hash: [u8; 32] = built.block.hash().into();
        proposed_blocks.insert(block_hash, built);

        let proposal = ConsensusMessage::Proposal {
            height: current_height,
            round: 0,
            block_hash,
            proposer: LOCAL_VALIDATOR,
        };

        events.extend(consensus.on_message(proposal));

        let finalized = process_consensus_events(
            events,
            &mut consensus,
            &rpc_state,
            &mut proposed_blocks,
            &mut execution_state,
            LOCAL_VALIDATOR,
        )
        .await;

        assert_eq!(
            finalized,
            Some(current_height),
            "height {current_height} should finalize"
        );
        current_height += 1;
    }

    // Verify post-production state via RPC
    let block_number: String = client
        .request("eth_blockNumber", jsonrpsee::rpc_params![])
        .await
        .expect("eth_blockNumber should work after block production");
    assert_eq!(block_number, "0x5", "should have produced 5 blocks");

    let balance: String = client
        .request(
            "eth_getBalance",
            jsonrpsee::rpc_params![format!("{funded_address:#x}"), "latest"],
        )
        .await
        .expect("eth_getBalance should work");
    assert_eq!(
        balance,
        format!("0x{:x}", funded_balance),
        "funded address balance should be preserved"
    );

    let latest_block: Value = client
        .request(
            "eth_getBlockByNumber",
            jsonrpsee::rpc_params!["latest", false],
        )
        .await
        .expect("eth_getBlockByNumber should work");
    assert!(latest_block.is_object(), "latest block should be an object");
    assert_eq!(latest_block["number"], "0x5");
    assert!(
        latest_block.get("hash").is_some(),
        "block hash should exist"
    );

    let gas_price: String = client
        .request("eth_gasPrice", jsonrpsee::rpc_params![])
        .await
        .expect("eth_gasPrice should work");
    assert!(gas_price.starts_with("0x"), "gas price should be hex");
}

async fn process_consensus_events(
    mut events: Vec<ConsensusEvent>,
    consensus: &mut ConsensusEngine,
    rpc_state: &Arc<RwLock<EthRpcState>>,
    proposed_blocks: &mut HashMap<[u8; 32], BuiltBlock>,
    execution_state: &mut InMemoryStateDB,
    local_validator: u64,
) -> Option<u64> {
    let mut finalized_height = None;

    while let Some(event) = events.pop() {
        match event {
            ConsensusEvent::NeedProposal { .. } => {}
            ConsensusEvent::RoundTimeout { .. } => panic!("unexpected round timeout"),
            ConsensusEvent::SendPrevote {
                height,
                round,
                block_hash,
            } => {
                let msg = ConsensusMessage::Prevote {
                    height,
                    round,
                    block_hash,
                    voter: local_validator,
                };
                let mut next = consensus.on_message(msg);
                events.append(&mut next);
            }
            ConsensusEvent::SendPrecommit {
                height,
                round,
                block_hash,
            } => {
                let msg = ConsensusMessage::Precommit {
                    height,
                    round,
                    block_hash,
                    voter: local_validator,
                };
                let mut next = consensus.on_message(msg);
                events.append(&mut next);
            }
            ConsensusEvent::BlockFinalized {
                height, block_hash, ..
            } => {
                let built = proposed_blocks
                    .remove(&block_hash)
                    .expect("finalized block hash must be in proposal cache");
                *execution_state = built.state.clone();
                {
                    let mut state = rpc_state.write().await;
                    state.append_block(built.block, built.receipts, built.state);
                }
                finalized_height = Some(height);
            }
        }
    }

    finalized_height
}
