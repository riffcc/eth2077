use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResourceDimension {
    Computation,
    Calldata,
    BlobData,
    Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionParams {
    pub dimension: ResourceDimension,
    pub base_fee: u128,
    pub target_usage: u64,
    pub max_usage: u64,
    pub elasticity_multiplier: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultidimTransaction {
    pub max_fee: u128,
    pub resource_usage: Vec<(ResourceDimension, u64)>,
    pub priority_fee: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeAllocation {
    pub dimension: ResourceDimension,
    pub base_cost: u128,
    pub allocated_fee: u128,
    pub usage: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultidimFeeResult {
    pub total_base_cost: u128,
    pub total_fee: u128,
    pub priority_fee_paid: u128,
    pub allocations: Vec<FeeAllocation>,
    pub is_sufficient: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MultidimFeeError {
    InsufficientFee { required: u128, provided: u128 },
    ExceedsMaxUsage {
        dimension: ResourceDimension,
        usage: u64,
        max: u64,
    },
    ZeroBaseFee { dimension: ResourceDimension },
    DuplicateDimension { dimension: ResourceDimension },
    NoResourceUsage,
}

fn find_params(
    params: &[DimensionParams],
    dimension: ResourceDimension,
) -> Option<&DimensionParams> {
    params.iter().find(|candidate| candidate.dimension == dimension)
}

pub fn compute_base_cost(
    params: &[DimensionParams],
    tx: &MultidimTransaction,
) -> Result<u128, MultidimFeeError> {
    if tx.resource_usage.is_empty() {
        return Err(MultidimFeeError::NoResourceUsage);
    }

    let mut seen = HashSet::new();
    let mut total = 0u128;

    for (dimension, usage) in &tx.resource_usage {
        if !seen.insert(*dimension) {
            return Err(MultidimFeeError::DuplicateDimension {
                dimension: *dimension,
            });
        }

        let Some(dimension_params) = find_params(params, *dimension) else {
            return Err(MultidimFeeError::ZeroBaseFee {
                dimension: *dimension,
            });
        };

        if dimension_params.base_fee == 0 {
            return Err(MultidimFeeError::ZeroBaseFee {
                dimension: *dimension,
            });
        }

        if *usage > dimension_params.max_usage {
            return Err(MultidimFeeError::ExceedsMaxUsage {
                dimension: *dimension,
                usage: *usage,
                max: dimension_params.max_usage,
            });
        }

        let base_cost = dimension_params.base_fee.saturating_mul(u128::from(*usage));
        total = total.saturating_add(base_cost);
    }

    Ok(total)
}

pub fn compute_fee_allocation(
    params: &[DimensionParams],
    tx: &MultidimTransaction,
) -> Result<MultidimFeeResult, MultidimFeeError> {
    let total_base_cost = compute_base_cost(params, tx)?;
    let is_sufficient = tx.max_fee >= total_base_cost;

    if !is_sufficient {
        return Err(MultidimFeeError::InsufficientFee {
            required: total_base_cost,
            provided: tx.max_fee,
        });
    }

    let remaining = tx.max_fee.saturating_sub(total_base_cost);
    let priority_fee_paid = tx.priority_fee.min(remaining);
    let mut allocations = Vec::with_capacity(tx.resource_usage.len());

    for (dimension, usage) in &tx.resource_usage {
        let dimension_params = find_params(params, *dimension).expect("validated in base cost");
        let base_cost = dimension_params.base_fee.saturating_mul(u128::from(*usage));
        allocations.push(FeeAllocation {
            dimension: *dimension,
            base_cost,
            allocated_fee: base_cost,
            usage: *usage,
        });
    }

    if priority_fee_paid > 0 && !allocations.is_empty() {
        if total_base_cost > 0 {
            let mut distributed = 0u128;
            for allocation in &mut allocations {
                let share = priority_fee_paid
                    .saturating_mul(allocation.base_cost)
                    .checked_div(total_base_cost)
                    .unwrap_or(0);
                allocation.allocated_fee = allocation.allocated_fee.saturating_add(share);
                distributed = distributed.saturating_add(share);
            }

            let remainder = priority_fee_paid.saturating_sub(distributed);
            allocations[0].allocated_fee = allocations[0].allocated_fee.saturating_add(remainder);
        } else {
            allocations[0].allocated_fee =
                allocations[0].allocated_fee.saturating_add(priority_fee_paid);
        }
    }

    Ok(MultidimFeeResult {
        total_base_cost,
        total_fee: total_base_cost.saturating_add(priority_fee_paid),
        priority_fee_paid,
        allocations,
        is_sufficient,
    })
}

pub fn update_base_fees(
    params: &mut [DimensionParams],
    actual_usage: &[(ResourceDimension, u64)],
) -> () {
    for dimension_params in params {
        let usage = actual_usage
            .iter()
            .find(|(dimension, _)| *dimension == dimension_params.dimension)
            .map(|(_, usage)| *usage)
            .unwrap_or(0);

        let adjustment = dimension_params.base_fee / 8;
        if usage > dimension_params.target_usage {
            dimension_params.base_fee = dimension_params.base_fee.saturating_add(adjustment);
        } else if usage < dimension_params.target_usage {
            dimension_params.base_fee = dimension_params.base_fee.saturating_sub(adjustment);
        }

        if dimension_params.base_fee == 0 {
            dimension_params.base_fee = 1;
        }
    }
}

pub fn validate_multidim_transaction(
    params: &[DimensionParams],
    tx: &MultidimTransaction,
) -> Result<(), Vec<MultidimFeeError>> {
    let mut errors = Vec::new();

    if tx.resource_usage.is_empty() {
        errors.push(MultidimFeeError::NoResourceUsage);
    }

    let mut seen = HashSet::new();
    let mut duplicate_reported = HashSet::new();
    let mut required = 0u128;

    for (dimension, usage) in &tx.resource_usage {
        if !seen.insert(*dimension) && duplicate_reported.insert(*dimension) {
            errors.push(MultidimFeeError::DuplicateDimension {
                dimension: *dimension,
            });
        }

        if let Some(dimension_params) = find_params(params, *dimension) {
            if *usage > dimension_params.max_usage {
                errors.push(MultidimFeeError::ExceedsMaxUsage {
                    dimension: *dimension,
                    usage: *usage,
                    max: dimension_params.max_usage,
                });
            }
            required =
                required.saturating_add(dimension_params.base_fee.saturating_mul(u128::from(*usage)));
        }
    }

    if required > tx.max_fee {
        errors.push(MultidimFeeError::InsufficientFee {
            required,
            provided: tx.max_fee,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
