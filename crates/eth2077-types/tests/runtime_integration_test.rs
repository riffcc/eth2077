use eth2077_types::runtime_integration::*;
use std::collections::HashMap;

fn approx_eq(left: f64, right: f64, eps: f64) {
    assert!(
        (left - right).abs() <= eps,
        "left={left}, right={right}, eps={eps}"
    );
}

fn trace(id: &str, verified: bool) -> ProofTrace {
    ProofTrace {
        theorem_id: id.to_string(),
        kind: ProofTraceKind::TheoremLink,
        artifact_path: format!("proofs/{id}.json"),
        verified,
    }
}

fn module(
    id: &str,
    phase: IntegrationPhase,
    compatibility: CompatibilityLevel,
    traces: Vec<ProofTrace>,
) -> RuntimeModule {
    RuntimeModule {
        id: id.to_string(),
        module_name: format!("module_{id}"),
        phase,
        slot: RuntimeSlot::Execution,
        compatibility,
        proof_traces: traces,
        dependencies: vec![],
        metadata: HashMap::new(),
    }
}

#[test]
fn default_config_is_valid() {
    let config = default_runtime_integration_config();
    assert_eq!(config.max_shim_modules, 8);
    assert!(config.require_proof_traces);
    assert_eq!(config.min_compatibility, CompatibilityLevel::Shim);
    assert!(!config.auto_activate);
    assert_eq!(config.test_coverage_min_pct, 90.0);
    assert!(config.rollback_on_failure);
    assert_eq!(validate_runtime_integration_config(&config), Ok(()));
}

#[test]
fn validation_collects_multiple_errors() {
    let mut config = default_runtime_integration_config();
    config.max_shim_modules = 2_000;
    config.auto_activate = true;
    config.min_compatibility = CompatibilityLevel::Incompatible;
    config.test_coverage_min_pct = 120.0;
    config
        .metadata
        .insert("   ".to_string(), "non-empty".to_string());
    let errors = validate_runtime_integration_config(&config).expect_err("expected errors");
    assert!(errors.iter().any(|error| error.field == "max_shim_modules"));
    assert!(errors
        .iter()
        .any(|error| error.field == "test_coverage_min_pct"));
    assert!(errors.iter().any(|error| error.field == "auto_activate"));
    assert!(errors
        .iter()
        .any(|error| error.field == "min_compatibility"));
    assert!(errors.iter().any(|error| error.field == "metadata"));
}

#[test]
fn stats_are_computed_from_modules() {
    let modules = vec![
        module(
            "consensus_bridge",
            IntegrationPhase::Activated,
            CompatibilityLevel::Full,
            vec![
                trace("thm.consensus.safety", true),
                trace("thm.consensus.liveness", true),
            ],
        ),
        module(
            "mempool_adapter",
            IntegrationPhase::Tested,
            CompatibilityLevel::Partial,
            vec![trace("thm.mempool.ordering", false)],
        ),
        module(
            "legacy_codec",
            IntegrationPhase::Configured,
            CompatibilityLevel::Incompatible,
            vec![],
        ),
    ];

    let stats = compute_runtime_integration_stats(&modules);
    assert_eq!(stats.total_modules, 3);
    assert_eq!(stats.by_phase.get(&IntegrationPhase::Activated), Some(&1));
    assert_eq!(stats.by_phase.get(&IntegrationPhase::Tested), Some(&1));
    assert_eq!(stats.by_phase.get(&IntegrationPhase::Configured), Some(&1));
    approx_eq(stats.traced_pct, 66.666_666_666, 1e-6);
    approx_eq(stats.avg_proof_traces, 1.0, 1e-12);
    approx_eq(stats.compatibility_score, 61.666_666_666, 1e-6);
    assert_eq!(stats.activated_count, 1);
}

#[test]
fn commitment_is_stable_and_changes_when_config_changes() {
    let mut first = default_runtime_integration_config();
    first
        .metadata
        .insert("chain".to_string(), "eth2077-devnet".to_string());
    first
        .metadata
        .insert("release".to_string(), "v0.3.0".to_string());

    let mut second = default_runtime_integration_config();
    second
        .metadata
        .insert("release".to_string(), "v0.3.0".to_string());
    second
        .metadata
        .insert("chain".to_string(), "eth2077-devnet".to_string());

    let commit_a = compute_runtime_integration_commitment(&first);
    let commit_b = compute_runtime_integration_commitment(&second);
    assert_eq!(commit_a, commit_b);
    assert_eq!(commit_a.len(), 64);
    assert!(commit_a.chars().all(|c| c.is_ascii_hexdigit()));

    second.test_coverage_min_pct = 91.0;
    let commit_c = compute_runtime_integration_commitment(&second);
    assert_ne!(commit_a, commit_c);
}
