use std::collections::HashSet;
use std::io;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_util::codec::Framed;
use tracing::{debug, warn};

use crate::codec::MessageCodec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerAddr {
    pub peer_id: u64,
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct Connection {
    peer_addr: SocketAddr,
    framed: Framed<TcpStream, MessageCodec>,
}

impl Connection {
    fn new(stream: TcpStream, peer_addr: SocketAddr) -> Self {
        Self {
            peer_addr,
            framed: Framed::new(stream, MessageCodec::default()),
        }
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    pub fn into_framed(self) -> Framed<TcpStream, MessageCodec> {
        self.framed
    }
}

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub listen_addr: SocketAddr,
    pub max_peers: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            max_peers: 50,
        }
    }
}

#[derive(Clone)]
pub struct Transport {
    config: TransportConfig,
    listener: Arc<TcpListener>,
    active_peers: Arc<RwLock<HashSet<SocketAddr>>>,
}

impl Transport {
    pub fn bind(config: TransportConfig) -> io::Result<Self> {
        let std_listener = StdTcpListener::bind(config.listen_addr)?;
        std_listener.set_nonblocking(true)?;
        let listener = TcpListener::from_std(std_listener)?;

        Ok(Self {
            config,
            listener: Arc::new(listener),
            active_peers: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub async fn accept(&self) -> io::Result<Connection> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;

            if !self.has_capacity().await {
                warn!(?peer_addr, "rejecting inbound peer: max peers reached");
                continue;
            }

            self.active_peers.write().await.insert(peer_addr);
            debug!(?peer_addr, "accepted inbound peer connection");
            return Ok(Connection::new(stream, peer_addr));
        }
    }

    pub async fn connect(&self, addr: SocketAddr) -> io::Result<Connection> {
        if !self.has_capacity().await {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "max peers reached",
            ));
        }

        let stream = TcpStream::connect(addr).await?;
        let peer_addr = stream.peer_addr()?;
        self.active_peers.write().await.insert(peer_addr);
        debug!(?peer_addr, "established outbound peer connection");

        Ok(Connection::new(stream, peer_addr))
    }

    pub async fn unregister_peer(&self, addr: SocketAddr) {
        self.active_peers.write().await.remove(&addr);
    }

    pub async fn active_peer_count(&self) -> usize {
        self.active_peers.read().await.len()
    }

    pub async fn active_peers(&self) -> Vec<SocketAddr> {
        self.active_peers.read().await.iter().copied().collect()
    }

    async fn has_capacity(&self) -> bool {
        self.active_peers.read().await.len() < self.config.max_peers
    }
}
