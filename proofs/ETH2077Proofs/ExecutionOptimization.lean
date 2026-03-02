namespace ETH2077Proofs.ExecutionOptimization

/-!
ETH2077 execution-scaling theorem pack.

This module proves core optimization properties for a throughput model with:
- partitioned execution,
- pipelined commit/finality,
- ingress/execution/oob bottlenecks.

These theorems are intentionally simple and machine-checkable so they can be
used as hard gates in CI while we evolve the runtime implementation.
-/

def baselineThroughput (singleLane : Nat) : Nat :=
  singleLane

def partitionedThroughput (shards singleLane : Nat) : Nat :=
  shards * singleLane

def pipelinedThroughput (pipelineDepth shards singleLane : Nat) : Nat :=
  pipelineDepth * partitionedThroughput shards singleLane

def sustainedThroughput (ingress execution oob : Nat) : Nat :=
  Nat.min ingress (Nat.min execution oob)

/-- Adding execution partitions can never reduce throughput. -/
theorem partitioning_never_worse
    (shards singleLane : Nat)
    (hShards : 1 <= shards) :
    baselineThroughput singleLane <= partitionedThroughput shards singleLane := by
  calc
    baselineThroughput singleLane = 1 * singleLane := by
      simp [baselineThroughput]
    _ <= shards * singleLane := by
      exact Nat.mul_le_mul_right singleLane hShards
    _ = partitionedThroughput shards singleLane := by
      simp [partitionedThroughput]

/-- Adding pipeline depth can never reduce throughput. -/
theorem pipelining_never_worse
    (pipelineDepth shards singleLane : Nat)
    (hDepth : 1 <= pipelineDepth) :
    partitionedThroughput shards singleLane <= pipelinedThroughput pipelineDepth shards singleLane := by
  calc
    partitionedThroughput shards singleLane
        = 1 * partitionedThroughput shards singleLane := by simp
    _ <= pipelineDepth * partitionedThroughput shards singleLane := by
      exact Nat.mul_le_mul_right (partitionedThroughput shards singleLane) hDepth
    _ = pipelinedThroughput pipelineDepth shards singleLane := by
      simp [pipelinedThroughput]

/-- Combined partitioning + pipelining is never worse than a single lane. -/
theorem pipelined_partitioned_never_worse
    (pipelineDepth shards singleLane : Nat)
    (hDepth : 1 <= pipelineDepth)
    (hShards : 1 <= shards) :
    baselineThroughput singleLane <= pipelinedThroughput pipelineDepth shards singleLane := by
  exact Nat.le_trans
    (partitioning_never_worse shards singleLane hShards)
    (pipelining_never_worse pipelineDepth shards singleLane hDepth)

/-- More than one shard yields strict throughput gain for positive single-lane capacity. -/
theorem partitioning_strict_gain
    (shards singleLane : Nat)
    (hShards : 1 < shards)
    (hLane : 0 < singleLane) :
    baselineThroughput singleLane < partitionedThroughput shards singleLane := by
  calc
    baselineThroughput singleLane = 1 * singleLane := by simp [baselineThroughput]
    _ < shards * singleLane := by
      exact Nat.mul_lt_mul_of_pos_right hShards hLane
    _ = partitionedThroughput shards singleLane := by simp [partitionedThroughput]

/-- If execution is the smallest lane, sustained throughput equals execution capacity. -/
theorem execution_bottleneck_characterization
    (ingress execution oob : Nat)
    (hExecIngress : execution <= ingress)
    (hExecOob : execution <= oob) :
    sustainedThroughput ingress execution oob = execution := by
  have hInner : Nat.min execution oob = execution := Nat.min_eq_left hExecOob
  have hOuter : Nat.min ingress execution = execution := Nat.min_eq_right hExecIngress
  simpa [sustainedThroughput, hInner] using hOuter

/-- Improving non-bottleneck lanes does not change sustained throughput. -/
theorem lifting_non_bottlenecks_does_not_change_sustained
    (ingress ingress' execution oob oob' : Nat)
    (hExecIngress : execution <= ingress)
    (hExecOob : execution <= oob)
    (hIngressGrow : ingress <= ingress')
    (hOobGrow : oob <= oob') :
    sustainedThroughput ingress' execution oob' = sustainedThroughput ingress execution oob := by
  have hOld : sustainedThroughput ingress execution oob = execution :=
    execution_bottleneck_characterization ingress execution oob hExecIngress hExecOob
  have hNew : sustainedThroughput ingress' execution oob' = execution :=
    execution_bottleneck_characterization ingress' execution oob'
      (Nat.le_trans hExecIngress hIngressGrow)
      (Nat.le_trans hExecOob hOobGrow)
  calc
    sustainedThroughput ingress' execution oob' = execution := hNew
    _ = sustainedThroughput ingress execution oob := by
      exact hOld.symm

/-- Lifting execution bottleneck strictly increases sustained throughput. -/
theorem lifting_execution_bottleneck_increases_sustained
    (ingress execution execution' oob : Nat)
    (hLift : execution < execution')
    (hExecPrimeIngress : execution' <= ingress)
    (hExecPrimeOob : execution' <= oob) :
    sustainedThroughput ingress execution oob < sustainedThroughput ingress execution' oob := by
  have hExecIngress : execution <= ingress :=
    Nat.le_trans (Nat.le_of_lt hLift) hExecPrimeIngress
  have hExecOob : execution <= oob :=
    Nat.le_trans (Nat.le_of_lt hLift) hExecPrimeOob
  have hOld : sustainedThroughput ingress execution oob = execution :=
    execution_bottleneck_characterization ingress execution oob hExecIngress hExecOob
  have hNew : sustainedThroughput ingress execution' oob = execution' :=
    execution_bottleneck_characterization ingress execution' oob hExecPrimeIngress hExecPrimeOob
  calc
    sustainedThroughput ingress execution oob = execution := hOld
    _ < execution' := hLift
    _ = sustainedThroughput ingress execution' oob := by exact hNew.symm

/-- Closed-form gain expression for pipelining + partitioning over baseline. -/
theorem pipelined_partitioned_closed_form
    (pipelineDepth shards singleLane : Nat) :
    pipelinedThroughput pipelineDepth shards singleLane =
      (pipelineDepth * shards) * baselineThroughput singleLane := by
  simp [pipelinedThroughput, partitionedThroughput, baselineThroughput, Nat.mul_assoc]

/-- Strict gain for pipelining + partitioning when capacity and width are both non-trivial. -/
theorem pipelined_partitioned_strict_gain
    (pipelineDepth shards singleLane : Nat)
    (hDepth : 1 <= pipelineDepth)
    (hShards : 1 < shards)
    (hLane : 0 < singleLane) :
    baselineThroughput singleLane < pipelinedThroughput pipelineDepth shards singleLane := by
  exact Nat.lt_of_lt_of_le
    (partitioning_strict_gain shards singleLane hShards hLane)
    (pipelining_never_worse pipelineDepth shards singleLane hDepth)

/--
If execution is the active bottleneck before and after scaling by `factor`,
sustained throughput scales exactly by that factor.
-/
theorem sustained_scales_with_execution_until_next_bottleneck
    (ingress execution oob factor : Nat)
    (hExecIngress : execution <= ingress)
    (hExecOob : execution <= oob)
    (hScaledIngress : factor * execution <= ingress)
    (hScaledOob : factor * execution <= oob) :
    sustainedThroughput ingress (factor * execution) oob =
      factor * sustainedThroughput ingress execution oob := by
  have hOld : sustainedThroughput ingress execution oob = execution :=
    execution_bottleneck_characterization ingress execution oob hExecIngress hExecOob
  have hNew : sustainedThroughput ingress (factor * execution) oob = factor * execution :=
    execution_bottleneck_characterization ingress (factor * execution) oob hScaledIngress hScaledOob
  calc
    sustainedThroughput ingress (factor * execution) oob = factor * execution := hNew
    _ = factor * sustainedThroughput ingress execution oob := by
      rw [hOld]

/--
When factor > 1 and execution has positive capacity (with sufficient headroom),
raising execution throughput yields a strict sustained-throughput increase.
-/
theorem sustained_strict_gain_with_execution_factor
    (ingress execution oob factor : Nat)
    (hFactor : 1 < factor)
    (hExecPos : 0 < execution)
    (hExecIngress : execution <= ingress)
    (hExecOob : execution <= oob)
    (hScaledIngress : factor * execution <= ingress)
    (hScaledOob : factor * execution <= oob) :
    sustainedThroughput ingress execution oob <
      sustainedThroughput ingress (factor * execution) oob := by
  have hOld : sustainedThroughput ingress execution oob = execution :=
    execution_bottleneck_characterization ingress execution oob hExecIngress hExecOob
  have hNew : sustainedThroughput ingress (factor * execution) oob = factor * execution :=
    execution_bottleneck_characterization ingress (factor * execution) oob hScaledIngress hScaledOob
  have hMulStrict : execution < factor * execution := by
    calc
      execution = 1 * execution := by simp
      _ < factor * execution := by
        exact Nat.mul_lt_mul_of_pos_right hFactor hExecPos
  calc
    sustainedThroughput ingress execution oob = execution := hOld
    _ < factor * execution := hMulStrict
    _ = sustainedThroughput ingress (factor * execution) oob := by exact hNew.symm

/--
If both ingress and OOB are below execution capacity, sustained throughput is
set by non-execution lanes (`min ingress oob`).
-/
theorem non_execution_bottleneck_characterization
    (ingress execution oob : Nat)
    (hOobExec : oob <= execution) :
    sustainedThroughput ingress execution oob = Nat.min ingress oob := by
  have hInner : Nat.min execution oob = oob := Nat.min_eq_right hOobExec
  simp [sustainedThroughput, hInner]

/--
Once execution has been lifted past both ingress and OOB capacity, further
execution lifts do not increase sustained throughput.
-/
theorem lifting_execution_past_bottleneck_plateaus
    (ingress execution execution' oob : Nat)
    (hOobExec : oob <= execution)
    (hLift : execution <= execution') :
    sustainedThroughput ingress execution' oob =
      sustainedThroughput ingress execution oob := by
  have hOobExec' : oob <= execution' := Nat.le_trans hOobExec hLift
  have hOld : sustainedThroughput ingress execution oob = Nat.min ingress oob :=
    non_execution_bottleneck_characterization ingress execution oob hOobExec
  have hNew : sustainedThroughput ingress execution' oob = Nat.min ingress oob :=
    non_execution_bottleneck_characterization ingress execution' oob hOobExec'
  calc
    sustainedThroughput ingress execution' oob = Nat.min ingress oob := hNew
    _ = sustainedThroughput ingress execution oob := by exact hOld.symm

end ETH2077Proofs.ExecutionOptimization
