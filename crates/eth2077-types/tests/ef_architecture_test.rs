use eth2077_types::ef_architecture::{
    compute_architecture_commitment, compute_dependency_depth, compute_ef_architecture_stats,
    default_ef_architecture_config, filter_relevant_items, prioritize_items,
    validate_ef_architecture_config, ActionableItem, ArchitectureLayer, EfArchitectureConfig,
    EfArchitectureValidationError, RoadmapTrack,
};

fn item(
    title: &str,
    track: RoadmapTrack,
    layer: ArchitectureLayer,
    priority: f64,
    complexity: f64,
    dependencies: &[&str],
    relevant: bool,
) -> ActionableItem {
    ActionableItem {
        title: title.to_string(),
        track,
        layer,
        priority,
        estimated_complexity: complexity,
        dependencies: dependencies
            .iter()
            .map(|dependency| dependency.to_string())
            .collect(),
        eth2077_relevant: relevant,
    }
}

#[test]
fn default_config_has_expected_values() {
    let config = default_ef_architecture_config();
    assert_eq!(config.min_priority, 0.5);
    assert_eq!(config.max_items, 64);
    assert!(config.include_dependencies);
    assert_eq!(config.tracks.len(), 6);
    assert_eq!(config.layers.len(), 6);
}

#[test]
fn validate_config_collects_expected_errors() {
    let config = EfArchitectureConfig {
        tracks: vec![RoadmapTrack::Surge, RoadmapTrack::Surge],
        layers: vec![ArchitectureLayer::Execution, ArchitectureLayer::Execution],
        min_priority: 1.5,
        max_items: 0,
        include_dependencies: false,
    };

    let errors = validate_ef_architecture_config(&config).unwrap_err();
    assert!(errors.contains(&EfArchitectureValidationError::PriorityOutOfRange { value: 1.5 }));
    assert!(errors.contains(&EfArchitectureValidationError::MaxItemsZero));
    assert!(errors.contains(&EfArchitectureValidationError::DuplicateTrack));
    assert!(errors.contains(&EfArchitectureValidationError::DuplicateLayer));
}

#[test]
fn validate_config_rejects_empty_track_and_layer_sets() {
    let config = EfArchitectureConfig {
        tracks: Vec::new(),
        layers: Vec::new(),
        min_priority: 0.5,
        max_items: 5,
        include_dependencies: true,
    };

    let errors = validate_ef_architecture_config(&config).unwrap_err();
    assert!(errors.contains(&EfArchitectureValidationError::EmptyTracks));
    assert!(errors.contains(&EfArchitectureValidationError::EmptyLayers));
}

#[test]
fn validate_config_accepts_valid_input() {
    let config = default_ef_architecture_config();
    assert_eq!(validate_ef_architecture_config(&config), Ok(()));
}

#[test]
fn prioritize_items_sorts_by_priority_then_complexity() {
    let mut items = vec![
        item(
            "A",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.9,
            0.6,
            &[],
            true,
        ),
        item(
            "B",
            RoadmapTrack::Verge,
            ArchitectureLayer::Consensus,
            0.95,
            0.8,
            &[],
            true,
        ),
        item(
            "C",
            RoadmapTrack::Purge,
            ArchitectureLayer::Networking,
            0.9,
            0.2,
            &[],
            true,
        ),
    ];

    prioritize_items(&mut items);
    assert_eq!(items[0].title, "B");
    assert_eq!(items[1].title, "C");
    assert_eq!(items[2].title, "A");
}

#[test]
fn filter_relevant_items_applies_filters_and_max_items() {
    let items = vec![
        item(
            "Keep1",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.9,
            0.4,
            &[],
            true,
        ),
        item(
            "DropLowPriority",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.1,
            0.1,
            &[],
            true,
        ),
        item(
            "DropTrack",
            RoadmapTrack::Splurge,
            ArchitectureLayer::Execution,
            0.9,
            0.1,
            &[],
            true,
        ),
        item(
            "DropIrrelevant",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.95,
            0.1,
            &[],
            false,
        ),
        item(
            "Keep2",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.85,
            0.2,
            &[],
            true,
        ),
    ];
    let config = EfArchitectureConfig {
        tracks: vec![RoadmapTrack::Surge],
        layers: vec![ArchitectureLayer::Execution],
        min_priority: 0.8,
        max_items: 1,
        include_dependencies: false,
    };

    let filtered = filter_relevant_items(&items, &config);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Keep1");
}

#[test]
fn filter_relevant_items_can_include_dependency_chain() {
    let items = vec![
        item(
            "Top",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.9,
            0.5,
            &["Mid"],
            true,
        ),
        item(
            "Mid",
            RoadmapTrack::Merge,
            ArchitectureLayer::Networking,
            0.2,
            0.8,
            &["Base"],
            false,
        ),
        item(
            "Base",
            RoadmapTrack::Purge,
            ArchitectureLayer::StateManagement,
            0.1,
            0.3,
            &[],
            false,
        ),
    ];
    let config = EfArchitectureConfig {
        tracks: vec![RoadmapTrack::Surge],
        layers: vec![ArchitectureLayer::Execution],
        min_priority: 0.8,
        max_items: 10,
        include_dependencies: true,
    };

    let filtered = filter_relevant_items(&items, &config);
    let titles: Vec<String> = filtered.into_iter().map(|entry| entry.title).collect();
    assert!(titles.contains(&"Top".to_string()));
    assert!(titles.contains(&"Mid".to_string()));
    assert!(titles.contains(&"Base".to_string()));
}

#[test]
fn compute_dependency_depth_finds_longest_chain() {
    let items = vec![
        item(
            "A",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.8,
            0.3,
            &["B"],
            true,
        ),
        item(
            "B",
            RoadmapTrack::Verge,
            ArchitectureLayer::Consensus,
            0.8,
            0.3,
            &["C"],
            true,
        ),
        item(
            "C",
            RoadmapTrack::Purge,
            ArchitectureLayer::Networking,
            0.8,
            0.3,
            &[],
            true,
        ),
    ];

    assert_eq!(compute_dependency_depth(&items), 2);
}

#[test]
fn compute_stats_uses_filtered_items() {
    let items = vec![
        item(
            "ExecA",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.9,
            0.2,
            &[],
            true,
        ),
        item(
            "ExecB",
            RoadmapTrack::Surge,
            ArchitectureLayer::Execution,
            0.8,
            0.6,
            &["ExecA"],
            true,
        ),
        item(
            "Ignored",
            RoadmapTrack::Splurge,
            ArchitectureLayer::Cryptography,
            0.95,
            0.4,
            &[],
            false,
        ),
    ];
    let config = EfArchitectureConfig {
        tracks: vec![RoadmapTrack::Surge],
        layers: vec![ArchitectureLayer::Execution],
        min_priority: 0.7,
        max_items: 10,
        include_dependencies: false,
    };

    let stats = compute_ef_architecture_stats(&items, &config);
    assert_eq!(stats.total_items, 2);
    assert_eq!(stats.relevant_items, 2);
    assert_eq!(stats.items_by_track, vec![(String::from("Surge"), 2)]);
    assert_eq!(stats.items_by_layer, vec![(String::from("Execution"), 2)]);
    assert!((stats.avg_priority - 0.85).abs() < 1e-9);
    assert!((stats.avg_complexity - 0.4).abs() < 1e-9);
    assert_eq!(stats.dependency_depth, 1);
}

#[test]
fn architecture_commitment_is_order_independent_and_content_sensitive() {
    let one = item(
        "A",
        RoadmapTrack::Surge,
        ArchitectureLayer::Execution,
        0.9,
        0.2,
        &[],
        true,
    );
    let two = item(
        "B",
        RoadmapTrack::Verge,
        ArchitectureLayer::Consensus,
        0.7,
        0.4,
        &["A"],
        true,
    );
    let a = vec![one.clone(), two.clone()];
    let b = vec![two.clone(), one.clone()];

    let hash_a = compute_architecture_commitment(&a);
    let hash_b = compute_architecture_commitment(&b);
    assert_eq!(hash_a, hash_b);

    let mut mutated = b.clone();
    mutated[0].priority = 0.71;
    let hash_mutated = compute_architecture_commitment(&mutated);
    assert_ne!(hash_a, hash_mutated);
}
