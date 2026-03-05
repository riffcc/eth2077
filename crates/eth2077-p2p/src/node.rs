use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;

use eth2077_oob_consensus::consensus::ConsensusMessage;
use eth2077_types::canonical::{Block, Transaction};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

use crate::codec::WireMessage;
use crate::gossip::{GossipConfig, GossipEngine};
use crate::peer::{PeerInfo, PeerManager, PeerState};
use crate::sync_protocol::{SyncEngine, SyncState};
use crate::transport::{Connection, GossipTransport, Transport, TransportConfig};

#[derive(Debug, Clone)]
pub struct P2pConfig {
    pub listen_addr: SocketAddr,
    pub boot_peers: Vec<SocketAddr>,
    pub peer_id: u64,
    pub chain_id: u64,
    pub max_peers: usize,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 30303)),
            boot_peers: Vec::new(),
            peer_id: 0,
            chain_id: 1,
            max_peers: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub enum P2pEvent {
    BlockReceived {
        from_peer: u64,
        block: Box<Block>,
    },
    TransactionReceived {
        from_peer: u64,
        tx: Box<Transaction>,
    },
    ConsensusMessageReceived {
        from_peer: u64,
        msg: ConsensusMessage,
    },
    PeerConnected {
        peer_id: u64,
    },
    PeerDisconnected {
        peer_id: u64,
    },
    SyncComplete {
        height: u64,
    },
}

#[derive(Debug)]
enum NodeInbound {
    PeerConnected {
        peer_id: u64,
        addr: SocketAddr,
        best_height: u64,
        best_hash: [u8; 32],
        sender: mpsc::Sender<WireMessage>,
    },
    PeerDisconnected {
        peer_id: u64,
    },
    Message {
        from_peer: u64,
        message: WireMessage,
    },
}

#[derive(Debug, Default)]
struct ConsensusRelayCache {
    seen: HashSet<[u8; 32]>,
    order: VecDeque<[u8; 32]>,
    max_size: usize,
}

impl ConsensusRelayCache {
    fn new(max_size: usize) -> Self {
        Self {
            seen: HashSet::new(),
            order: VecDeque::new(),
            max_size,
        }
    }

    fn should_relay(&mut self, payload: &[u8]) -> bool {
        let digest = Sha256::digest(payload);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&digest);

        if self.seen.contains(&hash) {
            return false;
        }

        self.seen.insert(hash);
        self.order.push_back(hash);
        while self.seen.len() > self.max_size {
            if let Some(oldest) = self.order.pop_front() {
                self.seen.remove(&oldest);
            }
        }

        true
    }
}

pub struct P2pNode<T: Transport = GossipTransport> {
    config: P2pConfig,
    transport: T,
    peer_manager: Arc<Mutex<PeerManager>>,
    gossip_engine: Arc<Mutex<GossipEngine>>,
    sync_engine: Arc<Mutex<SyncEngine>>,
    peer_senders: Arc<RwLock<HashMap<u64, mpsc::Sender<WireMessage>>>>,
    consensus_cache: Arc<Mutex<ConsensusRelayCache>>,
    started: bool,
}

impl P2pNode<GossipTransport> {
    pub fn new(config: P2pConfig) -> Self {
        let transport = GossipTransport::bind(TransportConfig {
            listen_addr: config.listen_addr,
            max_peers: config.max_peers,
        })
        .unwrap_or_else(|error| {
            panic!(
                "failed to bind p2p listener on {}: {error}",
                config.listen_addr
            )
        });

        Self::with_transport(config, transport)
    }
}

impl<T: Transport> P2pNode<T> {
    pub fn with_transport(config: P2pConfig, transport: T) -> Self {
        Self {
            peer_manager: Arc::new(Mutex::new(PeerManager::new(config.max_peers))),
            gossip_engine: Arc::new(Mutex::new(GossipEngine::new(GossipConfig::default()))),
            sync_engine: Arc::new(Mutex::new(SyncEngine::default())),
            peer_senders: Arc::new(RwLock::new(HashMap::new())),
            consensus_cache: Arc::new(Mutex::new(ConsensusRelayCache::new(1024))),
            config,
            transport,
            started: false,
        }
    }

    pub async fn start(&mut self) -> mpsc::Receiver<P2pEvent> {
        if self.started {
            warn!("p2p node start called multiple times; returning closed receiver");
            let (_event_tx, event_rx) = mpsc::channel(1);
            return event_rx;
        }
        self.started = true;

        info!(
            peer_id = self.config.peer_id,
            chain_id = self.config.chain_id,
            listen_addr = %self.config.listen_addr,
            "starting p2p node"
        );

        let (event_tx, event_rx) = mpsc::channel(256);
        let (inbound_tx, inbound_rx) = mpsc::channel(1024);

        Self::spawn_dispatch_loop(
            inbound_rx,
            event_tx,
            self.peer_manager.clone(),
            self.peer_senders.clone(),
            self.gossip_engine.clone(),
            self.sync_engine.clone(),
            self.consensus_cache.clone(),
        );

        self.spawn_accept_loop(inbound_tx.clone());
        self.spawn_bootstrap_dials(inbound_tx);

        event_rx
    }

    pub async fn broadcast_block(&self, block: &Block) {
        let message = {
            let mut gossip_engine = self.gossip_engine.lock().await;
            gossip_engine.on_new_block(block)
        };

        if let Some(message) = message {
            Self::broadcast_wire(self.peer_senders.clone(), message, None).await;
        }
    }

    pub async fn broadcast_tx(&self, tx: &Transaction) {
        let message = {
            let mut gossip_engine = self.gossip_engine.lock().await;
            gossip_engine.on_new_tx(tx)
        };

        if let Some(message) = message {
            Self::broadcast_wire(self.peer_senders.clone(), message, None).await;
        }
    }

    pub async fn broadcast_consensus_msg(&self, msg: &ConsensusMessage) {
        let msg_data = match serde_json::to_vec(msg) {
            Ok(bytes) => bytes,
            Err(error) => {
                debug!(%error, "failed to serialize consensus message");
                return;
            }
        };

        let should_relay = {
            let mut cache = self.consensus_cache.lock().await;
            cache.should_relay(&msg_data)
        };

        if !should_relay {
            return;
        }

        let wire = WireMessage::ConsensusMsg { msg_data };
        Self::broadcast_wire(self.peer_senders.clone(), wire, None).await;
    }

    pub async fn connected_peer_count(&self) -> usize {
        self.peer_manager.lock().await.connected_peers().len()
    }

    fn spawn_accept_loop(&self, inbound_tx: mpsc::Sender<NodeInbound>) {
        let transport = self.transport.clone();
        let sync_engine = self.sync_engine.clone();
        let peer_id = self.config.peer_id;
        let chain_id = self.config.chain_id;

        tokio::spawn(async move {
            loop {
                match transport.accept().await {
                    Ok(connection) => {
                        Self::spawn_connection_task(
                            connection,
                            transport.clone(),
                            inbound_tx.clone(),
                            sync_engine.clone(),
                            peer_id,
                            chain_id,
                        );
                    }
                    Err(error) => {
                        debug!(%error, "accept loop error");
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
    }

    fn spawn_bootstrap_dials(&self, inbound_tx: mpsc::Sender<NodeInbound>) {
        for boot_peer in self.config.boot_peers.clone() {
            if boot_peer == self.config.listen_addr {
                continue;
            }

            let transport = self.transport.clone();
            let sync_engine = self.sync_engine.clone();
            let peer_id = self.config.peer_id;
            let chain_id = self.config.chain_id;
            let inbound_tx_clone = inbound_tx.clone();

            tokio::spawn(async move {
                const MAX_DIAL_ATTEMPTS: usize = 20;

                for attempt in 1..=MAX_DIAL_ATTEMPTS {
                    match transport.connect(boot_peer).await {
                        Ok(connection) => {
                            info!(%boot_peer, attempt, "connected to boot peer");
                            Self::spawn_connection_task(
                                connection,
                                transport.clone(),
                                inbound_tx_clone.clone(),
                                sync_engine.clone(),
                                peer_id,
                                chain_id,
                            );
                            return;
                        }
                        Err(error) => {
                            debug!(%boot_peer, attempt, %error, "failed to dial boot peer");
                            sleep(Duration::from_millis(100)).await;
                        }
                    }
                }

                warn!(%boot_peer, "exhausted boot peer dial attempts");
            });
        }
    }

    fn spawn_connection_task(
        connection: Connection,
        transport: T,
        inbound_tx: mpsc::Sender<NodeInbound>,
        sync_engine: Arc<Mutex<SyncEngine>>,
        local_peer_id: u64,
        chain_id: u64,
    ) {
        tokio::spawn(async move {
            let peer_addr = connection.peer_addr();
            let mut framed = connection.into_framed();

            let best_height = { sync_engine.lock().await.local_height };
            let hello = WireMessage::Hello {
                peer_id: local_peer_id,
                chain_id,
                best_height,
                best_hash: [0u8; 32],
            };

            if let Err(error) = transport.send(&mut framed, hello).await {
                debug!(%error, ?peer_addr, "failed to send hello");
                transport.disconnect(peer_addr).await;
                return;
            }

            let (sender, mut receiver) = mpsc::channel::<WireMessage>(256);
            let mut remote_peer_id: Option<u64> = None;

            loop {
                tokio::select! {
                    outbound = receiver.recv() => {
                        match outbound {
                            Some(message) => {
                                if let Err(error) = transport.send(&mut framed, message).await {
                                    debug!(%error, ?peer_addr, "failed to send message to peer");
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    inbound = transport.recv(&mut framed) => {
                        match inbound {
                            Ok(Some(message)) => {
                                match message {
                                    WireMessage::Hello {
                                        peer_id,
                                        chain_id: remote_chain_id,
                                        best_height,
                                        best_hash,
                                    } => {
                                        if remote_chain_id != chain_id {
                                            warn!(
                                                expected_chain = chain_id,
                                                received_chain = remote_chain_id,
                                                remote_peer_id = peer_id,
                                                "dropping peer on mismatched chain id"
                                            );
                                            break;
                                        }

                                        if remote_peer_id.is_none() {
                                            remote_peer_id = Some(peer_id);
                                            let _ = inbound_tx
                                                .send(NodeInbound::PeerConnected {
                                                    peer_id,
                                                    addr: peer_addr,
                                                    best_height,
                                                    best_hash,
                                                    sender: sender.clone(),
                                                })
                                                .await;
                                        }
                                    }
                                    other => {
                                        if let Some(peer_id) = remote_peer_id {
                                            let _ = inbound_tx
                                                .send(NodeInbound::Message {
                                                    from_peer: peer_id,
                                                    message: other,
                                                })
                                                .await;
                                        } else {
                                            debug!(?peer_addr, "received non-hello message before handshake");
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                debug!(%error, ?peer_addr, "failed to decode frame");
                                break;
                            }
                            Ok(None) => break,
                        }
                    }
                }
            }

            transport.disconnect(peer_addr).await;
            if let Some(peer_id) = remote_peer_id {
                let _ = inbound_tx
                    .send(NodeInbound::PeerDisconnected { peer_id })
                    .await;
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_dispatch_loop(
        mut inbound_rx: mpsc::Receiver<NodeInbound>,
        event_tx: mpsc::Sender<P2pEvent>,
        peer_manager: Arc<Mutex<PeerManager>>,
        peer_senders: Arc<RwLock<HashMap<u64, mpsc::Sender<WireMessage>>>>,
        gossip_engine: Arc<Mutex<GossipEngine>>,
        sync_engine: Arc<Mutex<SyncEngine>>,
        consensus_cache: Arc<Mutex<ConsensusRelayCache>>,
    ) {
        tokio::spawn(async move {
            while let Some(inbound) = inbound_rx.recv().await {
                match inbound {
                    NodeInbound::PeerConnected {
                        peer_id,
                        addr,
                        best_height,
                        best_hash,
                        sender,
                    } => {
                        let mut info = PeerInfo::new(peer_id, addr, PeerState::Connected);
                        info.best_height = best_height;
                        info.best_hash = best_hash;

                        let accepted = {
                            let mut manager = peer_manager.lock().await;
                            manager.add_peer(info)
                        };

                        if !accepted {
                            warn!(peer_id, "peer manager is full; ignoring peer");
                            continue;
                        }

                        peer_senders.write().await.insert(peer_id, sender.clone());
                        let _ = event_tx.send(P2pEvent::PeerConnected { peer_id }).await;

                        let sync_request = {
                            let mut sync = sync_engine.lock().await;
                            if sync.needs_sync(best_height) {
                                match sync.state {
                                    SyncState::Idle | SyncState::Synced => {
                                        sync.start_sync(best_height)
                                    }
                                    SyncState::Syncing { target_height, .. }
                                        if best_height > target_height =>
                                    {
                                        sync.start_sync(best_height)
                                    }
                                    _ => {}
                                }
                                sync.next_request()
                            } else {
                                None
                            }
                        };

                        if let Some(request) = sync_request {
                            let _ = sender.send(request).await;
                        }
                    }
                    NodeInbound::PeerDisconnected { peer_id } => {
                        peer_manager.lock().await.remove_peer(peer_id);
                        peer_senders.write().await.remove(&peer_id);
                        let _ = event_tx.send(P2pEvent::PeerDisconnected { peer_id }).await;
                    }
                    NodeInbound::Message { from_peer, message } => {
                        peer_manager.lock().await.touch_peer(from_peer);

                        match message {
                            WireMessage::Hello { .. } => {
                                debug!(from_peer, "ignoring extra hello");
                            }
                            WireMessage::NewBlock { block_data, .. } => {
                                match serde_json::from_slice::<Block>(&block_data) {
                                    Ok(block) => {
                                        peer_manager.lock().await.update_best_height(
                                            from_peer,
                                            block.number(),
                                            block.hash().into(),
                                        );

                                        let relay = {
                                            let mut gossip = gossip_engine.lock().await;
                                            gossip.on_new_block(&block)
                                        };
                                        if let Some(relay_msg) = relay {
                                            Self::broadcast_wire(
                                                peer_senders.clone(),
                                                relay_msg,
                                                Some(from_peer),
                                            )
                                            .await;
                                        }

                                        let _ = event_tx
                                            .send(P2pEvent::BlockReceived {
                                                from_peer,
                                                block: Box::new(block),
                                            })
                                            .await;
                                    }
                                    Err(error) => {
                                        debug!(%error, from_peer, "failed to decode block payload");
                                    }
                                }
                            }
                            WireMessage::NewTransaction { tx_data } => {
                                match serde_json::from_slice::<Transaction>(&tx_data) {
                                    Ok(tx) => {
                                        let relay = {
                                            let mut gossip = gossip_engine.lock().await;
                                            gossip.on_new_tx(&tx)
                                        };
                                        if let Some(relay_msg) = relay {
                                            Self::broadcast_wire(
                                                peer_senders.clone(),
                                                relay_msg,
                                                Some(from_peer),
                                            )
                                            .await;
                                        }

                                        let _ = event_tx
                                            .send(P2pEvent::TransactionReceived {
                                                from_peer,
                                                tx: Box::new(tx),
                                            })
                                            .await;
                                    }
                                    Err(error) => {
                                        debug!(%error, from_peer, "failed to decode transaction payload");
                                    }
                                }
                            }
                            WireMessage::ConsensusMsg { msg_data } => {
                                let should_relay = {
                                    let mut cache = consensus_cache.lock().await;
                                    cache.should_relay(&msg_data)
                                };

                                if should_relay {
                                    Self::broadcast_wire(
                                        peer_senders.clone(),
                                        WireMessage::ConsensusMsg {
                                            msg_data: msg_data.clone(),
                                        },
                                        Some(from_peer),
                                    )
                                    .await;
                                }

                                match serde_json::from_slice::<ConsensusMessage>(&msg_data) {
                                    Ok(msg) => {
                                        let _ = event_tx
                                            .send(P2pEvent::ConsensusMessageReceived {
                                                from_peer,
                                                msg,
                                            })
                                            .await;
                                    }
                                    Err(error) => {
                                        debug!(%error, from_peer, "failed to decode consensus payload");
                                    }
                                }
                            }
                            WireMessage::GetBlocks {
                                from_height: _,
                                count: _,
                            } => {
                                Self::send_to_peer(
                                    peer_senders.clone(),
                                    from_peer,
                                    WireMessage::Blocks { blocks: Vec::new() },
                                )
                                .await;
                            }
                            WireMessage::Blocks { blocks } => {
                                let mut decoded_blocks = Vec::new();
                                for encoded in blocks {
                                    match serde_json::from_slice::<Block>(&encoded) {
                                        Ok(block) => decoded_blocks.push(block),
                                        Err(error) => {
                                            debug!(%error, "failed to decode block in sync batch")
                                        }
                                    }
                                }

                                let maybe_sync_complete = {
                                    let mut sync = sync_engine.lock().await;
                                    let was_synced = matches!(sync.state, SyncState::Synced);
                                    sync.on_blocks_received(&decoded_blocks);
                                    if !was_synced && matches!(sync.state, SyncState::Synced) {
                                        Some(sync.local_height)
                                    } else {
                                        None
                                    }
                                };

                                if let Some(height) = maybe_sync_complete {
                                    let _ = event_tx.send(P2pEvent::SyncComplete { height }).await;
                                }
                            }
                            WireMessage::Ping { nonce } => {
                                Self::send_to_peer(
                                    peer_senders.clone(),
                                    from_peer,
                                    WireMessage::Pong { nonce },
                                )
                                .await;
                            }
                            WireMessage::Pong { .. } => {}
                        }
                    }
                }
            }
        });
    }

    async fn send_to_peer(
        peer_senders: Arc<RwLock<HashMap<u64, mpsc::Sender<WireMessage>>>>,
        peer_id: u64,
        message: WireMessage,
    ) {
        let sender = { peer_senders.read().await.get(&peer_id).cloned() };
        if let Some(sender) = sender {
            let _ = sender.send(message).await;
        }
    }

    async fn broadcast_wire(
        peer_senders: Arc<RwLock<HashMap<u64, mpsc::Sender<WireMessage>>>>,
        message: WireMessage,
        except_peer: Option<u64>,
    ) {
        let senders = peer_senders.read().await.clone();
        for (peer_id, sender) in senders {
            if except_peer == Some(peer_id) {
                continue;
            }
            let _ = sender.send(message.clone()).await;
        }
    }
}
