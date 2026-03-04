use eth2077_execution::eip8007::{
    build_gas_table, compute_repricing_impact, glamsterdam_default_schedule, lookup_gas_cost,
    validate_repricing_schedule, GasRepricingError, GasRepricingRule, GasRepricingSchedule,
    OpcodeCategory,
};

#[test]
fn default_schedule_is_valid() {
    let schedule = glamsterdam_default_schedule();
    let result = validate_repricing_schedule(&schedule);
    assert_eq!(result, Ok(()));
}

#[test]
fn empty_schedule_rejected() {
    let schedule = GasRepricingSchedule {
        rules: Vec::new(),
        activation_block: 0,
        name: "empty".to_owned(),
    };

    let result = validate_repricing_schedule(&schedule);
    assert!(matches!(result, Err(ref errors) if errors.contains(&GasRepricingError::NoRules)));
}

#[test]
fn duplicate_opcodes_detected() {
    let schedule = GasRepricingSchedule {
        rules: vec![
            GasRepricingRule {
                opcode: 0x55,
                category: OpcodeCategory::Storage,
                old_gas: 20_000,
                new_gas: 5_000,
                rationale: "first",
            },
            GasRepricingRule {
                opcode: 0x55,
                category: OpcodeCategory::Storage,
                old_gas: 20_000,
                new_gas: 5_000,
                rationale: "duplicate",
            },
        ],
        activation_block: 0,
        name: "dupe".to_owned(),
    };

    let result = validate_repricing_schedule(&schedule);
    assert!(matches!(
        result,
        Err(ref errors)
            if errors.contains(&GasRepricingError::DuplicateOpcode { opcode: 0x55 })
    ));
}

#[test]
fn zero_gas_rejected() {
    let schedule = GasRepricingSchedule {
        rules: vec![GasRepricingRule {
            opcode: 0x31,
            category: OpcodeCategory::Environment,
            old_gas: 2_600,
            new_gas: 0,
            rationale: "invalid",
        }],
        activation_block: 0,
        name: "zero".to_owned(),
    };

    let result = validate_repricing_schedule(&schedule);
    assert!(matches!(
        result,
        Err(ref errors) if errors.contains(&GasRepricingError::ZeroGasCost { opcode: 0x31 })
    ));
}

#[test]
fn impact_computation_correct() {
    let schedule = glamsterdam_default_schedule();
    let impact = compute_repricing_impact(&schedule);

    assert_eq!(impact.total_opcodes_affected, 12);
    assert!((impact.avg_cost_change_percent - (-47.883_089_133_089_13)).abs() < 1e-9);
    assert!((impact.max_increase_percent - 50.0).abs() < 1e-9);
    assert!((impact.max_decrease_percent - 84.615_384_615_384_61).abs() < 1e-9);
    assert_eq!(
        impact.categories_affected,
        vec![
            OpcodeCategory::Storage,
            OpcodeCategory::Environment,
            OpcodeCategory::Call,
            OpcodeCategory::Hash,
            OpcodeCategory::Create,
        ]
    );
}

#[test]
fn lookup_before_activation() {
    let mut schedule = glamsterdam_default_schedule();
    schedule.activation_block = 1_000;

    let gas = lookup_gas_cost(&schedule, 0x55, 999);
    assert_eq!(gas, 20_000);
}

#[test]
fn lookup_after_activation() {
    let mut schedule = glamsterdam_default_schedule();
    schedule.activation_block = 1_000;

    let gas = lookup_gas_cost(&schedule, 0x55, 1_000);
    assert_eq!(gas, 5_000);
}

#[test]
fn gas_table_reflects_activation() {
    let mut schedule = glamsterdam_default_schedule();
    schedule.activation_block = 123;

    let table = build_gas_table(&schedule, 123);
    assert_eq!(table.get(&0x55), Some(&5_000));
    assert_eq!(table.get(&0x54), Some(&800));
    assert_eq!(table.get(&0x20), Some(&36));
    assert_eq!(table.get(&0xF0), Some(&48_000));
}
