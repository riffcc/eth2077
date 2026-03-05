use eth2077_oob_consensus::consensus::{ConsensusEngine, ConsensusEvent, ConsensusMessage};
use eth2077_oob_consensus::fast_path::FastPathConfig;
use eth2077_oob_consensus::validator::{Validator, ValidatorSet};

fn build_validator_set() -> ValidatorSet {
    ValidatorSet::new(vec![
        Validator {
            index: 0,
            weight: 1,
            public_key: [0; 32],
        },
        Validator {
            index: 1,
            weight: 1,
            public_key: [1; 32],
        },
        Validator {
            index: 2,
            weight: 1,
            public_key: [2; 32],
        },
        Validator {
            index: 3,
            weight: 1,
            public_key: [3; 32],
        },
    ])
}

fn build_engine(validator_set: ValidatorSet) -> ConsensusEngine {
    ConsensusEngine::new(
        validator_set,
        FastPathConfig {
            quorum_threshold: 3,
            timeout_ms: 1_000,
            optimistic_threshold: 10,
        },
    )
}

fn hash(byte: u8) -> [u8; 32] {
    [byte; 32]
}

#[test]
fn test_happy_path_finalization() {
    let validator_set = build_validator_set();
    let mut engine = build_engine(validator_set.clone());
    let height = 1;
    let round = 0;
    let block_hash = hash(7);
    let leader = validator_set.leader_for_round(height, round);

    let start_events = engine.start_height(height);
    assert!(start_events
        .iter()
        .any(|event| matches!(event, ConsensusEvent::NeedProposal { height: 1, round: 0, .. })));

    let proposal_events = engine.on_message(ConsensusMessage::Proposal {
        height,
        round,
        block_hash,
        proposer: leader,
    });
    assert!(proposal_events.iter().any(|event| {
        matches!(
            event,
            ConsensusEvent::SendPrevote {
                height: 1,
                round: 0,
                block_hash: Some(h),
            } if *h == block_hash
        )
    }));

    let mut saw_precommit_signal = false;
    for voter in 0..4 {
        let events = engine.on_message(ConsensusMessage::Prevote {
            height,
            round,
            block_hash: Some(block_hash),
            voter,
        });

        if events.iter().any(|event| {
            matches!(
                event,
                ConsensusEvent::SendPrecommit {
                    height: 1,
                    round: 0,
                    block_hash: Some(h),
                } if *h == block_hash
            )
        }) {
            saw_precommit_signal = true;
        }
    }
    assert!(saw_precommit_signal, "expected precommit to be triggered");

    let mut finalized = false;
    for voter in 0..4 {
        let events = engine.on_message(ConsensusMessage::Precommit {
            height,
            round,
            block_hash: Some(block_hash),
            voter,
        });

        if events.iter().any(|event| {
            matches!(
                event,
                ConsensusEvent::BlockFinalized {
                    height: 1,
                    block_hash: h,
                    round: 0,
                } if *h == block_hash
            )
        }) {
            finalized = true;
            break;
        }
    }

    assert!(finalized, "expected block to finalize");
}

#[test]
fn test_round_timeout_advances_leader() {
    let validator_set = build_validator_set();
    let mut engine = build_engine(validator_set.clone());
    let height = 1;
    let round0_leader = validator_set.leader_for_round(height, 0);

    engine.start_height(height);

    let timeout_events = engine.on_timeout();
    let round1_leader = validator_set.leader_for_round(height, 1);
    assert_ne!(round0_leader, round1_leader);
    assert!(timeout_events
        .iter()
        .any(|event| matches!(event, ConsensusEvent::RoundTimeout { height: 1, round: 0 })));
    assert!(timeout_events.iter().any(|event| {
        matches!(
            event,
            ConsensusEvent::NeedProposal {
                height: 1,
                round: 1,
                leader,
            } if *leader == round1_leader
        )
    }));

    let block_hash = hash(9);
    let proposal_events = engine.on_message(ConsensusMessage::Proposal {
        height,
        round: 1,
        block_hash,
        proposer: round1_leader,
    });
    assert!(proposal_events.iter().any(|event| {
        matches!(
            event,
            ConsensusEvent::SendPrevote {
                height: 1,
                round: 1,
                block_hash: Some(h),
            } if *h == block_hash
        )
    }));

    for voter in 0..4 {
        engine.on_message(ConsensusMessage::Prevote {
            height,
            round: 1,
            block_hash: Some(block_hash),
            voter,
        });
    }

    let mut finalized = false;
    for voter in 0..4 {
        let events = engine.on_message(ConsensusMessage::Precommit {
            height,
            round: 1,
            block_hash: Some(block_hash),
            voter,
        });

        if events.iter().any(|event| {
            matches!(
                event,
                ConsensusEvent::BlockFinalized {
                    height: 1,
                    block_hash: h,
                    round: 1,
                } if *h == block_hash
            )
        }) {
            finalized = true;
            break;
        }
    }

    assert!(finalized, "expected block to finalize in round 1");
}

#[test]
fn test_byzantine_prevote_still_finalizes() {
    let validator_set = build_validator_set();
    let mut engine = build_engine(validator_set.clone());
    let height = 1;
    let block_hash = hash(11);
    let byzantine_hash = hash(22);
    let leader = validator_set.leader_for_round(height, 0);

    engine.start_height(height);
    engine.on_message(ConsensusMessage::Proposal {
        height,
        round: 0,
        block_hash,
        proposer: leader,
    });

    engine.on_message(ConsensusMessage::Prevote {
        height,
        round: 0,
        block_hash: Some(block_hash),
        voter: 0,
    });
    engine.on_message(ConsensusMessage::Prevote {
        height,
        round: 0,
        block_hash: Some(block_hash),
        voter: 1,
    });
    engine.on_message(ConsensusMessage::Prevote {
        height,
        round: 0,
        block_hash: Some(block_hash),
        voter: 2,
    });
    engine.on_message(ConsensusMessage::Prevote {
        height,
        round: 0,
        block_hash: Some(byzantine_hash),
        voter: 3,
    });

    let mut finalized = false;
    for voter in 0..3 {
        let events = engine.on_message(ConsensusMessage::Precommit {
            height,
            round: 0,
            block_hash: Some(block_hash),
            voter,
        });

        if events.iter().any(|event| {
            matches!(
                event,
                ConsensusEvent::BlockFinalized {
                    height: 1,
                    block_hash: h,
                    round: 0,
                } if *h == block_hash
            )
        }) {
            finalized = true;
            break;
        }
    }

    assert!(finalized, "expected 3 honest precommits to finalize");
}

#[test]
fn test_locking_prevents_equivocation() {
    let validator_set = build_validator_set();
    let mut engine = build_engine(validator_set.clone());
    let height = 1;
    let h = hash(33);
    let h_prime = hash(44);

    let leader_round0 = validator_set.leader_for_round(height, 0);
    engine.start_height(height);
    engine.on_message(ConsensusMessage::Proposal {
        height,
        round: 0,
        block_hash: h,
        proposer: leader_round0,
    });

    for voter in 0..3 {
        engine.on_message(ConsensusMessage::Prevote {
            height,
            round: 0,
            block_hash: Some(h),
            voter,
        });
    }

    let timeout_events = engine.on_timeout();
    assert!(timeout_events
        .iter()
        .any(|event| matches!(event, ConsensusEvent::RoundTimeout { height: 1, round: 0 })));

    let leader_round1 = validator_set.leader_for_round(height, 1);
    let proposal_events = engine.on_message(ConsensusMessage::Proposal {
        height,
        round: 1,
        block_hash: h_prime,
        proposer: leader_round1,
    });

    assert!(proposal_events.iter().any(|event| {
        matches!(
            event,
            ConsensusEvent::SendPrevote {
                height: 1,
                round: 1,
                block_hash: Some(locked_hash),
            } if *locked_hash == h
        )
    }));
}
