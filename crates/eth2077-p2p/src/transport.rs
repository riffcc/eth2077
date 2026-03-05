use std::collections::HashSet;
use std::io;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tokio_util::codec::Framed;
use tracing::{debug, warn};

use crate::codec::{MessageCodec, WireMessage};

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
            framed: Framed::new(stream, MessageCodec),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportEvent {
    PeerConnected(SocketAddr),
    PeerDisconnected(SocketAddr),
}

#[async_trait]
pub trait Transport: Clone + Send + Sync + 'static {
    async fn accept(&self) -> io::Result<Connection>;
    async fn connect(&self, addr: SocketAddr) -> io::Result<Connection>;
    async fn disconnect(&self, addr: SocketAddr);
    async fn peers(&self) -> Vec<SocketAddr>;
    fn local_addr(&self) -> io::Result<SocketAddr>;
    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent>;
    async fn send(
        &self,
        framed: &mut Framed<TcpStream, MessageCodec>,
        msg: WireMessage,
    ) -> io::Result<()>;
    async fn recv(
        &self,
        framed: &mut Framed<TcpStream, MessageCodec>,
    ) -> io::Result<Option<WireMessage>>;
}

#[derive(Clone)]
pub struct GossipTransport {
    config: TransportConfig,
    listener: Arc<TcpListener>,
    active_peers: Arc<RwLock<HashSet<SocketAddr>>>,
    events_tx: broadcast::Sender<TransportEvent>,
}

impl GossipTransport {
    pub fn bind(config: TransportConfig) -> io::Result<Self> {
        let std_listener = StdTcpListener::bind(config.listen_addr)?;
        std_listener.set_nonblocking(true)?;
        let listener = TcpListener::from_std(std_listener)?;
        let (events_tx, _events_rx) = broadcast::channel(512);

        Ok(Self {
            config,
            listener: Arc::new(listener),
            active_peers: Arc::new(RwLock::new(HashSet::new())),
            events_tx,
        })
    }
}

#[async_trait]
impl Transport for GossipTransport {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    async fn accept(&self) -> io::Result<Connection> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;

            if !self.has_capacity().await {
                warn!(?peer_addr, "rejecting inbound peer: max peers reached");
                continue;
            }

            self.active_peers.write().await.insert(peer_addr);
            debug!(?peer_addr, "accepted inbound peer connection");
            let _ = self
                .events_tx
                .send(TransportEvent::PeerConnected(peer_addr));
            return Ok(Connection::new(stream, peer_addr));
        }
    }

    async fn connect(&self, addr: SocketAddr) -> io::Result<Connection> {
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
        let _ = self
            .events_tx
            .send(TransportEvent::PeerConnected(peer_addr));

        Ok(Connection::new(stream, peer_addr))
    }

    async fn disconnect(&self, addr: SocketAddr) {
        self.active_peers.write().await.remove(&addr);
        let _ = self.events_tx.send(TransportEvent::PeerDisconnected(addr));
    }

    async fn peers(&self) -> Vec<SocketAddr> {
        self.active_peers.read().await.iter().copied().collect()
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.events_tx.subscribe()
    }

    async fn send(
        &self,
        framed: &mut Framed<TcpStream, MessageCodec>,
        msg: WireMessage,
    ) -> io::Result<()> {
        framed.send(msg).await.map_err(io::Error::other)
    }

    async fn recv(
        &self,
        framed: &mut Framed<TcpStream, MessageCodec>,
    ) -> io::Result<Option<WireMessage>> {
        match framed.next().await {
            Some(Ok(msg)) => Ok(Some(msg)),
            Some(Err(error)) => Err(io::Error::other(error)),
            None => Ok(None),
        }
    }
}

impl GossipTransport {
    async fn has_capacity(&self) -> bool {
        self.active_peers.read().await.len() < self.config.max_peers
    }
}

#[derive(Clone)]
pub struct SpiralTransport;

#[async_trait]
impl Transport for SpiralTransport {
    async fn accept(&self) -> io::Result<Connection> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::accept is not implemented yet");
    }

    async fn connect(&self, _addr: SocketAddr) -> io::Result<Connection> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::connect is not implemented yet");
    }

    async fn disconnect(&self, _addr: SocketAddr) {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::disconnect is not implemented yet");
    }

    async fn peers(&self) -> Vec<SocketAddr> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::peers is not implemented yet");
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::local_addr is not implemented yet");
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::subscribe_events is not implemented yet");
    }

    async fn send(
        &self,
        _framed: &mut Framed<TcpStream, MessageCodec>,
        _msg: WireMessage,
    ) -> io::Result<()> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::send is not implemented yet");
    }

    async fn recv(
        &self,
        _framed: &mut Framed<TcpStream, MessageCodec>,
    ) -> io::Result<Option<WireMessage>> {
        // TODO: integrate Citadel's SPIRAL mesh transport once the networking backend is wired in.
        todo!("SpiralTransport::recv is not implemented yet");
    }
}
