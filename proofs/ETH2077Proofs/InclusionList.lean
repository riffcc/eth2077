namespace ETH2077Proofs.InclusionList

/-!
FOCIL-style inclusion-list precheck model for ETH2077.

This captures the safety rule behind `INCLUSION_LIST_UNSATISFIED`:
if any required transaction is missing or fails compatibility checks,
the aggregate payload validity predicate must fail.
-/

structure InclusionCheck where
  requiredPresent : Prop
  metadataCompatible : Prop
  nonceValid : Prop
  balanceValid : Prop
  gasWithinLimit : Prop

def InclusionCheck.ok (c : InclusionCheck) : Prop :=
  c.requiredPresent ∧
  c.metadataCompatible ∧
  c.nonceValid ∧
  c.balanceValid ∧
  c.gasWithinLimit

def payloadValid (checks : List InclusionCheck) : Prop :=
  ∀ c, c ∈ checks → c.ok

def viewMutationAllowed (viewFrozen : Bool) (incomingSlot frozenSlot : Nat) : Prop :=
  viewFrozen = false ∨ incomingSlot ≠ frozenSlot

theorem check_not_ok_of_missing
    (c : InclusionCheck)
    (hMissing : ¬ c.requiredPresent) :
    ¬ c.ok := by
  intro hOk
  exact hMissing hOk.1

theorem check_not_ok_of_metadata_mismatch
    (c : InclusionCheck)
    (hMeta : ¬ c.metadataCompatible) :
    ¬ c.ok := by
  intro hOk
  exact hMeta hOk.2.1

theorem check_not_ok_of_nonce_mismatch
    (c : InclusionCheck)
    (hNonce : ¬ c.nonceValid) :
    ¬ c.ok := by
  intro hOk
  exact hNonce hOk.2.2.1

theorem check_not_ok_of_insufficient_balance
    (c : InclusionCheck)
    (hBalance : ¬ c.balanceValid) :
    ¬ c.ok := by
  intro hOk
  exact hBalance hOk.2.2.2.1

theorem check_not_ok_of_gas_overflow
    (c : InclusionCheck)
    (hGas : ¬ c.gasWithinLimit) :
    ¬ c.ok := by
  intro hOk
  exact hGas hOk.2.2.2.2

theorem payload_not_valid_if_exists_invalid
    (checks : List InclusionCheck)
    (hInvalid : ∃ c, c ∈ checks ∧ ¬ c.ok) :
    ¬ payloadValid checks := by
  intro hValid
  rcases hInvalid with ⟨c, hMem, hNotOk⟩
  exact hNotOk (hValid c hMem)

theorem payload_not_valid_if_missing_required
    (checks : List InclusionCheck)
    (hMissing : ∃ c, c ∈ checks ∧ ¬ c.requiredPresent) :
    ¬ payloadValid checks := by
  apply payload_not_valid_if_exists_invalid
  rcases hMissing with ⟨c, hMem, hMiss⟩
  exact ⟨c, hMem, check_not_ok_of_missing c hMiss⟩

theorem view_mutation_blocked_when_frozen_same_slot
    (frozenSlot incomingSlot : Nat)
    (hSame : incomingSlot = frozenSlot) :
    ¬ viewMutationAllowed true incomingSlot frozenSlot := by
  intro hAllowed
  rcases hAllowed with hNotFrozen | hDifferent
  · cases hNotFrozen
  · exact hDifferent hSame

theorem view_mutation_allowed_when_slot_rotates
    (frozenSlot incomingSlot : Nat)
    (hDifferent : incomingSlot ≠ frozenSlot) :
    viewMutationAllowed true incomingSlot frozenSlot := by
  exact Or.inr hDifferent

end ETH2077Proofs.InclusionList
