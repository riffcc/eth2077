use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RoadmapTrack {
    Surge,
    Verge,
    Purge,
    Scourge,
    Splurge,
    Merge,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArchitectureLayer {
    Execution,
    Consensus,
    DataAvailability,
    Networking,
    Cryptography,
    StateManagement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionableItem {
    pub title: String,
    pub track: RoadmapTrack,
    pub layer: ArchitectureLayer,
    pub priority: f64,
    pub estimated_complexity: f64,
    pub dependencies: Vec<String>,
    pub eth2077_relevant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EfArchitectureConfig {
    pub tracks: Vec<RoadmapTrack>,
    pub layers: Vec<ArchitectureLayer>,
    pub min_priority: f64,
    pub max_items: usize,
    pub include_dependencies: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EfArchitectureValidationError {
    EmptyTracks,
    EmptyLayers,
    PriorityOutOfRange { value: f64 },
    MaxItemsZero,
    DuplicateTrack,
    DuplicateLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EfArchitectureStats {
    pub total_items: usize,
    pub relevant_items: usize,
    pub items_by_track: Vec<(String, usize)>,
    pub items_by_layer: Vec<(String, usize)>,
    pub avg_priority: f64,
    pub avg_complexity: f64,
    pub dependency_depth: usize,
}

pub fn default_ef_architecture_config() -> EfArchitectureConfig {
    EfArchitectureConfig {
        tracks: vec![
            RoadmapTrack::Surge,
            RoadmapTrack::Verge,
            RoadmapTrack::Purge,
            RoadmapTrack::Scourge,
            RoadmapTrack::Splurge,
            RoadmapTrack::Merge,
        ],
        layers: vec![
            ArchitectureLayer::Execution,
            ArchitectureLayer::Consensus,
            ArchitectureLayer::DataAvailability,
            ArchitectureLayer::Networking,
            ArchitectureLayer::Cryptography,
            ArchitectureLayer::StateManagement,
        ],
        min_priority: 0.5,
        max_items: 64,
        include_dependencies: true,
    }
}

pub fn validate_ef_architecture_config(
    config: &EfArchitectureConfig,
) -> Result<(), Vec<EfArchitectureValidationError>> {
    let mut errors = Vec::new();

    if config.tracks.is_empty() {
        errors.push(EfArchitectureValidationError::EmptyTracks);
    }

    if config.layers.is_empty() {
        errors.push(EfArchitectureValidationError::EmptyLayers);
    }

    if !config.min_priority.is_finite() || !(0.0..=1.0).contains(&config.min_priority) {
        errors.push(EfArchitectureValidationError::PriorityOutOfRange {
            value: config.min_priority,
        });
    }

    if config.max_items == 0 {
        errors.push(EfArchitectureValidationError::MaxItemsZero);
    }

    let mut seen_tracks: HashSet<RoadmapTrack> = HashSet::new();
    if config
        .tracks
        .iter()
        .any(|track| !seen_tracks.insert(*track))
    {
        errors.push(EfArchitectureValidationError::DuplicateTrack);
    }

    let mut seen_layers: HashSet<ArchitectureLayer> = HashSet::new();
    if config
        .layers
        .iter()
        .any(|layer| !seen_layers.insert(*layer))
    {
        errors.push(EfArchitectureValidationError::DuplicateLayer);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_ef_architecture_stats(
    items: &[ActionableItem],
    config: &EfArchitectureConfig,
) -> EfArchitectureStats {
    let filtered = filter_relevant_items(items, config);

    if filtered.is_empty() {
        return EfArchitectureStats {
            total_items: 0,
            relevant_items: 0,
            items_by_track: Vec::new(),
            items_by_layer: Vec::new(),
            avg_priority: 0.0,
            avg_complexity: 0.0,
            dependency_depth: 0,
        };
    }

    let mut track_counts: HashMap<String, usize> = HashMap::new();
    let mut layer_counts: HashMap<String, usize> = HashMap::new();
    let mut relevant_items = 0usize;
    let mut priority_sum = 0.0;
    let mut complexity_sum = 0.0;

    for item in &filtered {
        *track_counts
            .entry(roadmap_track_name(item.track).to_string())
            .or_insert(0) += 1;
        *layer_counts
            .entry(architecture_layer_name(item.layer).to_string())
            .or_insert(0) += 1;
        if item.eth2077_relevant {
            relevant_items += 1;
        }
        priority_sum += item.priority;
        complexity_sum += item.estimated_complexity;
    }

    let mut items_by_track: Vec<(String, usize)> = track_counts.into_iter().collect();
    items_by_track.sort_by(|left, right| left.0.cmp(&right.0));

    let mut items_by_layer: Vec<(String, usize)> = layer_counts.into_iter().collect();
    items_by_layer.sort_by(|left, right| left.0.cmp(&right.0));

    EfArchitectureStats {
        total_items: filtered.len(),
        relevant_items,
        items_by_track,
        items_by_layer,
        avg_priority: priority_sum / filtered.len() as f64,
        avg_complexity: complexity_sum / filtered.len() as f64,
        dependency_depth: compute_dependency_depth(&filtered),
    }
}

pub fn prioritize_items(items: &mut [ActionableItem]) {
    items.sort_by(|left, right| {
        right
            .priority
            .total_cmp(&left.priority)
            .then_with(|| {
                left.estimated_complexity
                    .total_cmp(&right.estimated_complexity)
            })
            .then_with(|| roadmap_track_rank(left.track).cmp(&roadmap_track_rank(right.track)))
            .then_with(|| {
                architecture_layer_rank(left.layer).cmp(&architecture_layer_rank(right.layer))
            })
            .then_with(|| left.title.cmp(&right.title))
    });
}

pub fn filter_relevant_items(
    items: &[ActionableItem],
    config: &EfArchitectureConfig,
) -> Vec<ActionableItem> {
    let track_filter: HashSet<RoadmapTrack> = config.tracks.iter().copied().collect();
    let layer_filter: HashSet<ArchitectureLayer> = config.layers.iter().copied().collect();

    let mut selected: Vec<ActionableItem> = items
        .iter()
        .filter(|item| item.eth2077_relevant)
        .filter(|item| track_filter.contains(&item.track))
        .filter(|item| layer_filter.contains(&item.layer))
        .filter(|item| item.priority >= config.min_priority)
        .cloned()
        .collect();

    if config.include_dependencies {
        let by_title: HashMap<&str, &ActionableItem> = items
            .iter()
            .map(|item| (item.title.as_str(), item))
            .collect();
        let mut known: HashSet<String> = selected.iter().map(|item| item.title.clone()).collect();

        let mut queue: Vec<String> = selected
            .iter()
            .flat_map(|item| item.dependencies.iter().cloned())
            .collect();

        while let Some(dependency_title) = queue.pop() {
            if known.contains(&dependency_title) {
                continue;
            }
            if let Some(dep_item) = by_title.get(dependency_title.as_str()) {
                selected.push((*dep_item).clone());
                known.insert(dependency_title);
                queue.extend(dep_item.dependencies.iter().cloned());
            }
        }
    }

    prioritize_items(&mut selected);
    if selected.len() > config.max_items {
        selected.truncate(config.max_items);
    }
    selected
}

pub fn compute_dependency_depth(items: &[ActionableItem]) -> usize {
    let index_by_title: HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.title.as_str(), index))
        .collect();
    let mut memo: HashMap<usize, usize> = HashMap::new();
    let mut visiting: HashSet<usize> = HashSet::new();

    let mut max_depth = 0usize;
    for (index, _) in items.iter().enumerate() {
        let depth = dependency_depth_from(index, items, &index_by_title, &mut memo, &mut visiting);
        max_depth = max_depth.max(depth);
    }
    max_depth
}

pub fn compute_architecture_commitment(items: &[ActionableItem]) -> [u8; 32] {
    let mut ordered = items.to_vec();
    ordered.sort_by(|left, right| {
        roadmap_track_rank(left.track)
            .cmp(&roadmap_track_rank(right.track))
            .then_with(|| {
                architecture_layer_rank(left.layer).cmp(&architecture_layer_rank(right.layer))
            })
            .then_with(|| left.priority.total_cmp(&right.priority))
            .then_with(|| {
                left.estimated_complexity
                    .total_cmp(&right.estimated_complexity)
            })
            .then_with(|| left.eth2077_relevant.cmp(&right.eth2077_relevant))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.dependencies.cmp(&right.dependencies))
    });

    let mut hasher = Sha256::new();
    hasher.update((ordered.len() as u64).to_be_bytes());

    for item in ordered {
        hash_string(&mut hasher, &item.title);
        hasher.update([roadmap_track_rank(item.track)]);
        hasher.update([architecture_layer_rank(item.layer)]);
        hasher.update(item.priority.to_be_bytes());
        hasher.update(item.estimated_complexity.to_be_bytes());
        hasher.update([u8::from(item.eth2077_relevant)]);

        hasher.update((item.dependencies.len() as u64).to_be_bytes());
        for dependency in item.dependencies {
            hash_string(&mut hasher, &dependency);
        }
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn dependency_depth_from(
    index: usize,
    items: &[ActionableItem],
    index_by_title: &HashMap<&str, usize>,
    memo: &mut HashMap<usize, usize>,
    visiting: &mut HashSet<usize>,
) -> usize {
    if let Some(cached) = memo.get(&index) {
        return *cached;
    }
    if !visiting.insert(index) {
        return 0;
    }

    let mut best = 0usize;
    for dependency in &items[index].dependencies {
        let dep_depth = if let Some(dep_index) = index_by_title.get(dependency.as_str()) {
            1 + dependency_depth_from(*dep_index, items, index_by_title, memo, visiting)
        } else {
            1
        };
        best = best.max(dep_depth);
    }

    visiting.remove(&index);
    memo.insert(index, best);
    best
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn roadmap_track_name(track: RoadmapTrack) -> &'static str {
    match track {
        RoadmapTrack::Surge => "Surge",
        RoadmapTrack::Verge => "Verge",
        RoadmapTrack::Purge => "Purge",
        RoadmapTrack::Scourge => "Scourge",
        RoadmapTrack::Splurge => "Splurge",
        RoadmapTrack::Merge => "Merge",
    }
}

fn architecture_layer_name(layer: ArchitectureLayer) -> &'static str {
    match layer {
        ArchitectureLayer::Execution => "Execution",
        ArchitectureLayer::Consensus => "Consensus",
        ArchitectureLayer::DataAvailability => "DataAvailability",
        ArchitectureLayer::Networking => "Networking",
        ArchitectureLayer::Cryptography => "Cryptography",
        ArchitectureLayer::StateManagement => "StateManagement",
    }
}

fn roadmap_track_rank(track: RoadmapTrack) -> u8 {
    match track {
        RoadmapTrack::Merge => 0,
        RoadmapTrack::Surge => 1,
        RoadmapTrack::Verge => 2,
        RoadmapTrack::Purge => 3,
        RoadmapTrack::Scourge => 4,
        RoadmapTrack::Splurge => 5,
    }
}

fn architecture_layer_rank(layer: ArchitectureLayer) -> u8 {
    match layer {
        ArchitectureLayer::Execution => 0,
        ArchitectureLayer::Consensus => 1,
        ArchitectureLayer::DataAvailability => 2,
        ArchitectureLayer::Networking => 3,
        ArchitectureLayer::Cryptography => 4,
        ArchitectureLayer::StateManagement => 5,
    }
}
