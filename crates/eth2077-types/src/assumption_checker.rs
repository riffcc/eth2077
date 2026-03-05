use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::theorem_registry::TheoremRegistry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationMethod {
    FormalProof,
    UnitTest,
    IntegrationTest,
    ManualReview,
    ExternalAudit,
    MonitoringAlert,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssumptionStatus {
    Active,
    UnderReview,
    Deprecated,
    Violated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assumption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub threat_model_anchor: Option<String>,
    pub referenced_by: Vec<String>,
    pub owner: String,
    pub validation_method: ValidationMethod,
    pub revisit_cadence_days: u32,
    pub last_reviewed: Option<String>,
    pub status: AssumptionStatus,
    pub mitigations: Vec<String>,
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

    pub fn get_by_id(&self, id: &str) -> Option<&Assumption> {
        self.assumptions.iter().find(|a| a.id == id)
    }

    pub fn get_overdue_reviews(&self, today: &str) -> Vec<&Assumption> {
        let Some(today_days) = parse_yyyy_mm_dd_to_days(today) else {
            return self
                .assumptions
                .iter()
                .filter(|a| a.last_reviewed.is_none())
                .collect();
        };

        self.assumptions
            .iter()
            .filter(|assumption| {
                let Some(last_reviewed) = &assumption.last_reviewed else {
                    return true;
                };

                let Some(last_reviewed_days) = parse_yyyy_mm_dd_to_days(last_reviewed) else {
                    return true;
                };

                last_reviewed_days + i64::from(assumption.revisit_cadence_days) < today_days
            })
            .collect()
    }

    pub fn get_by_owner(&self, owner: &str) -> Vec<&Assumption> {
        self.assumptions
            .iter()
            .filter(|a| a.owner == owner)
            .collect()
    }

    pub fn get_violated(&self) -> Vec<&Assumption> {
        self.assumptions
            .iter()
            .filter(|a| a.status == AssumptionStatus::Violated)
            .collect()
    }
}

fn parse_yyyy_mm_dd_to_days(value: &str) -> Option<i64> {
    let mut split = value.split('-');
    let year = split.next()?.parse::<i32>().ok()?;
    let month = split.next()?.parse::<u32>().ok()?;
    let day = split.next()?.parse::<u32>().ok()?;

    if split.next().is_some() || !is_valid_ymd(year, month, day) {
        return None;
    }

    Some(days_from_civil(year, month, day))
}

fn is_valid_ymd(year: i32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }

    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };

    day <= max_day
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = if adjusted_year >= 0 {
        adjusted_year / 400
    } else {
        (adjusted_year - 399) / 400
    };
    let year_of_era = adjusted_year - era * 400;
    let month_index = month as i32;
    let day_of_year =
        (153 * (month_index + if month_index > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    (era as i64) * 146_097 + day_of_era as i64 - 719_468
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
        results.push(AssumptionCheckResult::MissingAssumptions(
            missing_assumptions,
        ));
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
