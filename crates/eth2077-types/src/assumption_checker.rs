use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::theorem_registry::TheoremRegistry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assumption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub threat_model_anchor: Option<String>,
    pub referenced_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssumptionRegistry {
    pub assumptions: Vec<Assumption>,
}

impl AssumptionRegistry {
    pub fn load_from_json(path: impl AsRef<Path>) -> io::Result<Self> {
        let json = fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn save_to_json(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(path, json)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssumptionCheckResult {
    Ok,
    MissingAssumptions(Vec<String>),
    OrphanedAssumptions(Vec<String>),
    BrokenAnchors(Vec<String>),
}

pub fn check_assumption_coverage(
    theorems: &TheoremRegistry,
    assumptions: &AssumptionRegistry,
) -> Vec<AssumptionCheckResult> {
    let known_assumptions: HashSet<&str> = assumptions
        .assumptions
        .iter()
        .map(|assumption| assumption.id.as_str())
        .collect();

    let mut referenced_assumptions = HashSet::new();
    let mut missing_assumptions = HashSet::new();

    for theorem in &theorems.entries {
        for tag in &theorem.tags {
            if let Some(assumption_id) = tag.strip_prefix("assumption:") {
                let assumption_id = assumption_id.trim();

                if assumption_id.is_empty() || !known_assumptions.contains(assumption_id) {
                    missing_assumptions.insert(assumption_id.to_string());
                } else {
                    referenced_assumptions.insert(assumption_id.to_string());
                }
            }
        }
    }

    let mut orphaned_assumptions: Vec<String> = assumptions
        .assumptions
        .iter()
        .filter(|assumption| !referenced_assumptions.contains(&assumption.id))
        .map(|assumption| assumption.id.clone())
        .collect();
    orphaned_assumptions.sort();

    let mut broken_anchors: Vec<String> = assumptions
        .assumptions
        .iter()
        .filter_map(|assumption| {
            assumption
                .threat_model_anchor
                .as_ref()
                .filter(|anchor| anchor.trim().is_empty())
                .map(|_| assumption.id.clone())
        })
        .collect();
    broken_anchors.sort();

    let mut missing_assumptions: Vec<String> = missing_assumptions.into_iter().collect();
    missing_assumptions.sort();

    let mut results = Vec::new();

    if !missing_assumptions.is_empty() {
        results.push(AssumptionCheckResult::MissingAssumptions(missing_assumptions));
    }

    if !orphaned_assumptions.is_empty() {
        results.push(AssumptionCheckResult::OrphanedAssumptions(
            orphaned_assumptions,
        ));
    }

    if !broken_anchors.is_empty() {
        results.push(AssumptionCheckResult::BrokenAnchors(broken_anchors));
    }

    if results.is_empty() {
        vec![AssumptionCheckResult::Ok]
    } else {
        results
    }
}
