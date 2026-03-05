use std::collections::HashMap;

use crate::fast_path::{FastPathAccumulator, FastPathAttestation, FastPathConfig, FastPathOutcome};
use crate::validator::ValidatorSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Propose,
    Prevote,
    Precommit,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConsensusMessage {
    Proposal {
        height: u64,
        round: u32,
        block_hash: [u8; 32],
        proposer: u64,
    },
    Prevote {
        height: u64,
        round: u32,
        block_hash: Option<[u8; 32]>,
        voter: u64,
    },
    Precommit {
        height: u64,
        round: u32,
        block_hash: Option<[u8; 32]>,
        voter: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConsensusEvent {
    BlockFinalized {
        height: u64,
        block_hash: [u8; 32],
        round: u32,
    },
    RoundTimeout {
        height: u64,
        round: u32,
    },
    NeedProposal {
        height: u64,
        round: u32,
        leader: u64,
    },
    SendPrevote {
        height: u64,
        round: u32,
        block_hash: Option<[u8; 32]>,
    },
    SendPrecommit {
        height: u64,
        round: u32,
        block_hash: Option<[u8; 32]>,
    },
}

#[derive(Debug, Clone)]
struct RoundState {
    height: u64,
    round: u32,
    step: Step,
    proposals: HashMap<u32, [u8; 32]>,
    prevotes: HashMap<(u32, u64), Option<[u8; 32]>>,
    precommits: HashMap<(u32, u64), Option<[u8; 32]>>,
    locked_round: Option<u32>,
    locked_hash: Option<[u8; 32]>,
}

impl RoundState {
    fn new() -> Self {
        Self {
            height: 0,
            round: 0,
            step: Step::Propose,
            proposals: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),
            locked_round: None,
            locked_hash: None,
        }
    }

    fn reset_for_height(&mut self, height: u64) {
        self.height = height;
        self.round = 0;
        self.step = Step::Propose;
        self.proposals.clear();
        self.prevotes.clear();
        self.precommits.clear();
        self.locked_round = None;
        self.locked_hash = None;
    }

    fn advance_round(&mut self) {
        self.round = self.round.saturating_add(1);
        self.step = Step::Propose;
    }

    fn insert_prevote(&mut self, round: u32, voter: u64, block_hash: Option<[u8; 32]>) -> bool {
        let key = (round, voter);
        if self.prevotes.contains_key(&key) {
            return false;
        }
        self.prevotes.insert(key, block_hash);
        true
    }

    fn insert_precommit(&mut self, round: u32, voter: u64, block_hash: Option<[u8; 32]>) -> bool {
        let key = (round, voter);
        if self.precommits.contains_key(&key) {
            return false;
        }
        self.precommits.insert(key, block_hash);
        true
    }

    fn tally_prevotes(&self, round: u32, validator_set: &ValidatorSet) -> HashMap<[u8; 32], u64> {
        let mut tally = HashMap::new();

        for ((vote_round, voter), block_hash) in &self.prevotes {
            if *vote_round != round {
                continue;
            }

            let Some(hash) = block_hash else {
                continue;
            };

            let weight = validator_set.weight_of(*voter);
            if weight == 0 {
                continue;
            }

            let entry = tally.entry(*hash).or_insert(0u64);
            *entry = (*entry).saturating_add(weight);
        }

        tally
    }

    fn tally_precommits(&self, round: u32, validator_set: &ValidatorSet) -> HashMap<[u8; 32], u64> {
        let mut tally = HashMap::new();

        for ((vote_round, voter), block_hash) in &self.precommits {
            if *vote_round != round {
                continue;
            }

            let Some(hash) = block_hash else {
                continue;
            };

            let weight = validator_set.weight_of(*voter);
            if weight == 0 {
                continue;
            }

            let entry = tally.entry(*hash).or_insert(0u64);
            *entry = (*entry).saturating_add(weight);
        }

        tally
    }

    fn quorum_hash(tally: &HashMap<[u8; 32], u64>, quorum_weight: u64) -> Option<[u8; 32]> {
        tally
            .iter()
            .find_map(|(hash, weight)| (*weight >= quorum_weight).then_some(*hash))
    }
}

#[derive(Debug, Clone)]
pub struct ConsensusEngine {
    validator_set: ValidatorSet,
    state: RoundState,
    fast_path: FastPathAccumulator,
}

impl ConsensusEngine {
    pub fn new(validator_set: ValidatorSet, fast_path_config: FastPathConfig) -> Self {
        Self {
            validator_set,
            state: RoundState::new(),
            fast_path: FastPathAccumulator::new(fast_path_config),
        }
    }

    pub fn start_height(&mut self, height: u64) -> Vec<ConsensusEvent> {
        self.state.reset_for_height(height);
        self.fast_path.reset();

        vec![ConsensusEvent::NeedProposal {
            height,
            round: 0,
            leader: self.validator_set.leader_for_round(height, 0),
        }]
    }

    pub fn on_message(&mut self, msg: ConsensusMessage) -> Vec<ConsensusEvent> {
        if self.state.step == Step::Committed {
            return Vec::new();
        }

        match msg {
            ConsensusMessage::Proposal {
                height,
                round,
                block_hash,
                proposer,
            } => self.on_proposal(height, round, block_hash, proposer),
            ConsensusMessage::Prevote {
                height,
                round,
                block_hash,
                voter,
            } => self.on_prevote(height, round, block_hash, voter),
            ConsensusMessage::Precommit {
                height,
                round,
                block_hash,
                voter,
            } => self.on_precommit(height, round, block_hash, voter),
        }
    }

    pub fn on_timeout(&mut self) -> Vec<ConsensusEvent> {
        if self.state.step == Step::Committed {
            return Vec::new();
        }

        let timed_out_round = self.state.round;
        self.state.advance_round();
        self.fast_path.reset();

        vec![
            ConsensusEvent::RoundTimeout {
                height: self.state.height,
                round: timed_out_round,
            },
            ConsensusEvent::NeedProposal {
                height: self.state.height,
                round: self.state.round,
                leader: self
                    .validator_set
                    .leader_for_round(self.state.height, self.state.round),
            },
        ]
    }

    pub fn current_height(&self) -> u64 {
        self.state.height
    }

    pub fn current_round(&self) -> u32 {
        self.state.round
    }

    fn on_proposal(
        &mut self,
        height: u64,
        round: u32,
        block_hash: [u8; 32],
        proposer: u64,
    ) -> Vec<ConsensusEvent> {
        if height != self.state.height || round != self.state.round {
            return Vec::new();
        }

        let expected_leader = self.validator_set.leader_for_round(height, round);
        if proposer != expected_leader {
            return Vec::new();
        }

        self.state.proposals.insert(round, block_hash);
        self.state.step = Step::Prevote;

        let vote_hash = self.state.locked_hash.or(Some(block_hash));

        vec![ConsensusEvent::SendPrevote {
            height,
            round,
            block_hash: vote_hash,
        }]
    }

    fn on_prevote(
        &mut self,
        height: u64,
        round: u32,
        block_hash: Option<[u8; 32]>,
        voter: u64,
    ) -> Vec<ConsensusEvent> {
        if height != self.state.height || round != self.state.round {
            return Vec::new();
        }

        if !self.state.insert_prevote(round, voter, block_hash) {
            return Vec::new();
        }

        let mut events = Vec::new();

        if let Some(hash) = block_hash {
            let attestation = FastPathAttestation {
                block_hash: hash,
                attester_index: voter,
                signature: Vec::new(),
                received_at_ms: self
                    .state
                    .height
                    .saturating_mul(1_000)
                    .saturating_add(self.state.round as u64),
            };

            if let Some(FastPathOutcome::FastFinalized(_)) =
                self.fast_path.add_attestation(attestation)
            {
                self.state.step = Step::Committed;
                events.push(ConsensusEvent::BlockFinalized {
                    height,
                    block_hash: hash,
                    round,
                });
                return events;
            }
        }

        let tally = self.state.tally_prevotes(round, &self.validator_set);
        let quorum_weight = self.validator_set.quorum_weight();

        if let Some(quorum_hash) = RoundState::quorum_hash(&tally, quorum_weight) {
            let should_emit = !matches!(self.state.step, Step::Precommit)
                || self.state.locked_hash != Some(quorum_hash)
                || self.state.locked_round != Some(round);

            self.state.locked_round = Some(round);
            self.state.locked_hash = Some(quorum_hash);
            self.state.step = Step::Precommit;

            if should_emit {
                events.push(ConsensusEvent::SendPrecommit {
                    height,
                    round,
                    block_hash: Some(quorum_hash),
                });
            }
        }

        events
    }

    fn on_precommit(
        &mut self,
        height: u64,
        round: u32,
        block_hash: Option<[u8; 32]>,
        voter: u64,
    ) -> Vec<ConsensusEvent> {
        if height != self.state.height || round != self.state.round {
            return Vec::new();
        }

        if !self.state.insert_precommit(round, voter, block_hash) {
            return Vec::new();
        }

        let tally = self.state.tally_precommits(round, &self.validator_set);
        let quorum_weight = self.validator_set.quorum_weight();

        let Some(quorum_hash) = RoundState::quorum_hash(&tally, quorum_weight) else {
            return Vec::new();
        };

        self.state.step = Step::Committed;

        vec![ConsensusEvent::BlockFinalized {
            height,
            block_hash: quorum_hash,
            round,
        }]
    }
}
