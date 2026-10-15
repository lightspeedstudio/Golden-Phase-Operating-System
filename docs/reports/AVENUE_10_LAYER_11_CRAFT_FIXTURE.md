# Avenue 10A — Layer 11 Deterministic Craft Fixture

**Date:** 2026-07-24 (architect intake corrections)  
**Classification:** Avenue **10A** — Layer 11 deterministic craft fixture — **PASS**  
**Avenue 10B** — Glider-carried craft transport — **DONE (hosted-sim over sealed glider law)**; see `AVENUE_10B_wp1_adapter.md`, `AVENUE_10B_wp2_scenario.md`, `../terminal_craft_10b_demo.txt`  
**Architect target dir:** `CARGO_TARGET_DIR=target/architect_avenue10`  
**Standing rule:** The engineering is real; the honest version is the strong version.

---

## 1. Executive verdict

**Conditional PASS cleared to Avenue 10A PASS** after intake corrections:

- expanded tamper matrix with dedicated tests
- partial-recovery failure invariants
- enum/hash freeze tests
- full feature-matrix reproduction
- report claims aligned to measured evidence

Central mission result remains:

`detect → isolate → recover → prove` → `CRAFT FDIR PASS`

Final state: `(N,P,W,F) = (Φ3, Φ0, Φ3, Φ0)`  
(navigation locked, propulsion idle, power protected, FDIR normal).

This is **not** flight-qualified spacecraft software and does **not** claim FPGA craft transport.

---

## 1b. Project split

| Track | Status | Meaning |
|-------|--------|---------|
| **Avenue 10A** | **PASS** | GPL mission states, guards, FDIR, quarantine, recovery, statechain, integrity checkpoint, audited replay |
| **Avenue 10B** | **PENDING** | Replace `LocalCraftTransport` with sealed `phase_transport`; command glider inject/receive/ACK/replay; FDIR over physical carrier |

---

## 2. Mathematical mission cycle

Canonical compose (reused from `src/core/gphase_core.rs`, **not** reimplemented):

```
      0  1  2  3
0  | 0  1  2  3
1  | 1  3  3  2
2  | 2  3  3  1
3  | 3  2  1  0
```

Universal cycle (operands Φ1, Φ3, Φ1, Φ3):

| Step | Compose | Result |
|------|---------|--------|
| Φ0 ∘ Φ1 | | Φ1 |
| Φ1 ∘ Φ3 | | Φ2 |
| Φ2 ∘ Φ1 | | Φ3 |
| Φ3 ∘ Φ3 | | Φ0 |

Propulsion recovery (lawful; no silent Φ2→Φ0):

`Φ2 ∘ Φ1 → Φ3 ∘ Φ3 → Φ0` = firing → fault-contained → idle-safe.

Semantic guards sit **above** compose; they do not replace it.

---

## 3. Craft architecture

```
application / scenario
        ↓
craft command + guard
        ↓
canonical compose (gphase_core)
        ↓
StatechainLedger::record_transition
        ↓
LocalCraftTransport (temporary adapter — Avenue 10B replaces this)
        ↓
FDIR detect → isolate → recover → prove
        ↓
replay + integrity checkpoint
```

Modules under `src/kernel/craft/`. Algebra audit: craft code calls `compose`, `reflect`, `left_group`, `right_group` only — no local composition tables.

---

## 4–7. Subsystem state laws

| Subsystem | Φ0 | Φ1 | Φ2 | Φ3 |
|-----------|----|----|----|-----|
| Navigation | absent | stale | solving | locked |
| Propulsion | idle | armed | firing | fault |
| Power | off | ready | load | protected |
| FDIR | normal | detected | isolated | recovered_pending_proof |

Quarantine clears **only** after chain verify + checkpoint verify + replay agreement + FDIR Φ3→Φ0.

---

## 8. Detection

| Constant | Value |
|----------|------:|
| `PROP_FIRING_MAX_TICKS` | 5 |
| Detected dwell in scenario | 6 |
| Curvature support \|z\| | 3.29 (supporting only) |

Propulsion anomaly: `(P=Φ2 ∧ d_P > 5) ∨ (P=Φ3)`.

---

## 9. Partial-recovery invariant

Any recovery failure ⇒ quarantine remains set and `F_FDIR ≠ Φ0` until proof succeeds.

Dedicated tests (`craft_fdir_partial_recovery_tests.rs`):

- power-protection failure
- propulsion-reset failure
- `recover_propulsion` abort before FDIR recover
- replay failure blocks prove
- chain/checkpoint failure blocks prove

---

## 10–11. Statechain / replay

- `ObjectKind::Craft = 8`, `SubsystemKind::Craft = 6` (appended; priors frozen in tests)
- `TransitionProofOp` is a craft-side channel; Process event hash layout unchanged (`as_u8` discriminants)
- Replay recomputes craft associator parity; does not trust the stored craft bit
- Checkpoint = integrity HMAC (public key derivation) — **not** an unforgeable signature

Prove order (intake-hardened): verify_all → replay match → seal checkpoint → verify_checkpoints → FDIR prove → clear Q.

---

## 12. Propulsion-stuck causal order (required)

1. propulsion armed  
2. propulsion firing  
3. stuck condition injected  
4. dwell reaches 6  
5. fault detected / FDIR detected  
6. FDIR isolated / propulsion quarantined  
7. unsafe PROP_FIRE rejected  
8. propulsion marked fault  
9. power protected  
10. propulsion reset to idle  
11. FDIR recovered pending proof  
12. statechain PASS / checkpoint PASS / replay PASS  
13. live = replayed  
14. FDIR normal / quarantine cleared  
15. `CRAFT FDIR PASS`

---

## 13. Terminal transcript

`docs/terminal_craft_demo.txt`

---

## 14. Test results (architect reproduction)

```powershell
$env:CARGO_TARGET_DIR="target/architect_avenue10"
cargo fmt --check                         # PASS
cargo check                                # PASS
cargo check --features craft               # PASS
cargo test                                 # PASS — 367 passed, 0 failed
cargo test --features craft                # PASS — 415 passed, 0 failed
cargo test --all-features                  # FAIL (pre-existing) — std ∩ baremetal mutual exclusion
cargo run -p craft-mainframe               # PASS — CRAFT FDIR PASS
```

**`--all-features` note:** enabling every Cargo feature simultaneously compiles `std` and `baremetal` together, which this dual-target crate does not support (`platform::String`/`alloc` clashes). This is a pre-existing workspace limitation, not introduced by Avenue 10A. Meaningful matrix is default hosted + `craft`.

Final craft state from live run: `(N,P,W,F)=(Φ3,Φ0,Φ3,Φ0)`.

---

## 15. Tamper matrix (measured)

| Class | Status |
|-------|--------|
| event-hash alteration | TESTED — verify fails |
| parity alteration | TESTED — verify fails |
| object-ID substitution | TESTED — replay fails |
| payload-hash mismatch | TESTED — hash differs |
| live/replayed mismatch | TESTED — detected |
| entry reorder | TESTED — verify fails |
| non-tail deletion | TESTED — verify fails |
| transition operand alteration | TESTED — replay fails |
| recorded old-state alteration | TESTED — verify fails |
| operation-name alteration | TESTED — verify fails |
| episode-link alteration | TESTED — prove fails; Q held |
| checkpoint-head alteration | TESTED — checkpoint verify fails |
| sequence-number gap | TESTED — verify fails |
| subsystem-kind substitution | TESTED — replay fails |
| coordinated chain+cache rewrite | LIMITATION documented (external seal) |

---

## 16–17. Performance / limitations

- Hosted fixture; no FPGA timing claim.  
- `LocalCraftTransport` only (10B pending).  
- Integrity checkpoint ≠ unforgeable auth.  
- Curvature z≈0 in scripted episode; dwell detector alone caught the fault (as designed).

---

## 18. Supported claims (10A)

- Shared GPL mission cycle for nav/prop/power/FDIR  
- Guards + canonical compose  
- Lawful firing→fault→idle recovery  
- Quarantine until proof  
- Partial-recovery failures hold quarantine  
- Expanded tamper matrix as tabled above  
- Enum append compatibility  

## 19. Unsupported / overclaims rejected

- FPGA / physical craft messaging  
- Avenue 10B glider-carried transport  
- Flight qualification  
- Unforgeable mission signatures  

---

## 20. Reproduction

```powershell
cd C:\Users\richa\OneDrive\Desktop\FullOS\kairos
$env:CARGO_TARGET_DIR="target/architect_avenue10"
cargo fmt --check
cargo check
cargo check --features craft
cargo test
cargo test --features craft
cargo test --all-features
cargo run -p craft-mainframe
```

---

## 21. Avenue 10B — DONE (hosted-sim over sealed glider law)

Craft opcodes now bind to the `phase_transport` M=4 glider carrier:
- **WP-1** `PhaseTransportCraftAdapter` (`craft/transport_phase.rs`) — `CraftController.transport` is `Box<dyn CraftTransport>`; `new_over_glider()` routes every command as a Φ2 glider packet (endpoints on the main diagonal per the carrier's (−1,−1) velocity), send → tick(arrival) → `verify_trace` → ACK.
- **WP-2** `prop_stuck_over_glider_scenario` + `verify_packet_replay` — full detect→isolate→recover→prove FDIR over the carrier, packet log cross-checked against the statechain (opcode + ack + assoc_parity), `replay live==replayed`, sealed checkpoint. Live hosted transcript: `../terminal_craft_10b_demo.txt` (10 packets cross-checked, checkpoint seq=0 tick=18).

**Scope honesty:** hosted-simulation over the *sealed Avenue-2 glider law* (the law PHYSICALLY_VERIFIED on iCE40). Not live UART/on-silicon craft packet transport — that remains future work.

Remaining 10B+ ideas: on-silicon packet carrier (live UART); comms/payload subsystems; nav-stuck / power-overload scenarios.

---

## Acceptance gates (10A)

| Gate | Status |
|------|--------|
| Mission fixture CRAFT FDIR PASS | PASS |
| Tamper matrix matches evidence | PASS (see §15) |
| Partial-recovery holds quarantine | PASS |
| Enum Craft=8 / Subsystem=6 frozen | PASS |
| No duplicate craft algebra | PASS |
| Feature matrix (fmt/check/test ±craft) | PASS |
| `cargo test --all-features` | FAIL — pre-existing std∩baremetal (documented) |
| Avenue 10B (hosted-sim over sealed glider law) | PASS |

**Outer FullOS submodule pointer: not bumped (separate step).**
