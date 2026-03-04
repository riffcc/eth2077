namespace ETH2077Proofs.WitnessIntegrity

/-- Witness payload emitted by execution for OOB verification. -/
structure Witness where
  proofData : List Nat
  stateRoot : Nat
deriving DecidableEq

/-- Minimal execution result model used for proof scaffolding. -/
structure ExecutionResult where
  proofData : List Nat
  stateRoot : Nat
deriving DecidableEq

/-- A witness is valid if proof data is present and the committed state root matches. -/
def witnessValid (w : Witness) (commitmentStateRoot : Nat) : Prop :=
  w.proofData ≠ [] ∧ w.stateRoot = commitmentStateRoot

/-- Execution result validity mirrors witness validity requirements. -/
def executionValid (result : ExecutionResult) (commitmentStateRoot : Nat) : Prop :=
  result.proofData ≠ [] ∧ result.stateRoot = commitmentStateRoot

/-- Any state-root change away from commitment binding invalidates the witness. -/
theorem witness_binding
    (proofData : List Nat)
    (stateRoot commitmentStateRoot : Nat)
    (hChanged : stateRoot ≠ commitmentStateRoot) :
    ¬ witnessValid { proofData := proofData, stateRoot := stateRoot } commitmentStateRoot := by
  intro hValid
  exact hChanged hValid.2

/-- A valid execution result always yields a valid witness artifact. -/
theorem witness_completeness
    (result : ExecutionResult)
    (commitmentStateRoot : Nat)
    (hExec : executionValid result commitmentStateRoot) :
    witnessValid
      { proofData := result.proofData, stateRoot := result.stateRoot }
      commitmentStateRoot := by
  simpa [executionValid, witnessValid] using hExec

end ETH2077Proofs.WitnessIntegrity
