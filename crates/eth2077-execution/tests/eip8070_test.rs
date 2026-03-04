use eth2077_execution::eip8070::{
    cells_for_custody_group, compute_cell_commitment, compute_custody_assignment,
    compute_sparse_blob_stats, validate_sparse_blob_request, BlobCell, CustodyAssignment,
    SparseBlobError, SparseBlobRequest, CELLS_PER_BLOB,
};

#[test]
fn custody_assignment_deterministic() {
    let node_id = [0xAB; 32];
    let first = compute_custody_assignment(&node_id, 8);
    let second = compute_custody_assignment(&node_id, 8);

    assert_eq!(first, second);
}

#[test]
fn cells_for_group_correct() {
    let cells = cells_for_custody_group(0, 8, CELLS_PER_BLOB);
    let expected: Vec<u64> = (0..CELLS_PER_BLOB)
        .step_by(8)
        .map(|idx| idx as u64)
        .collect();

    assert_eq!(cells, expected);
}

#[test]
fn valid_request_passes() {
    let request = SparseBlobRequest {
        blob_index: 0,
        requested_cells: vec![0, 8, 16],
        custody_assignment: CustodyAssignment {
            node_id: [0x11; 32],
            custody_group: 0,
            total_groups: 8,
        },
    };

    assert_eq!(validate_sparse_blob_request(&request), Ok(()));
}

#[test]
fn empty_cell_request_rejected() {
    let request = SparseBlobRequest {
        blob_index: 0,
        requested_cells: Vec::new(),
        custody_assignment: CustodyAssignment {
            node_id: [0x11; 32],
            custody_group: 0,
            total_groups: 8,
        },
    };

    let errors = validate_sparse_blob_request(&request).expect_err("empty requests must fail");
    assert!(errors.contains(&SparseBlobError::EmptyCellRequest));
}

#[test]
fn invalid_cell_index_detected() {
    let request = SparseBlobRequest {
        blob_index: 0,
        requested_cells: vec![CELLS_PER_BLOB as u64],
        custody_assignment: CustodyAssignment {
            node_id: [0x11; 32],
            custody_group: 0,
            total_groups: 8,
        },
    };

    let errors = validate_sparse_blob_request(&request).expect_err("invalid cell index must fail");
    assert!(errors.contains(&SparseBlobError::InvalidCellIndex {
        cell_index: CELLS_PER_BLOB as u64,
        max: CELLS_PER_BLOB as u64 - 1,
    }));
}

#[test]
fn cell_not_in_custody_detected() {
    let request = SparseBlobRequest {
        blob_index: 0,
        requested_cells: vec![1],
        custody_assignment: CustodyAssignment {
            node_id: [0x11; 32],
            custody_group: 0,
            total_groups: 8,
        },
    };

    let errors = validate_sparse_blob_request(&request).expect_err("wrong custody cell must fail");
    assert!(errors.contains(&SparseBlobError::CellNotInCustody {
        cell_index: 1,
        custody_group: 0,
    }));
}

#[test]
fn stats_computation_correct() {
    let stats = compute_sparse_blob_stats(0, 8);

    assert_eq!(stats.total_cells, CELLS_PER_BLOB);
    assert_eq!(stats.sampled_cells, CELLS_PER_BLOB / 8);
    assert!((stats.sampling_ratio - 0.125).abs() < f64::EPSILON);
    assert!((stats.bandwidth_reduction_percent - 87.5).abs() < f64::EPSILON);
}

#[test]
fn cell_commitment_deterministic() {
    let cell = BlobCell {
        blob_index: 42,
        cell_index: 7,
        data: vec![1, 2, 3, 4, 5],
    };

    let first = compute_cell_commitment(&cell);
    let second = compute_cell_commitment(&cell);

    assert_eq!(first, second);
}
