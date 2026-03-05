use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CitadelModule {
    VoteAccumulator,
    ForkChoice,
    Finalization,
    SlashingDetector,
    AttestationPool,
    CommitteeShuffler,
    BlockProduction,
    StateTransition,
    EpochProcessing,
    SyncProtocol,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaceholderKind {
    Axiom,
    Sorry,
    Todo,
    Unimplemented,
    Stub,
    MockReturn,
    HardcodedValue,
    SkippedValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProofStatus {
    NotStarted,
    Sketched,
    PartialProof,
    FullProofDraft,
    FullProofReviewed,
    MachineChecked,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationPhase {
    Analysis,
    InterfaceDesign,
    CorePort,
    TestMigration,
    PlaceholderElimination,
    ProofCompletion,
    IntegrationTest,
    ProductionReady,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaceholderEntry {
    pub location: String,
    pub kind: PlaceholderKind,
    pub description: String,
    pub blocking_proofs: Vec<String>,
    pub severity: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleMigration {
    pub module: CitadelModule,
    pub phase: MigrationPhase,
    pub placeholders: Vec<PlaceholderEntry>,
    pub proof_status: ProofStatus,
    pub lines_ported: usize,
    pub lines_remaining: usize,
    pub dependencies: Vec<CitadelModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitadelPortConfig {
    pub target_module: CitadelModule,
    pub migrations: Vec<ModuleMigration>,
    pub max_allowed_placeholders: usize,
    pub require_machine_checked: bool,
    pub max_severity_allowed: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CitadelPortValidationError {
    EmptyMigrations,
    TooManyPlaceholders { count: usize, max: usize },
    HighSeverityPlaceholder { location: String, severity: u8 },
    CyclicDependency { modules: Vec<String> },
    ProofIncomplete { module: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CitadelPortStats {
    pub total_modules: usize,
    pub modules_complete: usize,
    pub total_placeholders: usize,
    pub critical_placeholders: usize,
    pub proof_coverage: f64,
    pub migration_progress: f64,
    pub estimated_remaining_work: f64,
    pub blocking_issues: Vec<String>,
}

fn phase_index(phase: &MigrationPhase) -> usize {
    match phase {
        MigrationPhase::Analysis => 0,
        MigrationPhase::InterfaceDesign => 1,
        MigrationPhase::CorePort => 2,
        MigrationPhase::TestMigration => 3,
        MigrationPhase::PlaceholderElimination => 4,
        MigrationPhase::ProofCompletion => 5,
        MigrationPhase::IntegrationTest => 6,
        MigrationPhase::ProductionReady => 7,
    }
}

fn proof_score(status: &ProofStatus) -> f64 {
    match status {
        ProofStatus::NotStarted => 0.0,
        ProofStatus::Sketched => 0.15,
        ProofStatus::PartialProof => 0.35,
        ProofStatus::FullProofDraft => 0.55,
        ProofStatus::FullProofReviewed => 0.75,
        ProofStatus::MachineChecked => 1.0,
        ProofStatus::Rejected => 0.0,
    }
}

fn find_dependency_cycle(migrations: &[ModuleMigration]) -> Option<Vec<String>> {
    let mut adjacency = Vec::with_capacity(migrations.len());
    for migration in migrations {
        let mut deps = Vec::new();
        for dependency in &migration.dependencies {
            if let Some(index) = migrations
                .iter()
                .position(|candidate| candidate.module == *dependency)
            {
                deps.push(index);
            }
        }
        adjacency.push(deps);
    }

    fn dfs(
        node: usize,
        adjacency: &[Vec<usize>],
        state: &mut [u8],
        stack: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        if state[node] == 1 {
            let start = stack.iter().position(|&entry| entry == node).unwrap_or(0);
            let mut cycle = stack[start..].to_vec();
            cycle.push(node);
            return Some(cycle);
        }

        if state[node] == 2 {
            return None;
        }

        state[node] = 1;
        stack.push(node);

        for &next in &adjacency[node] {
            if let Some(cycle) = dfs(next, adjacency, state, stack) {
                return Some(cycle);
            }
        }

        stack.pop();
        state[node] = 2;
        None
    }

    let mut state = vec![0_u8; migrations.len()];
    let mut stack = Vec::new();
    for index in 0..migrations.len() {
        if state[index] == 0 {
            if let Some(cycle) = dfs(index, &adjacency, &mut state, &mut stack) {
                return Some(
                    cycle
                        .into_iter()
                        .map(|entry| format!("{:?}", migrations[entry].module))
                        .collect(),
                );
            }
        }
    }

    None
}

pub fn default_citadel_port_config() -> CitadelPortConfig {
    CitadelPortConfig {
        target_module: CitadelModule::VoteAccumulator,
        migrations: vec![
            ModuleMigration {
                module: CitadelModule::VoteAccumulator,
                phase: MigrationPhase::PlaceholderElimination,
                placeholders: vec![
                    PlaceholderEntry {
                        location: "citadel/vote_accumulator.rs:142".to_string(),
                        kind: PlaceholderKind::Todo,
                        description: "Finalize tie-break logic for equal-weight votes".to_string(),
                        blocking_proofs: vec!["vote_accumulator_safety".to_string()],
                        severity: 3,
                    },
                    PlaceholderEntry {
                        location: "citadel/vote_accumulator.rs:219".to_string(),
                        kind: PlaceholderKind::SkippedValidation,
                        description: "Temporary bypass for malformed quorum witness edge cases"
                            .to_string(),
                        blocking_proofs: vec![
                            "quorum_witness_soundness".to_string(),
                            "fork_choice_monotonicity".to_string(),
                        ],
                        severity: 4,
                    },
                ],
                proof_status: ProofStatus::FullProofReviewed,
                lines_ported: 980,
                lines_remaining: 160,
                dependencies: vec![],
            },
            ModuleMigration {
                module: CitadelModule::ForkChoice,
                phase: MigrationPhase::CorePort,
                placeholders: vec![PlaceholderEntry {
                    location: "citadel/fork_choice.rs:87".to_string(),
                    kind: PlaceholderKind::Stub,
                    description: "Temporary stub for delayed attestations scoring".to_string(),
                    blocking_proofs: vec!["fork_choice_liveness".to_string()],
                    severity: 2,
                }],
                proof_status: ProofStatus::PartialProof,
                lines_ported: 530,
                lines_remaining: 420,
                dependencies: vec![CitadelModule::VoteAccumulator],
            },
            ModuleMigration {
                module: CitadelModule::Finalization,
                phase: MigrationPhase::ProductionReady,
                placeholders: vec![],
                proof_status: ProofStatus::MachineChecked,
                lines_ported: 1250,
                lines_remaining: 0,
                dependencies: vec![CitadelModule::VoteAccumulator, CitadelModule::ForkChoice],
            },
            ModuleMigration {
                module: CitadelModule::SlashingDetector,
                phase: MigrationPhase::TestMigration,
                placeholders: vec![PlaceholderEntry {
                    location: "citadel/slashing_detector.rs:301".to_string(),
                    kind: PlaceholderKind::HardcodedValue,
                    description: "Hardcoded replay window pending benchmark calibration"
                        .to_string(),
                    blocking_proofs: vec!["slashing_completeness".to_string()],
                    severity: 4,
                }],
                proof_status: ProofStatus::Sketched,
                lines_ported: 360,
                lines_remaining: 560,
                dependencies: vec![CitadelModule::VoteAccumulator],
            },
        ],
        max_allowed_placeholders: 8,
        require_machine_checked: false,
        max_severity_allowed: 4,
    }
}

pub fn validate_citadel_port(
    config: &CitadelPortConfig,
) -> Result<(), Vec<CitadelPortValidationError>> {
    let mut errors = Vec::new();

    if config.migrations.is_empty() {
        errors.push(CitadelPortValidationError::EmptyMigrations);
    }

    let total_placeholders = config
        .migrations
        .iter()
        .map(|migration| migration.placeholders.len())
        .sum::<usize>();

    if total_placeholders > config.max_allowed_placeholders {
        errors.push(CitadelPortValidationError::TooManyPlaceholders {
            count: total_placeholders,
            max: config.max_allowed_placeholders,
        });
    }

    for migration in &config.migrations {
        for placeholder in &migration.placeholders {
            if placeholder.severity > config.max_severity_allowed {
                errors.push(CitadelPortValidationError::HighSeverityPlaceholder {
                    location: placeholder.location.clone(),
                    severity: placeholder.severity,
                });
            }
        }
    }

    if let Some(cycle) = find_dependency_cycle(&config.migrations) {
        errors.push(CitadelPortValidationError::CyclicDependency { modules: cycle });
    }

    if config.require_machine_checked {
        for migration in &config.migrations {
            if migration.proof_status != ProofStatus::MachineChecked {
                errors.push(CitadelPortValidationError::ProofIncomplete {
                    module: format!("{:?}", migration.module),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_citadel_port_stats(config: &CitadelPortConfig) -> CitadelPortStats {
    let total_modules = config.migrations.len();
    let modules_complete = config
        .migrations
        .iter()
        .filter(|migration| {
            migration.phase == MigrationPhase::ProductionReady
                && migration.placeholders.is_empty()
                && migration.proof_status == ProofStatus::MachineChecked
        })
        .count();

    let total_placeholders = config
        .migrations
        .iter()
        .map(|migration| migration.placeholders.len())
        .sum();

    let critical_placeholders = config
        .migrations
        .iter()
        .flat_map(|migration| migration.placeholders.iter())
        .filter(|placeholder| placeholder.severity >= 4)
        .count();

    let proof_coverage = if total_modules == 0 {
        0.0
    } else {
        config
            .migrations
            .iter()
            .map(|migration| proof_score(&migration.proof_status))
            .sum::<f64>()
            / total_modules as f64
    };

    let migration_progress = if total_modules == 0 {
        0.0
    } else {
        config
            .migrations
            .iter()
            .map(|migration| phase_index(&migration.phase) as f64 / 7.0)
            .sum::<f64>()
            / total_modules as f64
    };

    let estimated_remaining_work = config
        .migrations
        .iter()
        .map(|migration| {
            let placeholder_penalty = migration
                .placeholders
                .iter()
                .map(|placeholder| 25.0 + placeholder.severity as f64 * 10.0)
                .sum::<f64>();
            migration.lines_remaining as f64 + placeholder_penalty
        })
        .sum::<f64>();

    let mut blocking_issues = Vec::new();
    for migration in &config.migrations {
        if migration.proof_status == ProofStatus::Rejected {
            blocking_issues.push(format!(
                "Rejected proof status for module {:?}",
                migration.module
            ));
        }
        if migration.phase == MigrationPhase::ProductionReady && migration.lines_remaining > 0 {
            blocking_issues.push(format!(
                "Module {:?} marked ProductionReady with {} lines remaining",
                migration.module, migration.lines_remaining
            ));
        }
        for placeholder in &migration.placeholders {
            if placeholder.severity >= 4 {
                blocking_issues.push(format!(
                    "High-severity placeholder ({}): {}",
                    placeholder.severity, placeholder.location
                ));
            }
        }
    }

    if let Some(cycle) = find_dependency_cycle(&config.migrations) {
        blocking_issues.push(format!(
            "Cyclic dependency detected: {}",
            cycle.join(" -> ")
        ));
    }

    CitadelPortStats {
        total_modules,
        modules_complete,
        total_placeholders,
        critical_placeholders,
        proof_coverage,
        migration_progress,
        estimated_remaining_work,
        blocking_issues,
    }
}

pub fn compute_citadel_port_commitment(config: &CitadelPortConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::CITADEL_PORT::V1");
    let serialized = serde_json::to_vec(config).expect("citadel port config must serialize");
    hasher.update(serialized);

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}
