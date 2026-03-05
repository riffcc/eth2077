use eth2077_types::ci_formal_gate::{
    compute_ci_formal_gate_commitment, compute_ci_formal_gate_stats, default_ci_formal_gate_config,
    validate_ci_formal_gate_config, CiFormalGateConfig, CriticalPathTier, GateMode, GateVerdict,
    PlaceholderDebt, PlaceholderDebtKind,
};
use std::collections::HashMap;

fn debt(id: &str, tier: CriticalPathTier, age_days: u64, module: &str) -> PlaceholderDebt {
    PlaceholderDebt {
        id: id.to_string(),
        kind: PlaceholderDebtKind::Sorry,
        module_path: module.to_string(),
        tier,
        introduced_commit: "abc123".to_string(),
        age_days,
        metadata: HashMap::new(),
    }
}

#[test]
fn enum_variants_are_constructible() {
    let modes = [
        GateMode::Advisory,
        GateMode::Warning,
        GateMode::Blocking,
        GateMode::Strict,
        GateMode::ReleaseCandidate,
        GateMode::Emergency,
    ];
    let debt_kinds = [
        PlaceholderDebtKind::Axiom,
        PlaceholderDebtKind::Sorry,
        PlaceholderDebtKind::Admit,
        PlaceholderDebtKind::Stub,
        PlaceholderDebtKind::TodoProof,
        PlaceholderDebtKind::DeferredLemma,
    ];
    let tiers = [
        CriticalPathTier::Tier1Safety,
        CriticalPathTier::Tier2Liveness,
        CriticalPathTier::Tier3Performance,
        CriticalPathTier::Tier4Convenience,
        CriticalPathTier::Tier5Optional,
        CriticalPathTier::Tier6Deprecated,
    ];
    let verdicts = [
        GateVerdict::Pass,
        GateVerdict::ConditionalPass,
        GateVerdict::SoftFail,
        GateVerdict::HardFail,
        GateVerdict::Error,
        GateVerdict::Exempted,
    ];
    assert_eq!(modes.len(), 6);
    assert_eq!(debt_kinds.len(), 6);
    assert_eq!(tiers.len(), 6);
    assert_eq!(verdicts.len(), 6);
}

#[test]
fn default_config_is_valid() {
    let config = default_ci_formal_gate_config();
    assert_eq!(validate_ci_formal_gate_config(&config), Ok(()));
}

#[test]
fn config_validation_collects_multiple_issues() {
    let config = CiFormalGateConfig {
        mode: GateMode::ReleaseCandidate,
        max_tier1_debt: 1,
        max_tier2_debt: 2,
        max_total_debt: 1,
        strict_on_release_branch: false,
        exemption_list: vec!["x".to_string(), " x ".to_string(), " ".to_string()],
        metadata: HashMap::from([(" ".to_string(), " ".to_string())]),
    };
    let errors = validate_ci_formal_gate_config(&config).unwrap_err();
    for field in [
        "max_tier1_debt",
        "max_tier2_debt",
        "max_total_debt",
        "strict_on_release_branch",
        "exemption_list",
        "metadata",
    ] {
        assert!(errors.iter().any(|e| e.field == field));
    }
}

#[test]
fn stats_respect_exemptions_and_thresholds() {
    let mut config = default_ci_formal_gate_config();
    config.max_tier2_debt = 1;
    config.max_total_debt = 3;
    config.exemption_list = vec!["d4".to_string()];
    let debts = vec![
        debt("d1", CriticalPathTier::Tier1Safety, 24, "core::safety"),
        debt("d2", CriticalPathTier::Tier2Liveness, 7, "core::live::a"),
        debt("d3", CriticalPathTier::Tier2Liveness, 3, "core::live::b"),
        debt("d4", CriticalPathTier::Tier3Performance, 30, "core::perf"),
    ];
    let stats = compute_ci_formal_gate_stats(&debts, &config);
    assert_eq!(stats.total_debt, 3);
    assert_eq!(stats.tier1_count, 1);
    assert_eq!(stats.tier2_count, 2);
    assert_eq!(stats.tier3_count, 0);
    assert_eq!(stats.oldest_debt_days, 24);
    assert_eq!(stats.verdict, GateVerdict::HardFail);
    assert_eq!(
        stats.blocked_modules,
        vec!["core::live::a", "core::live::b", "core::safety"]
    );
}

#[test]
fn strict_and_emergency_modes_drive_expected_verdicts() {
    let debts = vec![debt("d1", CriticalPathTier::Tier5Optional, 2, "misc::opt")];

    let mut strict = default_ci_formal_gate_config();
    strict.mode = GateMode::Strict;
    strict.max_tier1_debt = 0;
    strict.max_tier2_debt = 0;
    strict.max_total_debt = 0;
    strict.strict_on_release_branch = true;
    strict.exemption_list.clear();
    assert_eq!(
        compute_ci_formal_gate_stats(&debts, &strict).verdict,
        GateVerdict::HardFail
    );

    let mut emergency = default_ci_formal_gate_config();
    emergency.mode = GateMode::Emergency;
    emergency.strict_on_release_branch = false;
    assert_eq!(
        compute_ci_formal_gate_stats(&debts, &emergency).verdict,
        GateVerdict::Exempted
    );
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let mut left = default_ci_formal_gate_config();
    left.exemption_list = vec!["b".to_string(), "a".to_string(), "a".to_string()];
    left.metadata.clear();
    left.metadata.insert("y".to_string(), "2".to_string());
    left.metadata.insert("x".to_string(), "1".to_string());

    let mut right = default_ci_formal_gate_config();
    right.exemption_list = vec!["a".to_string(), "b".to_string()];
    right.metadata.clear();
    right.metadata.insert("x".to_string(), "1".to_string());
    right.metadata.insert("y".to_string(), "2".to_string());

    let hash_left = compute_ci_formal_gate_commitment(&left);
    assert_eq!(hash_left, compute_ci_formal_gate_commitment(&right));
    assert_eq!(hash_left.len(), 64);
    right.mode = GateMode::Warning;
    assert_ne!(hash_left, compute_ci_formal_gate_commitment(&right));
}
