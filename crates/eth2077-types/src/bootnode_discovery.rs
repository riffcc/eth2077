use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub const ETH2077_TESTNET_NODE_COUNT: usize = 48;
pub const DEFAULT_TARGET_PEER_COUNT: usize = 25;
pub const DEFAULT_MAX_PEER_COUNT: usize = 50;
pub const DEFAULT_BOOTNODE_COUNT: usize = 4;
pub const DEFAULT_REFRESH_INTERVAL_SECONDS: u64 = 30;
pub const DEFAULT_EVICTION_TIMEOUT_SECONDS: u64 = 300;
const MAX_TARGET_PEER_COUNT_ALLOWED: usize = 128;
const MAX_PEER_COUNT_ALLOWED: usize = 256;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryProtocol {
    Discv4,
    Discv5,
    DnsDiscovery,
    StaticPeers,
}
impl DiscoveryProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discv4 => "discv4",
            Self::Discv5 => "discv5",
            Self::DnsDiscovery => "dns-discovery",
            Self::StaticPeers => "static-peers",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeReachability {
    Public,
    BehindNat,
    Firewalled,
    Unknown,
}
impl NodeReachability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::BehindNat => "behind-nat",
            Self::Firewalled => "firewalled",
            Self::Unknown => "unknown",
        }
    }
    pub const fn is_reachable(self) -> bool {
        matches!(self, Self::Public | Self::BehindNat)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopologyStrategy {
    FullMesh,
    KademliaDht,
    StarTopology,
    HybridMesh,
}
impl TopologyStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullMesh => "full-mesh",
            Self::KademliaDht => "kademlia-dht",
            Self::StarTopology => "star-topology",
            Self::HybridMesh => "hybrid-mesh",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootnodeHealth {
    Online,
    Degraded,
    Offline,
    Syncing,
}
impl BootnodeHealth {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Degraded => "degraded",
            Self::Offline => "offline",
            Self::Syncing => "syncing",
        }
    }
    pub const fn is_healthy(self) -> bool {
        matches!(self, Self::Online | Self::Degraded)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bootnode {
    pub id: String,
    pub enode_url: String,
    pub enr_record: String,
    pub ip_address: String,
    pub tcp_port: u16,
    pub udp_port: u16,
    pub health: BootnodeHealth,
    pub region: String,
    pub connected_peers: usize,
    pub uptime_seconds: u64,
}
impl Bootnode {
    pub const fn is_healthy(&self) -> bool {
        self.health.is_healthy()
    }
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.ip_address, self.tcp_port)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub node_id: String,
    pub protocol: DiscoveryProtocol,
    pub reachability: NodeReachability,
    pub latency_ms: f64,
    pub discovered_at_unix: u64,
    pub last_seen_unix: u64,
    pub client_version: String,
    pub capabilities: Vec<String>,
}
impl DiscoveredPeer {
    pub const fn is_reachable(&self) -> bool {
        self.reachability.is_reachable()
    }
    pub fn has_valid_latency(&self) -> bool {
        self.latency_ms.is_finite() && self.latency_ms >= 0.0
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologySnapshot {
    pub timestamp_unix: u64,
    pub total_nodes: usize,
    pub connected_pairs: usize,
    pub avg_peer_count: f64,
    pub min_peer_count: usize,
    pub max_peer_count: usize,
    pub network_diameter: usize,
    pub partitioned: bool,
}
impl TopologySnapshot {
    pub const fn max_possible_pairs(&self) -> usize {
        self.total_nodes
            .saturating_mul(self.total_nodes.saturating_sub(1))
            / 2
    }
    pub fn connectivity_ratio(&self) -> f64 {
        let max_pairs = self.max_possible_pairs();
        if max_pairs == 0 {
            0.0
        } else {
            self.connected_pairs as f64 / max_pairs as f64
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootnodeDiscoveryConfig {
    pub discovery_protocol: DiscoveryProtocol,
    pub topology_strategy: TopologyStrategy,
    pub target_peer_count: usize,
    pub max_peer_count: usize,
    pub bootnode_count: usize,
    pub refresh_interval_seconds: u64,
    pub eviction_timeout_seconds: u64,
    pub dns_discovery_url: Option<String>,
    pub enable_nat_traversal: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootnodeDiscoveryValidationError {
    pub field: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootnodeDiscoveryStats {
    pub total_bootnodes: usize,
    pub healthy_bootnodes: usize,
    pub discovered_peers: usize,
    pub reachable_peers: usize,
    pub avg_latency_ms: f64,
    pub avg_peer_count: f64,
    pub network_partitioned: bool,
    pub commitment: [u8; 32],
}
impl Default for BootnodeDiscoveryConfig {
    fn default() -> Self {
        Self {
            discovery_protocol: DiscoveryProtocol::Discv5,
            topology_strategy: TopologyStrategy::KademliaDht,
            target_peer_count: DEFAULT_TARGET_PEER_COUNT,
            max_peer_count: DEFAULT_MAX_PEER_COUNT,
            bootnode_count: DEFAULT_BOOTNODE_COUNT,
            refresh_interval_seconds: DEFAULT_REFRESH_INTERVAL_SECONDS,
            eviction_timeout_seconds: DEFAULT_EVICTION_TIMEOUT_SECONDS,
            dns_discovery_url: None,
            enable_nat_traversal: true,
        }
    }
}
impl BootnodeDiscoveryConfig {
    pub fn validate(&self) -> Vec<BootnodeDiscoveryValidationError> {
        let mut errors = Vec::new();
        validate_peer_counts(self, &mut errors);
        validate_bootnode_count(self, &mut errors);
        validate_timers(self, &mut errors);
        validate_dns_settings(self, &mut errors);
        validate_topology_expectations(self, &mut errors);
        validate_nat_settings(self, &mut errors);
        errors
    }
}
pub fn compute_stats(
    bootnodes: &[Bootnode],
    peers: &[DiscoveredPeer],
    snapshot: Option<&TopologySnapshot>,
) -> BootnodeDiscoveryStats {
    let total_bootnodes = bootnodes.len();
    let healthy_bootnodes = bootnodes.iter().filter(|node| node.is_healthy()).count();
    let discovered_peers = peers.len();
    let reachable_peers = peers.iter().filter(|peer| peer.is_reachable()).count();
    let avg_latency_ms = compute_average_latency(peers);
    let avg_peer_count = resolve_average_peer_count(bootnodes, snapshot);
    let network_partitioned = snapshot.is_some_and(|topology| topology.partitioned);
    let commitment = compute_commitment(
        bootnodes,
        peers,
        snapshot,
        total_bootnodes,
        healthy_bootnodes,
        discovered_peers,
        reachable_peers,
        avg_latency_ms,
        avg_peer_count,
        network_partitioned,
    );
    BootnodeDiscoveryStats {
        total_bootnodes,
        healthy_bootnodes,
        discovered_peers,
        reachable_peers,
        avg_latency_ms,
        avg_peer_count,
        network_partitioned,
        commitment,
    }
}
pub fn is_healthy_network(snapshot: &TopologySnapshot, min_connectivity: f64) -> bool {
    if !min_connectivity.is_finite() || !(0.0..=1.0).contains(&min_connectivity) {
        return false;
    }
    if snapshot.total_nodes == 0 || snapshot.partitioned {
        return false;
    }
    let ratio = snapshot.connectivity_ratio();
    if ratio + f64::EPSILON < min_connectivity {
        return false;
    }
    let required_avg_peers =
        ((snapshot.total_nodes.saturating_sub(1)) as f64 * min_connectivity).ceil();
    if sanitize_f64(snapshot.avg_peer_count) + f64::EPSILON < required_avg_peers {
        return false;
    }
    snapshot.network_diameter <= healthy_diameter_ceiling(snapshot.total_nodes)
}
fn validate_peer_counts(
    config: &BootnodeDiscoveryConfig,
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
) {
    if config.target_peer_count == 0 {
        push_validation_error(
            errors,
            "target_peer_count",
            "target_peer_count must be greater than zero",
        );
    }
    if config.target_peer_count > MAX_TARGET_PEER_COUNT_ALLOWED {
        push_validation_error(
            errors,
            "target_peer_count",
            &format!(
                "target_peer_count must be <= {MAX_TARGET_PEER_COUNT_ALLOWED} for bounded memory pressure"
            ),
        );
    }
    if config.max_peer_count == 0 {
        push_validation_error(
            errors,
            "max_peer_count",
            "max_peer_count must be greater than zero",
        );
    }
    if config.max_peer_count > MAX_PEER_COUNT_ALLOWED {
        push_validation_error(
            errors,
            "max_peer_count",
            &format!(
                "max_peer_count must be <= {MAX_PEER_COUNT_ALLOWED} for bounded memory pressure"
            ),
        );
    }
    if config.target_peer_count > config.max_peer_count {
        push_validation_error(
            errors,
            "target_peer_count",
            "target_peer_count must be less than or equal to max_peer_count",
        );
    }
}
fn validate_bootnode_count(
    config: &BootnodeDiscoveryConfig,
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
) {
    if config.bootnode_count == 0 {
        push_validation_error(
            errors,
            "bootnode_count",
            "bootnode_count must be greater than zero",
        );
    }
    if config.bootnode_count > ETH2077_TESTNET_NODE_COUNT {
        push_validation_error(
            errors,
            "bootnode_count",
            &format!(
                "bootnode_count must be <= {ETH2077_TESTNET_NODE_COUNT} for a 48-node public testnet"
            ),
        );
    }
    if config.bootnode_count > config.max_peer_count {
        push_validation_error(
            errors,
            "bootnode_count",
            "bootnode_count should not exceed max_peer_count",
        );
    }
}
fn validate_timers(
    config: &BootnodeDiscoveryConfig,
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
) {
    if config.refresh_interval_seconds < 5 || config.refresh_interval_seconds > 3_600 {
        push_validation_error(
            errors,
            "refresh_interval_seconds",
            "refresh_interval_seconds must be within [5, 3600]",
        );
    }
    if config.eviction_timeout_seconds < 30 || config.eviction_timeout_seconds > 86_400 {
        push_validation_error(
            errors,
            "eviction_timeout_seconds",
            "eviction_timeout_seconds must be within [30, 86400]",
        );
    }
    if config.eviction_timeout_seconds <= config.refresh_interval_seconds {
        push_validation_error(
            errors,
            "eviction_timeout_seconds",
            "eviction_timeout_seconds must be greater than refresh_interval_seconds",
        );
    }
}
fn validate_dns_settings(
    config: &BootnodeDiscoveryConfig,
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
) {
    match (config.discovery_protocol, config.dns_discovery_url.as_ref()) {
        (DiscoveryProtocol::DnsDiscovery, None) => {
            push_validation_error(
                errors,
                "dns_discovery_url",
                "dns_discovery_url is required when discovery_protocol is DnsDiscovery",
            );
        }
        (DiscoveryProtocol::DnsDiscovery, Some(value)) => validate_dns_url(value, errors),
        (_, Some(value)) => {
            if value.trim().is_empty() {
                push_validation_error(
                    errors,
                    "dns_discovery_url",
                    "dns_discovery_url must not be empty when provided",
                );
            } else {
                validate_dns_url(value, errors);
            }
        }
        (_, None) => {}
    }
}
fn validate_topology_expectations(
    config: &BootnodeDiscoveryConfig,
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
) {
    match config.topology_strategy {
        TopologyStrategy::FullMesh => {
            let expected = ETH2077_TESTNET_NODE_COUNT.saturating_sub(1);
            if config.target_peer_count < expected {
                push_validation_error(
                    errors,
                    "target_peer_count",
                    &format!("FullMesh expects target_peer_count >= {expected}"),
                );
            }
        }
        TopologyStrategy::KademliaDht => {
            if config.target_peer_count < 8 {
                push_validation_error(
                    errors,
                    "target_peer_count",
                    "KademliaDht expects target_peer_count >= 8",
                );
            }
        }
        TopologyStrategy::StarTopology => {
            if config.bootnode_count < 1 {
                push_validation_error(
                    errors,
                    "bootnode_count",
                    "StarTopology requires at least one bootnode",
                );
            }
            if config.target_peer_count < 2 {
                push_validation_error(
                    errors,
                    "target_peer_count",
                    "StarTopology expects target_peer_count >= 2",
                );
            }
        }
        TopologyStrategy::HybridMesh => {
            if config.target_peer_count < 12 {
                push_validation_error(
                    errors,
                    "target_peer_count",
                    "HybridMesh expects target_peer_count >= 12",
                );
            }
        }
    }
}
fn validate_nat_settings(
    config: &BootnodeDiscoveryConfig,
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
) {
    if !config.enable_nat_traversal
        && matches!(config.discovery_protocol, DiscoveryProtocol::Discv5)
    {
        push_validation_error(
            errors,
            "enable_nat_traversal",
            "enable_nat_traversal=false is not recommended with Discv5",
        );
    }
}
fn validate_dns_url(value: &str, errors: &mut Vec<BootnodeDiscoveryValidationError>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        push_validation_error(
            errors,
            "dns_discovery_url",
            "dns_discovery_url must not be empty",
        );
        return;
    }
    let has_valid_scheme = trimmed.starts_with("enrtree://")
        || trimmed.starts_with("dns://")
        || trimmed.starts_with("https://");
    if !has_valid_scheme {
        push_validation_error(
            errors,
            "dns_discovery_url",
            "dns_discovery_url must start with enrtree://, dns://, or https://",
        );
    }
}
fn push_validation_error(
    errors: &mut Vec<BootnodeDiscoveryValidationError>,
    field: &str,
    message: &str,
) {
    errors.push(BootnodeDiscoveryValidationError {
        field: field.to_string(),
        message: message.to_string(),
    });
}
fn compute_average_latency(peers: &[DiscoveredPeer]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for peer in peers {
        if peer.has_valid_latency() {
            sum += peer.latency_ms;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}
fn resolve_average_peer_count(bootnodes: &[Bootnode], snapshot: Option<&TopologySnapshot>) -> f64 {
    if let Some(topology) = snapshot {
        return sanitize_f64(topology.avg_peer_count);
    }
    if bootnodes.is_empty() {
        return 0.0;
    }
    let total_connected: usize = bootnodes.iter().map(|node| node.connected_peers).sum();
    total_connected as f64 / bootnodes.len() as f64
}
fn compute_commitment(
    bootnodes: &[Bootnode],
    peers: &[DiscoveredPeer],
    snapshot: Option<&TopologySnapshot>,
    total_bootnodes: usize,
    healthy_bootnodes: usize,
    discovered_peers: usize,
    reachable_peers: usize,
    avg_latency_ms: f64,
    avg_peer_count: f64,
    network_partitioned: bool,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_usize(&mut hasher, "stats.total_bootnodes", total_bootnodes);
    append_usize(&mut hasher, "stats.healthy_bootnodes", healthy_bootnodes);
    append_usize(&mut hasher, "stats.discovered_peers", discovered_peers);
    append_usize(&mut hasher, "stats.reachable_peers", reachable_peers);
    append_f64(&mut hasher, "stats.avg_latency_ms", avg_latency_ms);
    append_f64(&mut hasher, "stats.avg_peer_count", avg_peer_count);
    append_bool(
        &mut hasher,
        "stats.network_partitioned",
        network_partitioned,
    );
    append_bootnodes(&mut hasher, bootnodes);
    append_peers(&mut hasher, peers);
    append_snapshot(&mut hasher, snapshot);
    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}
fn append_bootnodes(hasher: &mut Sha256, bootnodes: &[Bootnode]) {
    let mut ordered: Vec<&Bootnode> = bootnodes.iter().collect();
    ordered.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then(left.ip_address.cmp(&right.ip_address))
            .then(left.tcp_port.cmp(&right.tcp_port))
            .then(left.udp_port.cmp(&right.udp_port))
    });
    append_usize(hasher, "bootnodes.len", ordered.len());
    for node in ordered {
        append_str(hasher, "bootnode.id", &node.id);
        append_str(hasher, "bootnode.enode_url", &node.enode_url);
        append_str(hasher, "bootnode.enr_record", &node.enr_record);
        append_str(hasher, "bootnode.ip_address", &node.ip_address);
        append_usize(hasher, "bootnode.tcp_port", node.tcp_port as usize);
        append_usize(hasher, "bootnode.udp_port", node.udp_port as usize);
        append_str(hasher, "bootnode.health", node.health.as_str());
        append_str(hasher, "bootnode.region", &node.region);
        append_usize(hasher, "bootnode.connected_peers", node.connected_peers);
        append_u64(hasher, "bootnode.uptime_seconds", node.uptime_seconds);
    }
}
fn append_peers(hasher: &mut Sha256, peers: &[DiscoveredPeer]) {
    let mut ordered: Vec<&DiscoveredPeer> = peers.iter().collect();
    ordered.sort_by(|left, right| {
        left.node_id
            .cmp(&right.node_id)
            .then(left.protocol.as_str().cmp(right.protocol.as_str()))
            .then(left.discovered_at_unix.cmp(&right.discovered_at_unix))
    });
    append_usize(hasher, "peers.len", ordered.len());
    for peer in ordered {
        append_str(hasher, "peer.node_id", &peer.node_id);
        append_str(hasher, "peer.protocol", peer.protocol.as_str());
        append_str(hasher, "peer.reachability", peer.reachability.as_str());
        append_f64(hasher, "peer.latency_ms", peer.latency_ms);
        append_u64(hasher, "peer.discovered_at_unix", peer.discovered_at_unix);
        append_u64(hasher, "peer.last_seen_unix", peer.last_seen_unix);
        append_str(hasher, "peer.client_version", &peer.client_version);
        let mut capabilities = peer.capabilities.clone();
        capabilities.sort();
        append_usize(hasher, "peer.capabilities.len", capabilities.len());
        for capability in capabilities {
            append_str(hasher, "peer.capability", &capability);
        }
    }
}
fn append_snapshot(hasher: &mut Sha256, snapshot: Option<&TopologySnapshot>) {
    match snapshot {
        None => append_bool(hasher, "snapshot.present", false),
        Some(topology) => {
            append_bool(hasher, "snapshot.present", true);
            append_u64(hasher, "snapshot.timestamp_unix", topology.timestamp_unix);
            append_usize(hasher, "snapshot.total_nodes", topology.total_nodes);
            append_usize(hasher, "snapshot.connected_pairs", topology.connected_pairs);
            append_f64(hasher, "snapshot.avg_peer_count", topology.avg_peer_count);
            append_usize(hasher, "snapshot.min_peer_count", topology.min_peer_count);
            append_usize(hasher, "snapshot.max_peer_count", topology.max_peer_count);
            append_usize(
                hasher,
                "snapshot.network_diameter",
                topology.network_diameter,
            );
            append_bool(hasher, "snapshot.partitioned", topology.partitioned);
        }
    }
}
fn append_str(hasher: &mut Sha256, key: &str, value: &str) {
    hasher.update(key.as_bytes());
    hasher.update(b"=");
    hasher.update(value.as_bytes());
    hasher.update(b";");
}
fn append_usize(hasher: &mut Sha256, key: &str, value: usize) {
    append_str(hasher, key, &value.to_string());
}
fn append_u64(hasher: &mut Sha256, key: &str, value: u64) {
    append_str(hasher, key, &value.to_string());
}
fn append_f64(hasher: &mut Sha256, key: &str, value: f64) {
    let normalized = sanitize_f64(value);
    append_str(hasher, key, &format!("{normalized:.6}"));
}
fn append_bool(hasher: &mut Sha256, key: &str, value: bool) {
    append_str(hasher, key, if value { "1" } else { "0" });
}
fn sanitize_f64(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
fn healthy_diameter_ceiling(total_nodes: usize) -> usize {
    match total_nodes {
        0..=2 => 1,
        3..=8 => 3,
        9..=16 => 4,
        17..=32 => 6,
        _ => 8,
    }
}
