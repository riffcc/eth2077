use eth2077_bridge::engine_api::{
    spawn_engine_api_server, Bytes32, EngineApiService, ExecutionPayloadV3, ForkchoiceStateV1,
    PayloadAttributesV3, PayloadId,
};
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use serde_json::Value;

async fn start_server() -> (String, jsonrpsee::server::ServerHandle) {
    let running = spawn_engine_api_server(
        "127.0.0.1:0".parse().expect("valid bind address"),
        EngineApiService,
    )
    .await
    .expect("server should start");

    (format!("http://{}", running.local_addr), running.handle)
}

#[tokio::test]
async fn engine_new_payload_v3_returns_payload_status_shape() {
    let (url, handle) = start_server().await;
    let client = HttpClientBuilder::default()
        .build(url)
        .expect("http client should build");

    let result: Value = client
        .request(
            "engine_newPayloadV3",
            rpc_params![
                ExecutionPayloadV3::default(),
                vec![Bytes32::zero()],
                Bytes32::zero()
            ],
        )
        .await
        .expect("rpc should succeed");

    assert_eq!(result.get("status").and_then(Value::as_str), Some("VALID"));
    assert!(result.get("latestValidHash").is_some());
    assert!(result.get("validationError").is_some());

    handle.stop().expect("server stop should succeed");
}

#[tokio::test]
async fn engine_forkchoice_updated_v3_returns_expected_shape() {
    let (url, handle) = start_server().await;
    let client = HttpClientBuilder::default()
        .build(url)
        .expect("http client should build");

    let result: Value = client
        .request(
            "engine_forkchoiceUpdatedV3",
            rpc_params![
                ForkchoiceStateV1::default(),
                Some(PayloadAttributesV3::default())
            ],
        )
        .await
        .expect("rpc should succeed");

    let payload_status = result
        .get("payloadStatus")
        .expect("payloadStatus should be present");
    assert_eq!(
        payload_status.get("status").and_then(Value::as_str),
        Some("VALID")
    );
    assert!(payload_status.get("latestValidHash").is_some());
    assert!(payload_status.get("validationError").is_some());
    assert_eq!(
        result.get("payloadId").and_then(Value::as_str),
        Some("0x0000000000000000")
    );

    handle.stop().expect("server stop should succeed");
}

#[tokio::test]
async fn engine_get_payload_v3_returns_execution_payload_envelope_shape() {
    let (url, handle) = start_server().await;
    let client = HttpClientBuilder::default()
        .build(url)
        .expect("http client should build");

    let result: Value = client
        .request("engine_getPayloadV3", rpc_params![PayloadId::zero()])
        .await
        .expect("rpc should succeed");

    let execution_payload = result
        .get("executionPayload")
        .expect("executionPayload should be present");
    assert!(execution_payload.get("parentHash").is_some());
    assert!(execution_payload.get("feeRecipient").is_some());
    assert!(execution_payload.get("withdrawals").is_some());
    assert!(execution_payload.get("blobGasUsed").is_some());
    assert!(execution_payload.get("excessBlobGas").is_some());
    assert!(result.get("blockValue").is_some());
    assert!(result.get("blobsBundle").is_some());
    assert!(result.get("shouldOverrideBuilder").is_some());

    handle.stop().expect("server stop should succeed");
}
