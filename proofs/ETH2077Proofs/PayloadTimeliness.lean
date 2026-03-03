namespace ETH2077Proofs.PayloadTimeliness

/-!
EIP-7732 Phase-1 timeliness accounting invariants.
-/

/-- Slot-local accounting invariant: reveals cannot exceed announced headers. -/
def SlotInvariant (headerCount revealedOnTime lateReveal : Nat) : Prop :=
  revealedOnTime + lateReveal <= headerCount

def DeadlinePassed (currentUnixS deadlineUnixS : Nat) : Prop :=
  deadlineUnixS < currentUnixS

def Withheld (headerCount revealedOnTime lateReveal currentUnixS deadlineUnixS : Nat) : Prop :=
  0 < headerCount ∧
  revealedOnTime = 0 ∧
  lateReveal = 0 ∧
  DeadlinePassed currentUnixS deadlineUnixS

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

theorem withheld_implies_positive_headers
    (headerCount revealedOnTime lateReveal currentUnixS deadlineUnixS : Nat)
    (hWithheld : Withheld headerCount revealedOnTime lateReveal currentUnixS deadlineUnixS) :
    0 < headerCount := by
  exact hWithheld.1

theorem withheld_implies_no_reveals
    (headerCount revealedOnTime lateReveal currentUnixS deadlineUnixS : Nat)
    (hWithheld : Withheld headerCount revealedOnTime lateReveal currentUnixS deadlineUnixS) :
    revealedOnTime + lateReveal = 0 := by
  rcases hWithheld with ⟨_, hOnTime, hLate, _⟩
  simp [hOnTime, hLate]

inductive TimelinessStatus where
  | unknown
  | headerOnly
  | partialReveal
  | revealed
  | lateReveal
  | withheld
  | partialWithhold
  | orphanEnvelope
deriving DecidableEq

inductive PenaltyState where
  | active
  | recovered
deriving DecidableEq

def IsViolation (status : TimelinessStatus) : Bool :=
  status == TimelinessStatus.withheld ||
  status == TimelinessStatus.partialWithhold ||
  status == TimelinessStatus.lateReveal

def PenaltyTransition (status : TimelinessStatus) : PenaltyState :=
  if IsViolation status then PenaltyState.active else PenaltyState.recovered

theorem violation_transitions_to_active
    (status : TimelinessStatus)
    (h : IsViolation status = true) :
    PenaltyTransition status = PenaltyState.active := by
  simp [PenaltyTransition, h]

theorem non_violation_transitions_to_recovered
    (status : TimelinessStatus)
    (h : IsViolation status = false) :
    PenaltyTransition status = PenaltyState.recovered := by
  simp [PenaltyTransition, h]

theorem withheld_is_violation : IsViolation TimelinessStatus.withheld = true := by
  simp [IsViolation]

theorem late_reveal_is_violation : IsViolation TimelinessStatus.lateReveal = true := by
  simp [IsViolation]

theorem revealed_is_not_violation : IsViolation TimelinessStatus.revealed = false := by
  simp [IsViolation]

theorem recovery_after_reveal :
    PenaltyTransition TimelinessStatus.revealed = PenaltyState.recovered := by
  apply non_violation_transitions_to_recovered
  exact revealed_is_not_violation

end ETH2077Proofs.PayloadTimeliness
