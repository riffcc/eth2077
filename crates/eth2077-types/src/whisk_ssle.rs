use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LeaderElectionMode {
    Deterministic,  // Current RANDAO-based
    WhiskSSLE,      // Full Whisk secret leader election
    PartialWhisk,   // Whisk for proposers only, not attesters
    CommitteeWhisk, // Whisk applied to committee selection too
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WhiskPhase {
    ShufflingPhase,   // Trackers being shuffled
    SelectionGap,     // Gap before selection (PROPOSER_SELECTION_GAP)
    ProposerRevealed, // Proposer opens their commitment
    Idle,             // Between shuffling phases
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhiskConfig {
    pub mode: LeaderElectionMode,
    pub candidate_trackers_count: usize, // e.g. 16384
    pub proposer_trackers_count: usize,  // e.g. 32
    pub validators_per_shuffle: usize,   // e.g. 128
    pub epochs_per_shuffling_phase: u64, // e.g. 256
    pub proposer_selection_gap: u64,     // e.g. 2 epochs
    pub curdleproofs_n_blinders: usize,  // e.g. 4
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WhiskValidationError {
    ZeroCandidateTrackers,
    ZeroProposerTrackers,
    ProposerExceedsCandidates {
        proposers: usize,
        candidates: usize,
    },
    ZeroValidatorsPerShuffle,
    ShuffleLargerThanCandidates {
        shuffle_size: usize,
        candidates: usize,
    },
    SelectionGapTooLarge {
        gap: u64,
        shuffling_epochs: u64,
    },
    IncompatibleMode {
        mode: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhiskImpactStats {
    pub dos_resistance_improvement: f64,   // multiplier vs current
    pub shuffling_overhead_per_epoch: f64, // relative cost
    pub proof_size_bytes: usize,
    pub selection_entropy_bits: f64,
    pub latency_overhead_ms: f64,
    pub mode_comparison: Vec<(String, f64)>, // mode name -> DoS resistance score
    pub validator_anonymity_set: usize,
}

fn mode_name(mode: LeaderElectionMode) -> &'static str {
    match mode {
        LeaderElectionMode::Deterministic => "Deterministic",
        LeaderElectionMode::WhiskSSLE => "WhiskSSLE",
        LeaderElectionMode::PartialWhisk => "PartialWhisk",
        LeaderElectionMode::CommitteeWhisk => "CommitteeWhisk",
    }
}

fn mode_security_multiplier(mode: LeaderElectionMode) -> f64 {
    match mode {
        LeaderElectionMode::Deterministic => 1.0,
        LeaderElectionMode::PartialWhisk => 0.85,
        LeaderElectionMode::WhiskSSLE => 1.0,
        LeaderElectionMode::CommitteeWhisk => 1.2,
    }
}

fn mode_overhead_multiplier(mode: LeaderElectionMode) -> f64 {
    match mode {
        LeaderElectionMode::Deterministic => 0.0,
        LeaderElectionMode::PartialWhisk => 0.7,
        LeaderElectionMode::WhiskSSLE => 1.0,
        LeaderElectionMode::CommitteeWhisk => 1.35,
    }
}

fn is_whisk_enabled(mode: LeaderElectionMode) -> bool {
    !matches!(mode, LeaderElectionMode::Deterministic)
}

fn safe_log2(value: usize) -> f64 {
    (value.max(1) as f64).log2()
}

fn shuffle_density(config: &WhiskConfig) -> f64 {
    if config.candidate_trackers_count == 0 {
        0.0
    } else {
        config.validators_per_shuffle as f64 / config.candidate_trackers_count as f64
    }
}

pub fn default_whisk_config() -> WhiskConfig {
    WhiskConfig {
        mode: LeaderElectionMode::WhiskSSLE,
        candidate_trackers_count: 16_384,
        proposer_trackers_count: 32,
        validators_per_shuffle: 128,
        epochs_per_shuffling_phase: 256,
        proposer_selection_gap: 2,
        curdleproofs_n_blinders: 4,
    }
}

pub fn validate_whisk_config(config: &WhiskConfig) -> Result<(), Vec<WhiskValidationError>> {
    let mut errors = Vec::new();

    if config.candidate_trackers_count == 0 {
        errors.push(WhiskValidationError::ZeroCandidateTrackers);
    }

    if config.proposer_trackers_count == 0 {
        errors.push(WhiskValidationError::ZeroProposerTrackers);
    }

    if config.proposer_trackers_count > config.candidate_trackers_count {
        errors.push(WhiskValidationError::ProposerExceedsCandidates {
            proposers: config.proposer_trackers_count,
            candidates: config.candidate_trackers_count,
        });
    }

    if config.validators_per_shuffle == 0 {
        errors.push(WhiskValidationError::ZeroValidatorsPerShuffle);
    }

    if config.validators_per_shuffle > config.candidate_trackers_count {
        errors.push(WhiskValidationError::ShuffleLargerThanCandidates {
            shuffle_size: config.validators_per_shuffle,
            candidates: config.candidate_trackers_count,
        });
    }

    if config.proposer_selection_gap >= config.epochs_per_shuffling_phase {
        errors.push(WhiskValidationError::SelectionGapTooLarge {
            gap: config.proposer_selection_gap,
            shuffling_epochs: config.epochs_per_shuffling_phase,
        });
    }

    if matches!(config.mode, LeaderElectionMode::Deterministic)
        && (config.proposer_selection_gap > 0 || config.curdleproofs_n_blinders > 0)
    {
        errors.push(WhiskValidationError::IncompatibleMode {
            mode:
                "Deterministic mode requires proposer_selection_gap=0 and curdleproofs_n_blinders=0"
                    .to_string(),
        });
    }

    if is_whisk_enabled(config.mode) && config.curdleproofs_n_blinders == 0 {
        errors.push(WhiskValidationError::IncompatibleMode {
            mode: format!(
                "{} mode requires curdleproofs_n_blinders > 0",
                mode_name(config.mode)
            ),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_whisk_stats(config: &WhiskConfig) -> WhiskImpactStats {
    let dos_resistance_improvement = estimate_dos_resistance(config);
    let proof_size_bytes = estimate_proof_overhead(config);
    let validator_anonymity_set = compute_anonymity_set(config).max(1);
    let mode_comparison = compare_election_modes(config);

    let entropy_bonus = if is_whisk_enabled(config.mode) {
        (config.curdleproofs_n_blinders as f64 + 1.0).log2()
    } else {
        0.0
    };
    let selection_entropy_bits = safe_log2(validator_anonymity_set) + entropy_bonus;

    let shuffling_overhead_per_epoch = if matches!(config.mode, LeaderElectionMode::Deterministic) {
        0.0
    } else {
        let proof_term = proof_size_bytes as f64 / 1_000_000.0;
        let cadence_term = 1.0 / config.epochs_per_shuffling_phase.max(1) as f64;
        let base = shuffle_density(config) * 0.8 + proof_term * 1.1 + cadence_term * 0.25;
        base * mode_overhead_multiplier(config.mode)
    };

    let latency_overhead_ms = if matches!(config.mode, LeaderElectionMode::Deterministic) {
        0.0
    } else {
        let proof_verification_ms = (proof_size_bytes as f64 / 1024.0)
            * (0.15 + config.curdleproofs_n_blinders as f64 * 0.03);
        let gap_cost_ms = config.proposer_selection_gap as f64 * 3.5;
        let scheduler_cost_ms = shuffle_density(config) * 20.0;
        (proof_verification_ms + gap_cost_ms + scheduler_cost_ms)
            * mode_overhead_multiplier(config.mode)
    };

    WhiskImpactStats {
        dos_resistance_improvement,
        shuffling_overhead_per_epoch,
        proof_size_bytes,
        selection_entropy_bits,
        latency_overhead_ms,
        mode_comparison,
        validator_anonymity_set,
    }
}

pub fn estimate_dos_resistance(config: &WhiskConfig) -> f64 {
    if matches!(config.mode, LeaderElectionMode::Deterministic) {
        return 1.0;
    }

    let anonymity = compute_anonymity_set(config).max(1) as f64;
    let entropy_bits = anonymity.log2();
    let shuffle_effect = 1.0 + shuffle_density(config).clamp(0.0, 1.0) * 0.75;
    let phase_stability = 1.0 + (config.epochs_per_shuffling_phase.max(1) as f64).sqrt() / 20.0;
    let gap_penalty = 1.0 / (1.0 + config.proposer_selection_gap as f64 * 0.08);

    (1.0 + entropy_bits / 4.0)
        * shuffle_effect
        * phase_stability
        * gap_penalty
        * mode_security_multiplier(config.mode)
}

pub fn estimate_proof_overhead(config: &WhiskConfig) -> usize {
    if matches!(config.mode, LeaderElectionMode::Deterministic) {
        return 0;
    }

    let shuffle_statement_bytes = config.validators_per_shuffle.saturating_mul(32);
    let permutation_commitments_bytes = config.validators_per_shuffle.saturating_mul(32);
    let curdle_elements = config
        .validators_per_shuffle
        .saturating_div(2)
        .saturating_add(config.curdleproofs_n_blinders.saturating_mul(2));
    let curdleproofs_bytes = curdle_elements.saturating_mul(48);
    let k_commitments_bytes = config.proposer_trackers_count.saturating_mul(48);
    let opening_proofs_bytes = config
        .proposer_trackers_count
        .saturating_mul(32usize.saturating_add(config.curdleproofs_n_blinders.saturating_mul(8)));

    let base = shuffle_statement_bytes
        .saturating_add(permutation_commitments_bytes)
        .saturating_add(curdleproofs_bytes)
        .saturating_add(k_commitments_bytes)
        .saturating_add(opening_proofs_bytes);

    (base as f64 * mode_overhead_multiplier(config.mode)).round() as usize
}

pub fn compare_election_modes(config: &WhiskConfig) -> Vec<(String, f64)> {
    let modes = [
        LeaderElectionMode::Deterministic,
        LeaderElectionMode::WhiskSSLE,
        LeaderElectionMode::PartialWhisk,
        LeaderElectionMode::CommitteeWhisk,
    ];

    let mut weighting = HashMap::new();
    weighting.insert(LeaderElectionMode::Deterministic, 1.0);
    weighting.insert(LeaderElectionMode::WhiskSSLE, 1.0);
    weighting.insert(LeaderElectionMode::PartialWhisk, 0.95);
    weighting.insert(LeaderElectionMode::CommitteeWhisk, 1.05);

    let mut comparison = Vec::with_capacity(modes.len());
    for mode in modes {
        let mut variant = config.clone();
        variant.mode = mode;
        let base_score = estimate_dos_resistance(&variant);
        let weighted_score = base_score * weighting.get(&mode).copied().unwrap_or(1.0);
        comparison.push((mode_name(mode).to_string(), weighted_score));
    }

    comparison
}

pub fn compute_anonymity_set(config: &WhiskConfig) -> usize {
    let candidates = config.candidate_trackers_count.max(1);
    let proposers = config.proposer_trackers_count.max(1);
    let shuffle = config.validators_per_shuffle.max(1);

    match config.mode {
        LeaderElectionMode::Deterministic => 1,
        LeaderElectionMode::WhiskSSLE => candidates,
        LeaderElectionMode::PartialWhisk => candidates.saturating_div(4).saturating_add(proposers),
        LeaderElectionMode::CommitteeWhisk => {
            candidates.saturating_add(shuffle.saturating_mul(proposers))
        }
    }
}

pub fn compute_whisk_commitment(trackers: &[[u8; 32]]) -> [u8; 32] {
    let mut sorted = trackers.to_vec();
    sorted.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update((sorted.len() as u64).to_be_bytes());
    for tracker in sorted {
        hasher.update(tracker);
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}
