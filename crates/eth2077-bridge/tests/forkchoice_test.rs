use eth2077_bridge::engine_api::Bytes32;
use eth2077_bridge::forkchoice::ForkchoiceManager;

#[test]
fn forkchoice_manager_tracks_state_transitions() {
    let manager = ForkchoiceManager::new();

    let initial = manager.get_state();
    assert_eq!(initial.head_block_hash, Bytes32::zero());
    assert_eq!(initial.safe_block_hash, Bytes32::zero());
    assert_eq!(initial.finalized_block_hash, Bytes32::zero());

    let head = Bytes32(format!("0x{}", "11".repeat(32)));
    let safe = Bytes32(format!("0x{}", "22".repeat(32)));
    let finalized = Bytes32(format!("0x{}", "33".repeat(32)));

    manager.update_head(head.clone());
    manager.mark_safe(safe.clone());
    manager.mark_finalized(finalized.clone());

    let updated = manager.get_state();
    assert_eq!(updated.head_block_hash, head);
    assert_eq!(updated.safe_block_hash, safe);
    assert_eq!(updated.finalized_block_hash, finalized);
}

#[test]
fn forkchoice_manager_is_shared_across_clones() {
    let manager = ForkchoiceManager::new();
    let cloned = manager.clone();
    let head = Bytes32(format!("0x{}", "aa".repeat(32)));

    cloned.update_head(head.clone());

    assert_eq!(manager.get_state().head_block_hash, head);
}
