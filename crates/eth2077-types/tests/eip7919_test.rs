use eth2077_types::eip7919::{
    compute_smoothed_blob_base_fee, compute_utilization_ratio, validate_fee_params, BlobFeeError,
    BlobFeeParams, BlobFeeWindow,
};

fn sample_params() -> BlobFeeParams {
    BlobFeeParams {
        min_base_fee: 100,
        max_base_fee: 10_000,
        smoothing_factor: 0.5,
        adjustment_quotient: 8,
    }
}

#[test]
fn valid_params_pass_validation() {
    let params = sample_params();
    assert_eq!(validate_fee_params(&params), Ok(()));
}

#[test]
fn invalid_smoothing_factor_rejected() {
    let mut negative = sample_params();
    negative.smoothing_factor = -0.1;
    assert_eq!(
        validate_fee_params(&negative),
        Err(BlobFeeError::InvalidSmoothingFactor { value: -0.1 })
    );

    let mut too_high = sample_params();
    too_high.smoothing_factor = 1.1;
    assert_eq!(
        validate_fee_params(&too_high),
        Err(BlobFeeError::InvalidSmoothingFactor { value: 1.1 })
    );
}

#[test]
fn min_exceeds_max_rejected() {
    let mut params = sample_params();
    params.min_base_fee = 200;
    params.max_base_fee = 100;

    assert_eq!(
        validate_fee_params(&params),
        Err(BlobFeeError::MinExceedsMax { min: 200, max: 100 })
    );
}

#[test]
fn target_gas_zero_rejected() {
    let window = BlobFeeWindow {
        slot_gas_used: vec![100, 110, 90],
        window_size: 3,
        target_gas_per_slot: 0,
    };

    assert_eq!(
        compute_utilization_ratio(&window),
        Err(BlobFeeError::TargetGasZero)
    );
}

#[test]
fn smoothed_fee_with_underutilized_window_goes_down() {
    let params = sample_params();
    let window = BlobFeeWindow {
        slot_gas_used: vec![40, 50, 60, 50],
        window_size: 4,
        target_gas_per_slot: 100,
    };
    let current_fee = 1_000;

    let next_fee = compute_smoothed_blob_base_fee(&window, &params, current_fee).unwrap();
    assert!(next_fee < current_fee);
}

#[test]
fn smoothed_fee_with_overutilized_window_goes_up() {
    let params = sample_params();
    let window = BlobFeeWindow {
        slot_gas_used: vec![140, 150, 160, 150],
        window_size: 4,
        target_gas_per_slot: 100,
    };
    let current_fee = 1_000;

    let next_fee = compute_smoothed_blob_base_fee(&window, &params, current_fee).unwrap();
    assert!(next_fee > current_fee);
}

#[test]
fn utilization_ratio_computation() {
    let window = BlobFeeWindow {
        slot_gas_used: vec![50, 100, 150],
        window_size: 3,
        target_gas_per_slot: 100,
    };

    let ratio = compute_utilization_ratio(&window).unwrap();
    assert!((ratio - 1.0).abs() < 1e-12);
}

#[test]
fn fee_clamped_to_min_max_bounds() {
    let min_clamp_params = BlobFeeParams {
        min_base_fee: 900,
        max_base_fee: 1_100,
        smoothing_factor: 1.0,
        adjustment_quotient: 1,
    };
    let low_util_window = BlobFeeWindow {
        slot_gas_used: vec![0, 0, 0, 0],
        window_size: 4,
        target_gas_per_slot: 100,
    };
    let min_clamped =
        compute_smoothed_blob_base_fee(&low_util_window, &min_clamp_params, 1_000).unwrap();
    assert_eq!(min_clamped, 900);

    let high_util_window = BlobFeeWindow {
        slot_gas_used: vec![300, 300, 300, 300],
        window_size: 4,
        target_gas_per_slot: 100,
    };
    let max_clamped =
        compute_smoothed_blob_base_fee(&high_util_window, &min_clamp_params, 1_000).unwrap();
    assert_eq!(max_clamped, 1_100);
}
