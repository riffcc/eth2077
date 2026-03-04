use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobFeeWindow {
    pub slot_gas_used: Vec<u64>,
    pub window_size: usize,
    pub target_gas_per_slot: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobFeeParams {
    pub min_base_fee: u64,
    pub max_base_fee: u64,
    /// EMA alpha in the range [0.0, 1.0].
    pub smoothing_factor: f64,
    pub adjustment_quotient: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlobFeeError {
    WindowEmpty,
    InvalidSmoothingFactor { value: f64 },
    TargetGasZero,
    MinExceedsMax { min: u64, max: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdjustmentDirection {
    Up,
    Down,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobFeeSnapshot {
    pub slot: u64,
    pub computed_fee: u64,
    pub utilization_ratio: f64,
    pub adjustment_direction: AdjustmentDirection,
}

pub fn validate_fee_params(params: &BlobFeeParams) -> Result<(), BlobFeeError> {
    if !(0.0..=1.0).contains(&params.smoothing_factor) {
        return Err(BlobFeeError::InvalidSmoothingFactor {
            value: params.smoothing_factor,
        });
    }

    if params.min_base_fee > params.max_base_fee {
        return Err(BlobFeeError::MinExceedsMax {
            min: params.min_base_fee,
            max: params.max_base_fee,
        });
    }

    Ok(())
}

pub fn compute_utilization_ratio(window: &BlobFeeWindow) -> Result<f64, BlobFeeError> {
    if window.window_size == 0 || window.slot_gas_used.is_empty() {
        return Err(BlobFeeError::WindowEmpty);
    }

    if window.target_gas_per_slot == 0 {
        return Err(BlobFeeError::TargetGasZero);
    }

    let sample_count = window.slot_gas_used.len().min(window.window_size);
    if sample_count == 0 {
        return Err(BlobFeeError::WindowEmpty);
    }

    let start = window.slot_gas_used.len() - sample_count;
    let total_gas: u128 = window
        .slot_gas_used
        .iter()
        .skip(start)
        .map(|value| u128::from(*value))
        .sum();

    let avg_gas_used = total_gas as f64 / sample_count as f64;
    Ok(avg_gas_used / window.target_gas_per_slot as f64)
}

pub fn compute_smoothed_blob_base_fee(
    window: &BlobFeeWindow,
    params: &BlobFeeParams,
    current_base_fee: u64,
) -> Result<u64, BlobFeeError> {
    validate_fee_params(params)?;
    let utilization_ratio = compute_utilization_ratio(window)?;

    let clamped_current = current_base_fee.clamp(params.min_base_fee, params.max_base_fee);
    if params.adjustment_quotient == 0 {
        return Ok(clamped_current);
    }

    let deviation = utilization_ratio - 1.0;
    let adjustment = (clamped_current as f64 * deviation) / params.adjustment_quotient as f64;
    let smoothed_adjustment = params.smoothing_factor * adjustment;
    let next_fee = clamped_current as f64 + smoothed_adjustment;
    let bounded_fee = next_fee.clamp(params.min_base_fee as f64, params.max_base_fee as f64);

    Ok(bounded_fee.round() as u64)
}
