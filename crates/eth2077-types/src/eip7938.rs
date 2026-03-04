use serde::{Deserialize, Serialize};

pub const ACTIVATION_EPOCH: u64 = 369_017;
pub const INITIAL_GAS_LIMIT: u64 = 50_000_000;
pub const EPOCHS_PER_10X: u64 = 164_250;
pub const SLOTS_PER_EPOCH: u64 = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasLimitCurve {
    pub activation_epoch: u64,
    pub initial_gas_limit: u64,
    pub epochs_per_10x: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasLimitSnapshot {
    pub epoch: u64,
    pub slot: u64,
    pub target_gas_limit: u64,
    pub growth_factor: f64,
    pub epochs_since_activation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GasLimitError {
    EpochBeforeActivation { epoch: u64, activation: u64 },
    InvalidCurveParams,
    Overflow,
}

impl Default for GasLimitCurve {
    fn default() -> Self {
        Self {
            activation_epoch: ACTIVATION_EPOCH,
            initial_gas_limit: INITIAL_GAS_LIMIT,
            epochs_per_10x: EPOCHS_PER_10X,
        }
    }
}

fn validate_curve(curve: &GasLimitCurve) -> Result<(), GasLimitError> {
    if curve.initial_gas_limit == 0 || curve.epochs_per_10x == 0 {
        return Err(GasLimitError::InvalidCurveParams);
    }
    Ok(())
}

fn growth_factor_for_epoch(curve: &GasLimitCurve, epoch: u64) -> Result<(u64, f64), GasLimitError> {
    validate_curve(curve)?;

    if epoch < curve.activation_epoch {
        return Err(GasLimitError::EpochBeforeActivation {
            epoch,
            activation: curve.activation_epoch,
        });
    }

    let epochs_since_activation = epoch - curve.activation_epoch;
    let exponent = epochs_since_activation as f64 / curve.epochs_per_10x as f64;
    let growth_factor = 10f64.powf(exponent);
    Ok((epochs_since_activation, growth_factor))
}

pub fn compute_target_gas_limit(curve: &GasLimitCurve, epoch: u64) -> Result<u64, GasLimitError> {
    let (_, growth_factor) = growth_factor_for_epoch(curve, epoch)?;
    let target = curve.initial_gas_limit as f64 * growth_factor;

    if !target.is_finite() || target >= u64::MAX as f64 {
        return Ok(u64::MAX);
    }

    Ok(target.round() as u64)
}

pub fn compute_gas_limit_snapshot(
    curve: &GasLimitCurve,
    epoch: u64,
) -> Result<GasLimitSnapshot, GasLimitError> {
    let (epochs_since_activation, growth_factor) = growth_factor_for_epoch(curve, epoch)?;
    let target_gas_limit = compute_target_gas_limit(curve, epoch)?;
    let slot = epoch
        .checked_mul(SLOTS_PER_EPOCH)
        .ok_or(GasLimitError::Overflow)?;

    Ok(GasLimitSnapshot {
        epoch,
        slot,
        target_gas_limit,
        growth_factor,
        epochs_since_activation,
    })
}

pub fn epoch_for_target_gas_limit(
    curve: &GasLimitCurve,
    target: u64,
) -> Result<u64, GasLimitError> {
    validate_curve(curve)?;

    if target <= curve.initial_gas_limit {
        return Ok(curve.activation_epoch);
    }

    let ratio = target as f64 / curve.initial_gas_limit as f64;
    if !ratio.is_finite() || ratio <= 0.0 {
        return Err(GasLimitError::InvalidCurveParams);
    }

    let epochs_since_activation_f64 = (ratio.log10() * curve.epochs_per_10x as f64).ceil();
    if !epochs_since_activation_f64.is_finite() || epochs_since_activation_f64 > u64::MAX as f64 {
        return Err(GasLimitError::Overflow);
    }

    let epochs_since_activation = epochs_since_activation_f64 as u64;
    curve
        .activation_epoch
        .checked_add(epochs_since_activation)
        .ok_or(GasLimitError::Overflow)
}

pub fn validate_proposed_gas_limit(
    curve: &GasLimitCurve,
    epoch: u64,
    proposed: u64,
    tolerance_pct: f64,
) -> bool {
    if tolerance_pct.is_nan() || tolerance_pct < 0.0 {
        return false;
    }

    let target = match compute_target_gas_limit(curve, epoch) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let target_f64 = target as f64;
    let lower_bound = target_f64 * (1.0 - tolerance_pct);
    let upper_bound = target_f64 * (1.0 + tolerance_pct);
    let proposed_f64 = proposed as f64;

    proposed_f64 >= lower_bound && proposed_f64 <= upper_bound
}
