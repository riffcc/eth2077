use crate::engine_api::{
    Address, Bytes32, ExecutionPayloadV3, HexBytes, HexQuantity, WithdrawalV1,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eth2077Block {
    pub parent_hash: [u8; 32],
    pub fee_recipient: [u8; 20],
    pub state_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub base_fee_per_gas: u128,
    pub transactions: Vec<Vec<u8>>,
    pub withdrawals: Vec<WithdrawalV1>,
}

impl Eth2077Block {
    pub fn to_execution_payload(&self) -> ExecutionPayloadV3 {
        ExecutionPayloadV3 {
            parent_hash: Bytes32(encode_hex_bytes(&self.parent_hash)),
            fee_recipient: Address(encode_hex_bytes(&self.fee_recipient)),
            state_root: Bytes32(encode_hex_bytes(&self.state_root)),
            receipts_root: Bytes32(encode_hex_bytes(&self.receipts_root)),
            logs_bloom: HexBytes(encode_hex_bytes(&[0u8; 256])),
            prev_randao: Bytes32(encode_hex_bytes(&[0u8; 32])),
            block_number: encode_hex_quantity_u64(self.block_number),
            gas_limit: encode_hex_quantity_u64(self.gas_limit),
            gas_used: encode_hex_quantity_u64(self.gas_used),
            timestamp: encode_hex_quantity_u64(self.timestamp),
            extra_data: HexBytes::empty(),
            base_fee_per_gas: encode_hex_quantity_u128(self.base_fee_per_gas),
            block_hash: Bytes32(encode_hex_bytes(&[0u8; 32])),
            transactions: self
                .transactions
                .iter()
                .map(|tx| HexBytes(encode_hex_bytes(tx)))
                .collect(),
            withdrawals: self.withdrawals.clone(),
            blob_gas_used: HexQuantity::zero(),
            excess_blob_gas: HexQuantity::zero(),
        }
    }
}

impl From<ExecutionPayloadV3> for Eth2077Block {
    fn from(value: ExecutionPayloadV3) -> Self {
        let ExecutionPayloadV3 {
            parent_hash,
            fee_recipient,
            state_root,
            receipts_root,
            block_number,
            gas_limit,
            gas_used,
            timestamp,
            base_fee_per_gas,
            transactions,
            withdrawals,
            ..
        } = value;

        Self {
            parent_hash: decode_fixed_hex_bytes(&parent_hash.0),
            fee_recipient: decode_fixed_hex_bytes(&fee_recipient.0),
            state_root: decode_fixed_hex_bytes(&state_root.0),
            receipts_root: decode_fixed_hex_bytes(&receipts_root.0),
            block_number: decode_hex_quantity_u64(&block_number),
            gas_limit: decode_hex_quantity_u64(&gas_limit),
            gas_used: decode_hex_quantity_u64(&gas_used),
            timestamp: decode_hex_quantity_u64(&timestamp),
            base_fee_per_gas: decode_hex_quantity_u128(&base_fee_per_gas),
            transactions: transactions
                .into_iter()
                .map(|tx| decode_hex_bytes(&tx.0).unwrap_or_default())
                .collect(),
            withdrawals,
        }
    }
}

fn encode_hex_bytes(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(2 + (bytes.len() * 2));
    out.push_str("0x");

    for byte in bytes {
        out.push(HEX_CHARS[(byte >> 4) as usize] as char);
        out.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }

    out
}

fn decode_hex_bytes(input: &str) -> Option<Vec<u8>> {
    let raw = input.trim();
    let raw = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);

    if raw.is_empty() {
        return Some(Vec::new());
    }

    if raw.len() % 2 != 0 {
        return None;
    }

    let mut out = Vec::with_capacity(raw.len() / 2);
    for pair in raw.as_bytes().chunks_exact(2) {
        let hi = decode_hex_nibble(pair[0])?;
        let lo = decode_hex_nibble(pair[1])?;
        out.push((hi << 4) | lo);
    }

    Some(out)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_fixed_hex_bytes<const N: usize>(input: &str) -> [u8; N] {
    let decoded = decode_hex_bytes(input).unwrap_or_default();
    let mut out = [0u8; N];

    if decoded.len() >= N {
        out.copy_from_slice(&decoded[decoded.len() - N..]);
        return out;
    }

    out[N - decoded.len()..].copy_from_slice(&decoded);
    out
}

fn encode_hex_quantity_u64(value: u64) -> HexQuantity {
    HexQuantity(format!("0x{value:x}"))
}

fn encode_hex_quantity_u128(value: u128) -> HexQuantity {
    HexQuantity(format!("0x{value:x}"))
}

fn decode_hex_quantity_u64(value: &HexQuantity) -> u64 {
    let raw = value.0.trim();
    let raw = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);

    if raw.is_empty() {
        return 0;
    }

    u64::from_str_radix(raw, 16).unwrap_or_default()
}

fn decode_hex_quantity_u128(value: &HexQuantity) -> u128 {
    let raw = value.0.trim();
    let raw = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);

    if raw.is_empty() {
        return 0;
    }

    u128::from_str_radix(raw, 16).unwrap_or_default()
}
