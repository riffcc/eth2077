use eth2077_types::assumption_checker::{
    check_assumption_coverage, Assumption, AssumptionCheckResult, AssumptionRegistry,
};
use eth2077_types::theorem_registry::{
    TheoremEntry, TheoremRegistry, TheoremStatus, TheoremTier,
};

fn sample_theorem_registry(tags: &[Vec<&str>]) -> TheoremRegistry {
    TheoremRegistry {
        version: "1.0.0".to_string(),
        entries: tags
            .iter()
            .enumerate()
            .map(|(index, theorem_tags)| TheoremEntry {
                id: format!("THM-{:03}", index + 1),
                name: format!("Theorem {}", index + 1),
                description: "Test theorem".to_string(),
                tier: TheoremTier::Tier1,
                status: TheoremStatus::Proposed,
                dependencies: vec![],
                proof_artifact_path: None,
                tags: theorem_tags.iter().map(|tag| (*tag).to_string()).collect(),
            })
            .collect(),
    }
}

fn assumption(id: &str, anchor: Option<&str>) -> Assumption {
    Assumption {
        id: id.to_string(),
        name: format!("Assumption {id}"),
        description: "Test assumption".to_string(),
        threat_model_anchor: anchor.map(|value| value.to_string()),
        referenced_by: vec![],
    }
}

#[test]
fn valid_assumption_coverage_passes() {
    let theorem_registry = sample_theorem_registry(&[
        vec!["safety", "assumption:A-001"],
        vec!["liveness", "assumption:A-002"],
    ]);
    let assumption_registry = AssumptionRegistry {
        assumptions: vec![
            assumption("A-001", Some("TM-SAFETY-001")),
            assumption("A-002", Some("TM-LIVENESS-001")),
        ],
    };

    let result = check_assumption_coverage(&theorem_registry, &assumption_registry);
    assert_eq!(result, vec![AssumptionCheckResult::Ok]);
}

#[test]
fn missing_assumption_is_detected() {
    let theorem_registry = sample_theorem_registry(&[
        vec!["assumption:A-001"],
        vec!["assumption:A-404"],
    ]);
    let assumption_registry = AssumptionRegistry {
        assumptions: vec![assumption("A-001", Some("TM-SAFETY-001"))],
    };

    let result = check_assumption_coverage(&theorem_registry, &assumption_registry);
    assert_eq!(
        result,
        vec![AssumptionCheckResult::MissingAssumptions(vec![
            "A-404".to_string()
        ])]
    );
}

#[test]
fn orphaned_assumption_is_detected() {
    let theorem_registry = sample_theorem_registry(&[vec!["assumption:A-001"]]);
    let assumption_registry = AssumptionRegistry {
        assumptions: vec![
            assumption("A-001", Some("TM-SAFETY-001")),
            assumption("A-002", Some("TM-LIVENESS-001")),
        ],
    };

    let result = check_assumption_coverage(&theorem_registry, &assumption_registry);
    assert_eq!(
        result,
        vec![AssumptionCheckResult::OrphanedAssumptions(vec![
            "A-002".to_string()
        ])]
    );
}

#[test]
fn broken_anchor_is_detected() {
    let theorem_registry = sample_theorem_registry(&[vec!["assumption:A-001"]]);
    let assumption_registry = AssumptionRegistry {
        assumptions: vec![assumption("A-001", Some("   "))],
    };

    let result = check_assumption_coverage(&theorem_registry, &assumption_registry);
    assert_eq!(
        result,
        vec![AssumptionCheckResult::BrokenAnchors(vec![
            "A-001".to_string()
        ])]
    );
}
