use eth2077_types::eip7938::{
    compute_gas_limit_snapshot, compute_target_gas_limit, epoch_for_target_gas_limit,
    validate_proposed_gas_limit, GasLimitCurve, GasLimitError, ACTIVATION_EPOCH, EPOCHS_PER_10X,
    INITIAL_GAS_LIMIT, SLOTS_PER_EPOCH,
};

#[test]
fn activation_epoch_returns_initial_gas_limit() {
    let curve = GasLimitCurve::default();
    let target = compute_target_gas_limit(&curve, ACTIVATION_EPOCH).unwrap();
    assert_eq!(target, INITIAL_GAS_LIMIT);
}

#[test]
fn one_year_of_epochs_shows_growth() {
    let curve = GasLimitCurve::default();
    let one_year_epochs = 365 * 225;
    let epoch = ACTIVATION_EPOCH + one_year_epochs;
    let target = compute_target_gas_limit(&curve, epoch).unwrap();
    assert!(target > INITIAL_GAS_LIMIT);
}

#[test]
fn ten_x_growth_at_epochs_per_10x() {
    let curve = GasLimitCurve::default();
    let epoch = ACTIVATION_EPOCH + EPOCHS_PER_10X;
    let target = compute_target_gas_limit(&curve, epoch).unwrap();
    assert_eq!(target, INITIAL_GAS_LIMIT * 10);
}

#[test]
fn epoch_before_activation_returns_error() {
    let curve = GasLimitCurve::default();
    let err = compute_target_gas_limit(&curve, ACTIVATION_EPOCH - 1).unwrap_err();
    assert_eq!(
        err,
        GasLimitError::EpochBeforeActivation {
            epoch: ACTIVATION_EPOCH - 1,
            activation: ACTIVATION_EPOCH,
        }
    );
}

#[test]
fn snapshot_fields_populated_correctly() {
    let curve = GasLimitCurve::default();
    let epoch = ACTIVATION_EPOCH + 100;
    let snapshot = compute_gas_limit_snapshot(&curve, epoch).unwrap();
    let target = compute_target_gas_limit(&curve, epoch).unwrap();

    assert_eq!(snapshot.epoch, epoch);
    assert_eq!(snapshot.slot, epoch * SLOTS_PER_EPOCH);
    assert_eq!(snapshot.epochs_since_activation, 100);
    assert_eq!(snapshot.target_gas_limit, target);
    assert!(snapshot.growth_factor > 1.0);
}

#[test]
fn inverse_function_roundtrips() {
    let curve = GasLimitCurve::default();
    let epoch = ACTIVATION_EPOCH + EPOCHS_PER_10X;
    let target = compute_target_gas_limit(&curve, epoch).unwrap();
    let derived_epoch = epoch_for_target_gas_limit(&curve, target).unwrap();
    assert_eq!(derived_epoch, epoch);
}

#[test]
fn proposed_gas_limit_within_tolerance_passes() {
    let curve = GasLimitCurve::default();
    let epoch = ACTIVATION_EPOCH + 50_000;
    let target = compute_target_gas_limit(&curve, epoch).unwrap();
    let proposed = target * 105 / 100;
    assert!(validate_proposed_gas_limit(&curve, epoch, proposed, 0.10));
}

#[test]
fn proposed_gas_limit_outside_tolerance_fails() {
    let curve = GasLimitCurve::default();
    let epoch = ACTIVATION_EPOCH + 50_000;
    let target = compute_target_gas_limit(&curve, epoch).unwrap();
    let proposed = target * 80 / 100;
    assert!(!validate_proposed_gas_limit(&curve, epoch, proposed, 0.10));
}
