# ETH2077 Threat Model v1

ETH2077 is an experimental Ethereum client.
It combines a canonical execution pipeline with Citadel-style out-of-band (OOB) consensus primitives.
This document defines the v1 threat model baseline.
It is intended for protocol engineering, security review, and formal verification planning.

Model date is fixed at 2026-03-04.
The model assumes Ethereum-aligned execution semantics.
The model treats OOB consensus as an accelerator layer that must not violate safety.

Security goals in this model are:
- Preserve safety: no conflicting finalized state transitions.
- Preserve liveness under partial synchrony and bounded Byzantine stake.
- Preserve determinism across independent client implementations.
- Preserve data availability required for witness verification and replay.
- Preserve operator control of validator keys and signing authority.

The threat model format is intentionally operational.
Each threat maps to mitigations and explicit assumptions.
Assumptions are owned by named teams and require periodic validation.

## 1. Scope and Boundaries

### Scope Summary

What is in scope: ETH2077 client runtime, OOB consensus layer, execution pipeline, networking/gossip, validator key management.
What is out of scope: L2 rollups, bridge contracts, external MEV infrastructure.

### In-Scope Components

- ETH2077 process runtime and process-supervision model.
- Consensus-critical state machines inside the OOB lane.
- Execution payload admission, deterministic ordering, and import checks.
- Gossip ingress/egress, peer scoring, anti-amplification controls.
- Validator signing flow, key custody interfaces, slashing protection.
- Local persistence for consensus metadata and witness artifacts.
- Bootstrapping, peer discovery, and trusted checkpoint ingestion.
- Telemetry pipelines used for safety/liveness decisioning.

### Out-of-Scope Components

- L2 rollup sequencer logic and rollup fraud/validity proofs.
- Bridge contract economics, cross-domain message relays, and bridge guardians.
- External builder markets and third-party MEV relay trust assumptions.
- Custodial exchange operational security not operated by ETH2077 maintainers.
- Cloud-provider account compromise outside ETH2077-owned environments.

### Security-Critical Assets

- Validator private keys and signer authorization tokens.
- OOB fast-path accumulator commitments and quorum certificates.
- Finality witness data, inclusion proofs, and attestation payloads.
- Canonical block ordering decisions accepted by execution import.
- Peer identity and reputation state used for anti-eclipse controls.
- Trusted checkpoint roots used for weak-subjectivity protection.
- Build artifacts and dependency lockfiles used in releases.

### Trust Boundaries

- Boundary A: external network edge to ETH2077 P2P ingress.
- Boundary B: OOB consensus module to execution import gate.
- Boundary C: validator signer process to runtime over IPC or remote signer RPC.
- Boundary D: on-disk storage to process memory during decode and replay.
- Boundary E: CI/build infrastructure to signed release artifacts.

### Entry Points Considered by This Model

- Peer discovery responses and gossip message streams.
- RPC calls that influence forkchoice and payload admission.
- OOB bilateral coordination protocol message handlers.
- SPORE diff-sync object fetch and chunk reconciliation endpoints.
- Validator signing requests and slashing-protection state updates.
- Dependency update pipeline and release publication workflows.

### Security Invariants for v1

- Invariant S1: finalized execution roots never diverge among honest nodes.
- Invariant S2: OOB path cannot force invalid execution payload acceptance.
- Invariant S3: replayed or stale consensus messages are rejected deterministically.
- Invariant S4: key compromise blast radius is bounded by isolation controls.
- Invariant S5: any safety-critical failure emits actionable telemetry.

### Explicit Boundary Decisions

- ETH2077 models builder data as untrusted input even when signed.
- ETH2077 treats transport encryption as necessary but not sufficient.
- ETH2077 assumes local root compromise defeats most runtime mitigations.
- ETH2077 assumes upstream chain-level cryptography is unbroken.
- ETH2077 does not claim to secure user wallets or dapp frontends.

### Methodology Notes

- Threat identification uses STRIDE-inspired classification with BFT overlays.
- Impact scoring is chain safety first, then liveness, then operator cost.
- Likelihood scoring reflects expected testnet/mainnet exposure.
- Residual risk is accepted only with explicit owner and revisit cadence.
- This is a living document and will evolve with protocol maturity.

## 2. Adversary Classes

ETH2077 defines four primary adversary classes.
Each class is realistic for public blockchain operation.
Classes can compose in real incidents.
Threat tables reference one primary class per row for tractability.

### ADV-NET

- Name: **ADV-NET**.
- Type: Network-level adversary.
- Attack examples: eclipse attacks, partition attacks, BGP hijack, latency manipulation.
- Capability: can delay/drop/reorder messages between any set of nodes.
- Practical limits: cannot break authenticated cryptography directly.
- Typical resources: ISP influence, route hijack infrastructure, botnet relays.

### ADV-CRYPTO

- Name: **ADV-CRYPTO**.
- Type: Cryptographic adversary.
- Attack examples: signature forgery, hash collision, RNG weakness.
- Capability: bounded by computational hardness assumptions.
- Practical limits: cannot exceed assumed security level without new breaks.
- Typical resources: specialized compute, side-channel labs, malware for key extraction.

### ADV-BYZ

- Name: **ADV-BYZ**.
- Type: Byzantine validator adversary.
- Attack examples: equivocation, withholding, censorship.
- Capability: controls up to f < n/3 validator stake (standard BFT bound).
- Practical limits: cannot finalize conflicting histories if assumptions hold.
- Typical resources: coordinated validator cluster, bribery budget, cartel governance.

### ADV-IMPL

- Name: **ADV-IMPL**.
- Type: Implementation adversary.
- Attack examples: buffer overflow, logic bugs, dependency supply chain.
- Capability: exploits software defects in ETH2077 codebase.
- Practical limits: constrained by code hardening, testing, and runtime isolation.
- Typical resources: 0-day exploit development, malicious package publication, CI compromise.

### Cross-Class Composition Assumptions

- ADV-NET plus ADV-BYZ increases censorship and withholding effectiveness.
- ADV-IMPL can create cryptographic downgrade opportunities if unchecked.
- ADV-NET can magnify side-channel practicality via timing manipulation.
- ADV-BYZ may rent external network manipulation capabilities.
- Joint adversaries are assumed in worst-case tabletop exercises.

### Adversary Objectives Considered

- Force safety failure by finalizing conflicting state roots.
- Force liveness failure by stalling progress for extended epochs.
- Extract value through censorship, ordering manipulation, or griefing.
- Degrade operator trust in ETH2077 implementation reliability.
- Induce key misuse or compromise for persistent validator control.

## 3. Threat Categories

### 3.1 Network Threats

| ID | Threat | Adversary | Impact | Mitigation | Assumption Ref |
| --- | --- | --- | --- | --- | --- |
| THREAT-NET-001 | Eclipse attack against validator or proposer nodes | ADV-NET | Isolates target view; can induce conflicting local forkchoice and delayed finality | Peer diversity quotas by ASN/geo, authenticated peer identity, aggressive peer rotation, eclipse-detection heuristics | ASSUME-001, ASSUME-004, ASSUME-005, ASSUME-025 |
| THREAT-NET-002 | Partition attack across validator subsets | ADV-NET | Causes temporary consensus split, delayed finality, and slash-risk on recovery | Quorum-aware partition detectors, delayed commit under high divergence, multi-path relay networks, recovery runbooks | ASSUME-001, ASSUME-006, ASSUME-025 |
| THREAT-NET-003 | Message replay of stale consensus/gossip packets | ADV-NET | Re-injection of stale votes/witnesses increases confusion and CPU load; may trigger false penalties | Strict nonce+slot+epoch binding, TTL enforcement, replay caches, domain separation by chain ID and protocol version | ASSUME-002, ASSUME-003, ASSUME-026 |
| THREAT-NET-004 | Gossip flooding and amplification abuse | ADV-NET | Resource exhaustion, dropped legitimate traffic, liveness degradation | Token-bucket ingress limits, per-peer score decay, bounded fanout, adaptive backpressure and ban thresholds | ASSUME-005, ASSUME-024 |
| THREAT-NET-005 | Time manipulation (clock skew injection) | ADV-NET | Incorrect slot attribution and premature timeout transitions | Monotonic clock source, NTP/PTP sanity windows, cross-peer time median checks, bounded skew tolerance | ASSUME-003, ASSUME-006 |
| THREAT-NET-006 | DNS poisoning for bootstrap and seed discovery | ADV-NET | Redirects nodes to hostile peers and increases eclipse probability | DNSSEC validation, pinned bootstrap keys, out-of-band seed lists, bootstrap authenticity audits | ASSUME-004, ASSUME-025 |

Network threat analysis and rationale:

1. THREAT-NET-001 analysis.
- Primary failure mode is loss of viewpoint diversity at consensus edges.
- OOB fast-path is especially sensitive to clustered peer overlap.
- Detection signal is sudden drop in unique ASNs among active peers.
- Detection signal is sustained disagreement with checkpoint majority roots.
- Mitigation effectiveness depends on continuous peer reshuffling.
- Residual risk remains medium when infrastructure centralization is high.

2. THREAT-NET-002 analysis.
- Partition risk is elevated during cloud regional outages and route leaks.
- ETH2077 must avoid unsafe commits when quorum health is ambiguous.
- Detection signal is validator vote entropy collapse between clusters.
- Detection signal is delayed witness availability crossing configured SLA.
- Recovery requires deterministic merge logic and replay-safe reconciliation.
- Residual risk is high impact but lower likelihood than flooding.

3. THREAT-NET-003 analysis.
- Replay attacks target parser, cache, and penalty logic simultaneously.
- Consensus votes without strict domain tags are replay-vulnerable.
- Detection signal is abnormal duplicate rate by sender fingerprint.
- Detection signal is stale epoch messages arriving in concentrated bursts.
- Mitigation requires both cryptographic binding and operational TTL.
- Residual risk is controlled if caches resist eviction attacks.

4. THREAT-NET-004 analysis.
- Flooding is a practical, low-cost attack on public P2P surfaces.
- Attackers may blend valid-looking low-value objects to evade filters.
- Detection signal is queue depth growth with falling useful throughput.
- Detection signal is increasing CPU parse cost per accepted object.
- Mitigation relies on early drop decisions before expensive verification.
- Residual risk is moderate due to constant attacker adaptation.

5. THREAT-NET-005 analysis.
- Time skew can induce local misclassification of honest messages.
- OOB deadline logic amplifies impact of local clock corruption.
- Detection signal is local clock drift outside monotonic expected window.
- Detection signal is inconsistent peer slot claims versus global median.
- Mitigation includes fail-closed guardrails on extreme skew states.
- Residual risk is medium where time sources are homogeneous.

6. THREAT-NET-006 analysis.
- Bootstrap poisoning is often a precursor to eclipse execution.
- DNS-only trust is insufficient for consensus-critical bootstrapping.
- Detection signal is bootstrap host key mismatch or sudden churn.
- Detection signal is unusual concentration of peer IDs at startup.
- Mitigation depends on signed seeds and multi-channel distribution.
- Residual risk is reduced by periodic bootstrap integrity drills.

### 3.2 Cryptographic Threats

| ID | Threat | Adversary | Impact | Mitigation | Assumption Ref |
| --- | --- | --- | --- | --- | --- |
| THREAT-CRYPTO-001 | Signature forgery against validator attestations or certificates | ADV-CRYPTO | Fake quorum evidence could break safety if accepted | Strong signature schemes, strict domain separation tags, signature aggregation verification hardening, key isolation | ASSUME-007, ASSUME-021 |
| THREAT-CRYPTO-002 | Hash collision/preimage attack on commitment structures | ADV-CRYPTO | Commitment ambiguity can allow conflicting content under same digest | Collision-resistant hash choice, transcript binding, versioned hash agility plan, multi-hash transition path | ASSUME-008, ASSUME-026 |
| THREAT-CRYPTO-003 | RNG bias or entropy failure in key generation/nonces | ADV-CRYPTO | Predictable keys/nonces lead to key recovery and forged signatures | CSPRNG health tests, hardware entropy mixing, deterministic signing where applicable, signer attestation audits | ASSUME-009, ASSUME-021 |
| THREAT-CRYPTO-004 | Side-channel leakage during signing or key handling | ADV-CRYPTO | Partial secret leakage enables progressive key extraction | Constant-time cryptography, HSM/remote signer isolation, cache/timing hardening, process pinning and noise | ASSUME-010, ASSUME-021, ASSUME-028 |

Cryptographic threat analysis and rationale:

1. THREAT-CRYPTO-001 analysis.
- Forgery remains catastrophic even with low practical likelihood.
- Domain separation errors are a realistic implementation vector.
- Detection signal is signature acceptance anomalies across clients.
- Detection signal is quorum certificate shape mismatch against spec.
- Mitigation includes differential verification across independent libs.
- Residual risk follows upstream cryptographic research progress.

2. THREAT-CRYPTO-002 analysis.
- Full collisions are unlikely but must be modeled for agility planning.
- Commitment schemes must bind context, epoch, and protocol version.
- Detection signal is duplicate digest with inconsistent decoded payload.
- Detection signal is hash-validation disagreement in canary nodes.
- Mitigation is strongest when migration playbooks are pre-approved.
- Residual risk is low current likelihood with very high impact.

3. THREAT-CRYPTO-003 analysis.
- RNG failures historically cause practical key compromise incidents.
- Remote signer fleets are vulnerable to correlated entropy bugs.
- Detection signal is nonce reuse or suspicious entropy statistics.
- Detection signal is shared key fingerprints across deployments.
- Mitigation requires startup entropy checks and continuous telemetry.
- Residual risk is medium due to operational misconfiguration risk.

4. THREAT-CRYPTO-004 analysis.
- Side-channels become realistic in multi-tenant infrastructure.
- Signing frequency and predictability can amplify leakage rates.
- Detection signal is abnormal timing variance during signing calls.
- Detection signal is unexplained signer CPU cache contention.
- Mitigation requires hardware and software isolation together.
- Residual risk depends on operator discipline and tenancy model.

### 3.3 Consensus Threats

| ID | Threat | Adversary | Impact | Mitigation | Assumption Ref |
| --- | --- | --- | --- | --- | --- |
| THREAT-CONS-001 | Equivocation by validator set members | ADV-BYZ | Conflicting votes/certificates increase slash events and safety pressure | Double-sign detection, slashing enforcement, certificate uniqueness rules, signer-side anti-equivocation database | ASSUME-011, ASSUME-015 |
| THREAT-CONS-002 | Long-range attack using stale keys/checkpoints | ADV-BYZ | Historical rewrite attempts can mislead weakly synchronized nodes | Weak-subjectivity checkpointing, checkpoint signature validation, bounded historical sync windows, finalized root pinning | ASSUME-012, ASSUME-013 |
| THREAT-CONS-003 | Nothing-at-stake behavior in competing branches | ADV-BYZ | Increased fork surface and delayed convergence under incentive misalignment | Economic penalties, branch-choice accountability, reward policy for timely canonical attestations, slashing proofs | ASSUME-011, ASSUME-013 |
| THREAT-CONS-004 | Censorship attack on transactions or witnesses | ADV-BYZ | Fairness failure, inclusion delays, potential liveness degradation | Inclusion list mechanisms, relay diversity, censorship alarms, fallback proposer rotation and witness rebroadcast | ASSUME-014, ASSUME-001, ASSUME-006 |
| THREAT-CONS-005 | Finality reversion via coordinated Byzantine timing | ADV-BYZ | Safety failure if conflicting finality is accepted across partitions | Two-phase commit checks, quorum intersection proofs, delayed finality under uncertainty, audit-ready witness logs | ASSUME-013, ASSUME-015, ASSUME-016 |
| THREAT-CONS-006 | OOB fast-path manipulation to bias ordering/finality hints | ADV-BYZ | Can front-run ordering decisions and induce unsafe pressure on execution import | Fast-path bounded authority, mandatory slow-path reconciliation, quorum certificate verification, deterministic tie-breakers | ASSUME-015, ASSUME-016, ASSUME-030 |

Consensus threat analysis and rationale:

1. THREAT-CONS-001 analysis.
- Equivocation is expected and must be survivable by design.
- ETH2077 must make equivocation evidence cheap to verify.
- Detection signal is duplicate vote signatures for same round role.
- Detection signal is signer database conflict on monotonic counters.
- Mitigation quality depends on rapid slash-evidence propagation.
- Residual risk is medium due to operational signer mistakes.

2. THREAT-CONS-002 analysis.
- Long-range risk targets nodes that sync from untrusted history.
- Weak-subjectivity intervals must be operationally enforced.
- Detection signal is checkpoint chain mismatch against trusted root.
- Detection signal is deep-history certificate density anomalies.
- Mitigation depends on social distribution of trusted checkpoints.
- Residual risk rises if operators ignore checkpoint freshness policy.

3. THREAT-CONS-003 analysis.
- Nothing-at-stake behavior can appear as rational strategy drift.
- Incentive design and punishment certainty are both required.
- Detection signal is repeated multi-branch attestations per validator.
- Detection signal is branch support oscillation near decision boundary.
- Mitigation needs measurable accountability and timely enforcement.
- Residual risk is low-medium if slashing enforcement is credible.

4. THREAT-CONS-004 analysis.
- Censorship attacks are common under profit or policy pressures.
- OOB witness channels can be selectively suppressed.
- Detection signal is consistent exclusion of eligible transactions.
- Detection signal is witness arrival skew by relay path and region.
- Mitigation relies on path diversity and objective inclusion SLOs.
- Residual risk remains high under coordinated cartel behavior.

5. THREAT-CONS-005 analysis.
- Finality reversion is a top-severity event in any BFT system.
- ETH2077 must enforce strict quorum-intersection preconditions.
- Detection signal is conflicting finality certificates at same height.
- Detection signal is delayed witness reconciliation across clusters.
- Mitigation includes fail-stop behavior when certificate checks fail.
- Residual risk is low likelihood but critical impact.

6. THREAT-CONS-006 analysis.
- Fast-path gives performance gains and additional attack surface.
- Manipulated hints must never bypass canonical validity checks.
- Detection signal is divergence between fast-path and slow-path order.
- Detection signal is repeated fast-path leader dominance anomalies.
- Mitigation requires explicit precedence of safety over latency.
- Residual risk depends on correctness of reconciliation logic.

### 3.4 Implementation Threats

| ID | Threat | Adversary | Impact | Mitigation | Assumption Ref |
| --- | --- | --- | --- | --- | --- |
| THREAT-IMPL-001 | Memory safety defects in non-Rust or unsafe boundaries (mitigated by Rust) | ADV-IMPL | Potential remote code execution or state corruption | Rust-first policy, unsafe code review gates, sanitizer runs for FFI, memory-hardening CI profiles | ASSUME-017, ASSUME-027 |
| THREAT-IMPL-002 | Dependency supply chain compromise | ADV-IMPL | Backdoored artifacts can subvert runtime logic or leak keys | Locked dependency graph, provenance verification, reproducible builds, signature verification for releases | ASSUME-018, ASSUME-023 |
| THREAT-IMPL-003 | Integer overflow/underflow in consensus or accounting logic | ADV-IMPL | Incorrect state transitions, denial of service, potential consensus divergence | Checked arithmetic, saturating guards where needed, property-based tests, static analysis for numeric edges | ASSUME-019, ASSUME-027 |
| THREAT-IMPL-004 | Serialization/deserialization bugs and schema confusion | ADV-IMPL | Consensus split risk from parser disagreement or object confusion | Canonical serialization schema, strict length/type checks, differential fuzzing, reject-unknown critical fields | ASSUME-020, ASSUME-026, ASSUME-027 |

Implementation threat analysis and rationale:

1. THREAT-IMPL-001 analysis.
- Rust significantly reduces but does not eliminate memory risk.
- Unsafe blocks and FFI crossings are primary residual vectors.
- Detection signal is sanitizer crash or UB trace in CI fuzz jobs.
- Detection signal is malformed input causing parser panic loops.
- Mitigation requires mandatory unsafe rationale and code ownership.
- Residual risk is concentrated in performance-critical modules.

2. THREAT-IMPL-002 analysis.
- Supply chain attacks remain one of the highest practical risks.
- A single compromised crate can bypass protocol-level controls.
- Detection signal is lockfile drift outside approved change window.
- Detection signal is provenance mismatch in artifact attestation.
- Mitigation requires reproducible builds and offline verification.
- Residual risk remains medium-high due to ecosystem breadth.

3. THREAT-IMPL-003 analysis.
- Numeric edge cases often emerge under adversarial load.
- Overflow in timeout or stake math can alter consensus behavior.
- Detection signal is impossible-value telemetry at runtime.
- Detection signal is differential test mismatch near boundary inputs.
- Mitigation should prefer explicit newtypes for critical domains.
- Residual risk is medium until exhaustive boundary tests mature.

4. THREAT-IMPL-004 analysis.
- Serialization disagreement is a classic consensus split trigger.
- OOB witness payloads add additional schema complexity.
- Detection signal is decode disagreement across client variants.
- Detection signal is high reject rate after schema version bump.
- Mitigation requires strict canonical forms and fuzz corpora growth.
- Residual risk declines with continuous differential testing.

## 4. OOB Consensus Specific Threats

This section isolates threats unique to ETH2077's out-of-band consensus.
These threats exist even when canonical Ethereum execution logic is correct.
The OOB lane is treated as performance-critical but safety-subordinate.
Any disagreement between OOB and canonical checks must resolve in favor of canonical safety.

### OOB Threat Register

| ID | Threat | Adversary | Impact | Mitigation | Assumption Ref |
| --- | --- | --- | --- | --- | --- |
| THREAT-OOB-001 | Fast-path accumulator manipulation | ADV-BYZ | Biased ordering hints, premature confidence signals, potential safety pressure if unchecked | Accumulator commitments signed by quorum, mandatory slow-path confirmation, accumulator consistency proofs, slashable invalid hints | ASSUME-015, ASSUME-016, ASSUME-030 |
| THREAT-OOB-002 | Bilateral coordination protocol attacks | ADV-NET | Session hijack/downgrade and asymmetric view induction between peers | Mutual authentication, transcript binding to epoch/role, anti-downgrade negotiation, retry via independent channels | ASSUME-030, ASSUME-001, ASSUME-006 |
| THREAT-OOB-003 | SPORE diff-sync poisoning | ADV-IMPL | Malicious diff objects can trigger invalid witness reconstruction or CPU/memory exhaustion | Merkle-bound chunk verification, chunk-size limits, staged decode, poison-object quarantine and peer penalty | ASSUME-020, ASSUME-005, ASSUME-024 |
| THREAT-OOB-004 | Witness data withholding | ADV-BYZ | Prevents independent verification, delays finality confirmation, can force conservative fallback | Data availability sampling, witness escrow committees, deadline-triggered fallback to canonical slow path, withholding penalties | ASSUME-016, ASSUME-014, ASSUME-006 |

### OOB Threat Deep-Dive

1. THREAT-OOB-001 fast-path accumulator manipulation.
- Attack path: malicious validator coalition emits syntactically valid but strategically biased accumulator updates.
- Attack path: updates target latency-sensitive nodes to shape provisional ordering.
- Attack precondition: nodes rely on fast-path confidence without strict slow-path reconciliation.
- Safety concern: execution import may face pressure to accept unstable ordering hints.
- Liveness concern: repeated reconciliation failures increase fallback frequency.
- Control: require quorum certificate and accumulator monotonicity checks.
- Control: require deterministic replay of accumulator transitions from raw witness set.
- Detection: alert on abnormal fast-path/slow-path divergence ratio by epoch.
- Detection: alert on leader concentration above expected random baseline.
- Residual risk: medium until formal proof of reconciliation soundness is complete.

2. THREAT-OOB-002 bilateral coordination protocol attacks.
- Attack path: active MITM forces parameter downgrade or selective transcript truncation.
- Attack path: attacker delays one side to induce asymmetric round views.
- Attack precondition: incomplete transcript authentication or weak negotiation binding.
- Safety concern: inconsistent local interpretation of same coordination round.
- Liveness concern: repeated retries under adversarial packet shaping.
- Control: bind every step to session ID, epoch, role, and transcript hash.
- Control: reject non-monotonic state transitions and duplicate terminal states.
- Detection: track mismatch rate of transcript hashes for paired participants.
- Detection: track downgrade negotiation attempts and abort counts.
- Residual risk: medium in highly adversarial network perimeters.

3. THREAT-OOB-003 SPORE diff-sync poisoning.
- Attack path: malicious peers serve validly framed but semantically corrupt diffs.
- Attack path: attacker crafts high-cost diff graphs to maximize decode overhead.
- Attack precondition: decoder accepts deep object graphs before full validation.
- Safety concern: poisoned diffs can cause divergent reconstructed witness states.
- Liveness concern: decode stalls and memory pressure reduce consensus throughput.
- Control: verify each chunk against root commitment before graph materialization.
- Control: enforce strict depth, fanout, and byte-budget limits.
- Detection: anomaly detection on decode cost per accepted byte.
- Detection: quarantine peers with repeated poison-object signatures.
- Residual risk: medium due to ongoing format evolution.

4. THREAT-OOB-004 witness data withholding.
- Attack path: validators vote but delay or omit witness payload publication.
- Attack path: coalition selectively withholds from specific regions or peers.
- Attack precondition: insufficient independent witness retrieval paths.
- Safety concern: confidence in finality claims exceeds verifiable evidence.
- Liveness concern: prolonged fallback to slow path and throughput collapse.
- Control: availability sampling with objective quorum thresholds.
- Control: explicit withholding penalties linked to missed publication deadlines.
- Detection: witness publication latency SLO breaches by validator.
- Detection: region-specific witness gap metrics and correlation alarms.
- Residual risk: high impact; requires robust economic and protocol deterrence.

### OOB Security Design Rules

- OOB votes and witnesses are never trusted without verifiable linkage to canonical state roots.
- Fast-path outputs are hints until slow-path validation confirms consistency.
- OOB modules must be deterministic under replay with identical input transcript.
- OOB data plane is rate-limited independently from canonical gossip plane.
- Witness availability is a first-class safety signal, not an optional metric.
- Any OOB protocol version mismatch must fail closed on consensus-critical paths.

### OOB Validation Program for v1

- Property tests for accumulator monotonicity and conflict detection.
- Differential tests between OOB replay engine and canonical execution import gate.
- Fault-injection for witness withholding and delayed publication scenarios.
- Differential fuzzing for SPORE decoder and chunk reassembly logic.
- Transcript-level conformance tests for bilateral coordination state machine.
- Chaos tests for network delay/reorder patterns against fallback behavior.

## 5. Assumption Dependencies

- ASSUME-001: Validator nodes can maintain diverse peers across independent ASNs and regions during normal operation. — Owner: Networking Team, Validation: peer-diversity telemetry plus quarterly chaos partition drills, Revisit: Quarterly
- ASSUME-002: Gossip objects include unique nonce, slot, epoch, and bounded TTL suitable for replay rejection. — Owner: P2P Team, Validation: protocol conformance tests and replay-fuzz suite, Revisit: Quarterly
- ASSUME-003: Node clocks are monotonic and bounded-skew under hardened NTP/PTP discipline. — Owner: SRE Team, Validation: clock-skew monitors and drift fault-injection exercises, Revisit: Monthly
- ASSUME-004: Bootstrap discovery records are authenticated via DNSSEC and pinned bootstrap identities. — Owner: Networking Team, Validation: DNSSEC verification tests and seed-key rotation drills, Revisit: Quarterly
- ASSUME-005: Ingress rate limits and peer scoring are active and tuned to suppress amplification abuse. — Owner: P2P Team, Validation: adversarial load testing and scorecard regression checks, Revisit: Monthly
- ASSUME-006: Partial synchrony eventually holds after disruption, enabling protocol recovery. — Owner: Consensus Team, Validation: long-run network simulation with bounded-delay convergence criteria, Revisit: Semiannual
- ASSUME-007: Signature schemes in use retain expected hardness at deployed security parameters. — Owner: Cryptography Team, Validation: cryptographic review and parameter tracking against standards updates, Revisit: Semiannual
- ASSUME-008: Hash function choices remain collision and preimage resistant for model horizon. — Owner: Cryptography Team, Validation: annual cryptographic review and agility readiness tests, Revisit: Annual
- ASSUME-009: Entropy sources provide sufficient unpredictability for key generation and nonce derivation. — Owner: Security Engineering, Validation: entropy health metrics and startup self-tests in production, Revisit: Monthly
- ASSUME-010: Signing and key-handling paths are constant-time at relevant security boundaries. — Owner: Cryptography Team, Validation: side-channel analysis plus timing variance benchmarks, Revisit: Semiannual
- ASSUME-011: Equivocation evidence is slashable and penalties are enforceable in policy and implementation. — Owner: Consensus Team, Validation: slashing integration tests and incident tabletop replay, Revisit: Quarterly
- ASSUME-012: Weak-subjectivity checkpoints are distributed to operators with sufficient freshness. — Owner: Protocol Operations, Validation: checkpoint distribution audit and stale-checkpoint alerts, Revisit: Quarterly
- ASSUME-013: Economic finality assumptions hold with honest stake maintaining >2/3 threshold. — Owner: Research Team, Validation: stake distribution monitoring and adversarial economics review, Revisit: Quarterly
- ASSUME-014: Anti-censorship relay diversity is sufficient to prevent persistent single-path suppression. — Owner: Networking Team, Validation: relay diversity metrics and censorship simulation campaigns, Revisit: Quarterly
- ASSUME-015: Fast-path certificates require quorum intersection compatible with BFT safety proofs. — Owner: Consensus Team, Validation: formal model checking and executable spec conformance tests, Revisit: Quarterly
- ASSUME-016: Witness data availability committees and sampling thresholds are correctly parameterized. — Owner: Consensus Team, Validation: availability-sampling simulation and withholding stress tests, Revisit: Monthly
- ASSUME-017: Rust memory safety guarantees hold except in explicitly reviewed unsafe/FFI boundaries. — Owner: Runtime Team, Validation: unsafe-code audit gates plus sanitizer CI on FFI modules, Revisit: Quarterly
- ASSUME-018: Dependency graph is locked and provenance metadata is verifiable end to end. — Owner: Release Engineering, Validation: SBOM/provenance checks in CI and release signing verification, Revisit: Monthly
- ASSUME-019: Consensus-critical arithmetic uses checked operations with explicit overflow policy. — Owner: Runtime Team, Validation: static analysis and boundary property tests, Revisit: Quarterly
- ASSUME-020: Serialization formats are canonical, versioned, and consistently implemented across modules. — Owner: Runtime Team, Validation: cross-client differential codec testing and fuzz corpus growth, Revisit: Monthly
- ASSUME-021: Validator key management uses isolated signers/HSMs with strict access control. — Owner: Security Engineering, Validation: key-management audit and signer penetration testing, Revisit: Quarterly
- ASSUME-023: Build and release pipeline artifacts are reproducible and signed by trusted keys. — Owner: Release Engineering, Validation: reproducible-build attestation and signature verification audit, Revisit: Quarterly
- ASSUME-024: Telemetry and alerting pipelines provide low-latency detection for consensus anomalies. — Owner: SRE Team, Validation: alert fire-drills with measured MTTA/MTTR, Revisit: Monthly
- ASSUME-025: BGP hijack and route anomalies are detectable with multi-provider monitoring. — Owner: Networking Team, Validation: route-monitoring integration and hijack simulation exercises, Revisit: Quarterly
- ASSUME-026: Domain separation and chain-version tagging are uniformly applied to signed and hashed objects. — Owner: Cryptography Team, Validation: schema linting and protocol transcript audits, Revisit: Quarterly
- ASSUME-027: Differential testing against reference Ethereum clients catches semantic divergence early. — Owner: Runtime Team, Validation: nightly differential replay on adversarial corpora, Revisit: Monthly
- ASSUME-028: Runtime least-privilege and process isolation are enforced in production deployments. — Owner: Security Engineering, Validation: hardening baseline scans and privilege boundary tests, Revisit: Quarterly
- ASSUME-030: Bilateral coordination sessions use authenticated, transcript-bound state transitions. — Owner: OOB Protocol Team, Validation: protocol model checking and interop conformance suites, Revisit: Quarterly

## 6. Risk Matrix

Risk matrix covers the top 10 threats by combined impact, likelihood, and exploit practicality.
Likelihood and impact use a 1-5 ordinal scale.

Likelihood scale:
- L1 Rare: unlikely within one year of operation.
- L2 Unlikely: plausible but not expected without strong preconditions.
- L3 Possible: credible in normal adversarial conditions.
- L4 Likely: expected to be attempted and occasionally succeed.
- L5 Almost Certain: routinely attempted with frequent local success.

Impact scale:
- I1 Negligible: no safety effect; limited local operational cost.
- I2 Minor: transient degradation with straightforward recovery.
- I3 Moderate: measurable liveness loss or operator burden.
- I4 Major: severe degradation, slash risk, or prolonged instability.
- I5 Critical: safety failure, finality compromise, or chain-wide trust event.

### Top 10 Threats Included in Matrix

- THREAT-NET-001
- THREAT-NET-002
- THREAT-NET-004
- THREAT-CRYPTO-003
- THREAT-CONS-001
- THREAT-CONS-004
- THREAT-CONS-005
- THREAT-CONS-006
- THREAT-IMPL-002
- THREAT-OOB-004

### 5x5 Likelihood vs Impact Matrix

| Likelihood \ Impact | I1 Negligible | I2 Minor | I3 Moderate | I4 Major | I5 Critical |
| --- | --- | --- | --- | --- | --- |
| L5 Almost Certain |  |  | THREAT-NET-004 |  |  |
| L4 Likely |  |  |  | THREAT-NET-001, THREAT-CONS-004 | THREAT-OOB-004 |
| L3 Possible |  |  |  | THREAT-CRYPTO-003, THREAT-IMPL-002 | THREAT-NET-002, THREAT-CONS-001, THREAT-CONS-006 |
| L2 Unlikely |  |  |  |  | THREAT-CONS-005 |
| L1 Rare |  |  |  |  |  |

### Risk Ranking and Treatment Targets

1. THREAT-OOB-004 at L4/I5.
- Primary concern is verifiability collapse under selective withholding.
- Target treatment: enforce witness publication penalties and rapid fallback.
- Target metric: p95 witness availability latency below configured deadline.

2. THREAT-CONS-006 at L3/I5.
- Primary concern is manipulation of fast-path confidence before reconciliation.
- Target treatment: mandatory slow-path confirmation and deterministic tie-break.
- Target metric: zero unsafe imports where fast-path diverges.

3. THREAT-CONS-005 at L2/I5.
- Primary concern is finality reversion under coordinated Byzantine timing.
- Target treatment: strict quorum-intersection proofs and fail-stop checks.
- Target metric: zero conflicting finality certificates accepted.

4. THREAT-NET-002 at L3/I5.
- Primary concern is prolonged partition across validator cohorts.
- Target treatment: partition-aware commit delay and recovery automation.
- Target metric: bounded recovery time after synthetic partition tests.

5. THREAT-CONS-001 at L3/I5.
- Primary concern is equivocation and delayed slash evidence propagation.
- Target treatment: faster evidence gossip and signer anti-equivocation DB hardening.
- Target metric: slash evidence propagation within one epoch.

6. THREAT-NET-001 at L4/I4.
- Primary concern is validator eclipse causing local view distortion.
- Target treatment: aggressive peer diversity and startup seed hardening.
- Target metric: minimum active peer diversity threshold by ASN and region.

7. THREAT-CONS-004 at L4/I4.
- Primary concern is sustained censorship via cartel coordination.
- Target treatment: relay diversity and inclusion-list enforcement.
- Target metric: inclusion latency SLO for eligible transactions/witnesses.

8. THREAT-CRYPTO-003 at L3/I4.
- Primary concern is entropy failure causing key/nonce predictability.
- Target treatment: signer entropy health checks and deterministic nonce policy.
- Target metric: zero nonce reuse incidents in signer telemetry.

9. THREAT-IMPL-002 at L3/I4.
- Primary concern is compromised dependency or artifact provenance.
- Target treatment: reproducible builds and mandatory provenance verification.
- Target metric: 100% release artifact attestation coverage.

10. THREAT-NET-004 at L5/I3.
- Primary concern is persistent flooding pressure on gossip ingress.
- Target treatment: adaptive backpressure and pre-verify drop policy.
- Target metric: sustained useful throughput under adversarial load profile.

### Residual Risk Acceptance Notes

- No L5 impact risk is acceptable without a documented compensating control plan.
- Any risk with I5 requires quarterly security review sign-off.
- Threats mapped to OOB components require joint review with consensus and runtime owners.
- Risk ratings must be recalibrated after major protocol or network topology changes.
- Matrix scores are conservative defaults and can only move down with evidence.

## 7. Revision History

| Version | Date | Author | Changes |
| --- | --- | --- | --- |
| v1 | 2026-03-04 | ETH2077 Team | Initial threat model |
