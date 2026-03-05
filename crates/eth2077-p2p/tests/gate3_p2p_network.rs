use std::net::SocketAddr;
use std::sync::Once;

use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use eth2077_oob_consensus::consensus::ConsensusMessage;
use eth2077_p2p::node::{P2pConfig, P2pEvent, P2pNode};
use eth2077_types::canonical::{Block, Header};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};
use tracing::info;

fn init_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });
}

fn test_addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn test_block(height: u64) -> Block {
    Block {
        header: Header {
            parent_hash: B256::ZERO,
            ommers_hash: B256::ZERO,
            beneficiary: Address::ZERO,
            state_root: B256::ZERO,
            transactions_root: B256::ZERO,
            receipts_root: B256::ZERO,
            logs_bloom: Bloom::ZERO,
            difficulty: U256::from(1u64),
            number: height,
            gas_limit: 30_000_000,
            gas_used: 0,
            timestamp: height * 12,
            extra_data: Bytes::from(vec![height as u8]),
            mix_hash: B256::ZERO,
            nonce: 0,
            base_fee_per_gas: Some(1),
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
        },
        transactions: Vec::new(),
        ommers: Vec::new(),
    }
}

async fn wait_for_connectivity(node0: &P2pNode, node1: &P2pNode, node2: &P2pNode) {
    timeout(Duration::from_secs(5), async {
        loop {
            let c0 = node0.connected_peer_count().await;
            let c1 = node1.connected_peer_count().await;
            let c2 = node2.connected_peer_count().await;

            if c0 >= 2 && c1 >= 1 && c2 >= 1 {
                break;
            }

            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("nodes failed to connect in time");
}

async fn recv_block_event(rx: &mut mpsc::Receiver<P2pEvent>, expected_hash: B256) -> Block {
    loop {
        match rx.recv().await {
            Some(P2pEvent::BlockReceived { block, .. }) if block.hash() == expected_hash => {
                return block;
            }
            Some(_) => continue,
            None => panic!("event channel closed before block event"),
        }
    }
}

async fn recv_consensus_event(
    rx: &mut mpsc::Receiver<P2pEvent>,
    expected: &ConsensusMessage,
) -> ConsensusMessage {
    loop {
        match rx.recv().await {
            Some(P2pEvent::ConsensusMessageReceived { msg, .. }) if &msg == expected => return msg,
            Some(_) => continue,
            None => panic!("event channel closed before consensus event"),
        }
    }
}

#[tokio::test]
async fn test_three_node_block_propagation() {
    init_tracing();

    let addr0 = test_addr(19000);
    let addr1 = test_addr(19001);
    let addr2 = test_addr(19002);

    let mut node0 = P2pNode::new(P2pConfig {
        listen_addr: addr0,
        boot_peers: vec![],
        peer_id: 1,
        chain_id: 2077,
        max_peers: 50,
    });
    let mut node1 = P2pNode::new(P2pConfig {
        listen_addr: addr1,
        boot_peers: vec![addr0],
        peer_id: 2,
        chain_id: 2077,
        max_peers: 50,
    });
    let mut node2 = P2pNode::new(P2pConfig {
        listen_addr: addr2,
        boot_peers: vec![addr0],
        peer_id: 3,
        chain_id: 2077,
        max_peers: 50,
    });

    let mut _rx0 = node0.start().await;
    let mut rx1 = node1.start().await;
    let mut rx2 = node2.start().await;

    wait_for_connectivity(&node0, &node1, &node2).await;

    let block = test_block(1);
    let expected_hash = block.hash();
    node0.broadcast_block(&block).await;

    let node1_block = timeout(
        Duration::from_secs(2),
        recv_block_event(&mut rx1, expected_hash),
    )
    .await
    .expect("node1 should receive block")
    .hash();
    let node2_block = timeout(
        Duration::from_secs(2),
        recv_block_event(&mut rx2, expected_hash),
    )
    .await
    .expect("node2 should receive block")
    .hash();

    assert_eq!(node1_block, expected_hash);
    assert_eq!(node2_block, expected_hash);

    info!("Gate 3 PASS: Block propagated across 3-node network");
}

#[tokio::test]
async fn test_consensus_message_relay() {
    init_tracing();

    let addr0 = test_addr(19010);
    let addr1 = test_addr(19011);
    let addr2 = test_addr(19012);

    let mut node0 = P2pNode::new(P2pConfig {
        listen_addr: addr0,
        boot_peers: vec![],
        peer_id: 11,
        chain_id: 2077,
        max_peers: 50,
    });
    let mut node1 = P2pNode::new(P2pConfig {
        listen_addr: addr1,
        boot_peers: vec![addr0],
        peer_id: 12,
        chain_id: 2077,
        max_peers: 50,
    });
    let mut node2 = P2pNode::new(P2pConfig {
        listen_addr: addr2,
        boot_peers: vec![addr0],
        peer_id: 13,
        chain_id: 2077,
        max_peers: 50,
    });

    let mut _rx0 = node0.start().await;
    let mut rx1 = node1.start().await;
    let mut rx2 = node2.start().await;

    wait_for_connectivity(&node0, &node1, &node2).await;

    let proposal = ConsensusMessage::Proposal {
        height: 1,
        round: 0,
        block_hash: [7u8; 32],
        proposer: 11,
    };

    node0.broadcast_consensus_msg(&proposal).await;

    let node1_msg = timeout(
        Duration::from_secs(2),
        recv_consensus_event(&mut rx1, &proposal),
    )
    .await
    .expect("node1 should receive consensus message");
    let node2_msg = timeout(
        Duration::from_secs(2),
        recv_consensus_event(&mut rx2, &proposal),
    )
    .await
    .expect("node2 should receive consensus message");

    assert_eq!(node1_msg, proposal);
    assert_eq!(node2_msg, proposal);

    info!("Gate 3 PASS: Consensus message relayed across 3-node network");
}
