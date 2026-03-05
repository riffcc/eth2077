use std::{error::Error, net::SocketAddr};

use jsonrpsee::{
    core::RpcResult,
    server::{ServerBuilder, ServerHandle},
    RpcModule,
};
use serde::{Deserialize, Serialize};

fn zero_hex(bytes: usize) -> String {
    format!("0x{}", "00".repeat(bytes))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HexBytes(pub String);

impl HexBytes {
    pub fn empty() -> Self {
        Self("0x".to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HexQuantity(pub String);

impl HexQuantity {
    pub fn zero() -> Self {
        Self("0x0".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bytes32(pub String);

impl Bytes32 {
    pub fn zero() -> Self {
        Self(zero_hex(32))
    }
}

impl Default for Bytes32 {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address(pub String);

impl Address {
    pub fn zero() -> Self {
        Self(zero_hex(20))
    }
}

impl Default for Address {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PayloadId(pub String);

impl PayloadId {
    pub fn zero() -> Self {
        Self(zero_hex(8))
    }
}

impl Default for PayloadId {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalV1 {
    pub index: HexQuantity,
    pub validator_index: HexQuantity,
    pub address: Address,
    pub amount: HexQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV3 {
    pub parent_hash: Bytes32,
    pub fee_recipient: Address,
    pub state_root: Bytes32,
    pub receipts_root: Bytes32,
    pub logs_bloom: HexBytes,
    pub prev_randao: Bytes32,
    pub block_number: HexQuantity,
    pub gas_limit: HexQuantity,
    pub gas_used: HexQuantity,
    pub timestamp: HexQuantity,
    pub extra_data: HexBytes,
    pub base_fee_per_gas: HexQuantity,
    pub block_hash: Bytes32,
    pub transactions: Vec<HexBytes>,
    pub withdrawals: Vec<WithdrawalV1>,
    pub blob_gas_used: HexQuantity,
    pub excess_blob_gas: HexQuantity,
}

impl Default for ExecutionPayloadV3 {
    fn default() -> Self {
        Self {
            parent_hash: Bytes32::zero(),
            fee_recipient: Address::zero(),
            state_root: Bytes32::zero(),
            receipts_root: Bytes32::zero(),
            logs_bloom: HexBytes(zero_hex(256)),
            prev_randao: Bytes32::zero(),
            block_number: HexQuantity::zero(),
            gas_limit: HexQuantity::zero(),
            gas_used: HexQuantity::zero(),
            timestamp: HexQuantity::zero(),
            extra_data: HexBytes::empty(),
            base_fee_per_gas: HexQuantity::zero(),
            block_hash: Bytes32::zero(),
            transactions: Vec::new(),
            withdrawals: Vec::new(),
            blob_gas_used: HexQuantity::zero(),
            excess_blob_gas: HexQuantity::zero(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceStateV1 {
    pub head_block_hash: Bytes32,
    pub safe_block_hash: Bytes32,
    pub finalized_block_hash: Bytes32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadAttributesV3 {
    pub timestamp: HexQuantity,
    pub prev_randao: Bytes32,
    pub suggested_fee_recipient: Address,
    pub withdrawals: Vec<WithdrawalV1>,
    pub parent_beacon_block_root: Bytes32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayloadExecutionStatus {
    #[default]
    Valid,
    Invalid,
    Syncing,
    Accepted,
    InvalidBlockHash,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStatusV1 {
    pub status: PayloadExecutionStatus,
    pub latest_valid_hash: Option<Bytes32>,
    pub validation_error: Option<String>,
}

impl PayloadStatusV1 {
    pub fn valid() -> Self {
        Self {
            status: PayloadExecutionStatus::Valid,
            latest_valid_hash: Some(Bytes32::zero()),
            validation_error: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceUpdatedV3Response {
    pub payload_status: PayloadStatusV1,
    pub payload_id: Option<PayloadId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobsBundleV1 {
    pub commitments: Vec<HexBytes>,
    pub proofs: Vec<HexBytes>,
    pub blobs: Vec<HexBytes>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPayloadV3Response {
    pub execution_payload: ExecutionPayloadV3,
    pub block_value: HexQuantity,
    pub blobs_bundle: BlobsBundleV1,
    pub should_override_builder: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPayloadV3Request {
    pub execution_payload: ExecutionPayloadV3,
    pub expected_blob_versioned_hashes: Vec<Bytes32>,
    pub parent_beacon_block_root: Bytes32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceUpdatedV3Request {
    pub forkchoice_state: ForkchoiceStateV1,
    pub payload_attributes: Option<PayloadAttributesV3>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPayloadV3Request {
    pub payload_id: PayloadId,
}

#[derive(Debug, Clone, Default)]
pub struct EngineApiService;

impl EngineApiService {
    pub fn new_payload_v3(&self, _request: NewPayloadV3Request) -> PayloadStatusV1 {
        PayloadStatusV1::valid()
    }

    pub fn forkchoice_updated_v3(
        &self,
        request: ForkchoiceUpdatedV3Request,
    ) -> ForkchoiceUpdatedV3Response {
        ForkchoiceUpdatedV3Response {
            payload_status: PayloadStatusV1::valid(),
            payload_id: request.payload_attributes.map(|_| PayloadId::zero()),
        }
    }

    pub fn get_payload_v3(&self, _request: GetPayloadV3Request) -> GetPayloadV3Response {
        GetPayloadV3Response {
            execution_payload: ExecutionPayloadV3::default(),
            block_value: HexQuantity::zero(),
            blobs_bundle: BlobsBundleV1::default(),
            should_override_builder: false,
        }
    }
}

pub fn build_engine_rpc_module(service: EngineApiService) -> RpcModule<EngineApiService> {
    let mut module = RpcModule::new(service);

    module
        .register_method(
            "engine_newPayloadV3",
            |params, ctx, _| -> RpcResult<PayloadStatusV1> {
                let (
                    execution_payload,
                    expected_blob_versioned_hashes,
                    parent_beacon_block_root,
                ): (ExecutionPayloadV3, Vec<Bytes32>, Bytes32) = params.parse()?;

                let request = NewPayloadV3Request {
                    execution_payload,
                    expected_blob_versioned_hashes,
                    parent_beacon_block_root,
                };

                Ok(ctx.new_payload_v3(request))
            },
        )
        .expect("method names are static and unique");

    module
        .register_method(
            "engine_forkchoiceUpdatedV3",
            |params, ctx, _| -> RpcResult<ForkchoiceUpdatedV3Response> {
                let (forkchoice_state, payload_attributes): (
                    ForkchoiceStateV1,
                    Option<PayloadAttributesV3>,
                ) = params.parse()?;

                let request = ForkchoiceUpdatedV3Request {
                    forkchoice_state,
                    payload_attributes,
                };

                Ok(ctx.forkchoice_updated_v3(request))
            },
        )
        .expect("method names are static and unique");

    module
        .register_method(
            "engine_getPayloadV3",
            |params, ctx, _| -> RpcResult<GetPayloadV3Response> {
                let payload_id: PayloadId = params.one()?;
                let request = GetPayloadV3Request { payload_id };
                Ok(ctx.get_payload_v3(request))
            },
        )
        .expect("method names are static and unique");

    module
}

#[derive(Debug)]
pub struct RunningEngineApiServer {
    pub local_addr: SocketAddr,
    pub handle: ServerHandle,
}

pub async fn spawn_engine_api_server(
    bind_addr: SocketAddr,
    service: EngineApiService,
) -> Result<RunningEngineApiServer, Box<dyn Error + Send + Sync>> {
    let server = ServerBuilder::default().build(bind_addr).await?;
    let local_addr = server.local_addr()?;
    let module = build_engine_rpc_module(service);
    let handle = server.start(module);
    Ok(RunningEngineApiServer { local_addr, handle })
}
