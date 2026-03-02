namespace ETH2077Proofs.TestnetGates

/--
ETH2077 alpha testnet launch constants.
-/
def alphaValidatorCount : Nat := 48
def alphaFaultBudget : Nat := 15
def alphaQuorumThreshold : Nat := 31
def alphaChainId : Nat := 2077001

/-- The launch chain ID must be non-zero. -/
theorem alpha_chain_id_nonzero : 0 < alphaChainId := by
  decide

/-- Fault budget satisfies `3f + 1 <= n` for alpha validator count. -/
theorem alpha_fault_budget_valid :
    3 * alphaFaultBudget + 1 <= alphaValidatorCount := by
  decide

/-- Quorum threshold is bound by configured validator count. -/
theorem alpha_quorum_within_validator_set :
    alphaQuorumThreshold <= alphaValidatorCount := by
  decide

/-- Quorum threshold is strictly above simple majority for alpha launch. -/
theorem alpha_quorum_strict_majority :
    alphaValidatorCount / 2 < alphaQuorumThreshold := by
  decide

/--
Two alpha quorums intersect in at least one validator.
Using `2q - n > 0` for concrete launch constants.
-/
theorem alpha_quorum_intersection_positive :
    0 < 2 * alphaQuorumThreshold - alphaValidatorCount := by
  decide

end ETH2077Proofs.TestnetGates
