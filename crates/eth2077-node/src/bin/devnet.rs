use std::collections::HashMap;
use std::env;
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
use eth2077_p2p::node::{P2pConfig, P2pEvent, P2pNode};
use eth2077_types::canonical::Header;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
struct DevnetConfig {
    peer_id: u64,
    listen_port: u16,
    rpc_port: u16,
    boot_peers: Vec<SocketAddr>,
    block_time_ms: u64,
    chain_id: u64,
}

impl DevnetConfig {
    fn from_env() -> Self {
        Self {
            peer_id: parse_env("PEER_ID", 0u64),
            listen_port: parse_env("LISTEN_PORT", 30_303u16),
            rpc_port: parse_env("RPC_PORT", 8_545u16),
            boot_peers: parse_boot_peers(),
            block_time_ms: parse_env("BLOCK_TIME_MS", 2_000u64),
            chain_id: parse_env("CHAIN_ID", 2_077u64),
        }
    }

    fn listen_addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.listen_port))
    }

    fn rpc_bind(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.rpc_port))
    }
}

fn parse_env<T>(name: &str, default: T) -> T
where
    T: core::str::FromStr,
{
    match env::var(name) {
        Ok(value) => match value.parse::<T>() {
            Ok(parsed) => parsed,
            Err(_) => {
                warn!(env_var = name, value = %value, "invalid env var value; using default");
                default
            }
        },
        Err(_) => default,
    }
}

fn parse_boot_peers() -> Vec<SocketAddr> {
    let Ok(raw) = env::var("BOOT_PEERS") else {
        return Vec::new();
    };

    raw.split(',')
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return None;
            }

            match trimmed.parse::<SocketAddr>() {
                Ok(addr) => Some(addr),
                Err(_) => {
                    warn!(value = trimmed, "invalid boot peer address; skipping");
                    None
                }
            }
        })
        .collect()
}

async fn process_consensus_events(
    mut events: Vec<ConsensusEvent>,
    consensus: &mut ConsensusEngine,
    p2p_node: &P2pNode,
    rpc_state: &Arc<RwLock<EthRpcState>>,
    proposed_blocks: &mut HashMap<[u8; 32], BuiltBlock>,
    local_state: &mut InMemoryStateDB,
    local_validator: u64,
) {
    while let Some(event) = events.pop() {
        match event {
            ConsensusEvent::NeedProposal {
                height,
                round,
                leader,
            } => {
                debug!(height, round, leader, "consensus requires proposal");
            }
            ConsensusEvent::RoundTimeout { height, round } => {
                warn!(height, round, "consensus round timeout");
            }
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
                p2p_node.broadcast_consensus_msg(&msg).await;
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
                p2p_node.broadcast_consensus_msg(&msg).await;
                let mut next = consensus.on_message(msg);
                events.append(&mut next);
            }
            ConsensusEvent::BlockFinalized {
                height,
                block_hash,
                round,
            } => {
                info!(height, round, hash = %format_args!("0x{}", hex::encode(block_hash)), "block finalized");

                if let Some(built) = proposed_blocks.remove(&block_hash) {
                    *local_state = built.state.clone();

                    {
                        let mut state = rpc_state.write().await;
                        state.append_block(built.block.clone(), built.receipts, built.state);
                    }

                    p2p_node.broadcast_block(&built.block).await;
                } else {
                    warn!(height, hash = %format_args!("0x{}", hex::encode(block_hash)), "finalized hash not found in local proposal cache");
                }

                let mut next = consensus.start_height(height.saturating_add(1));
                events.append(&mut next);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = DevnetConfig::from_env();

    info!(
        peer_id = config.peer_id,
        listen_port = config.listen_port,
        rpc_port = config.rpc_port,
        block_time_ms = config.block_time_ms,
        chain_id = config.chain_id,
        boot_peers = ?config.boot_peers,
        "starting eth2077 devnet node"
    );

    let allocs = vec![
        (
            address!("0x1111111111111111111111111111111111111111"),
            U256::from(1_000_000_000_000_000_000_000u128),
        ),
        (
            address!("0x2222222222222222222222222222222222222222"),
            U256::from(1_000_000_000_000_000_000_000u128),
        ),
    ];

    let (genesis_block, genesis_state) = create_genesis_block(&allocs);
    let mut execution_state = genesis_state.clone();

    let rpc_state = Arc::new(RwLock::new(EthRpcState::new(
        config.chain_id,
        genesis_block.clone(),
        genesis_state.clone(),
    )));

    let (rpc_addr, _rpc_handle) =
        match spawn_eth_rpc_server(config.rpc_bind(), rpc_state.clone()).await {
            Ok(server) => server,
            Err(error) => {
                error!(%error, "failed to start JSON-RPC server");
                return;
            }
        };
    info!(%rpc_addr, "JSON-RPC server started");

    let p2p_config = P2pConfig {
        listen_addr: config.listen_addr(),
        boot_peers: config.boot_peers.clone(),
        peer_id: config.peer_id,
        chain_id: config.chain_id,
        max_peers: 50,
    };

    let mut p2p_node = P2pNode::new(p2p_config);
    let mut p2p_events = p2p_node.start().await;
    info!("P2P node started");

    let validators = vec![Validator {
        index: config.peer_id,
        weight: 1,
        public_key: [0u8; 32],
    }];
    let validator_set = ValidatorSet::new(validators);

    let fast_path_config = FastPathConfig {
        quorum_threshold: 1,
        timeout_ms: 5_000,
        optimistic_threshold: 1,
    };

    let mut consensus = ConsensusEngine::new(validator_set.clone(), fast_path_config);
    let mut proposed_blocks: HashMap<[u8; 32], BuiltBlock> = HashMap::new();

    let initial_events = consensus.start_height(genesis_block.number().saturating_add(1));
    process_consensus_events(
        initial_events,
        &mut consensus,
        &p2p_node,
        &rpc_state,
        &mut proposed_blocks,
        &mut execution_state,
        config.peer_id,
    )
    .await;

    let mut block_timer = interval(Duration::from_millis(config.block_time_ms.max(100)));

    loop {
        tokio::select! {
            _ = block_timer.tick() => {
                let height = consensus.current_height();
                let round = consensus.current_round();
                let leader = validator_set.leader_for_round(height, round);

                if leader != config.peer_id {
                    debug!(height, round, leader, "not leader for current round; skipping proposal");
                    continue;
                }

                let (parent_header, pending_txs): (Header, Vec<_>) = {
                    let mut state = rpc_state.write().await;
                    let Some(parent) = state.latest_block().cloned() else {
                        warn!("missing latest block in rpc state; cannot build proposal");
                        continue;
                    };
                    (parent.header, state.take_pending_txs())
                };

                let builder = BlockBuilder::new(BlockBuilderConfig {
                    beneficiary: Address::ZERO,
                    timestamp: None,
                    gas_limit: Some(parent_header.gas_limit),
                });

                let built = match builder.build_block(&parent_header, pending_txs, execution_state.clone()) {
                    Ok(built) => built,
                    Err(error) => {
                        error!(%error, height, round, "failed to build block");
                        continue;
                    }
                };

                let block_hash = built.block.hash();
                let block_hash_bytes: [u8; 32] = block_hash.into();
                let block_number = built.block.number();
                proposed_blocks.insert(block_hash_bytes, built);

                info!(
                    height,
                    round,
                    number = block_number,
                    hash = %format_args!("{block_hash:#x}"),
                    "proposing block"
                );

                let proposal = ConsensusMessage::Proposal {
                    height,
                    round,
                    block_hash: block_hash_bytes,
                    proposer: config.peer_id,
                };

                p2p_node.broadcast_consensus_msg(&proposal).await;

                let events = consensus.on_message(proposal);
                process_consensus_events(
                    events,
                    &mut consensus,
                    &p2p_node,
                    &rpc_state,
                    &mut proposed_blocks,
                    &mut execution_state,
                    config.peer_id,
                )
                .await;
            }
            maybe_event = p2p_events.recv() => {
                let Some(event) = maybe_event else {
                    warn!("p2p event channel closed; shutting down");
                    break;
                };

                match event {
                    P2pEvent::BlockReceived { from_peer, block } => {
                        let block_hash = block.hash();
                        execution_state.insert_block_hash(block.number(), block_hash);

                        {
                            let mut state = rpc_state.write().await;
                            state.append_block((*block).clone(), Vec::new(), execution_state.clone());
                        }

                        info!(
                            from_peer,
                            number = block.number(),
                            hash = %format_args!("{block_hash:#x}"),
                            "accepted block from peer"
                        );
                    }
                    P2pEvent::ConsensusMessageReceived { from_peer, msg } => {
                        debug!(from_peer, "received consensus message from peer");
                        let events = consensus.on_message(msg);
                        process_consensus_events(
                            events,
                            &mut consensus,
                            &p2p_node,
                            &rpc_state,
                            &mut proposed_blocks,
                            &mut execution_state,
                            config.peer_id,
                        )
                        .await;
                    }
                    P2pEvent::TransactionReceived { from_peer, tx } => {
                        debug!(from_peer, tx_hash = %format_args!("{:#x}", tx.hash), "received tx from peer (not queued in local mempool)");
                    }
                    P2pEvent::PeerConnected { peer_id } => {
                        info!(peer_id, "peer connected");
                    }
                    P2pEvent::PeerDisconnected { peer_id } => {
                        info!(peer_id, "peer disconnected");
                    }
                    P2pEvent::SyncComplete { height } => {
                        info!(height, "p2p sync complete");
                    }
                }
            }
        }
    }
}
