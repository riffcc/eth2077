namespace ETH2077Proofs.PayloadTimeliness

/-!
EIP-7732 Phase-1 timeliness accounting invariants.
-/

/-- Slot-local accounting invariant: reveals cannot exceed announced headers. -/
def SlotInvariant (headerCount revealedOnTime lateReveal : Nat) : Prop :=
  revealedOnTime + lateReveal <= headerCount

theorem invariant_holds_with_no_reveals (headerCount : Nat) :
    SlotInvariant headerCount 0 0 := by
  simp [SlotInvariant]

theorem invariant_preserved_by_on_time_reveal
    (headerCount revealedOnTime lateReveal : Nat)
    (_hInv : SlotInvariant headerCount revealedOnTime lateReveal)
    (hRoom : revealedOnTime + lateReveal < headerCount) :
    SlotInvariant headerCount (revealedOnTime + 1) lateReveal := by
  have hSucc : revealedOnTime + lateReveal + 1 <= headerCount :=
    Nat.succ_le_of_lt hRoom
  simpa [SlotInvariant, Nat.add_assoc, Nat.add_left_comm, Nat.add_comm] using hSucc

theorem invariant_preserved_by_late_reveal
    (headerCount revealedOnTime lateReveal : Nat)
    (_hInv : SlotInvariant headerCount revealedOnTime lateReveal)
    (hRoom : revealedOnTime + lateReveal < headerCount) :
    SlotInvariant headerCount revealedOnTime (lateReveal + 1) := by
  have hSucc : revealedOnTime + lateReveal + 1 <= headerCount :=
    Nat.succ_le_of_lt hRoom
  simpa [SlotInvariant, Nat.add_assoc, Nat.add_left_comm, Nat.add_comm] using hSucc

theorem on_time_reveals_bounded_by_headers
    (headerCount revealedOnTime lateReveal : Nat)
    (hInv : SlotInvariant headerCount revealedOnTime lateReveal) :
    revealedOnTime <= headerCount := by
  exact Nat.le_trans (Nat.le_add_right revealedOnTime lateReveal) hInv

theorem late_reveals_bounded_by_headers
    (headerCount revealedOnTime lateReveal : Nat)
    (hInv : SlotInvariant headerCount revealedOnTime lateReveal) :
    lateReveal <= headerCount := by
  exact Nat.le_trans (Nat.le_add_left lateReveal revealedOnTime) (by
    simpa [SlotInvariant, Nat.add_comm] using hInv)

end ETH2077Proofs.PayloadTimeliness
