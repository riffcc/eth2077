use eth2077_types::assumption_checker::{
    check_assumption_coverage, Assumption, AssumptionCheckResult, AssumptionRegistry,
    AssumptionStatus, ValidationMethod,
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
        owner: "core-team".to_string(),
        validation_method: ValidationMethod::ManualReview,
        revisit_cadence_days: 30,
        last_reviewed: Some("2026-03-01".to_string()),
        status: AssumptionStatus::Active,
        mitigations: vec!["Fallback procedure".to_string()],
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

#[test]
fn registry_lookup_helpers_work() {
    let mut violated = assumption("A-002", Some("TM-LIVENESS-001"));
    violated.owner = "consensus-team".to_string();
    violated.status = AssumptionStatus::Violated;

    let registry = AssumptionRegistry {
        assumptions: vec![assumption("A-001", Some("TM-SAFETY-001")), violated],
    };

    assert_eq!(
        registry.get_by_id("A-001").map(|a| a.name.as_str()),
        Some("Assumption A-001")
    );
    assert!(registry.get_by_id("A-404").is_none());

    let by_owner = registry.get_by_owner("consensus-team");
    assert_eq!(by_owner.len(), 1);
    assert_eq!(by_owner[0].id, "A-002");

    let violated = registry.get_violated();
    assert_eq!(violated.len(), 1);
    assert_eq!(violated[0].id, "A-002");
}

#[test]
fn overdue_review_detection_works() {
    let mut overdue = assumption("A-001", Some("TM-SAFETY-001"));
    overdue.revisit_cadence_days = 30;
    overdue.last_reviewed = Some("2026-01-01".to_string());

    let mut fresh = assumption("A-002", Some("TM-LIVENESS-001"));
    fresh.revisit_cadence_days = 90;
    fresh.last_reviewed = Some("2026-02-15".to_string());

    let mut never_reviewed = assumption("A-003", Some("TM-IMPL-001"));
    never_reviewed.last_reviewed = None;

    let registry = AssumptionRegistry {
        assumptions: vec![overdue, fresh, never_reviewed],
    };

    let overdue = registry.get_overdue_reviews("2026-03-04");
    assert_eq!(overdue.len(), 2);
    assert!(overdue.iter().any(|a| a.id == "A-001"));
    assert!(overdue.iter().any(|a| a.id == "A-003"));
}
