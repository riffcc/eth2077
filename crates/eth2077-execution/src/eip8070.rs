use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CELLS_PER_BLOB: usize = 128;
pub const DEFAULT_CUSTODY_GROUPS: usize = 8;
pub const FULL_SAMPLE_PROBABILITY: f64 = 0.15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobCell {
    pub blob_index: u64,
    pub cell_index: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustodyAssignment {
    pub node_id: [u8; 32],
    pub custody_group: u64,
    pub total_groups: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SparseBlobRequest {
    pub blob_index: u64,
    pub requested_cells: Vec<u64>,
    pub custody_assignment: CustodyAssignment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SparseBlobError {
    InvalidCellIndex { cell_index: u64, max: u64 },
    CustodyGroupOutOfRange { group: u64, total: u64 },
    EmptyCellRequest,
    CellNotInCustody { cell_index: u64, custody_group: u64 },
    DuplicateCellIndex { cell_index: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SparseBlobStats {
    pub total_cells: usize,
    pub sampled_cells: usize,
    pub sampling_ratio: f64,
    pub bandwidth_reduction_percent: f64,
    pub custody_group: u64,
}

pub fn compute_custody_assignment(node_id: &[u8; 32], total_groups: u64) -> CustodyAssignment {
    let hash: [u8; 32] = Sha256::digest(node_id).into();
    let hash_value = u64::from_be_bytes(hash[0..8].try_into().unwrap_or([0u8; 8]));
    let custody_group = if total_groups == 0 {
        0
    } else {
        hash_value % total_groups
    };

    CustodyAssignment {
        node_id: *node_id,
        custody_group,
        total_groups,
    }
}

pub fn cells_for_custody_group(
    custody_group: u64,
    total_groups: u64,
    cells_per_blob: usize,
) -> Vec<u64> {
    if total_groups == 0 {
        return Vec::new();
    }

    (0..cells_per_blob)
        .filter_map(|cell| {
            let cell = cell as u64;
            (cell % total_groups == custody_group).then_some(cell)
        })
        .collect()
}

pub fn validate_sparse_blob_request(
    request: &SparseBlobRequest,
) -> Result<(), Vec<SparseBlobError>> {
    let mut errors = Vec::new();

    if request.requested_cells.is_empty() {
        errors.push(SparseBlobError::EmptyCellRequest);
    }

    let custody_group = request.custody_assignment.custody_group;
    let total_groups = request.custody_assignment.total_groups;

    let custody_group_valid = total_groups != 0 && custody_group < total_groups;
    if !custody_group_valid {
        errors.push(SparseBlobError::CustodyGroupOutOfRange {
            group: custody_group,
            total: total_groups,
        });
    }

    let mut seen = HashSet::new();
    for &cell_index in &request.requested_cells {
        if !seen.insert(cell_index) {
            errors.push(SparseBlobError::DuplicateCellIndex { cell_index });
        }

        if cell_index >= CELLS_PER_BLOB as u64 {
            errors.push(SparseBlobError::InvalidCellIndex {
                cell_index,
                max: (CELLS_PER_BLOB as u64).saturating_sub(1),
            });
        }

        if custody_group_valid && (cell_index % total_groups != custody_group) {
            errors.push(SparseBlobError::CellNotInCustody {
                cell_index,
                custody_group,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_sparse_blob_stats(custody_group: u64, total_groups: u64) -> SparseBlobStats {
    let total_cells = CELLS_PER_BLOB;
    let sampled_cells = cells_for_custody_group(custody_group, total_groups, CELLS_PER_BLOB).len();
    let sampling_ratio = if total_cells == 0 {
        0.0
    } else {
        sampled_cells as f64 / total_cells as f64
    };
    let bandwidth_reduction_percent = (1.0 - sampling_ratio) * 100.0;

    SparseBlobStats {
        total_cells,
        sampled_cells,
        sampling_ratio,
        bandwidth_reduction_percent,
        custody_group,
    }
}

pub fn compute_cell_commitment(cell: &BlobCell) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(cell.blob_index.to_be_bytes());
    hasher.update(cell.cell_index.to_be_bytes());
    hasher.update(&cell.data);
    hasher.finalize().into()
}

pub fn should_full_sample(node_id: &[u8; 32]) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(b"full_sample");
    hasher.update(node_id);
    let digest: [u8; 32] = hasher.finalize().into();
    let value = u64::from_be_bytes(digest[0..8].try_into().unwrap_or([0u8; 8]));
    let threshold = (FULL_SAMPLE_PROBABILITY * 100.0) as u64;

    (value % 100) < threshold
}
