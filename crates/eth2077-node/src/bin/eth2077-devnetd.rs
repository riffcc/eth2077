use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eth2077_node::bootstrap;
use eth2077_types::ScenarioConfig;
use revm::context::{Context, TxEnv};
use revm::context_interface::block::calc_blob_gasprice;
use revm::context_interface::transaction::{Authorization, SignedAuthorization};
use revm::database::InMemoryDB;
use revm::primitives::alloy_primitives::{logs_bloom, Bloom};
use revm::primitives::eip4844::BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE;
use revm::primitives::{hardfork::SpecId, Address, Bytes, TxKind, B256, U256};
use revm::state::AccountInfo;
use revm::{Database, ExecuteCommitEvm, ExecuteEvm, MainBuilder, MainContext};
use rlp::Rlp;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_ACCOUNT_BALANCE_WEI: u128 = 0u128;
const MARKET_MERCHANT_ADDRESS: &str = "0x1111111111111111111111111111111111111111";

#[derive(Debug, Clone)]
struct Config {
    rpc_host: String,
    node_id: usize,
    nodes: usize,
    rpc_port: u16,
    p2p_port: u16,
    tick_ms: u64,
    chain_spec_path: PathBuf,
    data_dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct EngineTxMeta {
    hash: String,
    from: Option<String>,
    nonce: Option<u64>,
    gas_limit: Option<u64>,
    gas_price: Option<u128>,
    value: Option<U256>,
}

#[derive(Debug, Clone)]
struct Engine7732HeaderRecord {
    slot: u64,
    payload_header_root: String,
    parent_beacon_block_root: Option<String>,
    execution_block_hash: Option<String>,
    proposer: Option<String>,
    bid_value_wei: Option<U256>,
    view_id: Option<u64>,
    received_at_unix_s: u64,
}

#[derive(Debug, Clone)]
struct Engine7732EnvelopeRecord {
    slot: u64,
    payload_header_root: String,
    execution_block_hash: Option<String>,
    payload_body_hash: Option<String>,
    signer: Option<String>,
    data_available: bool,
    revealed_at_unix_s: u64,
}

#[derive(Debug, Clone)]
struct Engine7732PenaltyRecord {
    slot: u64,
    state: String,
    reason: String,
    last_status: String,
    activated_at_unix_s: u64,
    recovered_at_unix_s: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct NodeState {
    node_id: usize,
    nodes: usize,
    rpc_port: u16,
    p2p_port: u16,
    chain_id: u64,
    network: String,
    started_at_unix_s: u64,
    current_height: u64,
    finalized_height: u64,
    peers_target: usize,
    peers_connected: usize,
    ingress_capacity_tps: f64,
    execution_capacity_tps: f64,
    oob_capacity_tps: f64,
    bridge_replay_safe: bool,
    #[serde(skip_serializing)]
    pending_txs: Vec<String>,
    #[serde(skip_serializing)]
    txs: HashMap<String, TxRecord>,
    #[serde(skip_serializing)]
    block_txs: HashMap<u64, Vec<String>>,
    #[serde(skip_serializing)]
    block_timestamps: HashMap<u64, u64>,
    #[serde(skip_serializing)]
    nonce_by_address: HashMap<String, u64>,
    #[serde(skip_serializing)]
    pending_nonce_address_hint: Option<String>,
    #[serde(skip_serializing)]
    tx_counter: u64,
    #[serde(skip_serializing)]
    engine_required_il_txs: Vec<String>,
    #[serde(skip_serializing)]
    engine_required_il_meta: HashMap<String, EngineTxMeta>,
    #[serde(skip_serializing)]
    engine_required_il_slot: Option<u64>,
    #[serde(skip_serializing)]
    engine_required_il_updated_at_unix_s: Option<u64>,
    #[serde(skip_serializing)]
    engine_focil_committee: Vec<String>,
    #[serde(skip_serializing)]
    engine_focil_view_frozen: bool,
    #[serde(skip_serializing)]
    engine_focil_frozen_slot: Option<u64>,
    #[serde(skip_serializing)]
    engine_focil_frozen_il_root: Option<String>,
    #[serde(skip_serializing)]
    engine_focil_view_id: Option<u64>,
    #[serde(skip_serializing)]
    engine_7732_headers_by_slot: HashMap<u64, Vec<String>>,
    #[serde(skip_serializing)]
    engine_7732_headers_by_root: HashMap<String, Engine7732HeaderRecord>,
    #[serde(skip_serializing)]
    engine_7732_envelopes_by_root: HashMap<String, Engine7732EnvelopeRecord>,
    #[serde(skip_serializing)]
    engine_7732_penalties_by_slot: HashMap<u64, Engine7732PenaltyRecord>,
    #[serde(skip_serializing)]
    evm_db: InMemoryDB,
    #[serde(skip_serializing)]
    tx_receipts: HashMap<String, TxReceiptRecord>,
    #[serde(skip_serializing)]
    market_nfts: HashMap<u64, MarketNft>,
    #[serde(skip_serializing)]
    next_market_token_id: u64,
    #[serde(skip_serializing)]
    market_receipts: HashMap<String, MarketReceipt>,
}

#[derive(Debug, Clone)]
struct TxRecord {
    hash: String,
    from: String,
    to: Option<String>,
    nonce: u64,
    gas: u64,
    gas_price: u128,
    max_priority_fee_per_gas: Option<u128>,
    max_fee_per_blob_gas: Option<u128>,
    blob_versioned_hashes: Vec<String>,
    authorization_list_len: usize,
    value: String,
    input: String,
    tx_type: String,
    contract_address: Option<String>,
    block_number: Option<u64>,
    block_hash: Option<String>,
    transaction_index: Option<u64>,
}

#[derive(Debug, Clone)]
struct TxLogRecord {
    address: String,
    topics: Vec<String>,
    data: String,
    block_number: Option<u64>,
    block_hash: Option<String>,
    transaction_hash: Option<String>,
    transaction_index: Option<u64>,
    log_index: Option<u64>,
}

#[derive(Debug, Clone)]
struct TxReceiptRecord {
    tx_hash: String,
    from: String,
    to: Option<String>,
    contract_address: Option<String>,
    gas_used: u64,
    cumulative_gas_used: u64,
    effective_gas_price: u64,
    tx_type: String,
    status: bool,
    logs_bloom: String,
    block_number: Option<u64>,
    block_hash: Option<String>,
    transaction_index: Option<u64>,
    logs: Vec<TxLogRecord>,
}

#[derive(Debug, Clone)]
struct DecodedRawTx {
    tx_type: u8,
    chain_id: Option<u64>,
    nonce: u64,
    gas_limit: u64,
    gas_price: u128,
    max_priority_fee_per_gas: Option<u128>,
    max_fee_per_blob_gas: Option<u128>,
    blob_hashes: Vec<B256>,
    authorizations: Vec<SignedAuthorization>,
    to: Option<Address>,
    value: U256,
    data: Bytes,
}

#[derive(Debug, Clone)]
struct EvmExecOutcome {
    gas_used: u64,
    status: bool,
    output: Bytes,
    contract_address: Option<Address>,
    logs: Vec<revm::primitives::Log>,
}

#[derive(Debug, Clone)]
struct MarketNft {
    token_id: u64,
    name: String,
    description: String,
    image: String,
    owner: String,
    listed: bool,
    price_wei: u128,
    created_at_unix_s: u64,
    last_tx_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct MarketReceipt {
    tx_hash: String,
    token_id: u64,
    buyer: String,
    seller: String,
    price_wei: u128,
    chain_id: u64,
    block_number_hint: u64,
    timestamp_unix_s: u64,
    signature: String,
}

fn arg_value(args: &[String], flag: &str, default: &str) -> String {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
        .unwrap_or_else(|| default.to_string())
}

fn now_unix_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn write_json<P: Into<PathBuf>, T: Serialize>(path: P, value: &T) {
    let path: PathBuf = path.into();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    let payload = serde_json::to_string_pretty(value).expect("serialize json");
    fs::write(path, payload).expect("write json");
}

fn json_response(status: &str, body: &Value) -> String {
    let payload = serde_json::to_string(body).expect("serialize response");
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n{}",
        payload.len(),
        payload
    )
}

fn jsonrpc_response(id: &Value, result: Value) -> Value {
    serde_json::json!({
      "jsonrpc": "2.0",
      "id": id,
      "result": result
    })
}

fn jsonrpc_error(id: &Value, code: i64, message: &str) -> Value {
    serde_json::json!({
      "jsonrpc": "2.0",
      "id": id,
      "error": {
        "code": code,
        "message": message
      }
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn request_body_from_http(
    req_bytes: &[u8],
    header_end: Option<usize>,
    content_length: Option<usize>,
) -> String {
    let Some(h_end) = header_end else {
        return String::new();
    };
    let body_end = h_end + content_length.unwrap_or(0);
    let body_slice = if req_bytes.len() >= body_end {
        &req_bytes[h_end..body_end]
    } else if req_bytes.len() >= h_end {
        &req_bytes[h_end..]
    } else {
        &[]
    };
    String::from_utf8_lossy(body_slice).to_string()
}

fn block_hash(height: u64) -> String {
    let mut bytes = [0u8; 32];
    // Keep deterministic fake hashes, but ensure genesis hash is non-zero so
    // parent_hash relationships remain valid for indexers like Blockscout.
    let encoded_height = height.saturating_add(1);
    bytes[0..8].copy_from_slice(&encoded_height.to_be_bytes());
    let mut out = String::from("0x");
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn hex_u64(v: u64) -> String {
    format!("0x{v:x}")
}

fn hex_u128(v: u128) -> String {
    format!("0x{v:x}")
}

fn parse_hex_u64(input: &str) -> Option<u64> {
    let s = input.strip_prefix("0x").unwrap_or(input);
    u64::from_str_radix(s, 16).ok()
}

fn parse_hex_u128(input: &str) -> Option<u128> {
    let s = input.strip_prefix("0x").unwrap_or(input);
    u128::from_str_radix(s, 16).ok()
}

fn parse_hex_bytes(input: &str) -> Option<Vec<u8>> {
    let raw = input.trim().strip_prefix("0x").unwrap_or(input.trim());
    if raw.is_empty() {
        return Some(Vec::new());
    }
    if !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let normalized = if raw.len() % 2 == 1 {
        format!("0{raw}")
    } else {
        raw.to_string()
    };
    hex::decode(normalized).ok()
}

fn parse_hex_u256(input: &str) -> Option<U256> {
    let bytes = parse_hex_bytes(input)?;
    Some(U256::from_be_slice(&bytes))
}

fn canonical_address(input: &str) -> Option<String> {
    let raw = input.trim().trim_start_matches("0x");
    if raw.len() != 40 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", raw.to_ascii_lowercase()))
}

fn address_from_hex(input: &str) -> Option<Address> {
    let canonical = canonical_address(input)?;
    let bytes = parse_hex_bytes(&canonical)?;
    if bytes.len() != 20 {
        return None;
    }
    Some(Address::from_slice(&bytes))
}

fn address_to_hex(address: Address) -> String {
    format!("0x{}", hex::encode(address.as_slice()))
}

fn b256_to_hex(value: &B256) -> String {
    format!("0x{}", hex::encode(value.as_slice()))
}

fn bytes_to_hex(data: &[u8]) -> String {
    format!("0x{}", hex::encode(data))
}

fn u256_to_hex(value: U256) -> String {
    format!("0x{value:x}")
}

fn hex_to_u64_be(bytes: &[u8]) -> Result<u64, String> {
    if bytes.len() > 8 {
        return Err("integer overflows u64".to_string());
    }
    let mut out = 0u64;
    for b in bytes {
        out = (out << 8) | (*b as u64);
    }
    Ok(out)
}

fn hex_to_u128_be(bytes: &[u8]) -> Result<u128, String> {
    if bytes.len() > 16 {
        return Err("integer overflows u128".to_string());
    }
    let mut out = 0u128;
    for b in bytes {
        out = (out << 8) | (*b as u128);
    }
    Ok(out)
}

fn rlp_u64(rlp: &Rlp<'_>, idx: usize) -> Result<u64, String> {
    let raw: Vec<u8> = rlp
        .val_at(idx)
        .map_err(|e| format!("invalid u64 field {idx}: {e}"))?;
    hex_to_u64_be(&raw)
}

fn rlp_u128(rlp: &Rlp<'_>, idx: usize) -> Result<u128, String> {
    let raw: Vec<u8> = rlp
        .val_at(idx)
        .map_err(|e| format!("invalid u128 field {idx}: {e}"))?;
    hex_to_u128_be(&raw)
}

fn rlp_u256(rlp: &Rlp<'_>, idx: usize) -> Result<U256, String> {
    let raw: Vec<u8> = rlp
        .val_at(idx)
        .map_err(|e| format!("invalid u256 field {idx}: {e}"))?;
    Ok(U256::from_be_slice(&raw))
}

fn rlp_bytes(rlp: &Rlp<'_>, idx: usize) -> Result<Bytes, String> {
    let raw: Vec<u8> = rlp
        .val_at(idx)
        .map_err(|e| format!("invalid bytes field {idx}: {e}"))?;
    Ok(Bytes::from(raw))
}

fn rlp_optional_address(rlp: &Rlp<'_>, idx: usize) -> Result<Option<Address>, String> {
    let raw: Vec<u8> = rlp
        .val_at(idx)
        .map_err(|e| format!("invalid address field {idx}: {e}"))?;
    if raw.is_empty() {
        Ok(None)
    } else if raw.len() == 20 {
        Ok(Some(Address::from_slice(&raw)))
    } else {
        Err(format!(
            "invalid address length at field {idx}: {}",
            raw.len()
        ))
    }
}

fn rlp_b256_list(rlp: &Rlp<'_>, idx: usize) -> Result<Vec<B256>, String> {
    let list = rlp
        .at(idx)
        .map_err(|e| format!("invalid list field {idx}: {e}"))?;
    if !list.is_list() {
        return Err(format!("field {idx} must be a list"));
    }

    let count = list
        .item_count()
        .map_err(|e| format!("failed counting list items at field {idx}: {e}"))?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let raw: Vec<u8> = list
            .val_at(i)
            .map_err(|e| format!("invalid b256 element {i} at field {idx}: {e}"))?;
        if raw.len() != 32 {
            return Err(format!(
                "invalid b256 length at field {idx}[{i}]: {} (expected 32)",
                raw.len()
            ));
        }
        out.push(B256::from_slice(&raw));
    }
    Ok(out)
}

fn rlp_signed_authorization_list(
    rlp: &Rlp<'_>,
    idx: usize,
) -> Result<Vec<SignedAuthorization>, String> {
    let list = rlp
        .at(idx)
        .map_err(|e| format!("invalid authorization list field {idx}: {e}"))?;
    if !list.is_list() {
        return Err(format!("field {idx} must be an authorization list"));
    }

    let count = list
        .item_count()
        .map_err(|e| format!("failed counting authorization list items at field {idx}: {e}"))?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let auth = list
            .at(i)
            .map_err(|e| format!("invalid authorization entry {i} in field {idx}: {e}"))?;
        if !auth.is_list() {
            return Err(format!(
                "authorization entry {i} in field {idx} must be a list"
            ));
        }

        let chain_id = rlp_u256(&auth, 0)?;
        let address_raw: Vec<u8> = auth
            .val_at(1)
            .map_err(|e| format!("invalid authorization address at field {idx}[{i}]: {e}"))?;
        if address_raw.len() != 20 {
            return Err(format!(
                "invalid authorization address length at field {idx}[{i}]: {}",
                address_raw.len()
            ));
        }
        let nonce = rlp_u64(&auth, 2)?;
        let y_parity_raw = rlp_u64(&auth, 3)?;
        if y_parity_raw > 1 {
            return Err(format!(
                "invalid y_parity at field {idx}[{i}]: {y_parity_raw} (expected 0 or 1)"
            ));
        }
        let r = rlp_u256(&auth, 4)?;
        let s = rlp_u256(&auth, 5)?;

        let inner = Authorization {
            chain_id,
            address: Address::from_slice(&address_raw),
            nonce,
        };
        out.push(SignedAuthorization::new_unchecked(
            inner,
            y_parity_raw as u8,
            r,
            s,
        ));
    }

    Ok(out)
}

fn blob_base_fee_per_gas() -> u128 {
    calc_blob_gasprice(0, BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE)
}

fn decode_raw_tx(raw_tx: &str) -> Result<DecodedRawTx, String> {
    let bytes =
        parse_hex_bytes(raw_tx).ok_or_else(|| "raw transaction is not valid hex".to_string())?;
    if bytes.is_empty() {
        return Err("raw transaction payload is empty".to_string());
    }

    if bytes[0] >= 0xc0 {
        let rlp = Rlp::new(&bytes);
        if !rlp.is_list() {
            return Err("legacy transaction is not an RLP list".to_string());
        }
        let nonce = rlp_u64(&rlp, 0)?;
        let gas_price = rlp_u128(&rlp, 1)?;
        let gas_limit = rlp_u64(&rlp, 2)?;
        let to = rlp_optional_address(&rlp, 3)?;
        let value = rlp_u256(&rlp, 4)?;
        let data = rlp_bytes(&rlp, 5)?;
        return Ok(DecodedRawTx {
            tx_type: 0,
            chain_id: None,
            nonce,
            gas_limit,
            gas_price,
            max_priority_fee_per_gas: None,
            max_fee_per_blob_gas: None,
            blob_hashes: Vec::new(),
            authorizations: Vec::new(),
            to,
            value,
            data,
        });
    }

    let tx_type = bytes[0];
    let payload = &bytes[1..];
    let rlp = Rlp::new(payload);
    if !rlp.is_list() {
        return Err(format!(
            "typed transaction 0x{tx_type:02x} is not an RLP list"
        ));
    }

    match tx_type {
        // EIP-2930
        0x01 => {
            let chain_id = Some(rlp_u64(&rlp, 0)?);
            let nonce = rlp_u64(&rlp, 1)?;
            let gas_price = rlp_u128(&rlp, 2)?;
            let gas_limit = rlp_u64(&rlp, 3)?;
            let to = rlp_optional_address(&rlp, 4)?;
            let value = rlp_u256(&rlp, 5)?;
            let data = rlp_bytes(&rlp, 6)?;
            Ok(DecodedRawTx {
                tx_type,
                chain_id,
                nonce,
                gas_limit,
                gas_price,
                max_priority_fee_per_gas: None,
                max_fee_per_blob_gas: None,
                blob_hashes: Vec::new(),
                authorizations: Vec::new(),
                to,
                value,
                data,
            })
        }
        // EIP-1559
        0x02 => {
            let chain_id = Some(rlp_u64(&rlp, 0)?);
            let nonce = rlp_u64(&rlp, 1)?;
            let max_priority = rlp_u128(&rlp, 2)?;
            let max_fee = rlp_u128(&rlp, 3)?;
            let gas_limit = rlp_u64(&rlp, 4)?;
            let to = rlp_optional_address(&rlp, 5)?;
            let value = rlp_u256(&rlp, 6)?;
            let data = rlp_bytes(&rlp, 7)?;
            Ok(DecodedRawTx {
                tx_type,
                chain_id,
                nonce,
                gas_limit,
                gas_price: max_fee,
                max_priority_fee_per_gas: Some(max_priority),
                max_fee_per_blob_gas: None,
                blob_hashes: Vec::new(),
                authorizations: Vec::new(),
                to,
                value,
                data,
            })
        }
        // EIP-4844 (blob transaction)
        0x03 => {
            let chain_id = Some(rlp_u64(&rlp, 0)?);
            let nonce = rlp_u64(&rlp, 1)?;
            let max_priority = rlp_u128(&rlp, 2)?;
            let max_fee = rlp_u128(&rlp, 3)?;
            let gas_limit = rlp_u64(&rlp, 4)?;
            let to = rlp_optional_address(&rlp, 5)?;
            let value = rlp_u256(&rlp, 6)?;
            let data = rlp_bytes(&rlp, 7)?;
            let access_list = rlp
                .at(8)
                .map_err(|e| format!("invalid access list field 8: {e}"))?;
            if !access_list.is_list() {
                return Err("invalid access list field 8: expected list".to_string());
            }
            let max_fee_per_blob_gas = rlp_u128(&rlp, 9)?;
            let blob_hashes = rlp_b256_list(&rlp, 10)?;

            Ok(DecodedRawTx {
                tx_type,
                chain_id,
                nonce,
                gas_limit,
                gas_price: max_fee,
                max_priority_fee_per_gas: Some(max_priority),
                max_fee_per_blob_gas: Some(max_fee_per_blob_gas),
                blob_hashes,
                authorizations: Vec::new(),
                to,
                value,
                data,
            })
        }
        // EIP-7702 (set-code transaction)
        0x04 => {
            let chain_id = Some(rlp_u64(&rlp, 0)?);
            let nonce = rlp_u64(&rlp, 1)?;
            let max_priority = rlp_u128(&rlp, 2)?;
            let max_fee = rlp_u128(&rlp, 3)?;
            let gas_limit = rlp_u64(&rlp, 4)?;
            let to = rlp_optional_address(&rlp, 5)?;
            if to.is_none() {
                return Err("EIP-7702 transaction must include destination address".to_string());
            }
            let value = rlp_u256(&rlp, 6)?;
            let data = rlp_bytes(&rlp, 7)?;
            let access_list = rlp
                .at(8)
                .map_err(|e| format!("invalid access list field 8: {e}"))?;
            if !access_list.is_list() {
                return Err("invalid access list field 8: expected list".to_string());
            }
            let authorizations = rlp_signed_authorization_list(&rlp, 9)?;
            if authorizations.is_empty() {
                return Err("EIP-7702 authorization list must not be empty".to_string());
            }

            Ok(DecodedRawTx {
                tx_type,
                chain_id,
                nonce,
                gas_limit,
                gas_price: max_fee,
                max_priority_fee_per_gas: Some(max_priority),
                max_fee_per_blob_gas: None,
                blob_hashes: Vec::new(),
                authorizations,
                to,
                value,
                data,
            })
        }
        _ => Err(format!(
            "unsupported typed transaction: 0x{tx_type:02x} (supported: legacy, 0x01, 0x02, 0x03, 0x04)"
        )),
    }
}

fn pseudo_tx_hash(raw_tx: &str, counter: u64) -> String {
    let mut out = String::from("0x");
    for salt in 0u64..4 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        raw_tx.hash(&mut h);
        counter.hash(&mut h);
        salt.hash(&mut h);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

fn block_number_from_hash(hash: &str) -> Option<u64> {
    let raw = hash.strip_prefix("0x").unwrap_or(hash);
    if raw.len() != 64 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    // Reverse the +1 encoding from block_hash.
    parse_hex_u64(&raw[0..16]).and_then(|n| n.checked_sub(1))
}

fn tx_to_json(tx: &TxRecord, chain_id: u64) -> Value {
    let block_number = tx
        .block_number
        .map(hex_u64)
        .map(Value::String)
        .unwrap_or(Value::Null);
    let block_hash = tx
        .block_hash
        .as_ref()
        .map(|v| Value::String(v.clone()))
        .unwrap_or(Value::Null);
    let tx_index = tx
        .transaction_index
        .map(hex_u64)
        .map(Value::String)
        .unwrap_or(Value::Null);
    let max_priority_fee = tx
        .max_priority_fee_per_gas
        .map(hex_u128)
        .map(Value::String)
        .unwrap_or(Value::Null);
    let max_fee_per_blob_gas = tx
        .max_fee_per_blob_gas
        .map(hex_u128)
        .map(Value::String)
        .unwrap_or(Value::Null);
    serde_json::json!({
      "hash": tx.hash,
      "nonce": hex_u64(tx.nonce),
      "blockHash": block_hash,
      "blockNumber": block_number,
      "transactionIndex": tx_index,
      "from": tx.from,
      "to": tx.to,
      "value": tx.value,
      "gas": hex_u64(tx.gas),
      "gasPrice": hex_u128(tx.gas_price),
      "maxPriorityFeePerGas": max_priority_fee,
      "maxFeePerBlobGas": max_fee_per_blob_gas,
      "blobVersionedHashes": tx.blob_versioned_hashes,
      "authorizationListLength": hex_u64(tx.authorization_list_len as u64),
      "input": tx.input,
      "type": tx.tx_type,
      "chainId": hex_u64(chain_id),
      "v": "0x1",
      "r": "0x0",
      "s": "0x0"
    })
}

fn account_nonce_from_db(db: &mut InMemoryDB, address: Address) -> u64 {
    db.basic(address)
        .ok()
        .flatten()
        .map(|info| info.nonce)
        .unwrap_or(0)
}

fn account_balance_from_db(db: &mut InMemoryDB, address: Address) -> U256 {
    db.basic(address)
        .ok()
        .flatten()
        .map(|info| info.balance)
        .unwrap_or(U256::ZERO)
}

fn set_account_balance_in_db(db: &mut InMemoryDB, address: Address, balance: U256) {
    let mut info = db.basic(address).ok().flatten().unwrap_or_default();
    info.balance = balance;
    db.insert_account_info(address, info);
}

fn code_hex_from_db(db: &mut InMemoryDB, address: Address) -> String {
    let Some(info) = db.basic(address).ok().flatten() else {
        return "0x".to_string();
    };

    if let Some(code) = info.code {
        let raw = code.original_bytes();
        if raw.is_empty() {
            return "0x".to_string();
        }
        return bytes_to_hex(raw.as_ref());
    }

    if info.code_hash.is_zero() {
        return "0x".to_string();
    }

    match db.code_by_hash(info.code_hash) {
        Ok(code) => {
            let raw = code.original_bytes();
            if raw.is_empty() {
                "0x".to_string()
            } else {
                bytes_to_hex(raw.as_ref())
            }
        }
        Err(_) => "0x".to_string(),
    }
}

fn storage_hex_from_db(db: &mut InMemoryDB, address: Address, slot: U256) -> String {
    match db.storage(address, slot) {
        Ok(value) => format!("0x{value:064x}"),
        Err(_) => "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    }
}

fn execute_evm_transaction(
    state: &mut NodeState,
    caller: Address,
    decoded: &DecodedRawTx,
    commit: bool,
    block_number: u64,
    block_timestamp: u64,
) -> Result<EvmExecOutcome, String> {
    let mut db = std::mem::take(&mut state.evm_db);
    if db
        .basic(caller)
        .map_err(|e| format!("failed reading caller account: {e}"))?
        .is_none()
    {
        db.insert_account_info(caller, AccountInfo::default());
    }

    let ctx = Context::mainnet()
        .with_db(db)
        .modify_cfg_chained(|cfg| {
            cfg.set_spec_and_mainnet_gas_params(SpecId::PRAGUE);
            cfg.chain_id = state.chain_id;
            cfg.tx_chain_id_check = true;
        })
        .modify_block_chained(|block| {
            block.number = U256::from(block_number);
            block.timestamp = U256::from(block_timestamp);
            block.basefee = 0;
            block.gas_limit = 30_000_000;
        });

    let mut evm = ctx.build_mainnet();

    let mut tx_builder = TxEnv::builder()
        .tx_type(Some(decoded.tx_type))
        .caller(caller)
        .gas_limit(decoded.gas_limit.max(21_000))
        .max_fee_per_gas(decoded.gas_price)
        .nonce(decoded.nonce)
        .chain_id(decoded.chain_id.or(Some(state.chain_id)))
        .value(decoded.value)
        .data(decoded.data.clone());

    if let Some(max_priority_fee) = decoded.max_priority_fee_per_gas {
        tx_builder = tx_builder.gas_priority_fee(Some(max_priority_fee));
    }
    if let Some(max_fee_per_blob_gas) = decoded.max_fee_per_blob_gas {
        tx_builder = tx_builder
            .max_fee_per_blob_gas(max_fee_per_blob_gas)
            .blob_hashes(decoded.blob_hashes.clone());
    }
    if !decoded.authorizations.is_empty() {
        tx_builder = tx_builder.authorization_list_signed(decoded.authorizations.clone());
    }
    tx_builder = match decoded.to {
        Some(to) => tx_builder.kind(TxKind::Call(to)),
        None => tx_builder.kind(TxKind::Create),
    };

    let tx_env = tx_builder.build_fill();

    let exec_result = if commit {
        evm.transact_commit(tx_env)
            .map_err(|e| format!("evm transaction failed: {e}"))?
    } else {
        evm.transact(tx_env)
            .map_err(|e| format!("evm call failed: {e}"))?
            .result
    };

    state.evm_db = evm.ctx.journaled_state.database;

    Ok(EvmExecOutcome {
        gas_used: exec_result.gas_used(),
        status: exec_result.is_success(),
        output: exec_result.output().cloned().unwrap_or_default(),
        contract_address: exec_result.created_address(),
        logs: exec_result.logs().to_vec(),
    })
}

fn tx_logs_from_revm(tx_hash: &str, logs: &[revm::primitives::Log]) -> Vec<TxLogRecord> {
    logs.iter()
        .map(|log| TxLogRecord {
            address: address_to_hex(log.address),
            topics: log.data.topics().iter().map(b256_to_hex).collect(),
            data: bytes_to_hex(log.data.data.as_ref()),
            block_number: None,
            block_hash: None,
            transaction_hash: Some(tx_hash.to_string()),
            transaction_index: None,
            log_index: None,
        })
        .collect()
}

fn tx_log_to_json(log: &TxLogRecord) -> Value {
    serde_json::json!({
      "address": log.address,
      "topics": log.topics,
      "data": log.data,
      "blockNumber": log.block_number.map(hex_u64),
      "blockHash": log.block_hash,
      "transactionHash": log.transaction_hash,
      "transactionIndex": log.transaction_index.map(hex_u64),
      "logIndex": log.log_index.map(hex_u64),
      "removed": false
    })
}

fn tx_receipt_json(receipt: &TxReceiptRecord) -> Option<Value> {
    let block_number = receipt.block_number?;
    let block_hash = receipt.block_hash.clone()?;
    let tx_index = receipt.transaction_index.unwrap_or(0);
    let logs: Vec<Value> = receipt.logs.iter().map(tx_log_to_json).collect();
    Some(serde_json::json!({
      "transactionHash": receipt.tx_hash,
      "transactionIndex": hex_u64(tx_index),
      "blockHash": block_hash,
      "blockNumber": hex_u64(block_number),
      "from": receipt.from,
      "to": receipt.to,
      "cumulativeGasUsed": hex_u64(receipt.cumulative_gas_used),
      "gasUsed": hex_u64(receipt.gas_used),
      "contractAddress": receipt.contract_address,
      "logs": logs,
      "logsBloom": receipt.logs_bloom,
      "status": if receipt.status { "0x1" } else { "0x0" },
      "type": receipt.tx_type,
      "effectiveGasPrice": hex_u64(receipt.effective_gas_price)
    }))
}

fn market_nft_to_json(nft: &MarketNft) -> Value {
    serde_json::json!({
      "tokenId": hex_u64(nft.token_id),
      "name": nft.name,
      "description": nft.description,
      "image": nft.image,
      "owner": nft.owner,
      "listed": nft.listed,
      "priceWei": hex_u128(nft.price_wei),
      "createdAtUnixS": nft.created_at_unix_s,
      "lastTxHash": nft.last_tx_hash
    })
}

fn market_receipt_signature(receipt: &MarketReceipt) -> String {
    let payload = format!(
        "eth2077:market:receipt:v1:{}:{}:{}:{}:{}:{}:{}:{}",
        receipt.chain_id,
        receipt.tx_hash,
        receipt.token_id,
        receipt.buyer,
        receipt.seller,
        receipt.price_wei,
        receipt.block_number_hint,
        receipt.timestamp_unix_s
    );
    pseudo_tx_hash(
        &payload,
        receipt
            .chain_id
            .saturating_add(receipt.token_id)
            .saturating_add(receipt.block_number_hint)
            .saturating_add(receipt.timestamp_unix_s),
    )
}

fn market_receipt_to_json(receipt: &MarketReceipt) -> Value {
    serde_json::json!({
      "txHash": receipt.tx_hash,
      "tokenId": hex_u64(receipt.token_id),
      "buyer": receipt.buyer,
      "seller": receipt.seller,
      "priceWei": hex_u128(receipt.price_wei),
      "chainId": hex_u64(receipt.chain_id),
      "blockNumberHint": hex_u64(receipt.block_number_hint),
      "timestampUnixS": receipt.timestamp_unix_s,
      "signature": receipt.signature,
      "scheme": "ETH2077-MARKET-RECEIPT-V1"
    })
}

fn market_nft_list_json(state: &NodeState) -> Value {
    let mut nfts: Vec<MarketNft> = state.market_nfts.values().cloned().collect();
    nfts.sort_by_key(|n| n.token_id);
    Value::Array(nfts.iter().map(market_nft_to_json).collect())
}

fn eth2077_status_json(state: &NodeState, now_unix_s: u64) -> Value {
    let head_block = state.current_height;
    let finalized_block = state.finalized_height;
    let finality_lag_blocks = head_block.saturating_sub(finalized_block);
    let finality_lag_ms_estimate = finality_lag_blocks.saturating_mul(12_000);

    let window_size_blocks = 60u64;
    let window_start = head_block.saturating_sub(window_size_blocks.saturating_sub(1));
    let window_block_count = head_block.saturating_sub(window_start).saturating_add(1);
    let window_span_seconds = window_block_count.saturating_mul(12).max(1);

    let mut window_txs = 0u64;
    let mut non_empty_blocks = 0u64;
    for block in window_start..=head_block {
        let count = state
            .block_txs
            .get(&block)
            .map(|v| v.len() as u64)
            .unwrap_or(0);
        window_txs = window_txs.saturating_add(count);
        if count > 0 {
            non_empty_blocks = non_empty_blocks.saturating_add(1);
        }
    }

    let latest_block_txs = state
        .block_txs
        .get(&head_block)
        .map(|v| v.len() as u64)
        .unwrap_or(0);
    let finalized_block_txs = state
        .block_txs
        .get(&finalized_block)
        .map(|v| v.len() as u64)
        .unwrap_or(0);

    let avg_tps_window = window_txs as f64 / window_span_seconds as f64;
    let latest_block_timestamp = state
        .block_timestamps
        .get(&head_block)
        .copied()
        .unwrap_or(state.started_at_unix_s);

    serde_json::json!({
      "schema": "eth2077/status-v1",
      "timestampUnixS": now_unix_s,
      "nodeId": state.node_id,
      "nodes": state.nodes,
      "chainId": hex_u64(state.chain_id),
      "headBlock": hex_u64(head_block),
      "finalizedBlock": hex_u64(finalized_block),
      "finalityLagBlocks": hex_u64(finality_lag_blocks),
      "finalityLagMsEstimate": finality_lag_ms_estimate,
      "latestBlockTimestamp": latest_block_timestamp,
      "uptimeSeconds": now_unix_s.saturating_sub(state.started_at_unix_s),
      "peersConnected": state.peers_connected,
      "peersTarget": state.peers_target,
      "txPoolPending": state.pending_txs.len(),
      "knownTransactions": state.txs.len(),
      "window": {
        "startBlock": hex_u64(window_start),
        "endBlock": hex_u64(head_block),
        "blockCount": window_block_count,
        "spanSeconds": window_span_seconds,
        "txs": window_txs,
        "nonEmptyBlocks": non_empty_blocks,
        "avgTpsApprox": avg_tps_window
      },
      "latestBlockTxs": latest_block_txs,
      "finalizedBlockTxs": finalized_block_txs,
      "capacity": {
        "ingressTps": state.ingress_capacity_tps,
        "executionTps": state.execution_capacity_tps,
        "oobTps": state.oob_capacity_tps,
        "bridgeReplaySafe": state.bridge_replay_safe
      },
      "app": {
        "marketNfts": state.market_nfts.len(),
        "marketReceipts": state.market_receipts.len()
      }
    })
}

fn empty_block_template(number: u64, timestamp: u64) -> Value {
    let parent = if number == 0 {
        "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
    } else {
        block_hash(number.saturating_sub(1))
    };
    serde_json::json!({
      "number": hex_u64(number),
      "hash": block_hash(number),
      "parentHash": parent,
      "nonce": "0x0000000000000000",
      "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
      "logsBloom": format!("0x{}", "0".repeat(512)),
      "transactionsRoot": "0x56e81f171bcc55a6ff8345e69d706d3b9f2f4f4b6a4f0f4d8f4f0f4d8f4f0f4d",
      "stateRoot": format!("0x{}", "0".repeat(64)),
      "receiptsRoot": format!("0x{}", "0".repeat(64)),
      "miner": "0x0000000000000000000000000000000000000000",
      "difficulty": "0x0",
      "totalDifficulty": "0x0",
      "extraData": "0x65746832303737",
      "size": "0x0",
      "gasLimit": "0x1c9c380",
      "gasUsed": "0x0",
      "timestamp": hex_u64(timestamp),
      "transactions": [],
      "uncles": [],
      "baseFeePerGas": "0x3b9aca00"
    })
}

fn block_by_tag(state: &NodeState, tag: &str) -> Option<u64> {
    match tag {
        "latest" | "pending" => Some(state.current_height),
        "finalized" => Some(state.finalized_height),
        "earliest" => Some(0),
        _ => parse_hex_u64(tag),
    }
}

fn build_block_response(state: &NodeState, number: u64, full_txs: bool) -> Value {
    let ts = state
        .block_timestamps
        .get(&number)
        .copied()
        .unwrap_or(state.started_at_unix_s);
    let tx_hashes = state.block_txs.get(&number).cloned().unwrap_or_default();
    let tx_payload = if full_txs {
        Value::Array(
            tx_hashes
                .iter()
                .filter_map(|h| state.txs.get(h))
                .map(|tx| tx_to_json(tx, state.chain_id))
                .collect(),
        )
    } else {
        Value::Array(tx_hashes.iter().map(|h| Value::String(h.clone())).collect())
    };

    let mut block = empty_block_template(number, ts);
    if let Some(obj) = block.as_object_mut() {
        obj.insert("transactions".to_string(), tx_payload);
        obj.insert(
            "gasUsed".to_string(),
            Value::String(hex_u64((tx_hashes.len() as u64) * 21_000)),
        );
        obj.insert(
            "size".to_string(),
            Value::String(hex_u64(120 + (tx_hashes.len() as u64) * 128)),
        );
    }
    block
}

fn parse_u64_value(input: &Value) -> Option<u64> {
    if let Some(n) = input.as_u64() {
        return Some(n);
    }
    if let Some(s) = input.as_str() {
        return parse_hex_u64(s).or_else(|| s.parse::<u64>().ok());
    }
    None
}

fn parse_u128_value(input: &Value) -> Option<u128> {
    if let Some(n) = input.as_u64() {
        return Some(n as u128);
    }
    if let Some(s) = input.as_str() {
        return parse_hex_u128(s).or_else(|| s.parse::<u128>().ok());
    }
    None
}

fn parse_u256_value(input: &Value) -> Option<U256> {
    if let Some(n) = input.as_u64() {
        return Some(U256::from(n));
    }
    if let Some(s) = input.as_str() {
        return parse_hex_u256(s).or_else(|| s.parse::<u128>().ok().map(U256::from));
    }
    None
}

fn normalize_hash32(input: &str) -> Option<String> {
    let raw = input.trim().strip_prefix("0x").unwrap_or(input.trim());
    if raw.len() != 64 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", raw.to_ascii_lowercase()))
}

fn dedup_tx_meta_array(value: &Value) -> Vec<EngineTxMeta> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let Some(items) = value.as_array() else {
        return out;
    };
    for item in items {
        let entry = if let Some(hash) = item.as_str().and_then(normalize_hash32) {
            Some(EngineTxMeta {
                hash,
                ..EngineTxMeta::default()
            })
        } else if let Some(obj) = item.as_object() {
            let hash = obj
                .get("hash")
                .or_else(|| obj.get("txHash"))
                .or_else(|| obj.get("transactionHash"))
                .and_then(Value::as_str)
                .and_then(normalize_hash32);
            hash.map(|h| EngineTxMeta {
                hash: h,
                from: obj
                    .get("from")
                    .and_then(Value::as_str)
                    .and_then(canonical_address),
                nonce: obj.get("nonce").and_then(parse_u64_value),
                gas_limit: obj
                    .get("gas")
                    .or_else(|| obj.get("gasLimit"))
                    .and_then(parse_u64_value),
                gas_price: obj
                    .get("maxFeePerGas")
                    .or_else(|| obj.get("gasPrice"))
                    .and_then(parse_u128_value),
                value: obj.get("value").and_then(parse_u256_value),
            })
        } else {
            None
        };

        let Some(meta) = entry else {
            continue;
        };
        if seen.insert(meta.hash.clone()) {
            out.push(meta);
        }
    }
    out
}

fn payload_attributes_from_engine_request<'a>(req: &'a Value) -> Option<&'a Value> {
    req.get("payloadAttributes").or_else(|| {
        req.get("params")
            .and_then(Value::as_array)
            .and_then(|params| params.get(1))
            .and_then(|attrs| attrs.get("payloadAttributes"))
    })
}

fn execution_payload_from_engine_request<'a>(req: &'a Value) -> Option<&'a Value> {
    req.get("executionPayload").or_else(|| {
        req.get("params")
            .and_then(Value::as_array)
            .and_then(|params| params.first())
            .and_then(|payload| payload.get("executionPayload").or(Some(payload)))
    })
}

fn engine_request_has_inclusion_list(req: &Value) -> bool {
    payload_attributes_from_engine_request(req)
        .map(|attrs| {
            attrs.get("inclusionListTransactions").is_some() || attrs.get("inclusionList").is_some()
        })
        .unwrap_or(false)
}

fn engine_collect_inclusion_list_entries(req: &Value) -> Vec<EngineTxMeta> {
    let Some(attrs) = payload_attributes_from_engine_request(req) else {
        return Vec::new();
    };
    if let Some(list) = attrs.get("inclusionListTransactions") {
        return dedup_tx_meta_array(list);
    }
    if let Some(list) = attrs
        .get("inclusionList")
        .and_then(|il| il.get("transactions").or(Some(il)))
    {
        return dedup_tx_meta_array(list);
    }
    Vec::new()
}

fn engine_collect_payload_transaction_entries(req: &Value) -> Vec<EngineTxMeta> {
    if let Some(payload) = execution_payload_from_engine_request(req) {
        if let Some(txs) = payload.get("transactions") {
            return dedup_tx_meta_array(txs);
        }
    }
    if let Some(txs) = req.get("transactions") {
        return dedup_tx_meta_array(txs);
    }
    Vec::new()
}

fn dedup_string_array(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let Some(raw) = item.as_str() else {
            continue;
        };
        let member = canonical_address(raw).unwrap_or_else(|| raw.trim().to_ascii_lowercase());
        if seen.insert(member.clone()) {
            out.push(member);
        }
    }
    out
}

fn engine_collect_committee_members(req: &Value) -> Vec<String> {
    let Some(attrs) = payload_attributes_from_engine_request(req) else {
        return Vec::new();
    };
    if let Some(committee) = attrs.get("inclusionListCommittee") {
        return dedup_string_array(committee);
    }
    if let Some(committee) = attrs.get("committee") {
        return dedup_string_array(committee);
    }
    if let Some(committee) = attrs
        .get("inclusionList")
        .and_then(|il| il.get("committee"))
    {
        return dedup_string_array(committee);
    }
    Vec::new()
}

fn engine_slot_from_payload_attributes(req: &Value) -> Option<u64> {
    payload_attributes_from_engine_request(req)
        .and_then(|attrs| attrs.get("slot"))
        .and_then(parse_u64_value)
}

fn engine_view_id_from_payload_attributes(req: &Value) -> Option<u64> {
    payload_attributes_from_engine_request(req)
        .and_then(|attrs| attrs.get("viewId").or_else(|| attrs.get("view")))
        .and_then(parse_u64_value)
}

fn engine_payload_slot(req: &Value) -> Option<u64> {
    req.get("slot")
        .and_then(parse_u64_value)
        .or_else(|| {
            execution_payload_from_engine_request(req)
                .and_then(|payload| payload.get("slot").and_then(parse_u64_value))
        })
        .or_else(|| {
            execution_payload_from_engine_request(req)
                .and_then(|payload| payload.get("blockNumber").and_then(parse_u64_value))
        })
}

fn engine_inclusion_list_root(hashes: &[String]) -> String {
    let joined = hashes.join(",");
    bytes_to_hex(revm::primitives::keccak256(joined.as_bytes()).as_slice())
}

fn normalize_hash32_value(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).and_then(normalize_hash32)
}

fn engine_7732_payload_header_source<'a>(req: &'a Value) -> Option<&'a Value> {
    req.get("payloadHeader")
        .or_else(|| req.get("signedExecutionPayloadHeader"))
        .or_else(|| req.get("executionPayloadHeader"))
        .or_else(|| req.get("params").and_then(Value::as_array).and_then(|a| a.first()))
}

fn engine_7732_payload_envelope_source<'a>(req: &'a Value) -> Option<&'a Value> {
    req.get("payloadEnvelope")
        .or_else(|| req.get("signedExecutionPayloadEnvelope"))
        .or_else(|| req.get("executionPayloadEnvelope"))
        .or_else(|| req.get("params").and_then(Value::as_array).and_then(|a| a.first()))
}

fn engine_7732_payload_header_root_from_value(value: &Value) -> Option<String> {
    let message = value.get("message").unwrap_or(value);
    normalize_hash32_value(
        message
            .get("payloadHeaderRoot")
            .or_else(|| message.get("executionPayloadHeaderRoot"))
            .or_else(|| message.get("headerRoot"))
            .or_else(|| message.get("payloadRoot"))
            .or_else(|| message.get("root")),
    )
}

fn engine_7732_parse_header_record(
    req: &Value,
    fallback_slot: u64,
) -> Result<Engine7732HeaderRecord, String> {
    let source = engine_7732_payload_header_source(req)
        .ok_or_else(|| "missing payload header object".to_string())?;
    let message = source.get("message").unwrap_or(source);

    let slot = message
        .get("slot")
        .and_then(parse_u64_value)
        .or_else(|| req.get("slot").and_then(parse_u64_value))
        .unwrap_or(fallback_slot);
    let payload_header_root = engine_7732_payload_header_root_from_value(source).unwrap_or_else(|| {
        let encoded = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string());
        bytes_to_hex(revm::primitives::keccak256(encoded.as_bytes()).as_slice())
    });

    Ok(Engine7732HeaderRecord {
        slot,
        payload_header_root,
        parent_beacon_block_root: normalize_hash32_value(
            message
                .get("parentBeaconBlockRoot")
                .or_else(|| message.get("parentRoot")),
        ),
        execution_block_hash: normalize_hash32_value(
            message
                .get("executionBlockHash")
                .or_else(|| message.get("blockHash")),
        ),
        proposer: message
            .get("proposer")
            .or_else(|| message.get("proposerAddress"))
            .or_else(|| message.get("builder"))
            .and_then(Value::as_str)
            .map(|raw| canonical_address(raw).unwrap_or_else(|| raw.to_string())),
        bid_value_wei: message
            .get("bidValue")
            .or_else(|| message.get("bidValueWei"))
            .and_then(parse_u256_value),
        view_id: message
            .get("viewId")
            .or_else(|| message.get("view"))
            .and_then(parse_u64_value),
        received_at_unix_s: req
            .get("receivedAtUnixS")
            .or_else(|| message.get("receivedAtUnixS"))
            .and_then(parse_u64_value)
            .unwrap_or_else(now_unix_s),
    })
}

fn engine_7732_parse_envelope_record(
    req: &Value,
    fallback_slot: u64,
) -> Result<Engine7732EnvelopeRecord, String> {
    let source = engine_7732_payload_envelope_source(req)
        .ok_or_else(|| "missing payload envelope object".to_string())?;
    let message = source.get("message").unwrap_or(source);

    let slot = message
        .get("slot")
        .and_then(parse_u64_value)
        .or_else(|| req.get("slot").and_then(parse_u64_value))
        .unwrap_or(fallback_slot);

    let payload_header_root = engine_7732_payload_header_root_from_value(source).unwrap_or_else(|| {
        let encoded = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string());
        bytes_to_hex(revm::primitives::keccak256(encoded.as_bytes()).as_slice())
    });

    Ok(Engine7732EnvelopeRecord {
        slot,
        payload_header_root,
        execution_block_hash: normalize_hash32_value(
            message
                .get("executionBlockHash")
                .or_else(|| message.get("blockHash")),
        ),
        payload_body_hash: normalize_hash32_value(
            message
                .get("payloadBodyHash")
                .or_else(|| message.get("payloadHash"))
                .or_else(|| message.get("bodyRoot")),
        ),
        signer: message
            .get("signer")
            .or_else(|| message.get("builder"))
            .or_else(|| message.get("revealer"))
            .and_then(Value::as_str)
            .map(|raw| canonical_address(raw).unwrap_or_else(|| raw.to_string())),
        data_available: message
            .get("dataAvailable")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        revealed_at_unix_s: req
            .get("revealedAtUnixS")
            .or_else(|| message.get("revealedAtUnixS"))
            .and_then(parse_u64_value)
            .unwrap_or_else(now_unix_s),
    })
}

fn engine_7732_header_to_json(header: &Engine7732HeaderRecord) -> Value {
    serde_json::json!({
      "slot": hex_u64(header.slot),
      "payloadHeaderRoot": header.payload_header_root,
      "parentBeaconBlockRoot": header.parent_beacon_block_root,
      "executionBlockHash": header.execution_block_hash,
      "proposer": header.proposer,
      "bidValue": header.bid_value_wei.map(u256_to_hex),
      "viewId": header.view_id.map(hex_u64),
      "receivedAtUnixS": header.received_at_unix_s
    })
}

fn engine_7732_envelope_to_json(envelope: &Engine7732EnvelopeRecord) -> Value {
    serde_json::json!({
      "slot": hex_u64(envelope.slot),
      "payloadHeaderRoot": envelope.payload_header_root,
      "executionBlockHash": envelope.execution_block_hash,
      "payloadBodyHash": envelope.payload_body_hash,
      "signer": envelope.signer,
      "dataAvailable": envelope.data_available,
      "revealedAtUnixS": envelope.revealed_at_unix_s
    })
}

fn engine_7732_header_conflicts(
    existing: &Engine7732HeaderRecord,
    incoming: &Engine7732HeaderRecord,
) -> bool {
    existing.slot != incoming.slot
        || existing.payload_header_root != incoming.payload_header_root
        || existing.parent_beacon_block_root != incoming.parent_beacon_block_root
        || existing.execution_block_hash != incoming.execution_block_hash
        || existing.proposer != incoming.proposer
        || existing.bid_value_wei != incoming.bid_value_wei
        || existing.view_id != incoming.view_id
}

fn engine_7732_envelope_conflicts(
    existing: &Engine7732EnvelopeRecord,
    incoming: &Engine7732EnvelopeRecord,
) -> bool {
    existing.slot != incoming.slot
        || existing.payload_header_root != incoming.payload_header_root
        || existing.execution_block_hash != incoming.execution_block_hash
        || existing.payload_body_hash != incoming.payload_body_hash
        || existing.signer != incoming.signer
        || existing.data_available != incoming.data_available
}

fn engine_7732_penalty_to_json(record: &Engine7732PenaltyRecord) -> Value {
    serde_json::json!({
      "slot": hex_u64(record.slot),
      "state": record.state,
      "reason": record.reason,
      "lastStatus": record.last_status,
      "activatedAtUnixS": record.activated_at_unix_s,
      "recoveredAtUnixS": record.recovered_at_unix_s
    })
}

fn engine_7732_is_timeliness_violation(status: &str) -> bool {
    matches!(status, "WITHHELD" | "PARTIAL_WITHHOLD" | "LATE_REVEAL")
}

fn engine_7732_update_penalty_state(
    state: &mut NodeState,
    slot: u64,
    status: &str,
    reason: &str,
    current_unix_s: u64,
) -> Option<Engine7732PenaltyRecord> {
    if engine_7732_is_timeliness_violation(status) {
        let mut record = state.engine_7732_penalties_by_slot.get(&slot).cloned().unwrap_or(
            Engine7732PenaltyRecord {
                slot,
                state: "ACTIVE".to_string(),
                reason: reason.to_string(),
                last_status: status.to_string(),
                activated_at_unix_s: current_unix_s,
                recovered_at_unix_s: None,
            },
        );
        if record.state != "ACTIVE" {
            record.activated_at_unix_s = current_unix_s;
            record.recovered_at_unix_s = None;
        }
        record.state = "ACTIVE".to_string();
        record.reason = reason.to_string();
        record.last_status = status.to_string();
        state
            .engine_7732_penalties_by_slot
            .insert(slot, record.clone());
        Some(record)
    } else if let Some(existing) = state.engine_7732_penalties_by_slot.get_mut(&slot) {
        existing.last_status = status.to_string();
        if existing.state != "RECOVERED" {
            existing.state = "RECOVERED".to_string();
            existing.reason = reason.to_string();
            existing.recovered_at_unix_s = Some(current_unix_s);
        }
        Some(existing.clone())
    } else {
        None
    }
}

fn engine_7732_slot_deadline_unix_s(state: &NodeState, slot: u64) -> u64 {
    state
        .started_at_unix_s
        .saturating_add(slot.saturating_mul(12))
        .saturating_add(12)
}

fn engine_7732_current_unix_s_from_req(req: &Value) -> Option<u64> {
    req.get("currentUnixS")
        .and_then(parse_u64_value)
        .or_else(|| {
            req.get("params")
                .and_then(Value::as_array)
                .and_then(|params| params.get(1))
                .and_then(parse_u64_value)
        })
}

fn engine_7732_slot_status_response(state: &NodeState, slot: u64, current_unix_s: u64) -> Value {
    let header_roots = state
        .engine_7732_headers_by_slot
        .get(&slot)
        .cloned()
        .unwrap_or_default();
    let deadline = engine_7732_slot_deadline_unix_s(state, slot);
    let deadline_passed = current_unix_s > deadline;
    let mut revealed_on_time = Vec::new();
    let mut late_reveals = Vec::new();
    let mut pending_reveals = Vec::new();

    for root in &header_roots {
        match state.engine_7732_envelopes_by_root.get(root) {
            Some(env) if env.revealed_at_unix_s <= deadline => revealed_on_time.push(root.clone()),
            Some(env) => {
                let _ = env;
                late_reveals.push(root.clone())
            }
            None => pending_reveals.push(root.clone()),
        }
    }

    let orphan_roots: Vec<String> = state
        .engine_7732_envelopes_by_root
        .iter()
        .filter(|(_, env)| env.slot == slot && !state.engine_7732_headers_by_root.contains_key(&env.payload_header_root))
        .map(|(root, _)| root.clone())
        .collect();

    let aggregate_status = if header_roots.is_empty() && orphan_roots.is_empty() {
        "UNKNOWN"
    } else if header_roots.is_empty() && !orphan_roots.is_empty() {
        "ORPHAN_ENVELOPE"
    } else if pending_reveals.is_empty() && !late_reveals.is_empty() {
        "LATE_REVEAL"
    } else if pending_reveals.is_empty() {
        "REVEALED"
    } else if deadline_passed && revealed_on_time.is_empty() && late_reveals.is_empty() {
        "WITHHELD"
    } else if deadline_passed {
        "PARTIAL_WITHHOLD"
    } else if revealed_on_time.is_empty() {
        "HEADER_ONLY"
    } else {
        "PARTIAL_REVEAL"
    };
    let penalty = state
        .engine_7732_penalties_by_slot
        .get(&slot)
        .map(engine_7732_penalty_to_json)
        .unwrap_or(Value::Null);

    serde_json::json!({
      "slot": hex_u64(slot),
      "aggregateStatus": aggregate_status,
      "headerRoots": header_roots,
      "revealedOnTime": revealed_on_time,
      "lateReveals": late_reveals,
      "pendingReveals": pending_reveals,
      "orphanEnvelopes": orphan_roots,
      "headerCount": hex_u64(state.engine_7732_headers_by_slot.get(&slot).map(|v| v.len()).unwrap_or(0) as u64),
      "revealedCount": hex_u64(state
        .engine_7732_headers_by_slot
        .get(&slot)
        .map(|roots| roots.iter().filter(|r| state.engine_7732_envelopes_by_root.contains_key(*r)).count())
        .unwrap_or(0) as u64),
      "lateRevealCount": hex_u64(state
        .engine_7732_headers_by_slot
        .get(&slot)
        .map(|roots| roots.iter().filter(|r| {
          state
            .engine_7732_envelopes_by_root
            .get(*r)
            .map(|env| env.revealed_at_unix_s > deadline)
            .unwrap_or(false)
        }).count())
        .unwrap_or(0) as u64),
      "deadlineUnixS": deadline,
      "currentUnixS": current_unix_s,
      "deadlinePassed": deadline_passed,
      "penalty": penalty
    })
}

fn engine_7732_register_payload_header_response(state: &mut NodeState, req: &Value) -> Value {
    let fallback_slot = state.current_height.saturating_add(1);
    match engine_7732_parse_header_record(req, fallback_slot) {
        Ok(header) => {
            let slot = header.slot;
            let root = header.payload_header_root.clone();
            if let Some(existing) = state.engine_7732_headers_by_root.get(&root).cloned() {
                if engine_7732_header_conflicts(&existing, &header) {
                    return serde_json::json!({
                      "status": "INVALID",
                      "validationError": format!("conflicting header replay for root {}", root),
                      "existingHeader": engine_7732_header_to_json(&existing),
                      "incomingHeader": engine_7732_header_to_json(&header)
                    });
                }
                let slot_roots = state.engine_7732_headers_by_slot.entry(existing.slot).or_default();
                if !slot_roots.contains(&root) {
                    slot_roots.push(root.clone());
                }
                return serde_json::json!({
                  "status": "ACCEPTED",
                  "replayStatus": "DUPLICATE",
                  "slot": hex_u64(existing.slot),
                  "payloadHeaderRoot": root,
                  "knownHeadersAtSlot": hex_u64(slot_roots.len() as u64),
                  "header": engine_7732_header_to_json(&existing)
                });
            }
            state
                .engine_7732_headers_by_root
                .insert(root.clone(), header.clone());
            let slot_roots = state.engine_7732_headers_by_slot.entry(slot).or_default();
            if !slot_roots.contains(&root) {
                slot_roots.push(root.clone());
            }
            serde_json::json!({
              "status": "ACCEPTED",
              "replayStatus": "NEW",
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root,
              "knownHeadersAtSlot": hex_u64(slot_roots.len() as u64),
              "header": engine_7732_header_to_json(&header)
            })
        }
        Err(err) => serde_json::json!({
          "status": "INVALID",
          "validationError": err
        }),
    }
}

fn engine_7732_register_payload_envelope_response(state: &mut NodeState, req: &Value) -> Value {
    let fallback_slot = state.current_height.saturating_add(1);
    match engine_7732_parse_envelope_record(req, fallback_slot) {
        Ok(envelope) => {
            let root = envelope.payload_header_root.clone();
            let slot = envelope.slot;
            if let Some(header) = state.engine_7732_headers_by_root.get(&root) {
                if header.slot != slot {
                    return serde_json::json!({
                      "status": "INVALID",
                      "validationError": format!(
                        "envelope slot mismatch for root {}: header slot {}, envelope slot {}",
                        root,
                        header.slot,
                        slot
                      )
                    });
                }
            }
            let linkage_status = if state.engine_7732_headers_by_root.contains_key(&root) {
                "LINKED"
            } else {
                "ORPHAN"
            };
            if let Some(existing) = state.engine_7732_envelopes_by_root.get(&root).cloned() {
                if engine_7732_envelope_conflicts(&existing, &envelope) {
                    return serde_json::json!({
                      "status": "INVALID",
                      "validationError": format!("conflicting envelope replay for root {}", root),
                      "existingEnvelope": engine_7732_envelope_to_json(&existing),
                      "incomingEnvelope": engine_7732_envelope_to_json(&envelope)
                    });
                }
                return serde_json::json!({
                  "status": "ACCEPTED",
                  "replayStatus": "DUPLICATE",
                  "slot": hex_u64(existing.slot),
                  "payloadHeaderRoot": root,
                  "linkageStatus": linkage_status,
                  "envelope": engine_7732_envelope_to_json(&existing)
                });
            }
            state
                .engine_7732_envelopes_by_root
                .insert(root.clone(), envelope.clone());
            serde_json::json!({
              "status": "ACCEPTED",
              "replayStatus": "NEW",
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root,
              "linkageStatus": linkage_status,
              "envelope": engine_7732_envelope_to_json(&envelope)
            })
        }
        Err(err) => serde_json::json!({
          "status": "INVALID",
          "validationError": err
        }),
    }
}

fn engine_7732_get_payload_timeliness_response(state: &NodeState, req: &Value) -> Value {
    let slot_from_req = req
        .get("slot")
        .and_then(parse_u64_value)
        .or_else(|| {
            req.get("params")
                .and_then(Value::as_array)
                .and_then(|params| params.first())
                .and_then(parse_u64_value)
        });
    let root_from_req = normalize_hash32_value(
        req.get("payloadHeaderRoot").or_else(|| {
            req.get("params")
                .and_then(Value::as_array)
                .and_then(|params| params.first())
        }),
    );
    let slot = slot_from_req
        .or_else(|| {
            root_from_req
                .as_ref()
                .and_then(|root| state.engine_7732_headers_by_root.get(root).map(|h| h.slot))
        })
        .or_else(|| {
            root_from_req
                .as_ref()
                .and_then(|root| state.engine_7732_envelopes_by_root.get(root).map(|e| e.slot))
        })
        .unwrap_or(state.current_height.saturating_add(1));
    let now_unix = engine_7732_current_unix_s_from_req(req).unwrap_or_else(now_unix_s);
    engine_7732_slot_status_response(state, slot, now_unix)
}

fn engine_7732_get_payload_envelope_response(state: &NodeState, req: &Value) -> Value {
    let root = normalize_hash32_value(
        req.get("payloadHeaderRoot").or_else(|| {
            req.get("params")
                .and_then(Value::as_array)
                .and_then(|params| params.first())
        }),
    );
    root.and_then(|r| state.engine_7732_envelopes_by_root.get(&r).map(engine_7732_envelope_to_json))
        .unwrap_or(Value::Null)
}

fn engine_tx_meta_to_json(meta: &EngineTxMeta) -> Value {
    serde_json::json!({
      "hash": meta.hash,
      "from": meta.from,
      "nonce": meta.nonce.map(hex_u64),
      "gas": meta.gas_limit.map(hex_u64),
      "maxFeePerGas": meta.gas_price.map(hex_u128),
      "value": meta.value.map(u256_to_hex)
    })
}

fn engine_get_inclusion_list_response(state: &mut NodeState, req: &Value) -> Value {
    let requested_slot = req
        .get("slot")
        .and_then(parse_u64_value)
        .or_else(|| {
            req.get("params")
                .and_then(Value::as_array)
                .and_then(|params| params.first())
                .and_then(parse_u64_value)
        })
        .or(state.engine_required_il_slot)
        .unwrap_or(state.current_height.saturating_add(1));

    if state.engine_required_il_slot == Some(requested_slot) {
        state.engine_focil_view_frozen = true;
        state.engine_focil_frozen_slot = Some(requested_slot);
        state.engine_focil_frozen_il_root = Some(engine_inclusion_list_root(&state.engine_required_il_txs));
        if state.engine_focil_view_id.is_none() {
            state.engine_focil_view_id = Some(requested_slot);
        }
    }

    serde_json::json!({
      "slot": hex_u64(requested_slot),
      "transactions": state.engine_required_il_txs,
      "transactionDetails": state
        .engine_required_il_txs
        .iter()
        .filter_map(|h| state.engine_required_il_meta.get(h))
        .map(engine_tx_meta_to_json)
        .collect::<Vec<_>>(),
      "committee": state.engine_focil_committee.clone(),
      "viewId": state.engine_focil_view_id.map(hex_u64),
      "viewFreeze": {
        "frozen": state.engine_focil_view_frozen,
        "slot": state.engine_focil_frozen_slot.map(hex_u64),
        "inclusionListRoot": state.engine_focil_frozen_il_root.clone()
      },
      "maxBytesPerInclusionList": "0x2000",
      "updatedAtUnixS": state.engine_required_il_updated_at_unix_s
    })
}

fn engine_forkchoice_updated_response(state: &mut NodeState, req: &Value) -> Value {
    let fc_slot = engine_slot_from_payload_attributes(req)
        .or(state.engine_required_il_slot)
        .unwrap_or(state.current_height.saturating_add(1));
    let timeliness_now = engine_7732_current_unix_s_from_req(req).unwrap_or_else(now_unix_s);
    let timeliness = engine_7732_slot_status_response(state, fc_slot, timeliness_now);
    let timeliness_status = timeliness
        .get("aggregateStatus")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    if engine_7732_is_timeliness_violation(timeliness_status) {
        let penalty = engine_7732_update_penalty_state(
            state,
            fc_slot,
            timeliness_status,
            &format!(
                "EIP-7732 timeliness violation for slot {}: {}",
                fc_slot, timeliness_status
            ),
            timeliness_now,
        )
        .map(|record| engine_7732_penalty_to_json(&record))
        .unwrap_or(Value::Null);
        return serde_json::json!({
          "payloadStatus": {
            "status": "INVALID",
            "latestValidHash": "0x00",
            "validationError": format!("EIP-7732 timeliness violation for slot {}: {}", fc_slot, timeliness_status)
          },
          "payloadId": Value::Null,
          "timeliness": timeliness,
          "penalty": penalty
        });
    }
    let penalty = engine_7732_update_penalty_state(
        state,
        fc_slot,
        timeliness_status,
        "EIP-7732 timeliness recovered",
        timeliness_now,
    )
    .map(|record| engine_7732_penalty_to_json(&record))
    .unwrap_or(Value::Null);

    let fc_view_id = engine_view_id_from_payload_attributes(req).or(Some(fc_slot));
    let has_il_update = engine_request_has_inclusion_list(req);
    let il_entries = if has_il_update {
        engine_collect_inclusion_list_entries(req)
    } else {
        Vec::new()
    };
    let new_il_hashes: Vec<String> = il_entries.iter().map(|m| m.hash.clone()).collect();
    let new_il_root = if has_il_update {
        Some(engine_inclusion_list_root(&new_il_hashes))
    } else {
        None
    };

    if state.engine_focil_view_frozen
        && state.engine_focil_frozen_slot == Some(fc_slot)
        && has_il_update
        && new_il_root.as_ref() != state.engine_focil_frozen_il_root.as_ref()
    {
        return serde_json::json!({
          "payloadStatus": {
            "status": "INVALID",
            "latestValidHash": "0x00",
            "validationError": format!("FOCIL view frozen for slot {}; inclusion list is immutable", fc_slot)
          },
          "payloadId": Value::Null,
          "focil": {
            "requiredTransactions": hex_u64(state.engine_required_il_txs.len() as u64),
            "slot": state.engine_required_il_slot.map(hex_u64),
            "viewId": state.engine_focil_view_id.map(hex_u64),
            "viewFrozen": state.engine_focil_view_frozen,
            "committeeSize": hex_u64(state.engine_focil_committee.len() as u64),
            "frozenInclusionListRoot": state.engine_focil_frozen_il_root.clone()
          }
        });
    }

    if state.engine_focil_view_frozen && state.engine_focil_frozen_slot != Some(fc_slot) {
        state.engine_focil_view_frozen = false;
        state.engine_focil_frozen_slot = None;
        state.engine_focil_frozen_il_root = None;
    }

    let committee = engine_collect_committee_members(req);
    if !committee.is_empty() {
        state.engine_focil_committee = committee;
    }
    state.engine_focil_view_id = fc_view_id;

    if engine_request_has_inclusion_list(req) {
        let mut il_meta = HashMap::new();
        for entry in &il_entries {
            il_meta.insert(entry.hash.clone(), entry.clone());
        }
        state.engine_required_il_txs = new_il_hashes;
        state.engine_required_il_meta = il_meta;
        state.engine_required_il_slot = Some(fc_slot);
        state.engine_required_il_updated_at_unix_s = Some(now_unix_s());
    }

    serde_json::json!({
      "payloadStatus": {
        "status": "VALID",
        "latestValidHash": "0x00",
        "validationError": Value::Null
      },
      "payloadId": "0x0000000000000000",
      "timeliness": timeliness,
      "penalty": penalty,
      "focil": {
        "requiredTransactions": hex_u64(state.engine_required_il_txs.len() as u64),
        "slot": state.engine_required_il_slot.map(hex_u64),
        "viewId": state.engine_focil_view_id.map(hex_u64),
        "viewFrozen": state.engine_focil_view_frozen,
        "committeeSize": hex_u64(state.engine_focil_committee.len() as u64),
        "frozenInclusionListRoot": state.engine_focil_frozen_il_root.clone()
      }
    })
}

fn engine_new_payload_response(state: &mut NodeState, req: &Value) -> Value {
    const ENGINE_BLOCK_GAS_LIMIT: u64 = 30_000_000;

    let payload_entries = engine_collect_payload_transaction_entries(req);
    let payload_tx_set: HashSet<&str> = payload_entries.iter().map(|m| m.hash.as_str()).collect();
    let payload_meta: HashMap<&str, &EngineTxMeta> = payload_entries
        .iter()
        .map(|entry| (entry.hash.as_str(), entry))
        .collect();

    let missing: Vec<String> = state
        .engine_required_il_txs
        .iter()
        .filter(|tx| !payload_tx_set.contains(tx.as_str()))
        .cloned()
        .collect();

    let latest_valid_hash = execution_payload_from_engine_request(req)
        .and_then(|payload| payload.get("blockHash"))
        .and_then(Value::as_str)
        .and_then(normalize_hash32)
        .unwrap_or_else(|| "0x00".to_string());

    let payload_slot = engine_payload_slot(req)
        .or(state.engine_required_il_slot)
        .unwrap_or(state.current_height.saturating_add(1));
    let timeliness_now = engine_7732_current_unix_s_from_req(req).unwrap_or_else(now_unix_s);
    let timeliness = engine_7732_slot_status_response(state, payload_slot, timeliness_now);
    let timeliness_status = timeliness
        .get("aggregateStatus")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    if engine_7732_is_timeliness_violation(timeliness_status) {
        let penalty = engine_7732_update_penalty_state(
            state,
            payload_slot,
            timeliness_status,
            &format!(
                "EIP-7732 timeliness violation for slot {}: {}",
                payload_slot, timeliness_status
            ),
            timeliness_now,
        )
        .map(|record| engine_7732_penalty_to_json(&record))
        .unwrap_or(Value::Null);
        return serde_json::json!({
          "status": "INVALID",
          "latestValidHash": latest_valid_hash,
          "validationError": format!("EIP-7732 timeliness violation for slot {}: {}", payload_slot, timeliness_status),
          "timeliness": timeliness,
          "penalty": penalty
        });
    }
    let penalty = engine_7732_update_penalty_state(
        state,
        payload_slot,
        timeliness_status,
        "EIP-7732 timeliness recovered",
        timeliness_now,
    )
    .map(|record| engine_7732_penalty_to_json(&record))
    .unwrap_or(Value::Null);

    if !missing.is_empty() {
        let preview = missing.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        return serde_json::json!({
          "status": "INCLUSION_LIST_UNSATISFIED",
          "latestValidHash": latest_valid_hash,
          "validationError": format!("missing {} inclusion-list tx(s): {}", missing.len(), preview),
          "timeliness": timeliness,
          "penalty": penalty
        });
    }

    if state.engine_focil_view_frozen {
        if let Some(frozen_slot) = state.engine_focil_frozen_slot {
            if payload_slot != frozen_slot {
                return serde_json::json!({
                  "status": "INCLUSION_LIST_UNSATISFIED",
                  "latestValidHash": latest_valid_hash,
                  "validationError": format!(
                    "FOCIL view frozen at slot {}; payload targets slot {}",
                    frozen_slot, payload_slot
                  ),
                  "timeliness": timeliness,
                  "penalty": penalty
                });
            }
        }
    }

    let mut checks = Vec::new();
    for req_hash in &state.engine_required_il_txs {
        let Some(req_meta) = state.engine_required_il_meta.get(req_hash) else {
            continue;
        };
        let Some(payload_meta) = payload_meta.get(req_hash.as_str()) else {
            continue;
        };

        if let Some(required_from) = req_meta.from.as_ref() {
            match payload_meta.from.as_ref() {
                Some(payload_from) if payload_from == required_from => {}
                Some(payload_from) => checks.push(format!(
                    "{req_hash}: sender mismatch (required {required_from}, got {payload_from})"
                )),
                None => checks.push(format!("{req_hash}: payload sender missing")),
            }
        }
        if let Some(required_nonce) = req_meta.nonce {
            match payload_meta.nonce {
                Some(payload_nonce) if payload_nonce == required_nonce => {}
                Some(payload_nonce) => checks.push(format!(
                    "{req_hash}: nonce mismatch (required {}, got {})",
                    required_nonce, payload_nonce
                )),
                None => checks.push(format!("{req_hash}: payload nonce missing")),
            }
        }
        if let Some(required_gas_limit) = req_meta.gas_limit {
            match payload_meta.gas_limit {
                Some(payload_gas_limit) if payload_gas_limit == required_gas_limit => {}
                Some(payload_gas_limit) => checks.push(format!(
                    "{req_hash}: gas mismatch (required {}, got {})",
                    required_gas_limit, payload_gas_limit
                )),
                None => checks.push(format!("{req_hash}: payload gas missing")),
            }
        }
        if let Some(required_gas_price) = req_meta.gas_price {
            match payload_meta.gas_price {
                Some(payload_gas_price) if payload_gas_price == required_gas_price => {}
                Some(payload_gas_price) => checks.push(format!(
                    "{req_hash}: gas price mismatch (required 0x{:x}, got 0x{:x})",
                    required_gas_price, payload_gas_price
                )),
                None => checks.push(format!("{req_hash}: payload gas price missing")),
            }
        }
        if let Some(required_value) = req_meta.value {
            match payload_meta.value {
                Some(payload_value) if payload_value == required_value => {}
                Some(payload_value) => checks.push(format!(
                    "{req_hash}: value mismatch (required {}, got {})",
                    u256_to_hex(required_value),
                    u256_to_hex(payload_value)
                )),
                None => checks.push(format!("{req_hash}: payload value missing")),
            }
        }
    }

    let mut simulated_nonce = HashMap::new();
    let mut simulated_balance = HashMap::new();
    let mut total_known_gas = 0u64;
    let mut unknown_gas_count = 0usize;
    let required_set: HashSet<&str> = state
        .engine_required_il_txs
        .iter()
        .map(String::as_str)
        .collect();

    for payload in &payload_entries {
        if let Some(gas_limit) = payload.gas_limit {
            total_known_gas = total_known_gas.saturating_add(gas_limit);
        } else {
            unknown_gas_count = unknown_gas_count.saturating_add(1);
        }

        if !required_set.contains(payload.hash.as_str()) {
            continue;
        }
        let Some(from_str) = payload.from.as_ref() else {
            continue;
        };
        let Some(sender) = address_from_hex(from_str) else {
            checks.push(format!("{}: sender is not a valid address", payload.hash));
            continue;
        };

        let expected_nonce = *simulated_nonce
            .entry(from_str.clone())
            .or_insert_with(|| account_nonce_from_db(&mut state.evm_db, sender));
        if let Some(payload_nonce) = payload.nonce {
            if payload_nonce != expected_nonce {
                checks.push(format!(
                    "{}: nonce {} does not match expected {}",
                    payload.hash, payload_nonce, expected_nonce
                ));
                continue;
            }
        }

        if let Some(gas_limit) = payload.gas_limit {
            if gas_limit > ENGINE_BLOCK_GAS_LIMIT {
                checks.push(format!(
                    "{}: gas limit {} exceeds block gas limit {}",
                    payload.hash, gas_limit, ENGINE_BLOCK_GAS_LIMIT
                ));
            }
        }

        if let (Some(gas_limit), Some(gas_price), Some(value)) =
            (payload.gas_limit, payload.gas_price, payload.value)
        {
            let balance = simulated_balance
                .entry(from_str.clone())
                .or_insert_with(|| account_balance_from_db(&mut state.evm_db, sender));
            let required_wei = U256::from(gas_limit) * U256::from(gas_price) + value;
            if *balance < required_wei {
                checks.push(format!(
                    "{}: insufficient balance (need {}, have {})",
                    payload.hash,
                    u256_to_hex(required_wei),
                    u256_to_hex(*balance)
                ));
            } else {
                *balance -= required_wei;
            }
        }

        if payload.nonce.is_some() {
            simulated_nonce.insert(from_str.clone(), expected_nonce.saturating_add(1));
        }
    }

    if unknown_gas_count == 0
        && !payload_entries.is_empty()
        && total_known_gas > ENGINE_BLOCK_GAS_LIMIT
        && !required_set.is_empty()
    {
        checks.push(format!(
            "payload gas {} exceeds block gas limit {}",
            total_known_gas, ENGINE_BLOCK_GAS_LIMIT
        ));
    }

    if checks.is_empty() {
        serde_json::json!({
          "status": "VALID",
          "latestValidHash": latest_valid_hash,
          "validationError": Value::Null,
          "timeliness": timeliness,
          "penalty": penalty
        })
    } else {
        let preview = checks.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
        serde_json::json!({
          "status": "INCLUSION_LIST_UNSATISFIED",
          "latestValidHash": latest_valid_hash,
          "validationError": preview,
          "timeliness": timeliness,
          "penalty": penalty
        })
    }
}

fn handle_jsonrpc(state: &mut NodeState, req: &Value) -> Value {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = req.get("params").cloned().unwrap_or(Value::Array(vec![]));
    let now = now_unix_s();

    match method {
        "web3_clientVersion" => jsonrpc_response(&id, Value::String("ETH2077/devnetd/0.1".into())),
        "net_version" => jsonrpc_response(&id, Value::String(state.chain_id.to_string())),
        "net_listening" => jsonrpc_response(&id, Value::Bool(true)),
        "eth_chainId" => jsonrpc_response(&id, Value::String(hex_u64(state.chain_id))),
        "eth_mining" => jsonrpc_response(&id, Value::Bool(true)),
        "eth_syncing" => jsonrpc_response(&id, Value::Bool(false)),
        "eth_blockNumber" => jsonrpc_response(&id, Value::String(hex_u64(state.current_height))),
        "eth_getBlockByNumber" => {
            let block_tag = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("latest");
            let full_txs = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(num) = block_by_tag(state, block_tag) {
                jsonrpc_response(&id, build_block_response(state, num, full_txs))
            } else {
                jsonrpc_response(&id, Value::Null)
            }
        }
        "eth_getBlockByHash" => {
            let maybe_hash = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or_default();
            let full_txs = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(num) = block_number_from_hash(maybe_hash) {
                if num <= state.current_height {
                    jsonrpc_response(&id, build_block_response(state, num, full_txs))
                } else {
                    jsonrpc_response(&id, Value::Null)
                }
            } else {
                jsonrpc_response(&id, Value::Null)
            }
        }
        "eth_getLogs" => {
            let filter = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_object);
            let Some(filter) = filter else {
                return jsonrpc_error(&id, -32602, "invalid params: expected filter object");
            };

            let from_block = filter
                .get("fromBlock")
                .and_then(Value::as_str)
                .and_then(|tag| block_by_tag(state, tag))
                .unwrap_or(0);
            let to_block = filter
                .get("toBlock")
                .and_then(Value::as_str)
                .and_then(|tag| block_by_tag(state, tag))
                .unwrap_or(state.current_height);

            let (range_start, range_end) = if let Some(block_hash) = filter
                .get("blockHash")
                .and_then(Value::as_str)
                .and_then(block_number_from_hash)
            {
                (block_hash, block_hash)
            } else {
                (from_block.min(to_block), from_block.max(to_block))
            };

            let address_filter: Option<Vec<String>> = filter.get("address").and_then(|v| match v {
                Value::String(one) => canonical_address(one).map(|a| vec![a]),
                Value::Array(many) => {
                    let mut out = Vec::new();
                    for entry in many {
                        let Some(addr) = entry.as_str().and_then(canonical_address) else {
                            return None;
                        };
                        out.push(addr);
                    }
                    Some(out)
                }
                _ => None,
            });
            if filter.get("address").is_some() && address_filter.is_none() {
                return jsonrpc_error(&id, -32602, "invalid address filter");
            }

            let topic_filters: Vec<Vec<String>> = match filter.get("topics") {
                Some(Value::Array(arr)) => {
                    let mut parsed = Vec::new();
                    for entry in arr {
                        match entry {
                            Value::Null => parsed.push(Vec::new()),
                            Value::String(one) => parsed.push(vec![one.to_ascii_lowercase()]),
                            Value::Array(any) => {
                                let mut opts = Vec::new();
                                for topic in any {
                                    let Some(topic_str) = topic.as_str() else {
                                        return jsonrpc_error(
                                            &id,
                                            -32602,
                                            "invalid topics filter entry",
                                        );
                                    };
                                    opts.push(topic_str.to_ascii_lowercase());
                                }
                                parsed.push(opts);
                            }
                            _ => return jsonrpc_error(&id, -32602, "invalid topics filter entry"),
                        }
                    }
                    parsed
                }
                Some(_) => return jsonrpc_error(&id, -32602, "invalid topics filter"),
                None => Vec::new(),
            };

            let mut out = Vec::<Value>::new();
            for block_number in range_start..=range_end {
                let Some(tx_hashes) = state.block_txs.get(&block_number) else {
                    continue;
                };
                for tx_hash in tx_hashes {
                    let Some(receipt) = state.tx_receipts.get(tx_hash) else {
                        continue;
                    };
                    for log in &receipt.logs {
                        if let Some(addrs) = address_filter.as_ref() {
                            if !addrs.iter().any(|a| a == &log.address) {
                                continue;
                            }
                        }

                        let mut topic_ok = true;
                        for (idx, filter_options) in topic_filters.iter().enumerate() {
                            if filter_options.is_empty() {
                                continue;
                            }
                            let Some(topic) = log.topics.get(idx) else {
                                topic_ok = false;
                                break;
                            };
                            let topic_lc = topic.to_ascii_lowercase();
                            if !filter_options.iter().any(|v| v == &topic_lc) {
                                topic_ok = false;
                                break;
                            }
                        }
                        if !topic_ok {
                            continue;
                        }

                        out.push(tx_log_to_json(log));
                    }
                }
            }

            jsonrpc_response(&id, Value::Array(out))
        }
        "trace_block" | "trace_filter" | "eth_accounts" => {
            jsonrpc_response(&id, Value::Array(vec![]))
        }
        "trace_transaction" | "debug_traceTransaction" => jsonrpc_response(
            &id,
            serde_json::json!({ "gas": "0x0", "returnValue": "0x", "structLogs": [] }),
        ),
        "eth_gasPrice" => jsonrpc_response(&id, Value::String("0x3b9aca00".into())),
        "eth_maxPriorityFeePerGas" => jsonrpc_response(&id, Value::String("0x59682f00".into())),
        "eth_blobBaseFee" => {
            jsonrpc_response(&id, Value::String(hex_u128(blob_base_fee_per_gas())))
        }
        "eth_estimateGas" => {
            let call_obj = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_object);
            match call_obj {
                Some(call) => {
                    let caller = call
                        .get("from")
                        .and_then(Value::as_str)
                        .and_then(address_from_hex)
                        .or_else(|| {
                            state
                                .pending_nonce_address_hint
                                .as_deref()
                                .and_then(address_from_hex)
                        })
                        .unwrap_or(Address::ZERO);
                    let to = call
                        .get("to")
                        .and_then(Value::as_str)
                        .and_then(address_from_hex);
                    let data = call
                        .get("data")
                        .or_else(|| call.get("input"))
                        .and_then(Value::as_str)
                        .and_then(parse_hex_bytes)
                        .map(Bytes::from)
                        .unwrap_or_default();
                    let gas_limit = call
                        .get("gas")
                        .and_then(Value::as_str)
                        .and_then(parse_hex_u64)
                        .unwrap_or(30_000_000);
                    let gas_price = call
                        .get("gasPrice")
                        .and_then(Value::as_str)
                        .and_then(parse_hex_u128)
                        .unwrap_or(0);
                    let value = call
                        .get("value")
                        .and_then(Value::as_str)
                        .and_then(parse_hex_u256)
                        .unwrap_or(U256::ZERO);

                    let decoded = DecodedRawTx {
                        tx_type: 0x2,
                        chain_id: Some(state.chain_id),
                        nonce: account_nonce_from_db(&mut state.evm_db, caller),
                        gas_limit,
                        gas_price,
                        max_priority_fee_per_gas: Some(0),
                        max_fee_per_blob_gas: None,
                        blob_hashes: Vec::new(),
                        authorizations: Vec::new(),
                        to,
                        value,
                        data,
                    };
                    let block_number = state.current_height;
                    let block_ts = state
                        .started_at_unix_s
                        .saturating_add(block_number.saturating_mul(12));
                    match execute_evm_transaction(
                        state,
                        caller,
                        &decoded,
                        false,
                        block_number,
                        block_ts,
                    ) {
                        Ok(outcome) => {
                            let estimated = outcome.gas_used.max(21_000);
                            jsonrpc_response(&id, Value::String(hex_u64(estimated)))
                        }
                        Err(err) => {
                            jsonrpc_error(&id, -32000, &format!("estimation failed: {err}"))
                        }
                    }
                }
                None => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected call object as first parameter",
                ),
            }
        }
        "eth_call" => {
            let call_obj = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_object);
            let block_tag = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .unwrap_or("latest");

            match call_obj {
                Some(call) => {
                    let caller = call
                        .get("from")
                        .and_then(Value::as_str)
                        .and_then(address_from_hex)
                        .or_else(|| {
                            state
                                .pending_nonce_address_hint
                                .as_deref()
                                .and_then(address_from_hex)
                        })
                        .unwrap_or(Address::ZERO);
                    let to = call
                        .get("to")
                        .and_then(Value::as_str)
                        .and_then(address_from_hex);
                    let data = call
                        .get("data")
                        .or_else(|| call.get("input"))
                        .and_then(Value::as_str)
                        .and_then(parse_hex_bytes)
                        .map(Bytes::from)
                        .unwrap_or_default();
                    let gas_limit = call
                        .get("gas")
                        .and_then(Value::as_str)
                        .and_then(parse_hex_u64)
                        .unwrap_or(30_000_000);
                    let gas_price = call
                        .get("gasPrice")
                        .and_then(Value::as_str)
                        .and_then(parse_hex_u128)
                        .unwrap_or(0);
                    let value = call
                        .get("value")
                        .and_then(Value::as_str)
                        .and_then(parse_hex_u256)
                        .unwrap_or(U256::ZERO);

                    let decoded = DecodedRawTx {
                        tx_type: 0x2,
                        chain_id: Some(state.chain_id),
                        nonce: account_nonce_from_db(&mut state.evm_db, caller),
                        gas_limit,
                        gas_price,
                        max_priority_fee_per_gas: Some(0),
                        max_fee_per_blob_gas: None,
                        blob_hashes: Vec::new(),
                        authorizations: Vec::new(),
                        to,
                        value,
                        data,
                    };

                    let block_number =
                        block_by_tag(state, block_tag).unwrap_or(state.current_height);
                    let block_ts = state
                        .started_at_unix_s
                        .saturating_add(block_number.saturating_mul(12));
                    match execute_evm_transaction(
                        state,
                        caller,
                        &decoded,
                        false,
                        block_number,
                        block_ts,
                    ) {
                        Ok(outcome) => jsonrpc_response(
                            &id,
                            Value::String(bytes_to_hex(outcome.output.as_ref())),
                        ),
                        Err(err) => jsonrpc_error(&id, -32000, &format!("eth_call failed: {err}")),
                    }
                }
                None => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected call object as first parameter",
                ),
            }
        }
        "eth_getBalance" => {
            let address = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(address_from_hex);
            let balance = address
                .map(|addr| account_balance_from_db(&mut state.evm_db, addr))
                .unwrap_or(U256::from(DEFAULT_ACCOUNT_BALANCE_WEI));
            jsonrpc_response(&id, Value::String(u256_to_hex(balance)))
        }
        "eth_getTransactionCount" => {
            let address_str = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str);
            let address = address_str.and_then(address_from_hex);
            let nonce = address
                .map(|addr| account_nonce_from_db(&mut state.evm_db, addr))
                .unwrap_or(0);
            state.pending_nonce_address_hint = address_str.and_then(canonical_address);
            jsonrpc_response(&id, Value::String(hex_u64(nonce)))
        }
        "eth_getCode" => {
            let address = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(address_from_hex);
            let code = address
                .map(|addr| code_hex_from_db(&mut state.evm_db, addr))
                .unwrap_or_else(|| "0x".to_string());
            jsonrpc_response(&id, Value::String(code))
        }
        "eth_getStorageAt" => {
            let address = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(address_from_hex);
            let slot = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .and_then(parse_hex_u256)
                .unwrap_or(U256::ZERO);
            let out = address
                .map(|addr| storage_hex_from_db(&mut state.evm_db, addr, slot))
                .unwrap_or_else(|| {
                    "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
                });
            jsonrpc_response(&id, Value::String(out))
        }
        "eth_getTransactionByHash" => {
            let maybe_hash = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(|v| v.to_ascii_lowercase());
            let tx = maybe_hash
                .as_ref()
                .and_then(|h| state.txs.get(h))
                .map(|tx| tx_to_json(tx, state.chain_id))
                .unwrap_or(Value::Null);
            jsonrpc_response(&id, tx)
        }
        "eth_getTransactionReceipt" => {
            let maybe_hash = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(|v| v.to_ascii_lowercase());
            let receipt = maybe_hash
                .as_ref()
                .and_then(|h| state.tx_receipts.get(h))
                .and_then(tx_receipt_json)
                .unwrap_or(Value::Null);
            jsonrpc_response(&id, receipt)
        }
        "eth_getBlockTransactionCountByNumber" => {
            let block_tag = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("latest");
            let count = block_by_tag(state, block_tag)
                .and_then(|n| state.block_txs.get(&n).map(|v| v.len() as u64))
                .unwrap_or(0);
            jsonrpc_response(&id, Value::String(hex_u64(count)))
        }
        "eth_getBlockTransactionCountByHash" => {
            let count = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(block_number_from_hash)
                .and_then(|n| state.block_txs.get(&n).map(|v| v.len() as u64))
                .unwrap_or(0);
            jsonrpc_response(&id, Value::String(hex_u64(count)))
        }
        "eth_getUncleCountByBlockNumber" => jsonrpc_response(&id, Value::String("0x0".into())),
        "eth_getUncleCountByBlockHash" => jsonrpc_response(&id, Value::String("0x0".into())),
        "eth_getBlockReceipts" => {
            let block_tag = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("latest");
            let number =
                block_number_from_hash(block_tag).or_else(|| block_by_tag(state, block_tag));
            let receipts = number
                .and_then(|n| state.block_txs.get(&n).cloned())
                .map(|hashes| {
                    hashes
                        .iter()
                        .filter_map(|h| state.tx_receipts.get(h))
                        .filter_map(tx_receipt_json)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            jsonrpc_response(&id, Value::Array(receipts))
        }
        "net_peerCount" => {
            jsonrpc_response(&id, Value::String(hex_u64(state.peers_connected as u64)))
        }
        "txpool_content" => jsonrpc_response(
            &id,
            serde_json::json!({
              "pending": {},
              "queued": {}
            }),
        ),
        "eth_feeHistory" => {
            let latest = state.current_height;
            let oldest = latest.saturating_sub(9);
            let blob_base_fee = hex_u128(blob_base_fee_per_gas());
            jsonrpc_response(
                &id,
                serde_json::json!({
                  "oldestBlock": hex_u64(oldest),
                  "baseFeePerGas": vec!["0x3b9aca00"; 11],
                  "baseFeePerBlobGas": vec![blob_base_fee; 11],
                  "gasUsedRatio": vec![0.0; 10],
                  "blobGasUsedRatio": vec![0.0; 10],
                  "reward": []
                }),
            )
        }
        "engine_getInclusionListV1" => {
            jsonrpc_response(&id, engine_get_inclusion_list_response(state, req))
        }
        "engine_forkchoiceUpdatedV3" => {
            jsonrpc_response(&id, engine_forkchoice_updated_response(state, req))
        }
        "engine_newPayloadV3" => jsonrpc_response(&id, engine_new_payload_response(state, req)),
        "engine_registerExecutionPayloadHeaderV1" => jsonrpc_response(
            &id,
            engine_7732_register_payload_header_response(state, req),
        ),
        "engine_registerExecutionPayloadEnvelopeV1" => jsonrpc_response(
            &id,
            engine_7732_register_payload_envelope_response(state, req),
        ),
        "engine_getPayloadTimelinessV1" => jsonrpc_response(
            &id,
            engine_7732_get_payload_timeliness_response(state, req),
        ),
        "engine_getExecutionPayloadEnvelopeV1" => jsonrpc_response(
            &id,
            engine_7732_get_payload_envelope_response(state, req),
        ),
        "eth_sendRawTransaction" => {
            let raw_tx = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("0x");
            let decoded = match decode_raw_tx(raw_tx) {
                Ok(decoded) => decoded,
                Err(err) => return jsonrpc_error(&id, -32602, &format!("invalid raw tx: {err}")),
            };

            if let Some(tx_chain_id) = decoded.chain_id {
                if tx_chain_id != state.chain_id {
                    return jsonrpc_error(
                        &id,
                        -32000,
                        &format!(
                            "chain id mismatch: tx uses {tx_chain_id}, node is {}",
                            state.chain_id
                        ),
                    );
                }
            }

            let caller_str = match state.pending_nonce_address_hint.clone() {
                Some(caller) => caller,
                None => {
                    return jsonrpc_error(
                        &id,
                        -32000,
                        "missing sender hint; call eth_getTransactionCount(address) before eth_sendRawTransaction",
                    )
                }
            };
            let caller = match address_from_hex(&caller_str) {
                Some(caller) => caller,
                None => {
                    return jsonrpc_error(&id, -32000, "pending sender hint is not a valid address")
                }
            };

            let raw_bytes = parse_hex_bytes(raw_tx).unwrap_or_default();
            let tx_hash = bytes_to_hex(revm::primitives::keccak256(raw_bytes).as_slice());

            let block_number_hint = state.current_height.saturating_add(1);
            let block_ts = state
                .started_at_unix_s
                .saturating_add(block_number_hint.saturating_mul(12));
            let outcome = match execute_evm_transaction(
                state,
                caller,
                &decoded,
                true,
                block_number_hint,
                block_ts,
            ) {
                Ok(outcome) => outcome,
                Err(err) => {
                    return jsonrpc_error(&id, -32000, &format!("transaction rejected: {err}"))
                }
            };

            state.pending_nonce_address_hint = None;
            state.tx_counter = state.tx_counter.saturating_add(1);
            state
                .nonce_by_address
                .insert(caller_str.clone(), decoded.nonce.saturating_add(1));

            let tx = TxRecord {
                hash: tx_hash.clone(),
                from: caller_str.clone(),
                to: decoded.to.map(address_to_hex),
                nonce: decoded.nonce,
                gas: decoded.gas_limit,
                gas_price: decoded.gas_price,
                max_priority_fee_per_gas: decoded.max_priority_fee_per_gas,
                max_fee_per_blob_gas: decoded.max_fee_per_blob_gas,
                blob_versioned_hashes: decoded.blob_hashes.iter().map(b256_to_hex).collect(),
                authorization_list_len: decoded.authorizations.len(),
                value: u256_to_hex(decoded.value),
                input: bytes_to_hex(decoded.data.as_ref()),
                tx_type: format!("0x{:x}", decoded.tx_type),
                contract_address: outcome.contract_address.map(address_to_hex),
                block_number: None,
                block_hash: None,
                transaction_index: None,
            };
            let bloom: Bloom = logs_bloom(outcome.logs.iter());
            let receipt = TxReceiptRecord {
                tx_hash: tx_hash.clone(),
                from: caller_str,
                to: tx.to.clone(),
                contract_address: tx.contract_address.clone(),
                gas_used: outcome.gas_used,
                cumulative_gas_used: outcome.gas_used,
                effective_gas_price: decoded.gas_price.min(u64::MAX as u128) as u64,
                tx_type: tx.tx_type.clone(),
                status: outcome.status,
                logs_bloom: bytes_to_hex(bloom.data()),
                block_number: None,
                block_hash: None,
                transaction_index: None,
                logs: tx_logs_from_revm(&tx_hash, &outcome.logs),
            };

            state.pending_txs.push(tx_hash.clone());
            state.txs.insert(tx_hash.clone(), tx);
            state.tx_receipts.insert(tx_hash.clone(), receipt);
            jsonrpc_response(&id, Value::String(tx_hash))
        }
        "hardhat_setBalance" | "anvil_setBalance" | "eth2077_setBalance" => {
            let address = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(address_from_hex);
            let amount = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .and_then(parse_hex_u256);
            match (address, amount) {
                (Some(addr), Some(wei)) => {
                    set_account_balance_in_db(&mut state.evm_db, addr, wei);
                    jsonrpc_response(&id, Value::Bool(true))
                }
                _ => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected [address, amountWeiHex]",
                ),
            }
        }
        "eth2077_status" => jsonrpc_response(&id, eth2077_status_json(state, now)),
        "eth2077_marketList" => jsonrpc_response(&id, market_nft_list_json(state)),
        "eth2077_marketByOwner" => {
            let owner = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(canonical_address);
            match owner {
                Some(owner_addr) => {
                    let mut nfts: Vec<MarketNft> = state
                        .market_nfts
                        .values()
                        .filter(|n| n.owner == owner_addr)
                        .cloned()
                        .collect();
                    nfts.sort_by_key(|n| n.token_id);
                    jsonrpc_response(
                        &id,
                        Value::Array(nfts.iter().map(market_nft_to_json).collect()),
                    )
                }
                None => jsonrpc_error(&id, -32602, "invalid params: expected [ownerAddress]"),
            }
        }
        "eth2077_marketMint" => {
            let owner = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(canonical_address);
            let name = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let description = params
                .as_array()
                .and_then(|a| a.get(2))
                .and_then(Value::as_str)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let image = params
                .as_array()
                .and_then(|a| a.get(3))
                .and_then(Value::as_str)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let price_wei = params
                .as_array()
                .and_then(|a| a.get(4))
                .and_then(Value::as_str)
                .and_then(parse_hex_u128)
                .unwrap_or(1_000_000_000_000_000u128);

            match owner {
                Some(owner_addr) => {
                    let token_id = state.next_market_token_id;
                    state.next_market_token_id = state.next_market_token_id.saturating_add(1);

                    let tx_hash =
                        pseudo_tx_hash(&format!("market-mint:{}:{token_id}:{now}", owner_addr), state.tx_counter + now);
                    state.tx_counter = state.tx_counter.saturating_add(1);

                    let nonce = *state.nonce_by_address.get(&owner_addr).unwrap_or(&0);
                    state
                        .nonce_by_address
                        .insert(owner_addr.clone(), nonce.saturating_add(1));

                    let tx = TxRecord {
                        hash: tx_hash.clone(),
                        from: owner_addr.clone(),
                        to: Some(MARKET_MERCHANT_ADDRESS.to_string()),
                        nonce,
                        gas: 120_000,
                        gas_price: 1_000_000_000,
                        max_priority_fee_per_gas: Some(0),
                        max_fee_per_blob_gas: None,
                        blob_versioned_hashes: Vec::new(),
                        authorization_list_len: 0,
                        value: "0x0".to_string(),
                        input: format!("0x6d696e74{token_id:016x}"),
                        tx_type: "0x2".to_string(),
                        contract_address: None,
                        block_number: None,
                        block_hash: None,
                        transaction_index: None,
                    };
                    state.pending_txs.push(tx_hash.clone());
                    state.txs.insert(tx_hash.clone(), tx);

                    let nft = MarketNft {
                        token_id,
                        name: name.unwrap_or_else(|| format!("ETH2077 NFT #{token_id}")),
                        description: description.unwrap_or_else(|| {
                            "Minted on ETH2077 devnet marketplace".to_string()
                        }),
                        image: image.unwrap_or_else(|| {
                            format!("https://picsum.photos/seed/eth2077-{token_id}/640/640")
                        }),
                        owner: owner_addr,
                        listed: true,
                        price_wei,
                        created_at_unix_s: now,
                        last_tx_hash: Some(tx_hash),
                    };
                    state.market_nfts.insert(token_id, nft.clone());
                    jsonrpc_response(&id, market_nft_to_json(&nft))
                }
                None => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected [ownerAddress, name?, description?, imageUrl?, priceWeiHex?]",
                ),
            }
        }
        "eth2077_marketBuy" => {
            let token_id = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(parse_hex_u64);
            let buyer = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .and_then(canonical_address);
            let tx_hash_param = params
                .as_array()
                .and_then(|a| a.get(2))
                .and_then(Value::as_str)
                .map(|v| v.to_ascii_lowercase())
                .filter(|v| !v.is_empty());

            match (token_id, buyer) {
                (Some(id_u64), Some(buyer_addr)) => {
                    if let Some(hash) = tx_hash_param.as_ref() {
                        match state.txs.get(hash) {
                            Some(tx) if tx.from == buyer_addr => {}
                            Some(_) => {
                                return jsonrpc_error(&id, -32000, "tx sender does not match buyer")
                            }
                            None => {
                                return jsonrpc_error(&id, -32000, "unknown tx hash for market buy")
                            }
                        }
                    }

                    match state.market_nfts.get_mut(&id_u64) {
                        Some(nft) => {
                            if !nft.listed {
                                return jsonrpc_error(&id, -32000, "nft is not listed");
                            }
                            if nft.owner == buyer_addr {
                                return jsonrpc_error(&id, -32000, "buyer already owns this nft");
                            }
                            let seller = nft.owner.clone();
                            let price_wei = nft.price_wei;

                            nft.owner = buyer_addr;
                            nft.listed = false;
                            let effective_tx_hash = tx_hash_param.clone().unwrap_or_else(|| {
                                pseudo_tx_hash(
                                    &format!("market-buy:{id_u64}:{}:{now}", nft.owner),
                                    state.tx_counter.saturating_add(now),
                                )
                            });
                            nft.last_tx_hash = Some(effective_tx_hash.clone());

                            let mut receipt = MarketReceipt {
                                tx_hash: effective_tx_hash.clone(),
                                token_id: id_u64,
                                buyer: nft.owner.clone(),
                                seller,
                                price_wei,
                                chain_id: state.chain_id,
                                block_number_hint: state.current_height.saturating_add(1),
                                timestamp_unix_s: now,
                                signature: String::new(),
                            };
                            receipt.signature = market_receipt_signature(&receipt);
                            state
                                .market_receipts
                                .insert(effective_tx_hash, receipt.clone());

                            jsonrpc_response(
                                &id,
                                serde_json::json!({
                                  "nft": market_nft_to_json(nft),
                                  "receipt": market_receipt_to_json(&receipt)
                                }),
                            )
                        }
                        None => jsonrpc_error(&id, -32000, "unknown nft token id"),
                    }
                }
                _ => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected [tokenIdHex, buyerAddress, txHash?]",
                ),
            }
        }
        "eth2077_marketTransfer" => {
            let token_id = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(parse_hex_u64);
            let from = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .and_then(canonical_address);
            let to = params
                .as_array()
                .and_then(|a| a.get(2))
                .and_then(Value::as_str)
                .and_then(canonical_address);
            let tx_hash = params
                .as_array()
                .and_then(|a| a.get(3))
                .and_then(Value::as_str)
                .map(|v| v.to_ascii_lowercase())
                .filter(|v| !v.is_empty());

            match (token_id, from, to) {
                (Some(id_u64), Some(from_addr), Some(to_addr)) => {
                    if let Some(hash) = tx_hash.as_ref() {
                        match state.txs.get(hash) {
                            Some(tx) if tx.from == from_addr => {}
                            Some(_) => {
                                return jsonrpc_error(
                                    &id,
                                    -32000,
                                    "tx sender does not match transfer source",
                                )
                            }
                            None => {
                                return jsonrpc_error(
                                    &id,
                                    -32000,
                                    "unknown tx hash for market transfer",
                                )
                            }
                        }
                    }

                    match state.market_nfts.get_mut(&id_u64) {
                        Some(nft) => {
                            if nft.owner != from_addr {
                                return jsonrpc_error(
                                    &id,
                                    -32000,
                                    "only current owner can transfer nft",
                                );
                            }
                            nft.owner = to_addr;
                            nft.listed = false;
                            nft.last_tx_hash = tx_hash;
                            jsonrpc_response(&id, market_nft_to_json(nft))
                        }
                        None => jsonrpc_error(&id, -32000, "unknown nft token id"),
                    }
                }
                _ => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected [tokenIdHex, fromAddress, toAddress, txHash?]",
                ),
            }
        }
        "eth2077_marketRelist" => {
            let token_id = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .and_then(parse_hex_u64);
            let owner = params
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .and_then(canonical_address);
            let price_wei = params
                .as_array()
                .and_then(|a| a.get(2))
                .and_then(Value::as_str)
                .and_then(parse_hex_u128);

            match (token_id, owner, price_wei) {
                (Some(id_u64), Some(owner_addr), Some(new_price)) => {
                    match state.market_nfts.get_mut(&id_u64) {
                        Some(nft) => {
                            if nft.owner != owner_addr {
                                return jsonrpc_error(&id, -32000, "only owner can relist nft");
                            }
                            nft.listed = true;
                            nft.price_wei = new_price;
                            jsonrpc_response(&id, market_nft_to_json(nft))
                        }
                        None => jsonrpc_error(&id, -32000, "unknown nft token id"),
                    }
                }
                _ => jsonrpc_error(
                    &id,
                    -32602,
                    "invalid params: expected [tokenIdHex, ownerAddress, priceWeiHex]",
                ),
            }
        }
        "eth2077_marketReceipt" => {
            let tx_hash = params
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(|v| v.to_ascii_lowercase());
            match tx_hash {
                Some(hash) => {
                    let result = state
                        .market_receipts
                        .get(&hash)
                        .map(market_receipt_to_json)
                        .unwrap_or(Value::Null);
                    jsonrpc_response(&id, result)
                }
                None => jsonrpc_error(&id, -32602, "invalid params: expected [txHash]"),
            }
        }
        "eth2077_marketMode" => jsonrpc_response(
            &id,
            serde_json::json!({
              "mode": "devnet-native",
              "erc721Ready": false,
              "plannedContract": {
                "name": "ETH2077Market721",
                "symbol": "E77NFT",
                "address": Value::Null,
                "standard": "ERC-721"
              },
              "migrationNotes": [
                "When full EVM lane is enabled, list/mint/buy/relist/transfer flows map to ERC-721 + market contract calls.",
                "UI already isolates market RPC calls through marketRpc(), so the backend mode can switch without breaking UX."
              ]
            }),
        ),
        "rpc_modules" => jsonrpc_response(
            &id,
            serde_json::json!({
              "eth": "1.0",
              "net": "1.0",
              "web3": "1.0",
              "debug": "1.0",
              "trace": "1.0",
              "eth2077": "1.0",
              "hardhat": "1.0",
              "anvil": "1.0"
            }),
        ),
        _ => jsonrpc_error(&id, -32601, &format!("method not found: {method}")),
    }
}

fn handle_jsonrpc_payload(state: &mut NodeState, payload: &Value) -> Option<Value> {
    if let Some(batch) = payload.as_array() {
        if batch.is_empty() {
            return Some(jsonrpc_error(&Value::Null, -32600, "invalid request"));
        }
        let mut responses = Vec::new();
        for req in batch {
            if !req.is_object() {
                responses.push(jsonrpc_error(&Value::Null, -32600, "invalid request"));
                continue;
            }
            let has_id = req.get("id").is_some();
            let resp = handle_jsonrpc(state, req);
            if has_id {
                responses.push(resp);
            }
        }
        if responses.is_empty() {
            None
        } else {
            Some(Value::Array(responses))
        }
    } else if payload.is_object() {
        let has_id = payload.get("id").is_some();
        let resp = handle_jsonrpc(state, payload);
        if has_id {
            Some(resp)
        } else {
            None
        }
    } else {
        Some(jsonrpc_error(&Value::Null, -32600, "invalid request"))
    }
}

fn seed_market_nfts(now: u64) -> HashMap<u64, MarketNft> {
    let mut out = HashMap::new();

    let seed = [
        (
            1u64,
            "Citadel Genesis #1",
            "First generation ETH2077 demo marketplace NFT",
            "https://picsum.photos/seed/eth2077-genesis-1/640/640",
            1_500_000_000_000_000u128,
        ),
        (
            2u64,
            "Citadel Genesis #2",
            "Proof-of-concept NFT for ETH2077 marketplace flows",
            "https://picsum.photos/seed/eth2077-genesis-2/640/640",
            2_000_000_000_000_000u128,
        ),
        (
            3u64,
            "Citadel Genesis #3",
            "Live devnet collectible settled through MetaMask",
            "https://picsum.photos/seed/eth2077-genesis-3/640/640",
            3_000_000_000_000_000u128,
        ),
    ];

    for (token_id, name, description, image, price_wei) in seed {
        out.insert(
            token_id,
            MarketNft {
                token_id,
                name: name.to_string(),
                description: description.to_string(),
                image: image.to_string(),
                owner: MARKET_MERCHANT_ADDRESS.to_string(),
                listed: true,
                price_wei,
                created_at_unix_s: now,
                last_tx_hash: None,
            },
        );
    }

    out
}

fn build_state(cfg: &Config, chain_spec: &Value) -> NodeState {
    let scenario = ScenarioConfig {
        name: format!("devnet-node-{}", cfg.node_id),
        nodes: cfg.nodes,
        tx_count: 100_000,
        seed: 2077 + cfg.node_id as u64,
        ingress_tps_per_node: 55_000.0,
        execution_tps_per_node: 38_000.0,
        oob_tps_per_node: 62_000.0,
        mesh_efficiency: 0.75,
        base_rtt_ms: 18.0,
        jitter_ms: 4.0,
        commit_batch_size: 512,
        byzantine_fraction: 0.0,
        packet_loss_fraction: 0.01,
    };
    let (exec, oob, bridge) = bootstrap(&scenario);

    let chain_id = chain_spec["chain_id"].as_u64().unwrap_or(0);
    let network = chain_spec["name"]
        .as_str()
        .unwrap_or("eth2077-devnet")
        .to_string();
    let peers_target = cfg.nodes.saturating_sub(1);
    let started_at_unix_s = now_unix_s();
    let market_nfts = seed_market_nfts(started_at_unix_s);

    NodeState {
        node_id: cfg.node_id,
        nodes: cfg.nodes,
        rpc_port: cfg.rpc_port,
        p2p_port: cfg.p2p_port,
        chain_id,
        network,
        started_at_unix_s,
        current_height: 0,
        finalized_height: 0,
        peers_target,
        peers_connected: peers_target,
        ingress_capacity_tps: scenario.ingress_tps_per_node * cfg.nodes as f64,
        execution_capacity_tps: exec,
        oob_capacity_tps: oob,
        bridge_replay_safe: bridge,
        pending_txs: Vec::new(),
        txs: HashMap::new(),
        block_txs: HashMap::new(),
        block_timestamps: HashMap::from([(0u64, started_at_unix_s)]),
        nonce_by_address: HashMap::new(),
        pending_nonce_address_hint: None,
        tx_counter: 0,
        engine_required_il_txs: Vec::new(),
        engine_required_il_meta: HashMap::new(),
        engine_required_il_slot: None,
        engine_required_il_updated_at_unix_s: None,
        engine_focil_committee: Vec::new(),
        engine_focil_view_frozen: false,
        engine_focil_frozen_slot: None,
        engine_focil_frozen_il_root: None,
        engine_focil_view_id: None,
        engine_7732_headers_by_slot: HashMap::new(),
        engine_7732_headers_by_root: HashMap::new(),
        engine_7732_envelopes_by_root: HashMap::new(),
        engine_7732_penalties_by_slot: HashMap::new(),
        evm_db: InMemoryDB::default(),
        tx_receipts: HashMap::new(),
        market_nfts,
        next_market_token_id: 4,
        market_receipts: HashMap::new(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cfg = Config {
        rpc_host: arg_value(&args, "--rpc-host", "127.0.0.1"),
        node_id: arg_value(&args, "--node-id", "0").parse().unwrap_or(0),
        nodes: arg_value(&args, "--nodes", "4").parse().unwrap_or(4),
        rpc_port: arg_value(&args, "--rpc-port", "9545")
            .parse()
            .unwrap_or(9545),
        p2p_port: arg_value(&args, "--p2p-port", "30303")
            .parse()
            .unwrap_or(30303),
        tick_ms: arg_value(&args, "--tick-ms", "1000")
            .parse()
            .unwrap_or(1000),
        chain_spec_path: PathBuf::from(arg_value(
            &args,
            "--chain-spec",
            "artifacts/testnet-alpha/chain-spec.json",
        )),
        data_dir: PathBuf::from(arg_value(
            &args,
            "--data-dir",
            "artifacts/devnet-local/node-0",
        )),
    };

    fs::create_dir_all(&cfg.data_dir).expect("create data dir");
    let chain_spec_raw = fs::read_to_string(&cfg.chain_spec_path).expect("read chain spec");
    let chain_spec: Value = serde_json::from_str(&chain_spec_raw).expect("parse chain spec");

    let state = Arc::new(Mutex::new(build_state(&cfg, &chain_spec)));
    let state_for_tick = Arc::clone(&state);
    let data_dir_for_tick = cfg.data_dir.clone();
    let tick_every = Duration::from_millis(cfg.tick_ms.max(50));

    thread::spawn(move || loop {
        thread::sleep(tick_every);
        let snapshot = {
            let mut guard = state_for_tick.lock().expect("state lock");
            guard.current_height = guard.current_height.saturating_add(1);
            guard.finalized_height = guard.current_height.saturating_sub(2);
            let current_block = guard.current_height;
            let current_block_hash = block_hash(current_block);
            let current_block_ts = now_unix_s();
            guard
                .block_timestamps
                .insert(current_block, current_block_ts);
            let pending_hashes = guard.pending_txs.clone();
            guard.pending_txs.clear();

            if !pending_hashes.is_empty() {
                guard
                    .block_txs
                    .insert(current_block, pending_hashes.clone());
                let mut cumulative_gas_used = 0u64;
                let mut next_log_index = 0u64;
                for (idx, tx_hash) in pending_hashes.iter().enumerate() {
                    if let Some(tx) = guard.txs.get_mut(tx_hash) {
                        tx.block_number = Some(current_block);
                        tx.block_hash = Some(current_block_hash.clone());
                        tx.transaction_index = Some(idx as u64);
                    }
                    if let Some(receipt) = guard.tx_receipts.get_mut(tx_hash) {
                        cumulative_gas_used = cumulative_gas_used.saturating_add(receipt.gas_used);
                        receipt.cumulative_gas_used = cumulative_gas_used;
                        receipt.block_number = Some(current_block);
                        receipt.block_hash = Some(current_block_hash.clone());
                        receipt.transaction_index = Some(idx as u64);
                        for log in &mut receipt.logs {
                            log.block_number = Some(current_block);
                            log.block_hash = Some(current_block_hash.clone());
                            log.transaction_hash = Some(tx_hash.clone());
                            log.transaction_index = Some(idx as u64);
                            log.log_index = Some(next_log_index);
                            next_log_index = next_log_index.saturating_add(1);
                        }
                    }
                }
            }
            guard.clone()
        };
        write_json(data_dir_for_tick.join("status.json"), &snapshot);
    });

    let listener =
        TcpListener::bind((cfg.rpc_host.as_str(), cfg.rpc_port)).expect("bind devnet rpc listener");
    println!(
        "eth2077-devnetd node {} listening on {}:{} (chain-spec: {})",
        cfg.node_id,
        cfg.rpc_host,
        cfg.rpc_port,
        cfg.chain_spec_path.display()
    );

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut req_bytes = Vec::with_capacity(8192);
        let mut content_length: Option<usize> = None;
        let mut header_end: Option<usize> = None;

        loop {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            req_bytes.extend_from_slice(&buf[..n]);

            if header_end.is_none() {
                header_end = find_subslice(&req_bytes, b"\r\n\r\n").map(|i| i + 4);
                if let Some(h_end) = header_end {
                    let header_str = String::from_utf8_lossy(&req_bytes[..h_end]);
                    for line in header_str.lines() {
                        let lower = line.to_ascii_lowercase();
                        if lower.starts_with("content-length:") {
                            let val = line
                                .split_once(':')
                                .map(|(_, rhs)| rhs.trim())
                                .and_then(|s| s.parse::<usize>().ok());
                            content_length = val;
                            break;
                        }
                    }
                }
            }

            if let Some(h_end) = header_end {
                let expected = h_end + content_length.unwrap_or(0);
                if req_bytes.len() >= expected {
                    break;
                }
            }

            if req_bytes.len() > 2 * 1024 * 1024 {
                break;
            }
        }

        let req = String::from_utf8_lossy(&req_bytes);
        let request_line = req.lines().next().unwrap_or("");

        let response = if request_line.starts_with("OPTIONS / ") {
            "HTTP/1.1 204 No Content\r\nConnection: close\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n"
                .to_string()
        } else if request_line.starts_with("GET /healthz ") {
            json_response("200 OK", &serde_json::json!({ "ok": true }))
        } else if request_line.starts_with("GET /status ") {
            let snapshot = state.lock().expect("state lock").clone();
            json_response("200 OK", &serde_json::to_value(snapshot).expect("to value"))
        } else if request_line.starts_with("POST / ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            match serde_json::from_str::<Value>(&body) {
                Ok(json_req) if json_req.is_object() || json_req.is_array() => {
                    let mut guard = state.lock().expect("state lock");
                    if let Some(rpc_resp) = handle_jsonrpc_payload(&mut guard, &json_req) {
                        json_response("200 OK", &rpc_resp)
                    } else {
                        "HTTP/1.1 204 No Content\r\nConnection: close\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n".to_string()
                    }
                }
                Ok(_) => json_response(
                    "200 OK",
                    &jsonrpc_error(&Value::Null, -32600, "invalid request"),
                ),
                Err(_) => json_response(
                    "200 OK",
                    &jsonrpc_error(&Value::Null, -32700, "parse error"),
                ),
            }
        } else if request_line.starts_with("GET /engine/v1/capabilities ") {
            json_response(
                "200 OK",
                &serde_json::json!({
                    "capabilities": [
                        "engine_newPayloadV3",
                        "engine_forkchoiceUpdatedV3",
                        "engine_getPayloadV3",
                        "engine_getInclusionListV1",
                        "engine_registerExecutionPayloadHeaderV1",
                        "engine_registerExecutionPayloadEnvelopeV1",
                        "engine_getPayloadTimelinessV1",
                        "engine_getExecutionPayloadEnvelopeV1"
                    ]
                }),
            )
        } else if request_line.starts_with("POST /engine/v1/getInclusionListV1 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let mut guard = state.lock().expect("state lock");
                engine_get_inclusion_list_response(&mut guard, &req_json)
            };
            json_response("200 OK", &result)
        } else if request_line.starts_with("POST /engine/v1/newPayloadV3 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let mut guard = state.lock().expect("state lock");
                engine_new_payload_response(&mut guard, &req_json)
            };
            json_response("200 OK", &result)
        } else if request_line.starts_with("POST /engine/v1/forkchoiceUpdatedV3 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let mut guard = state.lock().expect("state lock");
                engine_forkchoice_updated_response(&mut guard, &req_json)
            };
            json_response("200 OK", &result)
        } else if request_line.starts_with("POST /engine/v1/registerExecutionPayloadHeaderV1 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let mut guard = state.lock().expect("state lock");
                engine_7732_register_payload_header_response(&mut guard, &req_json)
            };
            json_response("200 OK", &result)
        } else if request_line.starts_with("POST /engine/v1/registerExecutionPayloadEnvelopeV1 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let mut guard = state.lock().expect("state lock");
                engine_7732_register_payload_envelope_response(&mut guard, &req_json)
            };
            json_response("200 OK", &result)
        } else if request_line.starts_with("POST /engine/v1/getPayloadTimelinessV1 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let guard = state.lock().expect("state lock");
                engine_7732_get_payload_timeliness_response(&guard, &req_json)
            };
            json_response("200 OK", &result)
        } else if request_line.starts_with("POST /engine/v1/getExecutionPayloadEnvelopeV1 ") {
            let body = request_body_from_http(&req_bytes, header_end, content_length);
            let req_json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let result = {
                let guard = state.lock().expect("state lock");
                engine_7732_get_payload_envelope_response(&guard, &req_json)
            };
            json_response("200 OK", &result)
        } else {
            json_response(
                "404 Not Found",
                &serde_json::json!({ "error": "not found", "path": request_line }),
            )
        };

        let _ = stream.write_all(response.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlp::RlpStream;

    fn typed_tx_hex(tx_type: u8, payload: Vec<u8>) -> String {
        let mut bytes = Vec::with_capacity(1 + payload.len());
        bytes.push(tx_type);
        bytes.extend_from_slice(&payload);
        bytes_to_hex(&bytes)
    }

    #[test]
    fn decode_eip4844_raw_tx_extracts_blob_fields() {
        let to = vec![0x22u8; 20];
        let mut blob_hash = [0u8; 32];
        blob_hash[0] = 0x01; // KZG versioned hash prefix
        blob_hash[31] = 0x7f;

        // Type 0x03 payload:
        // [chain_id, nonce, max_priority, max_fee, gas_limit, to, value, data, access_list, max_fee_per_blob_gas, blob_hashes, y_parity, r, s]
        let mut s = RlpStream::new_list(14);
        s.append(&2077003u64);
        s.append(&0u64);
        s.append(&1_000_000_000u64);
        s.append(&2_000_000_000u64);
        s.append(&30_000_000u64);
        s.append(&to);
        s.append(&0u64);
        s.append(&Vec::<u8>::new());
        s.begin_list(0); // access list
        s.append(&10u64); // max fee per blob gas
        s.begin_list(1); // blob hashes
        s.append(&blob_hash.to_vec());
        s.append(&0u8); // y parity
        s.append(&Vec::<u8>::new()); // r
        s.append(&Vec::<u8>::new()); // s

        let raw = typed_tx_hex(0x03, s.out().to_vec());
        let decoded = decode_raw_tx(&raw).expect("decode 0x03");

        assert_eq!(decoded.tx_type, 0x03);
        assert_eq!(decoded.chain_id, Some(2077003));
        assert_eq!(decoded.max_priority_fee_per_gas, Some(1_000_000_000));
        assert_eq!(decoded.gas_price, 2_000_000_000);
        assert_eq!(decoded.max_fee_per_blob_gas, Some(10));
        assert_eq!(decoded.blob_hashes.len(), 1);
        assert_eq!(decoded.blob_hashes[0], B256::from(blob_hash));
    }

    #[test]
    fn decode_unknown_typed_tx_reports_supported_types() {
        let s = RlpStream::new_list(0);
        let raw = typed_tx_hex(0x09, s.out().to_vec());
        let err = decode_raw_tx(&raw).expect_err("unsupported tx type should fail");
        assert!(err.contains("0x01"));
        assert!(err.contains("0x02"));
        assert!(err.contains("0x03"));
        assert!(err.contains("0x04"));
    }

    #[test]
    fn decode_eip7702_raw_tx_extracts_authorizations() {
        let to = vec![0x44u8; 20];
        let auth_addr = vec![0x55u8; 20];

        // Type 0x04 payload:
        // [chain_id, nonce, max_priority, max_fee, gas_limit, to, value, data, access_list, authorization_list, y_parity, r, s]
        let mut s = RlpStream::new_list(13);
        s.append(&2077003u64);
        s.append(&1u64);
        s.append(&1_000_000_000u64);
        s.append(&2_000_000_000u64);
        s.append(&30_000_000u64);
        s.append(&to);
        s.append(&0u64);
        s.append(&Vec::<u8>::new());
        s.begin_list(0); // access list
        s.begin_list(1); // authorization list
        s.begin_list(6); // one signed authorization
        s.append(&2077003u64); // auth chain id
        s.append(&auth_addr);
        s.append(&0u64); // auth nonce
        s.append(&0u8); // auth y parity
        s.append(&Vec::<u8>::new()); // auth r
        s.append(&Vec::<u8>::new()); // auth s
        s.append(&0u8); // tx y parity
        s.append(&Vec::<u8>::new()); // tx r
        s.append(&Vec::<u8>::new()); // tx s

        let raw = typed_tx_hex(0x04, s.out().to_vec());
        let decoded = decode_raw_tx(&raw).expect("decode 0x04");

        assert_eq!(decoded.tx_type, 0x04);
        assert_eq!(decoded.chain_id, Some(2077003));
        assert_eq!(decoded.authorizations.len(), 1);
        assert_eq!(decoded.max_priority_fee_per_gas, Some(1_000_000_000));
        assert_eq!(decoded.gas_price, 2_000_000_000);
    }

    fn test_state(chain_id: u64) -> NodeState {
        let cfg = Config {
            rpc_host: "127.0.0.1".to_string(),
            node_id: 0,
            nodes: 1,
            rpc_port: 0,
            p2p_port: 0,
            tick_ms: 1000,
            chain_spec_path: PathBuf::from("tests/chain-spec.json"),
            data_dir: PathBuf::from("artifacts/test-state"),
        };
        let chain_spec = serde_json::json!({
            "chain_id": chain_id,
            "name": "eth2077-test"
        });
        build_state(&cfg, &chain_spec)
    }

    fn rpc(state: &mut NodeState, id: u64, method: &str, params: Value) -> Value {
        handle_jsonrpc(
            state,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params
            }),
        )
    }

    fn rpc_raw(state: &mut NodeState, req: Value) -> Value {
        handle_jsonrpc(state, &req)
    }

    #[test]
    fn send_eip4844_raw_tx_via_jsonrpc_persists_blob_fields() {
        let mut state = test_state(2077003);
        let sender = "0x375249129507aec9309abc1f5c055494200c7c32";
        let to = vec![0x22u8; 20];
        let mut blob_hash = [0u8; 32];
        blob_hash[0] = 0x01;
        blob_hash[31] = 0x7f;

        let set_balance = rpc(
            &mut state,
            1,
            "eth2077_setBalance",
            serde_json::json!([sender, "0x3635c9adc5dea00000"]),
        );
        assert_eq!(set_balance["result"], Value::Bool(true));

        let nonce_resp = rpc(
            &mut state,
            2,
            "eth_getTransactionCount",
            serde_json::json!([sender, "latest"]),
        );
        assert_eq!(nonce_resp["result"], Value::String("0x0".to_string()));

        let mut s = RlpStream::new_list(14);
        s.append(&state.chain_id);
        s.append(&0u64);
        s.append(&1_000_000_000u64);
        s.append(&2_000_000_000u64);
        s.append(&30_000_000u64);
        s.append(&to);
        s.append(&0u64);
        s.append(&Vec::<u8>::new());
        s.begin_list(0);
        s.append(&10u64);
        s.begin_list(1);
        s.append(&blob_hash.to_vec());
        s.append(&0u8);
        s.append(&Vec::<u8>::new());
        s.append(&Vec::<u8>::new());
        let raw = typed_tx_hex(0x03, s.out().to_vec());

        let send_resp = rpc(
            &mut state,
            3,
            "eth_sendRawTransaction",
            serde_json::json!([raw]),
        );
        let tx_hash = send_resp["result"]
            .as_str()
            .expect("tx hash result")
            .to_string();

        let tx_resp = rpc(
            &mut state,
            4,
            "eth_getTransactionByHash",
            serde_json::json!([tx_hash]),
        );
        assert_eq!(tx_resp["result"]["type"], Value::String("0x3".to_string()));
        assert_eq!(
            tx_resp["result"]["maxFeePerBlobGas"],
            Value::String("0xa".to_string())
        );
        let blob_hashes = tx_resp["result"]["blobVersionedHashes"]
            .as_array()
            .expect("blob hashes array");
        assert_eq!(blob_hashes.len(), 1);
    }

    #[test]
    fn send_eip7702_raw_tx_via_jsonrpc_persists_authorization_length() {
        let mut state = test_state(2077003);
        let sender = "0x375249129507aec9309abc1f5c055494200c7c32";
        let to = vec![0x44u8; 20];
        let auth_addr = vec![0x55u8; 20];

        let set_balance = rpc(
            &mut state,
            10,
            "eth2077_setBalance",
            serde_json::json!([sender, "0x3635c9adc5dea00000"]),
        );
        assert_eq!(set_balance["result"], Value::Bool(true));

        let nonce_resp = rpc(
            &mut state,
            11,
            "eth_getTransactionCount",
            serde_json::json!([sender, "latest"]),
        );
        assert_eq!(nonce_resp["result"], Value::String("0x0".to_string()));

        let mut s = RlpStream::new_list(13);
        s.append(&state.chain_id);
        s.append(&0u64);
        s.append(&1_000_000_000u64);
        s.append(&2_000_000_000u64);
        s.append(&30_000_000u64);
        s.append(&to);
        s.append(&0u64);
        s.append(&Vec::<u8>::new());
        s.begin_list(0); // access list
        s.begin_list(1); // authorization list
        s.begin_list(6); // one authorization
        s.append(&state.chain_id);
        s.append(&auth_addr);
        s.append(&0u64);
        s.append(&0u8);
        s.append(&Vec::<u8>::new());
        s.append(&Vec::<u8>::new());
        s.append(&0u8); // tx y parity
        s.append(&Vec::<u8>::new()); // tx r
        s.append(&Vec::<u8>::new()); // tx s
        let raw = typed_tx_hex(0x04, s.out().to_vec());

        let send_resp = rpc(
            &mut state,
            12,
            "eth_sendRawTransaction",
            serde_json::json!([raw]),
        );
        let tx_hash = send_resp["result"]
            .as_str()
            .expect("tx hash result")
            .to_string();

        let tx_resp = rpc(
            &mut state,
            13,
            "eth_getTransactionByHash",
            serde_json::json!([tx_hash]),
        );
        assert_eq!(tx_resp["result"]["type"], Value::String("0x4".to_string()));
        assert_eq!(
            tx_resp["result"]["authorizationListLength"],
            Value::String("0x1".to_string())
        );
    }

    #[test]
    fn fee_history_includes_blob_fee_fields() {
        let mut state = test_state(2077003);
        let resp = rpc(
            &mut state,
            20,
            "eth_feeHistory",
            serde_json::json!(["0xa", "latest", []]),
        );
        let result = resp["result"].as_object().expect("feeHistory object");
        assert!(result.get("baseFeePerBlobGas").is_some());
        assert!(result.get("blobGasUsedRatio").is_some());
        let base_fee = rpc(&mut state, 21, "eth_blobBaseFee", serde_json::json!([]));
        assert_eq!(
            base_fee["result"],
            Value::String(hex_u128(blob_base_fee_per_gas()))
        );
    }

    #[test]
    fn engine_forkchoice_sets_inclusion_list_and_get_returns_it() {
        let mut state = test_state(2077003);
        let h1 = format!("0x{}", "11".repeat(32));
        let h2 = format!("0x{}", "22".repeat(32));
        let head_hash = block_hash(state.current_height);

        let fc_resp = rpc(
            &mut state,
            30,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              { "headBlockHash": head_hash },
              {
                "payloadAttributes": {
                  "slot": "0x2a",
                  "inclusionListTransactions": [h1, h2]
                }
              }
            ]),
        );
        assert_eq!(fc_resp["result"]["payloadStatus"]["status"], "VALID");

        let il_resp = rpc(
            &mut state,
            31,
            "engine_getInclusionListV1",
            serde_json::json!(["0x2a"]),
        );
        assert_eq!(il_resp["result"]["slot"], "0x2a");
        let txs = il_resp["result"]["transactions"]
            .as_array()
            .expect("transactions array");
        assert_eq!(txs.len(), 2);
    }

    #[test]
    fn engine_new_payload_reports_unsatisfied_when_missing_inclusion_tx() {
        let mut state = test_state(2077003);
        let h1 = format!("0x{}", "aa".repeat(32));
        let h2 = format!("0x{}", "bb".repeat(32));
        let next_block_hash = block_hash(state.current_height.saturating_add(1));

        let _ = rpc(
            &mut state,
            40,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "inclusionListTransactions": [h1, h2]
                }
              }
            ]),
        );

        let np_resp = rpc(
            &mut state,
            41,
            "engine_newPayloadV3",
            serde_json::json!([
              {
                "blockHash": next_block_hash,
                "transactions": [format!("0x{}", "aa".repeat(32))]
              }
            ]),
        );

        assert_eq!(
            np_resp["result"]["status"],
            Value::String("INCLUSION_LIST_UNSATISFIED".to_string())
        );
        assert!(
            np_resp["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("missing")
        );
    }

    #[test]
    fn engine_new_payload_is_valid_when_inclusion_list_satisfied() {
        let mut state = test_state(2077003);
        let h1 = format!("0x{}", "cc".repeat(32));
        let h2 = format!("0x{}", "dd".repeat(32));
        let next_block_hash = block_hash(state.current_height.saturating_add(1));

        let _ = rpc(
            &mut state,
            50,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "inclusionListTransactions": [h1, h2]
                }
              }
            ]),
        );

        let np_resp = rpc(
            &mut state,
            51,
            "engine_newPayloadV3",
            serde_json::json!([
              {
                "blockHash": next_block_hash,
                "transactions": [
                  format!("0x{}", "cc".repeat(32)),
                  format!("0x{}", "dd".repeat(32))
                ]
              }
            ]),
        );
        assert_eq!(np_resp["result"]["status"], "VALID");
    }

    #[test]
    fn engine_new_payload_reports_unsatisfied_for_required_metadata_mismatch() {
        let mut state = test_state(2077003);
        let sender = "0x375249129507aec9309abc1f5c055494200c7c32";
        let h1 = format!("0x{}", "77".repeat(32));
        let next_block_hash = block_hash(state.current_height.saturating_add(1));

        let set_balance = rpc(
            &mut state,
            60,
            "eth2077_setBalance",
            serde_json::json!([sender, "0x56bc75e2d63100000"]),
        );
        assert_eq!(set_balance["result"], Value::Bool(true));

        let _ = rpc(
            &mut state,
            61,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "inclusionListTransactions": [
                    {
                      "hash": h1,
                      "from": sender,
                      "nonce": "0x0",
                      "gas": "0x5208",
                      "gasPrice": "0x3b9aca00",
                      "value": "0x0"
                    }
                  ]
                }
              }
            ]),
        );

        let np_resp = rpc(
            &mut state,
            62,
            "engine_newPayloadV3",
            serde_json::json!([
              {
                "blockHash": next_block_hash,
                "transactions": [
                  {
                    "hash": format!("0x{}", "77".repeat(32)),
                    "from": sender,
                    "nonce": "0x1",
                    "gas": "0x5208",
                    "gasPrice": "0x3b9aca00",
                    "value": "0x0"
                  }
                ]
              }
            ]),
        );

        assert_eq!(np_resp["result"]["status"], "INCLUSION_LIST_UNSATISFIED");
        assert!(
            np_resp["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("nonce mismatch")
        );
    }

    #[test]
    fn engine_new_payload_reports_unsatisfied_for_required_insufficient_balance() {
        let mut state = test_state(2077003);
        let sender = "0x375249129507aec9309abc1f5c055494200c7c32";
        let h1 = format!("0x{}", "88".repeat(32));
        let next_block_hash = block_hash(state.current_height.saturating_add(1));

        let _ = rpc(
            &mut state,
            70,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "inclusionListTransactions": [
                    {
                      "hash": h1,
                      "from": sender,
                      "nonce": "0x0",
                      "gas": "0x5208",
                      "gasPrice": "0x3b9aca00",
                      "value": "0xde0b6b3a7640000"
                    }
                  ]
                }
              }
            ]),
        );

        let np_resp = rpc(
            &mut state,
            71,
            "engine_newPayloadV3",
            serde_json::json!([
              {
                "blockHash": next_block_hash,
                "transactions": [
                  {
                    "hash": format!("0x{}", "88".repeat(32)),
                    "from": sender,
                    "nonce": "0x0",
                    "gas": "0x5208",
                    "gasPrice": "0x3b9aca00",
                    "value": "0xde0b6b3a7640000"
                  }
                ]
              }
            ]),
        );

        assert_eq!(np_resp["result"]["status"], "INCLUSION_LIST_UNSATISFIED");
        assert!(
            np_resp["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("insufficient balance")
        );
    }

    #[test]
    fn engine_get_inclusion_list_freezes_view_and_blocks_mutation_same_slot() {
        let mut state = test_state(2077003);
        let h1 = format!("0x{}", "99".repeat(32));
        let h2 = format!("0x{}", "aa".repeat(32));
        let slot = "0x2a";

        let _ = rpc(
            &mut state,
            80,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "slot": slot,
                  "inclusionListTransactions": [h1]
                }
              }
            ]),
        );

        let il_resp = rpc(
            &mut state,
            81,
            "engine_getInclusionListV1",
            serde_json::json!([slot]),
        );
        assert_eq!(il_resp["result"]["viewFreeze"]["frozen"], Value::Bool(true));

        let fc_mutation_resp = rpc(
            &mut state,
            82,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "slot": slot,
                  "inclusionListTransactions": [h2]
                }
              }
            ]),
        );
        assert_eq!(fc_mutation_resp["result"]["payloadStatus"]["status"], "INVALID");
        assert!(
            fc_mutation_resp["result"]["payloadStatus"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("view frozen")
        );
    }

    #[test]
    fn engine_forkchoice_rotates_view_on_new_slot_after_freeze() {
        let mut state = test_state(2077003);
        let h1 = format!("0x{}", "ab".repeat(32));
        let h2 = format!("0x{}", "bc".repeat(32));

        let _ = rpc(
            &mut state,
            90,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "slot": "0x2a",
                  "inclusionListTransactions": [h1]
                }
              }
            ]),
        );

        let _ = rpc(
            &mut state,
            91,
            "engine_getInclusionListV1",
            serde_json::json!(["0x2a"]),
        );

        let fc_next_slot_resp = rpc(
            &mut state,
            92,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "slot": "0x2b",
                  "viewId": "0x2b",
                  "inclusionListTransactions": [h2]
                }
              }
            ]),
        );
        assert_eq!(fc_next_slot_resp["result"]["payloadStatus"]["status"], "VALID");
        assert_eq!(fc_next_slot_resp["result"]["focil"]["viewFrozen"], Value::Bool(false));
        assert_eq!(fc_next_slot_resp["result"]["focil"]["viewId"], "0x2b");
    }

    #[test]
    fn engine_new_payload_rejects_payload_slot_mismatch_when_view_frozen() {
        let mut state = test_state(2077003);
        let h1 = format!("0x{}", "cd".repeat(32));
        let block_hash_hint = block_hash(state.current_height.saturating_add(1));

        let _ = rpc(
            &mut state,
            100,
            "engine_forkchoiceUpdatedV3",
            serde_json::json!([
              {},
              {
                "payloadAttributes": {
                  "slot": "0x2a",
                  "inclusionListTransactions": [h1]
                }
              }
            ]),
        );

        let _ = rpc(
            &mut state,
            101,
            "engine_getInclusionListV1",
            serde_json::json!(["0x2a"]),
        );

        let np_resp = rpc(
            &mut state,
            102,
            "engine_newPayloadV3",
            serde_json::json!([
              {
                "blockHash": block_hash_hint,
                "slot": "0x2b",
                "transactions": [format!("0x{}", "cd".repeat(32))]
              }
            ]),
        );

        assert_eq!(np_resp["result"]["status"], "INCLUSION_LIST_UNSATISFIED");
        assert!(
            np_resp["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("view frozen")
        );
    }

    #[test]
    fn engine_7732_header_registration_shows_header_only_timeliness() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "ef".repeat(32));
        let block_hash_hint = block_hash(state.current_height.saturating_add(1));

        let register_header = rpc(
            &mut state,
            110,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": "0x10",
              "payloadHeaderRoot": root,
              "executionBlockHash": block_hash_hint,
              "bidValue": "0x2a"
            }]),
        );
        assert_eq!(register_header["result"]["status"], "ACCEPTED");

        let timeliness = rpc(
            &mut state,
            111,
            "engine_getPayloadTimelinessV1",
            serde_json::json!(["0x10"]),
        );
        assert_eq!(timeliness["result"]["aggregateStatus"], "HEADER_ONLY");
        assert_eq!(timeliness["result"]["headerCount"], "0x1");
        assert_eq!(timeliness["result"]["revealedCount"], "0x0");
    }

    #[test]
    fn engine_7732_on_time_reveal_transitions_to_revealed() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "01".repeat(32));
        let on_time_reveal_at = state.started_at_unix_s + (0x11 * 12) + 5;

        let _ = rpc(
            &mut state,
            120,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": "0x11",
              "payloadHeaderRoot": root
            }]),
        );

        let _ = rpc(
            &mut state,
            121,
            "engine_registerExecutionPayloadEnvelopeV1",
            serde_json::json!([{
              "slot": "0x11",
              "payloadHeaderRoot": format!("0x{}", "01".repeat(32)),
              "revealedAtUnixS": on_time_reveal_at
            }]),
        );

        let timeliness = rpc(
            &mut state,
            122,
            "engine_getPayloadTimelinessV1",
            serde_json::json!(["0x11"]),
        );
        assert_eq!(timeliness["result"]["aggregateStatus"], "REVEALED");
        assert_eq!(timeliness["result"]["revealedCount"], "0x1");
        assert_eq!(timeliness["result"]["lateRevealCount"], "0x0");

        let envelope = rpc(
            &mut state,
            123,
            "engine_getExecutionPayloadEnvelopeV1",
            serde_json::json!([format!("0x{}", "01".repeat(32))]),
        );
        assert_eq!(
            envelope["result"]["payloadHeaderRoot"],
            Value::String(format!("0x{}", "01".repeat(32)))
        );
    }

    #[test]
    fn engine_7732_late_reveal_is_reported() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "23".repeat(32));
        let slot = 0x12u64;

        let _ = rpc(
            &mut state,
            130,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root
            }]),
        );

        let late_reveal_at = state.started_at_unix_s + (slot * 12) + 25;
        let _ = rpc(
            &mut state,
            131,
            "engine_registerExecutionPayloadEnvelopeV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": format!("0x{}", "23".repeat(32)),
              "revealedAtUnixS": late_reveal_at
            }]),
        );

        let timeliness = rpc(
            &mut state,
            132,
            "engine_getPayloadTimelinessV1",
            serde_json::json!([hex_u64(slot)]),
        );
        assert_eq!(timeliness["result"]["aggregateStatus"], "LATE_REVEAL");
        assert_eq!(timeliness["result"]["lateRevealCount"], "0x1");
    }

    #[test]
    fn engine_7732_withheld_is_reported_after_deadline() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "34".repeat(32));
        let slot = 0x13u64;
        let deadline = state.started_at_unix_s + (slot * 12) + 12;

        let _ = rpc(
            &mut state,
            140,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root
            }]),
        );

        let timeliness = rpc(
            &mut state,
            141,
            "engine_getPayloadTimelinessV1",
            serde_json::json!([hex_u64(slot), hex_u64(deadline + 1)]),
        );
        assert_eq!(timeliness["result"]["aggregateStatus"], "WITHHELD");
        assert_eq!(timeliness["result"]["deadlinePassed"], Value::Bool(true));
    }

    #[test]
    fn engine_forkchoice_rejects_withheld_7732_slot() {
        let mut state = test_state(2077003);
        let slot = 0x14u64;
        let root = format!("0x{}", "45".repeat(32));
        let deadline = state.started_at_unix_s + (slot * 12) + 12;

        let _ = rpc(
            &mut state,
            150,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root
            }]),
        );

        let fc_resp = rpc_raw(
            &mut state,
            serde_json::json!({
              "jsonrpc": "2.0",
              "id": 151,
              "method": "engine_forkchoiceUpdatedV3",
              "currentUnixS": hex_u64(deadline + 1),
              "params": [
                {},
                {
                  "payloadAttributes": {
                    "slot": hex_u64(slot)
                  }
                }
              ]
            }),
        );
        assert_eq!(fc_resp["result"]["payloadStatus"]["status"], "INVALID");
        assert!(
            fc_resp["result"]["payloadStatus"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("timeliness violation")
        );
        assert_eq!(fc_resp["result"]["timeliness"]["aggregateStatus"], "WITHHELD");
        assert_eq!(fc_resp["result"]["penalty"]["state"], "ACTIVE");
        assert_eq!(fc_resp["result"]["penalty"]["lastStatus"], "WITHHELD");
    }

    #[test]
    fn engine_new_payload_rejects_withheld_7732_slot() {
        let mut state = test_state(2077003);
        let slot = 0x15u64;
        let root = format!("0x{}", "56".repeat(32));
        let deadline = state.started_at_unix_s + (slot * 12) + 12;
        let next_block_hash = block_hash(state.current_height.saturating_add(1));

        let _ = rpc(
            &mut state,
            160,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root
            }]),
        );

        let np_resp = rpc_raw(
            &mut state,
            serde_json::json!({
              "jsonrpc": "2.0",
              "id": 161,
              "method": "engine_newPayloadV3",
              "currentUnixS": hex_u64(deadline + 1),
              "params": [
                {
                  "slot": hex_u64(slot),
                  "blockHash": next_block_hash,
                  "transactions": []
                }
              ]
            }),
        );

        assert_eq!(np_resp["result"]["status"], "INVALID");
        assert!(
            np_resp["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("timeliness violation")
        );
        assert_eq!(np_resp["result"]["timeliness"]["aggregateStatus"], "WITHHELD");
        assert_eq!(np_resp["result"]["penalty"]["state"], "ACTIVE");
        assert_eq!(np_resp["result"]["penalty"]["lastStatus"], "WITHHELD");
    }

    #[test]
    fn engine_7732_penalty_recovers_after_reveal() {
        let mut state = test_state(2077003);
        let slot = 0x16u64;
        let root = format!("0x{}", "67".repeat(32));
        let deadline = state.started_at_unix_s + (slot * 12) + 12;

        let _ = rpc(
            &mut state,
            170,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": root
            }]),
        );

        let first_fc = rpc_raw(
            &mut state,
            serde_json::json!({
              "jsonrpc": "2.0",
              "id": 171,
              "method": "engine_forkchoiceUpdatedV3",
              "currentUnixS": hex_u64(deadline + 1),
              "params": [
                {},
                {
                  "payloadAttributes": {
                    "slot": hex_u64(slot)
                  }
                }
              ]
            }),
        );
        assert_eq!(first_fc["result"]["payloadStatus"]["status"], "INVALID");
        assert_eq!(first_fc["result"]["penalty"]["state"], "ACTIVE");

        let _ = rpc(
            &mut state,
            172,
            "engine_registerExecutionPayloadEnvelopeV1",
            serde_json::json!([{
              "slot": hex_u64(slot),
              "payloadHeaderRoot": format!("0x{}", "67".repeat(32)),
              "revealedAtUnixS": hex_u64(deadline)
            }]),
        );

        let recovered_fc = rpc_raw(
            &mut state,
            serde_json::json!({
              "jsonrpc": "2.0",
              "id": 173,
              "method": "engine_forkchoiceUpdatedV3",
              "currentUnixS": hex_u64(deadline + 2),
              "params": [
                {},
                {
                  "payloadAttributes": {
                    "slot": hex_u64(slot)
                  }
                }
              ]
            }),
        );
        assert_eq!(recovered_fc["result"]["payloadStatus"]["status"], "VALID");
        assert_eq!(recovered_fc["result"]["timeliness"]["aggregateStatus"], "REVEALED");
        assert_eq!(recovered_fc["result"]["penalty"]["state"], "RECOVERED");
        assert_eq!(recovered_fc["result"]["penalty"]["lastStatus"], "REVEALED");
        assert!(recovered_fc["result"]["penalty"]["recoveredAtUnixS"].is_u64());
    }

    #[test]
    fn engine_7732_header_duplicate_replay_is_idempotent() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "88".repeat(32));
        let slot = "0x20";

        let first = rpc(
            &mut state,
            180,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": slot,
              "payloadHeaderRoot": root
            }]),
        );
        assert_eq!(first["result"]["status"], "ACCEPTED");
        assert_eq!(first["result"]["replayStatus"], "NEW");
        assert_eq!(first["result"]["knownHeadersAtSlot"], "0x1");

        let duplicate = rpc(
            &mut state,
            181,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": slot,
              "payloadHeaderRoot": format!("0x{}", "88".repeat(32))
            }]),
        );
        assert_eq!(duplicate["result"]["status"], "ACCEPTED");
        assert_eq!(duplicate["result"]["replayStatus"], "DUPLICATE");
        assert_eq!(duplicate["result"]["knownHeadersAtSlot"], "0x1");
    }

    #[test]
    fn engine_7732_header_conflicting_replay_is_rejected() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "99".repeat(32));

        let _ = rpc(
            &mut state,
            190,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": "0x21",
              "payloadHeaderRoot": root
            }]),
        );

        let conflict = rpc(
            &mut state,
            191,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": "0x22",
              "payloadHeaderRoot": format!("0x{}", "99".repeat(32))
            }]),
        );
        assert_eq!(conflict["result"]["status"], "INVALID");
        assert!(
            conflict["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("conflicting header replay")
        );
    }

    #[test]
    fn engine_7732_envelope_slot_mismatch_for_known_header_is_rejected() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "aa".repeat(32));

        let _ = rpc(
            &mut state,
            200,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": "0x23",
              "payloadHeaderRoot": root
            }]),
        );

        let mismatch = rpc(
            &mut state,
            201,
            "engine_registerExecutionPayloadEnvelopeV1",
            serde_json::json!([{
              "slot": "0x24",
              "payloadHeaderRoot": format!("0x{}", "aa".repeat(32))
            }]),
        );
        assert_eq!(mismatch["result"]["status"], "INVALID");
        assert!(
            mismatch["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("slot mismatch")
        );
    }

    #[test]
    fn engine_7732_envelope_conflicting_replay_is_rejected() {
        let mut state = test_state(2077003);
        let root = format!("0x{}", "bb".repeat(32));

        let _ = rpc(
            &mut state,
            210,
            "engine_registerExecutionPayloadHeaderV1",
            serde_json::json!([{
              "slot": "0x25",
              "payloadHeaderRoot": root
            }]),
        );

        let first = rpc(
            &mut state,
            211,
            "engine_registerExecutionPayloadEnvelopeV1",
            serde_json::json!([{
              "slot": "0x25",
              "payloadHeaderRoot": format!("0x{}", "bb".repeat(32)),
              "payloadBodyHash": format!("0x{}", "01".repeat(32)),
              "dataAvailable": true
            }]),
        );
        assert_eq!(first["result"]["status"], "ACCEPTED");
        assert_eq!(first["result"]["replayStatus"], "NEW");

        let conflict = rpc(
            &mut state,
            212,
            "engine_registerExecutionPayloadEnvelopeV1",
            serde_json::json!([{
              "slot": "0x25",
              "payloadHeaderRoot": format!("0x{}", "bb".repeat(32)),
              "payloadBodyHash": format!("0x{}", "02".repeat(32)),
              "dataAvailable": true
            }]),
        );
        assert_eq!(conflict["result"]["status"], "INVALID");
        assert!(
            conflict["result"]["validationError"]
                .as_str()
                .unwrap_or_default()
                .contains("conflicting envelope replay")
        );
    }
}
