use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OpcodeCategory {
    Computation,
    Memory,
    Storage,
    Call,
    Create,
    Log,
    Hash,
    Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasRepricingRule {
    pub opcode: u8,
    pub category: OpcodeCategory,
    pub old_gas: u64,
    pub new_gas: u64,
    pub rationale: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasRepricingSchedule {
    pub rules: Vec<GasRepricingRule>,
    pub activation_block: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GasRepricingError {
    DuplicateOpcode { opcode: u8 },
    ZeroGasCost { opcode: u8 },
    NoRules,
    InvalidActivationBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepricingImpact {
    pub total_opcodes_affected: usize,
    pub avg_cost_change_percent: f64,
    pub max_increase_percent: f64,
    pub max_decrease_percent: f64,
    pub categories_affected: Vec<OpcodeCategory>,
}

pub fn glamsterdam_default_schedule() -> GasRepricingSchedule {
    GasRepricingSchedule {
        rules: vec![
            GasRepricingRule {
                opcode: 0x55,
                category: OpcodeCategory::Storage,
                old_gas: 20_000,
                new_gas: 5_000,
                rationale: "Reduce storage write bottleneck costs",
            },
            GasRepricingRule {
                opcode: 0x54,
                category: OpcodeCategory::Storage,
                old_gas: 2_100,
                new_gas: 800,
                rationale: "Reduce storage read bottleneck costs",
            },
            GasRepricingRule {
                opcode: 0x31,
                category: OpcodeCategory::Environment,
                old_gas: 2_600,
                new_gas: 400,
                rationale: "Lower account state access overhead",
            },
            GasRepricingRule {
                opcode: 0x3B,
                category: OpcodeCategory::Environment,
                old_gas: 2_600,
                new_gas: 400,
                rationale: "Lower external code size access overhead",
            },
            GasRepricingRule {
                opcode: 0x3C,
                category: OpcodeCategory::Environment,
                old_gas: 2_600,
                new_gas: 400,
                rationale: "Lower external code copy access overhead",
            },
            GasRepricingRule {
                opcode: 0x3F,
                category: OpcodeCategory::Environment,
                old_gas: 2_600,
                new_gas: 400,
                rationale: "Lower external code hash access overhead",
            },
            GasRepricingRule {
                opcode: 0xF1,
                category: OpcodeCategory::Call,
                old_gas: 2_600,
                new_gas: 700,
                rationale: "Rebalance call overhead for throughput",
            },
            GasRepricingRule {
                opcode: 0xFA,
                category: OpcodeCategory::Call,
                old_gas: 2_600,
                new_gas: 700,
                rationale: "Rebalance static call overhead for throughput",
            },
            GasRepricingRule {
                opcode: 0xF4,
                category: OpcodeCategory::Call,
                old_gas: 2_600,
                new_gas: 700,
                rationale: "Rebalance delegate call overhead for throughput",
            },
            GasRepricingRule {
                opcode: 0x20,
                category: OpcodeCategory::Hash,
                old_gas: 30,
                new_gas: 36,
                rationale: "Align hashing cost with observed execution pressure",
            },
            GasRepricingRule {
                opcode: 0xF0,
                category: OpcodeCategory::Create,
                old_gas: 32_000,
                new_gas: 48_000,
                rationale: "Increase contract creation cost for global balance",
            },
            GasRepricingRule {
                opcode: 0xF5,
                category: OpcodeCategory::Create,
                old_gas: 32_000,
                new_gas: 48_000,
                rationale: "Increase deterministic creation cost for global balance",
            },
        ],
        activation_block: 0,
        name: "glamsterdam".to_owned(),
    }
}

pub fn validate_repricing_schedule(
    schedule: &GasRepricingSchedule,
) -> Result<(), Vec<GasRepricingError>> {
    let mut errors = Vec::new();

    if schedule.rules.is_empty() {
        errors.push(GasRepricingError::NoRules);
    }

    let mut seen_opcodes: HashMap<u8, bool> = HashMap::new();
    for rule in &schedule.rules {
        if seen_opcodes.insert(rule.opcode, true).is_some() {
            errors.push(GasRepricingError::DuplicateOpcode {
                opcode: rule.opcode,
            });
        }

        if rule.old_gas == 0 || rule.new_gas == 0 {
            errors.push(GasRepricingError::ZeroGasCost {
                opcode: rule.opcode,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_repricing_impact(schedule: &GasRepricingSchedule) -> RepricingImpact {
    let total = schedule.rules.len();

    if total == 0 {
        return RepricingImpact {
            total_opcodes_affected: 0,
            avg_cost_change_percent: 0.0,
            max_increase_percent: 0.0,
            max_decrease_percent: 0.0,
            categories_affected: Vec::new(),
        };
    }

    let mut sum_change_percent = 0.0_f64;
    let mut max_increase_percent = 0.0_f64;
    let mut max_decrease_percent = 0.0_f64;
    let mut categories_affected: Vec<OpcodeCategory> = Vec::new();

    for rule in &schedule.rules {
        let change_percent = if rule.old_gas == 0 {
            0.0
        } else {
            ((rule.new_gas as f64 - rule.old_gas as f64) / rule.old_gas as f64) * 100.0
        };
        sum_change_percent += change_percent;

        if change_percent > max_increase_percent {
            max_increase_percent = change_percent;
        }

        if change_percent < 0.0 {
            let decrease_percent = -change_percent;
            if decrease_percent > max_decrease_percent {
                max_decrease_percent = decrease_percent;
            }
        }

        if !categories_affected.contains(&rule.category) {
            categories_affected.push(rule.category);
        }
    }

    RepricingImpact {
        total_opcodes_affected: total,
        avg_cost_change_percent: sum_change_percent / total as f64,
        max_increase_percent,
        max_decrease_percent,
        categories_affected,
    }
}

pub fn lookup_gas_cost(schedule: &GasRepricingSchedule, opcode: u8, block_number: u64) -> u64 {
    for rule in &schedule.rules {
        if rule.opcode == opcode {
            if block_number >= schedule.activation_block {
                return rule.new_gas;
            }
            return rule.old_gas;
        }
    }
    0
}

pub fn build_gas_table(schedule: &GasRepricingSchedule, block_number: u64) -> HashMap<u8, u64> {
    let mut gas_table = HashMap::new();
    for rule in &schedule.rules {
        let effective_cost = if block_number >= schedule.activation_block {
            rule.new_gas
        } else {
            rule.old_gas
        };
        gas_table.insert(rule.opcode, effective_cost);
    }
    gas_table
}
