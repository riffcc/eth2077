use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TheoremTier {
    Tier1,
    Tier2,
    Tier3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TheoremStatus {
    Proposed,
    ProofInProgress,
    ProofComplete,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TheoremEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: TheoremTier,
    pub status: TheoremStatus,
    pub dependencies: Vec<String>,
    pub proof_artifact_path: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TheoremRegistry {
    pub version: String,
    pub entries: Vec<TheoremEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TheoremRegistryValidationError {
    DuplicateId(String),
    MissingDependency {
        theorem_id: String,
        dependency_id: String,
    },
}

impl TheoremRegistry {
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

    pub fn validate(&self) -> Result<(), Vec<TheoremRegistryValidationError>> {
        let mut errors = Vec::new();
        let mut ids = HashSet::new();

        for entry in &self.entries {
            if !ids.insert(entry.id.clone()) {
                errors.push(TheoremRegistryValidationError::DuplicateId(
                    entry.id.clone(),
                ));
            }
        }

        for entry in &self.entries {
            for dependency in &entry.dependencies {
                if !ids.contains(dependency) {
                    errors.push(TheoremRegistryValidationError::MissingDependency {
                        theorem_id: entry.id.clone(),
                        dependency_id: dependency.clone(),
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

    pub fn get_by_tier(&self, tier: TheoremTier) -> Vec<&TheoremEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.tier == tier)
            .collect()
    }

    pub fn get_by_id(&self, id: &str) -> Option<&TheoremEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}
