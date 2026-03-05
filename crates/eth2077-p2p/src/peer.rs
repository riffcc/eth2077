use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Connecting,
    Handshaking,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: u64,
    pub addr: SocketAddr,
    pub state: PeerState,
    pub best_height: u64,
    pub best_hash: [u8; 32],
    pub connected_at: Instant,
    pub last_seen: Instant,
}

impl PeerInfo {
    pub fn new(peer_id: u64, addr: SocketAddr, state: PeerState) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            addr,
            state,
            best_height: 0,
            best_hash: [0u8; 32],
            connected_at: now,
            last_seen: now,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeerManager {
    pub peers: HashMap<u64, PeerInfo>,
    pub max_peers: usize,
}

impl PeerManager {
    pub fn new(max_peers: usize) -> Self {
        Self {
            peers: HashMap::new(),
            max_peers,
        }
    }

    pub fn add_peer(&mut self, peer: PeerInfo) -> bool {
        let at_capacity = self.peers.len() >= self.max_peers;
        match self.peers.entry(peer.peer_id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(peer);
                true
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                if at_capacity {
                    false
                } else {
                    entry.insert(peer);
                    true
                }
            }
        }
    }

    pub fn remove_peer(&mut self, peer_id: u64) -> Option<PeerInfo> {
        self.peers.remove(&peer_id)
    }

    pub fn get_peer(&self, peer_id: u64) -> Option<&PeerInfo> {
        self.peers.get(&peer_id)
    }

    pub fn update_best_height(&mut self, peer_id: u64, height: u64, hash: [u8; 32]) {
        if let Some(peer) = self.peers.get_mut(&peer_id) {
            peer.best_height = height;
            peer.best_hash = hash;
            peer.last_seen = Instant::now();
        }
    }

    pub fn connected_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .values()
            .filter(|peer| peer.state == PeerState::Connected)
            .cloned()
            .collect()
    }

    pub fn best_peer(&self) -> Option<PeerInfo> {
        self.peers
            .values()
            .filter(|peer| peer.state == PeerState::Connected)
            .max_by_key(|peer| peer.best_height)
            .cloned()
    }

    pub fn touch_peer(&mut self, peer_id: u64) {
        if let Some(peer) = self.peers.get_mut(&peer_id) {
            peer.last_seen = Instant::now();
        }
    }
}
