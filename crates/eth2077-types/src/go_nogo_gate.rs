//! Go/No-Go gate matrix and launch checklist signoff types for ETH2077.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateVerdict {
    Go,
    NoGo,
    ConditionalGo,
    Deferred,
}

impl GateVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::NoGo => "nogo",
            Self::ConditionalGo => "conditional-go",
            Self::Deferred => "deferred",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ChecklistCategory {
    Security,
    Performance,
    Consensus,
    Infrastructure,
    Governance,
    Testing,
}

impl ChecklistCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Consensus => "consensus",
            Self::Infrastructure => "infrastructure",
            Self::Governance => "governance",
            Self::Testing => "testing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignoffStatus {
    Pending,
    Approved,
    Rejected,
    Waived,
}

impl SignoffStatus {
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Pending | Self::Rejected)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Waived => "waived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Critical,
    High,
    Medium,
    Low,
    Negligible,
}

impl RiskLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Negligible => "negligible",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub category: ChecklistCategory,
    pub description: String,
    pub status: SignoffStatus,
    pub assignee: String,
    pub risk_if_skipped: RiskLevel,
    pub evidence_url: Option<String>,
    pub signed_off_at_unix: Option<u64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDecision {
    pub gate_name: String,
    pub verdict: GateVerdict,
    pub decided_at_unix: u64,
    pub decided_by: String,
    pub blocking_items: Vec<String>,
    pub conditions: Vec<String>,
    pub next_review_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchChecklist {
    pub id: String,
    pub name: String,
    pub target_network: String,
    pub items: Vec<ChecklistItem>,
    pub gates: Vec<GateDecision>,
    pub created_at_unix: u64,
    pub target_launch_unix: Option<u64>,
    pub final_verdict: Option<GateVerdict>,
}

impl LaunchChecklist {
    pub fn is_launch_ready(&self, config: &GoNogoGateConfig) -> bool {
        if !config.validate().is_empty() {
            return false;
        }

        if self.items.is_empty() || self.gates.is_empty() {
            return false;
        }

        if !final_verdict_allows_launch(self.final_verdict) {
            return false;
        }

        let stats = compute_stats(self);
        let (go_count, nogo_count, conditional_count, deferred_count) = gate_verdict_counts(self);

        if go_count < config.required_go_gates {
            return false;
        }
        if conditional_count > config.max_conditional_gates {
            return false;
        }
        if nogo_count > 0 || stats.nogo_gates > 0 {
            return false;
        }
        if deferred_count > 0 {
            return false;
        }
        if stats.critical_risks_open > config.max_critical_risks_open {
            return false;
        }
        if approved_signoff_count(self) < config.min_signoff_count {
            return false;
        }
        if !has_all_required_categories_approved(self, &config.required_categories) {
            return false;
        }
        if config.auto_nogo_on_security_fail && has_rejected_security_item(self) {
            return false;
        }

        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoNogoGateConfig {
    pub required_go_gates: usize,
    pub max_conditional_gates: usize,
    pub required_categories: Vec<ChecklistCategory>,
    pub min_signoff_count: usize,
    pub max_critical_risks_open: usize,
    pub review_cadence_hours: u64,
    pub auto_nogo_on_security_fail: bool,
}

impl Default for GoNogoGateConfig {
    fn default() -> Self {
        Self {
            required_go_gates: 5,
            max_conditional_gates: 2,
            required_categories: vec![
                ChecklistCategory::Security,
                ChecklistCategory::Performance,
                ChecklistCategory::Consensus,
                ChecklistCategory::Infrastructure,
                ChecklistCategory::Testing,
            ],
            min_signoff_count: 3,
            max_critical_risks_open: 0,
            review_cadence_hours: 24,
            auto_nogo_on_security_fail: true,
        }
    }
}

impl GoNogoGateConfig {
    pub fn validate(&self) -> Vec<GoNogoGateValidationError> {
        let mut errors = Vec::new();

        validate_required_go_gates(self, &mut errors);
        validate_conditional_gate_budget(self, &mut errors);
        validate_required_categories(self, &mut errors);
        validate_signoff_threshold(self, &mut errors);
        validate_review_cadence(self, &mut errors);
        validate_security_policy(self, &mut errors);

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoNogoGateValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoNogoGateStats {
    pub total_items: usize,
    pub approved_items: usize,
    pub rejected_items: usize,
    pub pending_items: usize,
    pub go_gates: usize,
    pub nogo_gates: usize,
    pub critical_risks_open: usize,
    pub approval_rate: f64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, Copy)]
struct GoNogoDerivedStats {
    total_items: usize,
    approved_items: usize,
    rejected_items: usize,
    pending_items: usize,
    go_gates: usize,
    nogo_gates: usize,
    critical_risks_open: usize,
    approval_rate: f64,
}

pub fn compute_stats(checklist: &LaunchChecklist) -> GoNogoGateStats {
    let total_items = checklist.items.len();
    let approved_items = checklist
        .items
        .iter()
        .filter(|item| item.status == SignoffStatus::Approved)
        .count();
    let rejected_items = checklist
        .items
        .iter()
        .filter(|item| item.status == SignoffStatus::Rejected)
        .count();
    let pending_items = checklist
        .items
        .iter()
        .filter(|item| item.status == SignoffStatus::Pending)
        .count();

    let go_gates = checklist
        .gates
        .iter()
        .filter(|gate| gate.verdict == GateVerdict::Go)
        .count();
    let nogo_gates = checklist
        .gates
        .iter()
        .filter(|gate| gate.verdict == GateVerdict::NoGo)
        .count();

    let critical_risks_open = checklist
        .items
        .iter()
        .filter(|item| item.risk_if_skipped == RiskLevel::Critical && item.status.is_open())
        .count();

    let approval_rate = if total_items == 0 {
        0.0
    } else {
        approved_items as f64 / total_items as f64
    };

    let derived = GoNogoDerivedStats {
        total_items,
        approved_items,
        rejected_items,
        pending_items,
        go_gates,
        nogo_gates,
        critical_risks_open,
        approval_rate,
    };
    let commitment = compute_commitment(checklist, &derived);

    GoNogoGateStats {
        total_items: derived.total_items,
        approved_items: derived.approved_items,
        rejected_items: derived.rejected_items,
        pending_items: derived.pending_items,
        go_gates: derived.go_gates,
        nogo_gates: derived.nogo_gates,
        critical_risks_open: derived.critical_risks_open,
        approval_rate: derived.approval_rate,
        commitment,
    }
}

fn validate_required_go_gates(
    config: &GoNogoGateConfig,
    errors: &mut Vec<GoNogoGateValidationError>,
) {
    if config.required_go_gates == 0 {
        push_error(
            errors,
            "required_go_gates",
            "required_go_gates must be greater than zero",
        );
    }
}

fn validate_conditional_gate_budget(
    config: &GoNogoGateConfig,
    errors: &mut Vec<GoNogoGateValidationError>,
) {
    if config.max_conditional_gates > config.required_go_gates {
        push_error(
            errors,
            "max_conditional_gates",
            "max_conditional_gates must be less than or equal to required_go_gates",
        );
    }
}

fn validate_required_categories(
    config: &GoNogoGateConfig,
    errors: &mut Vec<GoNogoGateValidationError>,
) {
    if config.required_categories.is_empty() {
        push_error(
            errors,
            "required_categories",
            "required_categories must include at least one category",
        );
        return;
    }

    let mut seen = BTreeSet::new();
    let mut has_duplicates = false;
    for category in &config.required_categories {
        if !seen.insert(*category) {
            has_duplicates = true;
            break;
        }
    }

    if has_duplicates {
        push_error(
            errors,
            "required_categories",
            "required_categories contains duplicate categories",
        );
    }
}

fn validate_signoff_threshold(
    config: &GoNogoGateConfig,
    errors: &mut Vec<GoNogoGateValidationError>,
) {
    if config.min_signoff_count == 0 {
        push_error(
            errors,
            "min_signoff_count",
            "min_signoff_count must be greater than zero",
        );
    }
}

fn validate_review_cadence(config: &GoNogoGateConfig, errors: &mut Vec<GoNogoGateValidationError>) {
    if config.review_cadence_hours == 0 {
        push_error(
            errors,
            "review_cadence_hours",
            "review_cadence_hours must be greater than zero",
        );
    }

    if config.review_cadence_hours > 24 * 30 {
        push_error(
            errors,
            "review_cadence_hours",
            "review_cadence_hours must be <= 720 hours (30 days)",
        );
    }
}

fn validate_security_policy(
    config: &GoNogoGateConfig,
    errors: &mut Vec<GoNogoGateValidationError>,
) {
    if config.auto_nogo_on_security_fail
        && !config
            .required_categories
            .contains(&ChecklistCategory::Security)
    {
        push_error(
            errors,
            "required_categories",
            "required_categories must include Security when auto_nogo_on_security_fail is true",
        );
    }
}

fn push_error(errors: &mut Vec<GoNogoGateValidationError>, field: &str, message: &str) {
    errors.push(GoNogoGateValidationError {
        field: field.to_string(),
        message: message.to_string(),
    });
}

fn gate_verdict_counts(checklist: &LaunchChecklist) -> (usize, usize, usize, usize) {
    let mut go = 0;
    let mut nogo = 0;
    let mut conditional = 0;
    let mut deferred = 0;

    for gate in &checklist.gates {
        if gate.verdict == GateVerdict::Go {
            go += 1;
            continue;
        }
        if gate.verdict == GateVerdict::NoGo {
            nogo += 1;
            continue;
        }
        if gate.verdict == GateVerdict::ConditionalGo {
            conditional += 1;
            continue;
        }
        if gate.verdict == GateVerdict::Deferred {
            deferred += 1;
        }
    }

    (go, nogo, conditional, deferred)
}

fn approved_signoff_count(checklist: &LaunchChecklist) -> usize {
    let mut signers = BTreeSet::new();
    for item in &checklist.items {
        if item.status != SignoffStatus::Approved {
            continue;
        }
        let signer = item.assignee.trim();
        if signer.is_empty() {
            continue;
        }
        signers.insert(signer.to_string());
    }
    signers.len()
}

fn has_all_required_categories_approved(
    checklist: &LaunchChecklist,
    required_categories: &[ChecklistCategory],
) -> bool {
    let mut approved_categories = BTreeSet::new();

    for item in &checklist.items {
        if item.status == SignoffStatus::Approved {
            approved_categories.insert(item.category);
        }
    }

    required_categories
        .iter()
        .all(|category| approved_categories.contains(category))
}

fn has_rejected_security_item(checklist: &LaunchChecklist) -> bool {
    checklist.items.iter().any(|item| {
        item.category == ChecklistCategory::Security && item.status == SignoffStatus::Rejected
    })
}

fn final_verdict_allows_launch(verdict: Option<GateVerdict>) -> bool {
    !matches!(verdict, Some(GateVerdict::NoGo | GateVerdict::Deferred))
}

fn compute_commitment(checklist: &LaunchChecklist, stats: &GoNogoDerivedStats) -> [u8; 32] {
    let mut item_signatures = checklist
        .items
        .iter()
        .map(normalized_item_signature)
        .collect::<Vec<_>>();
    item_signatures.sort_unstable();

    let mut gate_signatures = checklist
        .gates
        .iter()
        .map(normalized_gate_signature)
        .collect::<Vec<_>>();
    gate_signatures.sort_unstable();

    let payload = CommitmentPayload {
        checklist_id: checklist.id.clone(),
        checklist_name: checklist.name.clone(),
        target_network: checklist.target_network.clone(),
        created_at_unix: checklist.created_at_unix,
        target_launch_unix: checklist.target_launch_unix,
        final_verdict: checklist.final_verdict.map(GateVerdict::as_str),
        total_items: stats.total_items,
        approved_items: stats.approved_items,
        rejected_items: stats.rejected_items,
        pending_items: stats.pending_items,
        go_gates: stats.go_gates,
        nogo_gates: stats.nogo_gates,
        critical_risks_open: stats.critical_risks_open,
        approval_rate: stats.approval_rate,
        item_signatures,
        gate_signatures,
    };

    let encoded = serde_json::to_vec(&payload)
        .expect("serializing go/no-go commitment payload should not fail");

    let mut hasher = Sha256::new();
    hasher.update(encoded);
    hasher.finalize().into()
}

fn normalized_item_signature(item: &ChecklistItem) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        item.id,
        item.category.as_str(),
        item.description,
        item.status.as_str(),
        item.assignee,
        item.risk_if_skipped.as_str(),
        item.evidence_url.as_deref().unwrap_or(""),
        option_u64_to_string(item.signed_off_at_unix),
        item.notes.as_deref().unwrap_or(""),
    )
}

fn normalized_gate_signature(gate: &GateDecision) -> String {
    let mut blocking_items = gate.blocking_items.clone();
    let mut conditions = gate.conditions.clone();
    blocking_items.sort_unstable();
    conditions.sort_unstable();

    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        gate.gate_name,
        gate.verdict.as_str(),
        gate.decided_at_unix,
        gate.decided_by,
        blocking_items.join(","),
        conditions.join(","),
        option_u64_to_string(gate.next_review_unix),
    )
}

fn option_u64_to_string(value: Option<u64>) -> String {
    value.map_or_else(String::new, |v| v.to_string())
}

#[derive(Debug, Clone, Serialize)]
struct CommitmentPayload {
    checklist_id: String,
    checklist_name: String,
    target_network: String,
    created_at_unix: u64,
    target_launch_unix: Option<u64>,
    final_verdict: Option<&'static str>,
    total_items: usize,
    approved_items: usize,
    rejected_items: usize,
    pending_items: usize,
    go_gates: usize,
    nogo_gates: usize,
    critical_risks_open: usize,
    approval_rate: f64,
    item_signatures: Vec<String>,
    gate_signatures: Vec<String>,
}
