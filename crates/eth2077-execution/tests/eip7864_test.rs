use eth2077_execution::eip7864::{
    estimate_gas_cost, validate_state_access_request, StateAccessError, StateAccessRequest,
    StateAccessType,
};

fn base_request() -> StateAccessRequest {
    StateAccessRequest {
        access_type: StateAccessType::StorageSlot,
        target_address: [0x11; 20],
        storage_slot: Some([0x22; 32]),
        block_number: 1_000,
        proof_data: vec![0xAA; 64],
    }
}

#[test]
fn valid_request_passes_validation() {
    let request = base_request();
    let result = validate_state_access_request(&request, 1_005, 128, 1024);
    assert_eq!(result, Ok(()));
}

#[test]
fn block_too_old_rejected() {
    let request = base_request();
    let result = validate_state_access_request(&request, 1_200, 100, 1024);
    assert_eq!(
        result,
        Err(StateAccessError::BlockTooOld {
            requested: 1_000,
            oldest_allowed: 1_100,
        })
    );
}

#[test]
fn block_in_future_rejected() {
    let mut request = base_request();
    request.block_number = 2_000;
    let result = validate_state_access_request(&request, 1_999, 100, 1024);
    assert_eq!(
        result,
        Err(StateAccessError::BlockInFuture {
            requested: 2_000,
            current: 1_999,
        })
    );
}

#[test]
fn storage_slot_required_for_storage_slot_type() {
    let mut request = base_request();
    request.storage_slot = None;
    let result = validate_state_access_request(&request, 1_005, 128, 1024);
    assert_eq!(result, Err(StateAccessError::StorageSlotRequiredForType));
}

#[test]
fn storage_slot_not_allowed_for_account_balance_type() {
    let mut request = base_request();
    request.access_type = StateAccessType::AccountBalance;
    let result = validate_state_access_request(&request, 1_005, 128, 1024);
    assert_eq!(result, Err(StateAccessError::StorageSlotNotAllowedForType));
}

#[test]
fn proof_too_large_rejected() {
    let mut request = base_request();
    request.proof_data = vec![0xBB; 128];
    let result = validate_state_access_request(&request, 1_005, 128, 64);
    assert_eq!(
        result,
        Err(StateAccessError::ProofTooLarge { size: 128, max: 64 })
    );
}

#[test]
fn gas_estimation_scales_with_proof_size() {
    let mut small = base_request();
    small.proof_data = vec![0xAA; 32];

    let mut large = base_request();
    large.proof_data = vec![0xAA; 256];

    let small_gas = estimate_gas_cost(&small);
    let large_gas = estimate_gas_cost(&large);

    assert!(large_gas > small_gas);
}
