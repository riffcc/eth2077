use std::env;
use std::fs;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

#[derive(Debug, Clone, Serialize)]
struct ValidatorSpec {
    id: usize,
    execution_address: String,
    consensus_pubkey: String,
    initial_stake_wei: String,
}

#[derive(Debug, Clone, Serialize)]
struct AllocAccount {
    address: String,
    balance_wei: String,
    role: String,
}

#[derive(Debug, Clone, Serialize)]
struct GenesisSpec {
    network_name: String,
    chain_id: u64,
    genesis_timestamp: u64,
    fork_epoch: u64,
    validator_count: usize,
    fault_budget: usize,
    quorum_threshold: usize,
    validators: Vec<ValidatorSpec>,
    alloc: Vec<AllocAccount>,
}

#[derive(Debug, Clone, Serialize)]
struct Metadata {
    artifact_version: String,
    seed: u64,
    node_count: usize,
    bootnode_count: usize,
    chain_id: u64,
    genesis_timestamp: u64,
    fork_epoch: u64,
    notes: String,
}

fn arg_value(args: &[String], flag: &str, default: &str) -> String {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
        .unwrap_or_else(|| default.to_string())
}

fn random_hex(rng: &mut XorShift64, bytes: usize) -> String {
    let mut out = String::with_capacity(2 * bytes + 2);
    out.push_str("0x");
    for _ in 0..bytes {
        let b = (rng.next_u64() & 0xff) as u8;
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn write_pretty_json<P: AsRef<Path>, T: Serialize>(path: P, value: &T) {
    let payload = serde_json::to_string_pretty(value).expect("serialize json");
    fs::write(path, payload).expect("write json");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let output_dir = arg_value(&args, "--output-dir", "artifacts/testnet-alpha");
    let network_name = arg_value(&args, "--network", "eth2077-alpha");
    let seed: u64 = arg_value(&args, "--seed", "2077").parse().unwrap_or(2077);
    let chain_id: u64 = arg_value(&args, "--chain-id", "2077001")
        .parse()
        .unwrap_or(2077001);
    let genesis_timestamp: u64 = arg_value(&args, "--genesis-ts", "1772409600")
        .parse()
        .unwrap_or(1772409600);
    let fork_epoch: u64 = arg_value(&args, "--fork-epoch", "0")
        .parse()
        .unwrap_or(0);
    let validator_count: usize = arg_value(&args, "--validators", "48")
        .parse()
        .unwrap_or(48);
    let bootnode_count: usize = arg_value(&args, "--bootnodes", "8")
        .parse()
        .unwrap_or(8);
    let user_alloc_count: usize = arg_value(&args, "--alloc-accounts", "512")
        .parse()
        .unwrap_or(512);

    let mut rng = XorShift64::new(seed);

    let fault_budget = validator_count.saturating_sub(1) / 3;
    let quorum_threshold = 2 * fault_budget + 1;
    let base_stake = "1000000000000000000".to_string();
    let default_balance = "100000000000000000000000".to_string();

    let mut validators = Vec::with_capacity(validator_count);
    let mut alloc = Vec::with_capacity(validator_count + user_alloc_count + 1);

    for id in 0..validator_count {
        let execution_address = random_hex(&mut rng, 20);
        let consensus_pubkey = random_hex(&mut rng, 48);
        validators.push(ValidatorSpec {
            id,
            execution_address: execution_address.clone(),
            consensus_pubkey,
            initial_stake_wei: base_stake.clone(),
        });
        alloc.push(AllocAccount {
            address: execution_address,
            balance_wei: default_balance.clone(),
            role: "validator".to_string(),
        });
    }

    for _ in 0..user_alloc_count {
        alloc.push(AllocAccount {
            address: random_hex(&mut rng, 20),
            balance_wei: default_balance.clone(),
            role: "user".to_string(),
        });
    }

    let faucet = AllocAccount {
        address: random_hex(&mut rng, 20),
        balance_wei: "100000000000000000000000000".to_string(),
        role: "faucet".to_string(),
    };
    alloc.push(faucet);

    let mut bootnodes = Vec::with_capacity(bootnode_count);
    for i in 0..bootnode_count {
        let pubkey = random_hex(&mut rng, 32);
        let ip = format!("10.207.7.{}", 10 + i);
        let enode = format!("enode://{}@{}:30303", &pubkey[2..], ip);
        bootnodes.push(enode);
    }

    let genesis = GenesisSpec {
        network_name: network_name.clone(),
        chain_id,
        genesis_timestamp,
        fork_epoch,
        validator_count,
        fault_budget,
        quorum_threshold,
        validators: validators.clone(),
        alloc: alloc.clone(),
    };

    let chain_spec = serde_json::json!({
        "name": network_name,
        "chain_id": chain_id,
        "genesis_timestamp": genesis_timestamp,
        "fork_epoch": fork_epoch,
        "validator_count": validator_count,
        "fault_budget": fault_budget,
        "quorum_threshold": quorum_threshold,
        "bootnodes": bootnodes,
        "alloc_accounts": alloc.len(),
        "notes": "Deterministic ETH2077 testnet artifact. Regenerate from seed to verify reproducibility."
    });

    let metadata = Metadata {
        artifact_version: "eth2077-testnet-v0".to_string(),
        seed,
        node_count: validator_count,
        bootnode_count,
        chain_id,
        genesis_timestamp,
        fork_epoch,
        notes: "Dev/testnet artifacts only. Deterministic by seed, not production key generation."
            .to_string(),
    };

    fs::create_dir_all(&output_dir).expect("create output dir");
    write_pretty_json(format!("{output_dir}/chain-spec.json"), &chain_spec);
    write_pretty_json(format!("{output_dir}/genesis.json"), &genesis);
    write_pretty_json(format!("{output_dir}/metadata.json"), &metadata);
    fs::write(format!("{output_dir}/bootnodes.txt"), bootnodes.join("\n") + "\n")
        .expect("write bootnodes");

    println!("Wrote deterministic testnet artifacts to: {}", output_dir);
    println!("- chain-spec: {}/chain-spec.json", output_dir);
    println!("- genesis: {}/genesis.json", output_dir);
    println!("- metadata: {}/metadata.json", output_dir);
    println!("- bootnodes: {}/bootnodes.txt", output_dir);
    println!(
        "Parameters: seed={}, chain_id={}, validators={}, quorum={}, fault_budget={}",
        seed, chain_id, validator_count, quorum_threshold, fault_budget
    );
}
