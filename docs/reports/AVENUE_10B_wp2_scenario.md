# Avenue 10B, WP-2 — End-to-End FDIR-over-Carrier Scenario + Captured Transcript

**Date:** 2026-07-24
**Scope:** Build on WP-1's `CraftController::new_over_glider()` /
`PhaseTransportCraftAdapter` to run the full propulsion-stuck FDIR mission
with every command physically crossing the (hosted-sim) Avenue 2 glider
carrier, then cross-check the transported packet log against the audited
statechain via packet replay. Capture a real hosted-shell transcript and add
an integration test suite.
**Standing rules honored:** `compose` is never reimplemented — the new
scenario function calls the existing `prop_stuck_scenario`, which itself
only calls `CraftController::execute`/`apply_fdir`, both of which import
`compose` from `core::gphase_core`. No git commit made — all work is left
in the tree.

**HONESTY NOTE (do not skip):** this is a **hosted-simulation** scenario.
`CraftController::new_over_glider()` wires the craft fixture to
`phase_transport::TransportController`, which executes the M=4 diagonal
lattice's compose/reflect/glider law **entirely in software**, in-process.
No FPGA, no serial link, no physical hardware was touched by this work
package. The glider law being simulated is the same one that was
separately silicon-verified on iCE40/iCE Max (see `fpga/evidence/`), but
running it here is not a claim of live-on-silicon execution — it is a
software model of the compose kernel plus the phase-transport carrier,
run together, hosted.

---

## 1. Files added / changed

| File | Change |
|------|--------|
| `src/kernel/craft/mod.rs` | **+2 new public functions**: `verify_packet_replay(&CraftController) -> Result<String, CraftError>` (packet-vs-statechain cross-check) and `prop_stuck_over_glider_scenario(&mut CraftController) -> Result<String, CraftError>` (runs `prop_stuck_scenario` then `verify_packet_replay`). +1 new `#[cfg(test)]` test (`prop_stuck_over_glider_passes`). New `use crate::kernel::state_proof::ObjectKind;` import. |
| `src/userland/craft_cmd.rs` | New `run_scenario_prop_stuck_glider(ctx)` shell entry point, mirroring `run_scenario_prop_stuck`'s shape (builds `CraftController::new_over_glider()`, runs the scenario, stores the controller in the `CRAFT` thread-local so later `craft status`/`craft quarantine status` etc. see the post-mission state). Added `prop_stuck_over_glider_scenario` to the `craft` import list. |
| `src/userland/commands/dispatch.rs` | `Command::ScenarioRun` dispatch: `"prop-stuck"` still routes to the local-transport scenario; `"prop-stuck-glider"` (new) and `"craft-prop-stuck-glider"` (previously silently aliased to the *local*-transport scenario — a WP-1-era placeholder) now both route to `run_scenario_prop_stuck_glider`, i.e. the real glider-carrier path. |
| `Cargo.toml` | New `[[test]]` entry `craft_glider_scenario_tests`, `required-features = ["craft"]`. |
| `tests/craft_glider_scenario_tests.rs` | **new.** 6 tests (below). |
| `tests/craft_shell_tests.rs` | +2 assertions/tests: parser recognizes `scenario run prop-stuck-glider`; `scenario_glider_api_pass` exercises the public API end to end. |
| `docs/terminal_craft_10b_demo.txt` | **new.** Real captured hosted-shell transcript (see §4). |
| `docs/reports/AVENUE_10B_wp2_scenario.md` | this report. |

No changes to `src/kernel/phase_transport/**`, `src/kernel/craft/transport_phase.rs`, `src/kernel/craft/controller.rs`, `src/kernel/craft/fdir.rs`, or `src/kernel/craft/replay.rs` — WP-2 only adds a new orchestration layer on top of WP-1's adapter and the existing FDIR/replay machinery; none of it was reimplemented.

---

## 2. `verify_packet_replay` — what it actually checks

For every object (`navigation`, `propulsion`, `power`), walks the object's
statechain (`ctl.ledger.chain(ObjectKind::Craft, id)`), and for each event
that has a matching `proof_ops` entry carrying `Some(packet_id)` (i.e. every
transition that actually crossed the transport, as opposed to internal FDIR
bookkeeping which never gets a packet id):

1. Resolves the `CraftPacketRecord` from `craft.transport.records()` by
   `packet_id`.
2. **Opcode match.** For ordinary transitions, asserts the transported
   `record.opcode` equals the ledger's `operation_name` for that same
   `(object, sequence)`. One legitimate exception, discovered by running
   the test suite rather than assumed up front (see §5): the
   quarantine-reject path (`execute()`'s `rejected_quarantined_operation`
   branch) sends the packet with the *attempted* command as its wire
   opcode (e.g. `"prop_fire"`) but audits it under the wrapper op name
   `"rejected_quarantined_operation"`. For that case the check instead
   recomputes `hash_payload(record.opcode.as_bytes())` and confirms it
   equals `event.proof.payload_hash` — cryptographically tying the wire
   opcode to the audited reject record instead of naively string-matching
   two different vocabularies.
3. **Ack check.** Every transported op must be acknowledged, *except*
   rejected-while-quarantined sends, which `execute()` deliberately never
   acknowledges (the command was refused, not delivered) — confirmed by
   reading `controller.rs`'s reject branch, which calls `.send()` but not
   `.acknowledge()`.
4. **Assoc parity.** Recomputes `FdirController::recompute_assoc_parity`
   from the proof triple and confirms it equals the `craft_parity` value
   recorded at transition time.
5. After the per-packet loop, an independent statechain-level check:
   `craft.replay_episode(0)` (pure hash-chain / compose / assoc_parity
   reconstruction, no packet data) must equal `craft.snapshot_states()`
   (the live controller). This reuses the existing `replay_episode`
   machinery unmodified — no new replay logic was written.
6. Confirms `craft.last_checkpoint` is `Some(..)` (sealed by
   `FdirController::prove`) and reports its sequence/tick.

If any of these fail, `verify_packet_replay` returns `CraftError::Replay(..)`
— it does not silently pass.

---

## 3. New tests (`tests/craft_glider_scenario_tests.rs`, 6 tests)

1. `glider_scenario_prints_fdir_pass_and_replay_pass` — runs the full mission over the glider carrier, asserts the report contains `CRAFT FDIR PASS`, `CRAFT GLIDER SCENARIO PASS`, the cross-check header, the live==replayed line, and the sealed-checkpoint line.
2. `glider_scenario_runs_detect_isolate_recover_prove_in_order` — reads the FDIR object's statechain directly, asserts `fdir_detect` < `fdir_isolate` < `fdir_recover` < `fdir_prove` by sequence position, and that `last_checkpoint` is `Some`.
3. `transported_packet_log_matches_statechain_audited_commands` — independently (outside `verify_packet_replay`) walks `c.proof_ops`, resolves each packet-bearing transition against `c.transport.records()` and the statechain event at that sequence, and asserts opcode agreement for every non-reject op.
4. `assoc_parity_verifies_end_to_end` — recomputes assoc_parity for every recorded `proof_op` and asserts it matches what was stored; also calls `ledger.verify_all()`.
5. `glider_and_local_transport_reach_the_same_final_mission_state` — runs `prop_stuck_scenario` over `CraftController::new()` (local transport) and `prop_stuck_over_glider_scenario` over `CraftController::new_over_glider()` side by side, asserts `snapshot_states()` are identical, and pins the expected final values (nav locked, prop idle, power protected, fdir normal, not quarantined) — same mission, same outcome, different transport, as requested.
6. `shell_scenario_dispatch_runs_glider_scenario` — parser-level check that `scenario run prop-stuck-glider` parses to `Command::ScenarioRun { name: "prop-stuck-glider" }`.

---

## 4. Live transcript — captured, not fabricated

Captured by piping shell commands into the real hosted runtime binary and
recording its actual stdout:

```
export CARGO_TARGET_DIR="$TEMP/kairos-target-wp2"
printf 'craft init\nscenario run prop-stuck-glider\ncraft status\ncraft quarantine status\nexit\n' \
  | cargo run --bin kairos_host --features craft --quiet
```

Saved verbatim to `docs/terminal_craft_10b_demo.txt` (boot splash included,
unedited). It shows, in order:

- Glider packets crossing with real packet ids (`pkt=1,3,5,7,9,11,13,15,17,18`), each tagged with the endpoint pair it crossed.
- `ack=true` on every delivered packet, `ack=false` on the one rejected (`prop_fire` refused while propulsion is quarantined) — by design, not a bug.
- `FDIR: normal --operand Φ1--> detected` → `detected --operand Φ3--> isolated` → `isolated --operand Φ1--> recovered_pending_proof` → `recovered_pending_proof --operand Φ3--> normal` (detect → isolate → recover → prove).
- `PACKET REPLAY CROSS-CHECK` block listing all 10 packet-bearing transitions with `op=...`, `ack=...`, `assoc_parity=...`, ending `packets cross-checked: 10 (opcode+ack+assoc_parity match audited statechain)` and `statechain replay: live == replayed (OK)`.
- `sealed checkpoint: sequence=0 tick=18`.
- `CRAFT GLIDER SCENARIO PASS`.
- Follow-up `craft status` / `craft quarantine status` commands confirming the post-mission state (propulsion unquarantined, idle; power protected).

This is the same `kairos_host` hosted-shell mechanism used to produce
`docs/terminal_demo.txt`; `docs/terminal_craft_demo.txt` (WP-1-era) was
instead captured by running the `craft-mainframe` product bin directly —
both are legitimate "real hosted runtime" capture paths, this report used
the shell path because it also exercises the parser/dispatch wiring
(`scenario run prop-stuck-glider`) added in this work package, not just the
kernel API.

---

## 5. Exact verification commands and pass counts

```
export CARGO_TARGET_DIR="$TEMP/kairos-target-wp2"

cargo test --features craft --lib
  -> 43 passed; 0 failed

cargo test --features craft \
  --test craft_tests --test craft_fdir_tests --test craft_replay_tests \
  --test craft_shell_tests --test craft_enum_compat_tests \
  --test craft_transport_phase_tests --test craft_glider_scenario_tests
  -> craft_tests:                  10 passed; 0 failed
  -> craft_fdir_tests:               5 passed; 0 failed
  -> craft_replay_tests:            19 passed; 0 failed
  -> craft_shell_tests:              3 passed; 0 failed
  -> craft_enum_compat_tests:        3 passed; 0 failed
  -> craft_transport_phase_tests:    3 passed; 0 failed
  -> craft_glider_scenario_tests:    6 passed; 0 failed
```

Additional regression check (not in the requested list, run anyway):

```
cargo test --features craft --test craft_fdir_partial_recovery_tests
  -> 5 passed; 0 failed
```

**Total: 92 tests (43 lib + 49 across the 7 requested craft suites), 0
failures, 0 regressions.** (97 including the extra
`craft_fdir_partial_recovery_tests` sweep.)

`cargo check --lib` with default features (no `craft`) also compiles clean
— the entire `kernel::craft` module tree stays behind
`#[cfg(feature = "craft")]` at `src/kernel/mod.rs`, so this work package is
fully inert in a non-craft build. (One pre-existing warning,
`unused import: core::cell::RefCell` in `src/userland/craft_cmd.rs` under
the no-craft build, predates this session — the `RefCell` import there is
unconditional but its only use is inside `#[cfg(feature = "craft")]` code;
not touched by WP-2 and not introduced by it.)

---

## 6. Friction / findings

- **The reject-path opcode mismatch was a real bug, not a false positive in
  the new check.** First draft of `verify_packet_replay` string-matched
  `record.opcode == event.proof.operation_name` unconditionally and failed
  on the very first test run:
  `packet 17 transported opcode 'prop_fire' != audited op 'rejected_quarantined_operation'`.
  Reading `controller.rs`'s reject branch showed this is intentional
  craft-fixture behavior (the wire carries the attempted command, the
  ledger audits it under a wrapper op name) — not a defect to "fix", but a
  vocabulary difference the cross-check needed to understand and verify
  cryptographically (via payload hash) rather than paper over.
- **The same reject packet is also never acknowledged**, again by design
  (`execute()`'s reject branch calls `.send()` but not `.acknowledge()`).
  The first draft's blanket ack requirement also failed on this packet;
  fixed the same way — read the actual code path instead of guessing, then
  special-cased it with a comment explaining why.
- **`craft-prop-stuck-glider` was already wired to the *wrong* scenario.**
  Pre-WP-2, `scenario run craft-prop-stuck-glider` silently ran
  `prop_stuck_scenario` over the **local** transport — the name promised a
  glider-carrier run but never delivered one. This work package repoints
  that alias to the real glider path and adds `prop-stuck-glider` as the
  primary name. Flagging this explicitly since it's a behavior change to
  an existing (if never-actually-glider) command name, not a pure
  addition.
- No other blockers.
