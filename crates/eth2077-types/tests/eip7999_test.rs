use eth2077_types::eip7999::{
    compute_base_cost, compute_fee_allocation, update_base_fees, validate_multidim_transaction,
    DimensionParams, MultidimFeeError, MultidimTransaction, ResourceDimension,
};

fn sample_params() -> Vec<DimensionParams> {
    vec![
        DimensionParams {
            dimension: ResourceDimension::Computation,
            base_fee: 10,
            target_usage: 100,
            max_usage: 200,
            elasticity_multiplier: 2,
        },
        DimensionParams {
            dimension: ResourceDimension::Calldata,
            base_fee: 4,
            target_usage: 50,
            max_usage: 120,
            elasticity_multiplier: 2,
        },
        DimensionParams {
            dimension: ResourceDimension::BlobData,
            base_fee: 2,
            target_usage: 80,
            max_usage: 160,
            elasticity_multiplier: 2,
        },
        DimensionParams {
            dimension: ResourceDimension::Storage,
            base_fee: 6,
            target_usage: 40,
            max_usage: 100,
            elasticity_multiplier: 2,
        },
    ]
}

#[test]
fn basic_fee_computation() {
    let params = sample_params();
    let tx = MultidimTransaction {
        max_fee: 1_000,
        resource_usage: vec![
            (ResourceDimension::Computation, 3),
            (ResourceDimension::Calldata, 5),
        ],
        priority_fee: 10,
    };

    let total = compute_base_cost(&params, &tx).unwrap();
    assert_eq!(total, 50);
}

#[test]
fn insufficient_fee_detected() {
    let params = sample_params();
    let tx = MultidimTransaction {
        max_fee: 40,
        resource_usage: vec![
            (ResourceDimension::Computation, 3),
            (ResourceDimension::Calldata, 5),
        ],
        priority_fee: 10,
    };

    let result = compute_fee_allocation(&params, &tx);
    assert_eq!(
        result,
        Err(MultidimFeeError::InsufficientFee {
            required: 50,
            provided: 40,
        })
    );
}

#[test]
fn exceeds_max_usage_detected() {
    let params = sample_params();
    let tx = MultidimTransaction {
        max_fee: 10_000,
        resource_usage: vec![(ResourceDimension::Computation, 300)],
        priority_fee: 0,
    };

    let result = compute_base_cost(&params, &tx);
    assert_eq!(
        result,
        Err(MultidimFeeError::ExceedsMaxUsage {
            dimension: ResourceDimension::Computation,
            usage: 300,
            max: 200,
        })
    );
}

#[test]
fn duplicate_dimension_detected() {
    let params = sample_params();
    let tx = MultidimTransaction {
        max_fee: 10_000,
        resource_usage: vec![
            (ResourceDimension::Computation, 3),
            (ResourceDimension::Computation, 2),
        ],
        priority_fee: 0,
    };

    let result = compute_base_cost(&params, &tx);
    assert_eq!(
        result,
        Err(MultidimFeeError::DuplicateDimension {
            dimension: ResourceDimension::Computation,
        })
    );
}

#[test]
fn fee_allocation_distributes_correctly() {
    let params = sample_params();
    let tx = MultidimTransaction {
        max_fee: 150,
        resource_usage: vec![
            (ResourceDimension::Computation, 3), // 30 base
            (ResourceDimension::Storage, 10),    // 60 base
        ],
        priority_fee: 60,
    };

    let result = compute_fee_allocation(&params, &tx).unwrap();
    assert_eq!(result.total_base_cost, 90);
    assert_eq!(result.priority_fee_paid, 60);
    assert_eq!(result.total_fee, 150);
    assert!(result.is_sufficient);
    assert_eq!(result.allocations.len(), 2);
    assert_eq!(result.allocations[0].allocated_fee, 50);
    assert_eq!(result.allocations[1].allocated_fee, 100);
}

#[test]
fn base_fee_update_increases_on_over_target() {
    let mut params = sample_params();
    update_base_fees(&mut params, &[(ResourceDimension::Computation, 150)]);

    let computation = params
        .iter()
        .find(|p| p.dimension == ResourceDimension::Computation)
        .unwrap();
    assert_eq!(computation.base_fee, 11);
}

#[test]
fn base_fee_update_decreases_on_under_target() {
    let mut params = sample_params();
    update_base_fees(&mut params, &[(ResourceDimension::Computation, 10)]);

    let computation = params
        .iter()
        .find(|p| p.dimension == ResourceDimension::Computation)
        .unwrap();
    assert_eq!(computation.base_fee, 9);
}

#[test]
fn multi_error_validation_collects_all_errors() {
    let params = sample_params();
    let tx = MultidimTransaction {
        max_fee: 1,
        resource_usage: vec![
            (ResourceDimension::Computation, 300), // exceeds
            (ResourceDimension::Computation, 5),   // duplicate
            (ResourceDimension::Calldata, 1),
        ],
        priority_fee: 0,
    };

    let errors = validate_multidim_transaction(&params, &tx).unwrap_err();
    assert!(errors.contains(&MultidimFeeError::DuplicateDimension {
        dimension: ResourceDimension::Computation
    }));
    assert!(errors.contains(&MultidimFeeError::ExceedsMaxUsage {
        dimension: ResourceDimension::Computation,
        usage: 300,
        max: 200,
    }));
    assert!(errors.contains(&MultidimFeeError::InsufficientFee {
        required: 3054,
        provided: 1,
    }));
}
