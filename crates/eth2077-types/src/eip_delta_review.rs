use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChangeCategory {
    Normative,
    Informational,
    Editorial,
    TestVector,
    SecurityCritical,
    GasSchedule,
    Deprecation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ImpactLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ImplementationStatus {
    NotStarted,
    InProgress,
    NeedsReview,
    Implemented,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeltaEntry {
    pub eip_number: u64,
    pub pr_number: u64,
    pub category: ChangeCategory,
    pub impact: ImpactLevel,
    pub description: String,
    pub affected_sections: Vec<String>,
    pub implementation_status: ImplementationStatus,
    pub breaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeltaReviewConfig {
    pub auto_classify_threshold: f64,
    pub require_test_vectors: bool,
    pub max_entries_per_review: usize,
    pub track_implementation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeltaReviewError {
    EmptyReview,
    DuplicateEntry { eip: u64, pr: u64 },
    MissingDescription { index: usize },
    TooManyEntries { count: usize, max: usize },
    InvalidEipNumber { eip: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeltaReviewStats {
    pub total_entries: usize,
    pub normative_count: usize,
    pub breaking_count: usize,
    pub security_critical_count: usize,
    pub implementation_coverage: f64,
    pub category_distribution: Vec<(String, usize)>,
    pub impact_distribution: Vec<(String, usize)>,
    pub avg_impact_score: f64,
}

pub fn default_review_config() -> DeltaReviewConfig {
    DeltaReviewConfig {
        auto_classify_threshold: 0.8,
        require_test_vectors: true,
        max_entries_per_review: 100,
        track_implementation: true,
    }
}

pub fn validate_delta_review(
    entries: &[DeltaEntry],
    config: &DeltaReviewConfig,
) -> Result<(), Vec<DeltaReviewError>> {
    let mut errors = Vec::new();

    if entries.is_empty() {
        errors.push(DeltaReviewError::EmptyReview);
    }

    if entries.len() > config.max_entries_per_review {
        errors.push(DeltaReviewError::TooManyEntries {
            count: entries.len(),
            max: config.max_entries_per_review,
        });
    }

    let mut seen_pairs: HashMap<(u64, u64), usize> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.eip_number == 0 {
            errors.push(DeltaReviewError::InvalidEipNumber {
                eip: entry.eip_number,
            });
        }

        if entry.description.trim().is_empty() {
            errors.push(DeltaReviewError::MissingDescription { index });
        }

        let key = (entry.eip_number, entry.pr_number);
        if seen_pairs.contains_key(&key) {
            errors.push(DeltaReviewError::DuplicateEntry {
                eip: entry.eip_number,
                pr: entry.pr_number,
            });
        } else {
            seen_pairs.insert(key, index);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_review_stats(entries: &[DeltaEntry]) -> DeltaReviewStats {
    let mut category_counts: HashMap<ChangeCategory, usize> = HashMap::new();
    let mut impact_counts: HashMap<ImpactLevel, usize> = HashMap::new();

    let mut normative_count = 0usize;
    let mut breaking_count = 0usize;
    let mut security_critical_count = 0usize;
    let mut total_impact_score = 0u64;

    for entry in entries {
        *category_counts.entry(entry.category).or_insert(0) += 1;
        *impact_counts.entry(entry.impact).or_insert(0) += 1;

        if is_normative_category(entry.category) {
            normative_count += 1;
        }
        if entry.breaking {
            breaking_count += 1;
        }
        if entry.category == ChangeCategory::SecurityCritical {
            security_critical_count += 1;
        }
        total_impact_score += impact_score(entry.impact) as u64;
    }

    let mut category_distribution: Vec<(String, usize)> = category_counts
        .into_iter()
        .map(|(category, count)| (category_name(category).to_string(), count))
        .collect();
    category_distribution.sort_by(|left, right| left.0.cmp(&right.0));

    let mut impact_distribution: Vec<(String, usize)> = impact_counts
        .into_iter()
        .map(|(impact, count)| (impact_name(impact).to_string(), count))
        .collect();
    impact_distribution.sort_by(|left, right| left.0.cmp(&right.0));

    let avg_impact_score = if entries.is_empty() {
        0.0
    } else {
        total_impact_score as f64 / entries.len() as f64
    };

    DeltaReviewStats {
        total_entries: entries.len(),
        normative_count,
        breaking_count,
        security_critical_count,
        implementation_coverage: compute_implementation_coverage(entries),
        category_distribution,
        impact_distribution,
        avg_impact_score,
    }
}

pub fn classify_impact(
    category: ChangeCategory,
    breaking: bool,
    affected_sections: &[String],
) -> ImpactLevel {
    let touches_consensus_critical = affected_sections
        .iter()
        .any(|section| has_consensus_critical_keyword(section));
    let broad_surface_area = affected_sections.len() >= 4;

    match category {
        ChangeCategory::SecurityCritical => {
            if breaking || touches_consensus_critical {
                ImpactLevel::Critical
            } else {
                ImpactLevel::High
            }
        }
        ChangeCategory::GasSchedule => {
            if breaking || touches_consensus_critical {
                ImpactLevel::High
            } else {
                ImpactLevel::Medium
            }
        }
        ChangeCategory::Normative => {
            if breaking && (touches_consensus_critical || broad_surface_area) {
                ImpactLevel::Critical
            } else if breaking {
                ImpactLevel::High
            } else if touches_consensus_critical || broad_surface_area {
                ImpactLevel::Medium
            } else {
                ImpactLevel::Low
            }
        }
        ChangeCategory::Deprecation => {
            if breaking {
                ImpactLevel::High
            } else {
                ImpactLevel::Medium
            }
        }
        ChangeCategory::TestVector => {
            if breaking || touches_consensus_critical {
                ImpactLevel::Medium
            } else {
                ImpactLevel::Low
            }
        }
        ChangeCategory::Informational => {
            if breaking {
                ImpactLevel::Medium
            } else {
                ImpactLevel::Low
            }
        }
        ChangeCategory::Editorial => {
            if breaking {
                ImpactLevel::Low
            } else {
                ImpactLevel::None
            }
        }
    }
}

pub fn prioritize_entries(entries: &mut [DeltaEntry]) {
    entries.sort_by(|left, right| {
        impact_rank(right.impact)
            .cmp(&impact_rank(left.impact))
            .then_with(|| right.breaking.cmp(&left.breaking))
            .then_with(|| category_rank(left.category).cmp(&category_rank(right.category)))
            .then_with(|| left.eip_number.cmp(&right.eip_number))
            .then_with(|| left.pr_number.cmp(&right.pr_number))
            .then_with(|| left.description.cmp(&right.description))
    });
}

pub fn compute_implementation_coverage(entries: &[DeltaEntry]) -> f64 {
    let normative_entries: Vec<&DeltaEntry> = entries
        .iter()
        .filter(|entry| is_normative_category(entry.category))
        .collect();

    if normative_entries.is_empty() {
        return 1.0;
    }

    let covered = normative_entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.implementation_status,
                ImplementationStatus::Implemented | ImplementationStatus::NotApplicable
            )
        })
        .count();

    covered as f64 / normative_entries.len() as f64
}

pub fn compute_review_commitment(entries: &[DeltaEntry]) -> [u8; 32] {
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by(|left, right| {
        left.eip_number
            .cmp(&right.eip_number)
            .then_with(|| left.pr_number.cmp(&right.pr_number))
            .then_with(|| category_rank(left.category).cmp(&category_rank(right.category)))
            .then_with(|| impact_rank(left.impact).cmp(&impact_rank(right.impact)))
            .then_with(|| left.breaking.cmp(&right.breaking))
            .then_with(|| {
                implementation_status_rank(left.implementation_status)
                    .cmp(&implementation_status_rank(right.implementation_status))
            })
            .then_with(|| left.description.cmp(&right.description))
            .then_with(|| left.affected_sections.cmp(&right.affected_sections))
    });

    let mut hasher = Sha256::new();
    hasher.update((sorted_entries.len() as u64).to_be_bytes());

    for entry in sorted_entries {
        hasher.update(entry.eip_number.to_be_bytes());
        hasher.update(entry.pr_number.to_be_bytes());
        hasher.update([category_rank(entry.category)]);
        hasher.update([impact_rank(entry.impact)]);
        hasher.update([implementation_status_rank(entry.implementation_status)]);
        hasher.update([u8::from(entry.breaking)]);
        hash_string(&mut hasher, &entry.description);

        hasher.update((entry.affected_sections.len() as u64).to_be_bytes());
        for section in entry.affected_sections {
            hash_string(&mut hasher, &section);
        }
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn has_consensus_critical_keyword(section: &str) -> bool {
    let lower = section.to_ascii_lowercase();
    [
        "consensus",
        "fork",
        "state transition",
        "block validity",
        "execution",
        "validator",
        "signature",
        "finality",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn category_name(category: ChangeCategory) -> &'static str {
    match category {
        ChangeCategory::Normative => "Normative",
        ChangeCategory::Informational => "Informational",
        ChangeCategory::Editorial => "Editorial",
        ChangeCategory::TestVector => "TestVector",
        ChangeCategory::SecurityCritical => "SecurityCritical",
        ChangeCategory::GasSchedule => "GasSchedule",
        ChangeCategory::Deprecation => "Deprecation",
    }
}

fn impact_name(impact: ImpactLevel) -> &'static str {
    match impact {
        ImpactLevel::None => "None",
        ImpactLevel::Low => "Low",
        ImpactLevel::Medium => "Medium",
        ImpactLevel::High => "High",
        ImpactLevel::Critical => "Critical",
    }
}

fn impact_score(impact: ImpactLevel) -> u8 {
    match impact {
        ImpactLevel::None => 0,
        ImpactLevel::Low => 1,
        ImpactLevel::Medium => 2,
        ImpactLevel::High => 3,
        ImpactLevel::Critical => 4,
    }
}

fn is_normative_category(category: ChangeCategory) -> bool {
    matches!(
        category,
        ChangeCategory::Normative
            | ChangeCategory::SecurityCritical
            | ChangeCategory::GasSchedule
            | ChangeCategory::Deprecation
    )
}

fn category_rank(category: ChangeCategory) -> u8 {
    match category {
        ChangeCategory::SecurityCritical => 0,
        ChangeCategory::Normative => 1,
        ChangeCategory::GasSchedule => 2,
        ChangeCategory::Deprecation => 3,
        ChangeCategory::TestVector => 4,
        ChangeCategory::Informational => 5,
        ChangeCategory::Editorial => 6,
    }
}

fn impact_rank(impact: ImpactLevel) -> u8 {
    match impact {
        ImpactLevel::None => 0,
        ImpactLevel::Low => 1,
        ImpactLevel::Medium => 2,
        ImpactLevel::High => 3,
        ImpactLevel::Critical => 4,
    }
}

fn implementation_status_rank(status: ImplementationStatus) -> u8 {
    match status {
        ImplementationStatus::NotStarted => 0,
        ImplementationStatus::InProgress => 1,
        ImplementationStatus::NeedsReview => 2,
        ImplementationStatus::Implemented => 3,
        ImplementationStatus::NotApplicable => 4,
    }
}
