use reqwest::blocking::Client;
use rlp::RlpStream;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct Args {
    rpc_urls: Vec<String>,
    sender: String,
    to: String,
    sender_count: usize,
    workers: usize,
    tx_count: usize,
    gas_limit: u64,
    max_priority_fee_per_gas: u64,
    max_fee_per_gas: u64,
    value_wei: u64,
    fund_wei_hex: String,
    poll_ms: u64,
    deadline_seconds: u64,
    output_json: PathBuf,
    output_md: PathBuf,
}

impl Args {
    fn default_with_timestamp(ts: u64) -> Self {
        let base = format!("reports/live-tps-{ts}");
        Self {
            rpc_urls: vec!["http://127.0.0.1:9545".to_string()],
            sender: "0x375249129507aec9309abc1f5c055494200c7c32".to_string(),
            to: "0x1111111111111111111111111111111111111111".to_string(),
            sender_count: 1,
            workers: 1,
            tx_count: 2_000,
            gas_limit: 21_000,
            max_priority_fee_per_gas: 1_000_000_000,
            max_fee_per_gas: 2_000_000_000,
            value_wei: 0,
            fund_wei_hex: "0x3635c9adc5dea0000000".to_string(),
            poll_ms: 25,
            deadline_seconds: 120,
            output_json: PathBuf::from(format!("{base}.json")),
            output_md: PathBuf::from(format!("{base}.md")),
        }
    }
}

#[derive(Debug, Clone)]
struct SubmittedTx {
    submit_s: f64,
    endpoint_idx: usize,
}

#[derive(Debug, Serialize)]
struct LiveBenchReport {
    benchmark_kind: &'static str,
    rpc_urls: Vec<String>,
    sender: String,
    to: String,
    chain_id: u64,
    generated_at_unix_s: u64,
    sender_hint_mode: bool,
    sender_count: usize,
    worker_count: usize,
    tx_count_target: usize,
    tx_submitted: usize,
    tx_confirmed: usize,
    submit_duration_s: f64,
    submit_tps: f64,
    confirm_window_s: f64,
    confirmed_tps: f64,
    p50_confirmation_ms: f64,
    p95_confirmation_ms: f64,
    p99_confirmation_ms: f64,
    avg_confirmation_ms: f64,
    start_block: u64,
    end_block: u64,
    blocks_spanned: u64,
    chain_txs_in_spanned_blocks: u64,
    sender_submitted_counts: HashMap<String, usize>,
    endpoint_submitted_counts: HashMap<String, usize>,
    send_errors_sample: Vec<String>,
    notes: Vec<String>,
}

fn now_unix_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn parse_address(addr: &str) -> Result<[u8; 20], String> {
    let raw = addr.trim().strip_prefix("0x").unwrap_or(addr.trim());
    if raw.len() != 40 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("invalid address: {addr}"));
    }
    let bytes = hex::decode(raw).map_err(|e| format!("invalid address hex: {e}"))?;
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn canonical_address(addr: &str) -> Result<String, String> {
    let bytes = parse_address(addr)?;
    Ok(format!("0x{}", hex::encode(bytes)))
}

fn address_to_hex(addr: [u8; 20]) -> String {
    format!("0x{}", hex::encode(addr))
}

fn add_u64_to_address(addr: &mut [u8; 20], mut n: u64) {
    let mut carry = 0u16;
    for i in 0..8 {
        let idx = 19 - i;
        let add = (n & 0xff) as u16;
        n >>= 8;
        let sum = addr[idx] as u16 + add + carry;
        addr[idx] = (sum & 0xff) as u8;
        carry = sum >> 8;
    }
    let mut idx = 11usize;
    while carry > 0 && idx < 20 {
        let sum = addr[idx] as u16 + carry;
        addr[idx] = (sum & 0xff) as u8;
        carry = sum >> 8;
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
}

fn derive_senders(base: &str, count: usize) -> Result<Vec<String>, String> {
    let canonical = canonical_address(base)?;
    let base_addr = parse_address(&canonical)?;
    let mut out = Vec::with_capacity(count.max(1));
    for i in 0..count.max(1) {
        let mut addr = base_addr;
        add_u64_to_address(&mut addr, i as u64);
        out.push(address_to_hex(addr));
    }
    Ok(out)
}

fn parse_hex_u64(input: &str) -> Result<u64, String> {
    let s = input.trim().strip_prefix("0x").unwrap_or(input.trim());
    u64::from_str_radix(s, 16).map_err(|e| format!("invalid hex u64 {input}: {e}"))
}

fn percentile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let idx = ((sorted.len().saturating_sub(1) as f64) * q).round() as usize;
    sorted[idx.min(sorted.len().saturating_sub(1))]
}

fn avg(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn hex_u64(v: u64) -> String {
    format!("0x{v:x}")
}

fn typed_tx_hex(tx_type: u8, payload: Vec<u8>) -> String {
    let mut bytes = Vec::with_capacity(1 + payload.len());
    bytes.push(tx_type);
    bytes.extend_from_slice(&payload);
    format!("0x{}", hex::encode(bytes))
}

fn build_type2_raw_tx(
    chain_id: u64,
    nonce: u64,
    to: [u8; 20],
    value_wei: u64,
    gas_limit: u64,
    max_priority_fee_per_gas: u64,
    max_fee_per_gas: u64,
) -> String {
    let mut s = RlpStream::new_list(8);
    s.append(&chain_id);
    s.append(&nonce);
    s.append(&max_priority_fee_per_gas);
    s.append(&max_fee_per_gas);
    s.append(&gas_limit);
    s.append(&to.to_vec());
    s.append(&value_wei);
    s.append(&Vec::<u8>::new());
    typed_tx_hex(0x02, s.out().to_vec())
}

fn rpc_call(
    client: &Client,
    rpc_url: &str,
    req_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    *req_id = req_id.saturating_add(1);
    let request = json!({
        "jsonrpc": "2.0",
        "id": *req_id,
        "method": method,
        "params": params
    });

    let response = client
        .post(rpc_url)
        .json(&request)
        .send()
        .map_err(|e| format!("http error for {method}: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("failed reading response body for {method}: {e}"))?;
    if !status.is_success() {
        return Err(format!("http status {status} for {method}: {body}"));
    }

    let parsed: Value = serde_json::from_str(&body)
        .map_err(|e| format!("invalid json-rpc response for {method}: {e}; body={body}"))?;
    if let Some(err) = parsed.get("error") {
        return Err(format!("json-rpc error for {method}: {err}"));
    }
    parsed
        .get("result")
        .cloned()
        .ok_or_else(|| format!("missing result for {method}: {parsed}"))
}

fn ensure_parent(path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn parse_csv_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default_with_timestamp(now_unix_s());
    let mut it = env::args().skip(1);
    while let Some(flag) = it.next() {
        let val = it
            .next()
            .ok_or_else(|| format!("missing value for argument: {flag}"))?;
        match flag.as_str() {
            "--rpc-url" => args.rpc_urls = vec![val],
            "--rpc-urls" => {
                let list = parse_csv_list(&val);
                if list.is_empty() {
                    return Err("--rpc-urls must include at least one endpoint".to_string());
                }
                args.rpc_urls = list;
            }
            "--sender" => args.sender = val,
            "--to" => args.to = val,
            "--sender-count" => {
                args.sender_count = val
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --sender-count: {e}"))?
                    .max(1)
            }
            "--workers" => {
                args.workers = val
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --workers: {e}"))?
                    .max(1)
            }
            "--tx-count" => {
                args.tx_count = val
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --tx-count: {e}"))?
            }
            "--gas-limit" => {
                args.gas_limit = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --gas-limit: {e}"))?
            }
            "--max-priority-fee-per-gas" => {
                args.max_priority_fee_per_gas = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --max-priority-fee-per-gas: {e}"))?
            }
            "--max-fee-per-gas" => {
                args.max_fee_per_gas = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --max-fee-per-gas: {e}"))?
            }
            "--value-wei" => {
                args.value_wei = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --value-wei: {e}"))?
            }
            "--fund-wei-hex" => args.fund_wei_hex = val,
            "--poll-ms" => {
                args.poll_ms = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --poll-ms: {e}"))?
            }
            "--deadline-seconds" => {
                args.deadline_seconds = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --deadline-seconds: {e}"))?
            }
            "--output-json" => args.output_json = PathBuf::from(val),
            "--output-md" => args.output_md = PathBuf::from(val),
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }

    if args.rpc_urls.is_empty() {
        return Err("must provide at least one RPC endpoint".to_string());
    }

    Ok(args)
}

fn write_markdown(report: &LiveBenchReport, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut md = String::new();
    md.push_str("# ETH2077 Live TPS Benchmark\n\n");
    md.push_str(
        "This report is from live JSON-RPC transaction submission and receipt confirmation.\n\n",
    );
    md.push_str("## Summary\n\n");
    md.push_str(&format!("- rpc_urls: `{}`\n", report.rpc_urls.join(",")));
    md.push_str(&format!("- chain_id: `{}`\n", report.chain_id));
    md.push_str(&format!("- sender_count: `{}`\n", report.sender_count));
    md.push_str(&format!("- worker_count: `{}`\n", report.worker_count));
    md.push_str(&format!("- tx target: `{}`\n", report.tx_count_target));
    md.push_str(&format!("- tx submitted: `{}`\n", report.tx_submitted));
    md.push_str(&format!("- tx confirmed: `{}`\n", report.tx_confirmed));
    md.push_str(&format!("- submit TPS: `{:.2}`\n", report.submit_tps));
    md.push_str(&format!("- confirmed TPS: `{:.2}`\n", report.confirmed_tps));
    md.push_str(&format!(
        "- confirmation p50/p95/p99 (ms): `{:.2} / {:.2} / {:.2}`\n",
        report.p50_confirmation_ms, report.p95_confirmation_ms, report.p99_confirmation_ms
    ));
    md.push_str(&format!(
        "- avg confirmation latency (ms): `{:.2}`\n",
        report.avg_confirmation_ms
    ));
    md.push_str(&format!(
        "- blocks spanned: `{}` ({} -> {})\n",
        report.blocks_spanned, report.start_block, report.end_block
    ));
    md.push_str(&format!(
        "- chain txs in spanned blocks: `{}`\n",
        report.chain_txs_in_spanned_blocks
    ));

    md.push_str("\n## Per Sender\n\n");
    for (sender, count) in &report.sender_submitted_counts {
        md.push_str(&format!("- `{sender}`: `{count}` submitted\n"));
    }

    md.push_str("\n## Per Endpoint\n\n");
    for (endpoint, count) in &report.endpoint_submitted_counts {
        md.push_str(&format!("- `{endpoint}`: `{count}` submitted\n"));
    }

    md.push_str("\n## Notes\n\n");
    if report.notes.is_empty() {
        md.push_str("- none\n");
    } else {
        for note in &report.notes {
            md.push_str(&format!("- {note}\n"));
        }
    }

    if !report.send_errors_sample.is_empty() {
        md.push_str("\n## Send Error Sample\n\n");
        for err in &report.send_errors_sample {
            md.push_str(&format!("- `{}`\n", err));
        }
    }

    ensure_parent(path)?;
    fs::write(path, md)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args().map_err(|e| format!("argument error: {e}"))?;
    let sender = canonical_address(&args.sender).map_err(|e| format!("invalid sender: {e}"))?;
    let to = canonical_address(&args.to).map_err(|e| format!("invalid to: {e}"))?;
    let to_bytes = parse_address(&to).map_err(|e| format!("invalid to: {e}"))?;
    let senders = derive_senders(&sender, args.sender_count)?;

    let mut notes = Vec::new();
    if args.workers > senders.len() {
        notes.push(format!(
            "workers={} reduced to sender_count={} to avoid sender nonce contention",
            args.workers,
            senders.len()
        ));
    }
    let worker_count = args.workers.max(1).min(senders.len().max(1));

    let primary_rpc = args
        .rpc_urls
        .first()
        .ok_or_else(|| "missing primary rpc url".to_string())?
        .clone();

    let primary_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("failed building http client: {e}"))?;
    let mut req_id = 0u64;

    let chain_id_hex = rpc_call(
        &primary_client,
        &primary_rpc,
        &mut req_id,
        "eth_chainId",
        Value::Array(vec![]),
    )
    .map_err(|e| format!("failed eth_chainId: {e}"))?;
    let chain_id = chain_id_hex
        .as_str()
        .ok_or_else(|| "eth_chainId result is not string".to_string())
        .and_then(parse_hex_u64)
        .map_err(|e| format!("failed parsing chain id: {e}"))?;

    let start_block_hex = rpc_call(
        &primary_client,
        &primary_rpc,
        &mut req_id,
        "eth_blockNumber",
        Value::Array(vec![]),
    )
    .map_err(|e| format!("failed eth_blockNumber before run: {e}"))?;
    let start_block = start_block_hex
        .as_str()
        .ok_or_else(|| "eth_blockNumber result is not string".to_string())
        .and_then(parse_hex_u64)
        .map_err(|e| format!("failed parsing start block: {e}"))?;

    for (i, sender_addr) in senders.iter().enumerate() {
        let endpoint = &args.rpc_urls[i % args.rpc_urls.len()];
        let mut fund_req_id = 100_000 + i as u64;
        match rpc_call(
            &primary_client,
            endpoint,
            &mut fund_req_id,
            "eth2077_setBalance",
            json!([sender_addr, args.fund_wei_hex]),
        ) {
            Ok(Value::Bool(true)) => {}
            Ok(v) => notes.push(format!(
                "eth2077_setBalance non-true for sender {sender_addr} on {endpoint}: {v}"
            )),
            Err(e) => notes.push(format!(
                "eth2077_setBalance failed for sender {sender_addr} on {endpoint}: {e}"
            )),
        }
    }

    let benchmark_start = Instant::now();
    let next_job = Arc::new(AtomicUsize::new(0));
    let submitted = Arc::new(Mutex::new(HashMap::<String, SubmittedTx>::new()));
    let send_errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let sender_counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let endpoint_counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let endpoint_locks: Arc<Vec<Mutex<()>>> =
        Arc::new((0..args.rpc_urls.len()).map(|_| Mutex::new(())).collect());

    let mut handles = Vec::with_capacity(worker_count);
    for worker_idx in 0..worker_count {
        let rpc_urls = args.rpc_urls.clone();
        let senders_local = senders.clone();
        let next_job = Arc::clone(&next_job);
        let submitted = Arc::clone(&submitted);
        let send_errors = Arc::clone(&send_errors);
        let sender_counts = Arc::clone(&sender_counts);
        let endpoint_counts = Arc::clone(&endpoint_counts);
        let endpoint_locks = Arc::clone(&endpoint_locks);
        let gas_limit = args.gas_limit;
        let max_priority_fee_per_gas = args.max_priority_fee_per_gas;
        let max_fee_per_gas = args.max_fee_per_gas;
        let value_wei = args.value_wei;
        let tx_count = args.tx_count;

        let handle = thread::spawn(move || {
            let client = match Client::builder().timeout(Duration::from_secs(30)).build() {
                Ok(c) => c,
                Err(e) => {
                    let mut errors = send_errors.lock().expect("send_errors lock poisoned");
                    if errors.len() < 20 {
                        errors.push(format!("worker {worker_idx}: failed to build client: {e}"));
                    }
                    return;
                }
            };

            let sender_idx = worker_idx % senders_local.len();
            let sender_addr = senders_local[sender_idx].clone();
            let endpoint_idx = sender_idx % rpc_urls.len();
            let endpoint = rpc_urls[endpoint_idx].clone();
            let mut req_id = 1_000_000_u64 + worker_idx as u64 * 100_000;

            loop {
                let job = next_job.fetch_add(1, Ordering::Relaxed);
                if job >= tx_count {
                    break;
                }

                let _guard = endpoint_locks[endpoint_idx]
                    .lock()
                    .expect("endpoint lock poisoned");

                let nonce_hex = match rpc_call(
                    &client,
                    &endpoint,
                    &mut req_id,
                    "eth_getTransactionCount",
                    json!([sender_addr, "latest"]),
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        let mut errors = send_errors.lock().expect("send_errors lock poisoned");
                        if errors.len() < 20 {
                            errors.push(format!(
                                "worker {worker_idx} nonce lookup failed ({endpoint}, {sender_addr}): {e}"
                            ));
                        }
                        continue;
                    }
                };

                let nonce = match nonce_hex.as_str().and_then(|s| parse_hex_u64(s).ok()) {
                    Some(v) => v,
                    None => {
                        let mut errors = send_errors.lock().expect("send_errors lock poisoned");
                        if errors.len() < 20 {
                            errors.push(format!(
                                "worker {worker_idx} invalid nonce ({endpoint}, {sender_addr}): {nonce_hex}"
                            ));
                        }
                        continue;
                    }
                };

                let raw_tx = build_type2_raw_tx(
                    chain_id,
                    nonce,
                    to_bytes,
                    value_wei.saturating_add(job as u64),
                    gas_limit,
                    max_priority_fee_per_gas,
                    max_fee_per_gas,
                );
                let tx_hash = match rpc_call(
                    &client,
                    &endpoint,
                    &mut req_id,
                    "eth_sendRawTransaction",
                    json!([raw_tx]),
                ) {
                    Ok(v) => match v.as_str() {
                        Some(hash) => hash.to_string(),
                        None => {
                            let mut errors = send_errors.lock().expect("send_errors lock poisoned");
                            if errors.len() < 20 {
                                errors.push(format!(
                                    "worker {worker_idx} invalid send result ({endpoint}, {sender_addr}): {v}"
                                ));
                            }
                            continue;
                        }
                    },
                    Err(e) => {
                        let mut errors = send_errors.lock().expect("send_errors lock poisoned");
                        if errors.len() < 20 {
                            errors.push(format!(
                                "worker {worker_idx} send failed ({endpoint}, {sender_addr}): {e}"
                            ));
                        }
                        continue;
                    }
                };

                let submit_s = benchmark_start.elapsed().as_secs_f64();
                {
                    let mut map = submitted.lock().expect("submitted lock poisoned");
                    map.insert(
                        tx_hash,
                        SubmittedTx {
                            submit_s,
                            endpoint_idx,
                        },
                    );
                }
                {
                    let mut map = sender_counts.lock().expect("sender_counts lock poisoned");
                    *map.entry(sender_addr.clone()).or_insert(0) += 1;
                }
                {
                    let mut map = endpoint_counts
                        .lock()
                        .expect("endpoint_counts lock poisoned");
                    *map.entry(endpoint.clone()).or_insert(0) += 1;
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    let submit_duration_s = benchmark_start.elapsed().as_secs_f64();
    let submitted_snapshot = submitted.lock().expect("submitted lock poisoned").clone();
    let tx_submitted = submitted_snapshot.len();
    let submit_tps = if submit_duration_s > 0.0 {
        tx_submitted as f64 / submit_duration_s
    } else {
        0.0
    };

    let mut pending: HashSet<String> = submitted_snapshot.keys().cloned().collect();
    let mut confirm_at_s: HashMap<String, f64> = HashMap::new();
    let poll_deadline = Instant::now() + Duration::from_secs(args.deadline_seconds);
    let poll_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("failed building poll client: {e}"))?;
    let mut poll_req_id = 9_000_000_u64;

    while !pending.is_empty() && Instant::now() < poll_deadline {
        let batch: Vec<String> = pending.iter().cloned().collect();
        for hash in batch {
            let endpoint_idx = submitted_snapshot
                .get(&hash)
                .map(|s| s.endpoint_idx)
                .unwrap_or(0)
                .min(args.rpc_urls.len().saturating_sub(1));
            let endpoint = &args.rpc_urls[endpoint_idx];
            let receipt = match rpc_call(
                &poll_client,
                endpoint,
                &mut poll_req_id,
                "eth_getTransactionReceipt",
                json!([hash]),
            ) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if receipt.is_null() {
                continue;
            }
            if receipt.get("blockNumber").is_none() || receipt["blockNumber"].is_null() {
                continue;
            }
            if let Some(hash_str) = receipt.get("transactionHash").and_then(Value::as_str) {
                confirm_at_s.insert(
                    hash_str.to_string(),
                    benchmark_start.elapsed().as_secs_f64(),
                );
                pending.remove(hash_str);
            }
        }

        if !pending.is_empty() {
            thread::sleep(Duration::from_millis(args.poll_ms));
        }
    }

    let mut latencies_ms = Vec::new();
    for (hash, submitted_tx) in &submitted_snapshot {
        if let Some(confirm_s) = confirm_at_s.get(hash) {
            latencies_ms.push((confirm_s - submitted_tx.submit_s) * 1000.0);
        }
    }
    let tx_confirmed = latencies_ms.len();

    let min_submit_s = submitted_snapshot
        .values()
        .map(|s| s.submit_s)
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);
    let max_confirm_s = confirm_at_s
        .values()
        .copied()
        .max_by(f64::total_cmp)
        .unwrap_or(min_submit_s);
    let confirm_window_s = (max_confirm_s - min_submit_s).max(0.0);
    let confirmed_tps = if confirm_window_s > 0.0 {
        tx_confirmed as f64 / confirm_window_s
    } else {
        0.0
    };

    if tx_confirmed < tx_submitted {
        notes.push(format!(
            "confirmation deadline reached with pending txs: {}",
            tx_submitted.saturating_sub(tx_confirmed)
        ));
    }

    let end_block_hex = rpc_call(
        &primary_client,
        &primary_rpc,
        &mut req_id,
        "eth_blockNumber",
        Value::Array(vec![]),
    )
    .map_err(|e| format!("failed eth_blockNumber after run: {e}"))?;
    let end_block = end_block_hex
        .as_str()
        .ok_or_else(|| "eth_blockNumber result is not string".to_string())
        .and_then(parse_hex_u64)
        .map_err(|e| format!("failed parsing end block: {e}"))?;
    let blocks_spanned = end_block.saturating_sub(start_block);

    let mut chain_txs_in_spanned_blocks = 0u64;
    if end_block > start_block {
        for block in (start_block + 1)..=end_block {
            let block_res = rpc_call(
                &primary_client,
                &primary_rpc,
                &mut req_id,
                "eth_getBlockByNumber",
                json!([hex_u64(block), false]),
            );
            if let Ok(v) = block_res {
                if let Some(txs) = v.get("transactions").and_then(Value::as_array) {
                    chain_txs_in_spanned_blocks =
                        chain_txs_in_spanned_blocks.saturating_add(txs.len() as u64);
                }
            }
        }
    }

    if args.rpc_urls.len() > 1 && chain_txs_in_spanned_blocks < tx_confirmed as u64 {
        notes.push(format!(
            "primary endpoint observed {} / {} confirmed txs over spanned blocks; this multi-endpoint run may be aggregating partially independent lanes",
            chain_txs_in_spanned_blocks, tx_confirmed
        ));
    }

    notes.push(
        "sender-hint mode is active: node currently requires eth_getTransactionCount(sender) before each eth_sendRawTransaction"
            .to_string(),
    );

    let mut sender_submitted_counts = sender_counts
        .lock()
        .expect("sender_counts lock poisoned")
        .clone();
    for sender_addr in &senders {
        sender_submitted_counts
            .entry(sender_addr.clone())
            .or_insert(0);
    }

    let mut endpoint_submitted_counts = endpoint_counts
        .lock()
        .expect("endpoint_counts lock poisoned")
        .clone();
    for endpoint in &args.rpc_urls {
        endpoint_submitted_counts
            .entry(endpoint.clone())
            .or_insert(0);
    }

    let send_errors_sample = send_errors
        .lock()
        .expect("send_errors lock poisoned")
        .clone();

    let report = LiveBenchReport {
        benchmark_kind: "live-rpc",
        rpc_urls: args.rpc_urls.clone(),
        sender,
        to,
        chain_id,
        generated_at_unix_s: now_unix_s(),
        sender_hint_mode: true,
        sender_count: senders.len(),
        worker_count,
        tx_count_target: args.tx_count,
        tx_submitted,
        tx_confirmed,
        submit_duration_s,
        submit_tps,
        confirm_window_s,
        confirmed_tps,
        p50_confirmation_ms: percentile(&latencies_ms, 0.50),
        p95_confirmation_ms: percentile(&latencies_ms, 0.95),
        p99_confirmation_ms: percentile(&latencies_ms, 0.99),
        avg_confirmation_ms: avg(&latencies_ms),
        start_block,
        end_block,
        blocks_spanned,
        chain_txs_in_spanned_blocks,
        sender_submitted_counts,
        endpoint_submitted_counts,
        send_errors_sample,
        notes,
    };

    ensure_parent(&args.output_json)?;
    ensure_parent(&args.output_md)?;
    fs::write(&args.output_json, serde_json::to_string_pretty(&report)?)?;
    write_markdown(&report, &args.output_md)?;

    println!(
        "live-bench => submitted={}, confirmed={}, confirmed_tps={:.2}, p50/p95/p99={:.2}/{:.2}/{:.2} ms",
        report.tx_submitted,
        report.tx_confirmed,
        report.confirmed_tps,
        report.p50_confirmation_ms,
        report.p95_confirmation_ms,
        report.p99_confirmation_ms
    );
    println!("json: {}", args.output_json.display());
    println!("md: {}", args.output_md.display());

    Ok(())
}
