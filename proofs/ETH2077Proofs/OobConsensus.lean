namespace ETH2077Proofs.OobConsensus

/-- Minimal OOB commitment model used for theorem scaffolding. -/
structure OobCommitment where
  signature : Nat
  blockNumber : Nat
  blockHash : Nat
deriving DecidableEq

/-- A commitment is valid if signature is non-zero and block number is positive. -/
def oobCommitmentValid (c : OobCommitment) : Prop :=
  c.signature ≠ 0 ∧ 0 < c.blockNumber

/-- A zero signature is always rejected. -/
theorem valid_commitment_nonzero_sig
    (blockNumber blockHash : Nat) :
    ¬ oobCommitmentValid
      { signature := 0, blockNumber := blockNumber, blockHash := blockHash } := by
  simp [oobCommitmentValid]

/-- If a block is finalized at `t`, any later check time is still at or after `t`. -/
theorem finality_monotonic
    (finalizedAt checkAt checkAt' : Nat)
    (hFinalized : finalizedAt <= checkAt)
    (hSubsequent : checkAt <= checkAt') :
    finalizedAt <= checkAt' := by
  exact Nat.le_trans hFinalized hSubsequent

/-- Idempotent finalization: finalizing the same hash twice is equivalent to once. -/
def finalizeOnce (finalizedHashes : List Nat) (blockHash : Nat) : List Nat :=
  if blockHash ∈ finalizedHashes then finalizedHashes else blockHash :: finalizedHashes

theorem no_double_finality
    (finalizedHashes : List Nat)
    (blockHash : Nat) :
    finalizeOnce (finalizeOnce finalizedHashes blockHash) blockHash =
      finalizeOnce finalizedHashes blockHash := by
  unfold finalizeOnce
  by_cases hMem : blockHash ∈ finalizedHashes
  · simp [hMem]
  · simp [hMem]

/-- Quorum overlap lower bound for quorum size `n - f`. -/
def quorumOverlapLowerBound (n f : Nat) : Nat :=
  2 * (n - f) - n

/--
If `2 * (n - f) > n`, then two quorums of size `n - f` overlap by at least one
member, ensuring consistency under the usual honest-overlap interpretation.
-/
theorem quorum_overlap_ensures_consistency
    (n f : Nat)
    (hOverlap : 2 * (n - f) > n) :
    1 <= quorumOverlapLowerBound n f := by
  unfold quorumOverlapLowerBound
  exact Nat.succ_le_of_lt (Nat.sub_pos_of_lt hOverlap)

end ETH2077Proofs.OobConsensus
