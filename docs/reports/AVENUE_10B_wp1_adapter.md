# Avenue 10B, WP-1 — Craft Fixture Bridged to the Real Glider Carrier

**Date:** 2026-07-24
**Scope:** Replace the craft fixture's in-process transport stand-in with the
silicon-verified glider packet carrier (`phase_transport::TransportController`)
behind a pluggable interface. `LocalCraftTransport` remains the default.
**Standing rule:** compose is imported from `core::gphase_core`, never
reimplemented. No git commit made — work is left in the tree.

---

## 1. Files added / changed

| File | Change |
|------|--------|
| `src/kernel/craft/transport_phase.rs` | **new.** `PhaseTransportCraftAdapter`, `impl CraftTransport` over `TransportController`. `#[cfg(feature = "craft")]`. |
| `src/kernel/craft/mod.rs` | `pub mod transport_phase;` (gated) + re-export `PhaseTransportCraftAdapter`. |
| `src/kernel/craft/transport.rs` | `CraftTransport` gains a `core::fmt::Debug` supertrait (needed so `CraftController` can still `#[derive(Debug)]` over a `Box<dyn CraftTransport>` field). No change to the three method signatures given in the brief. |
| `src/kernel/craft/controller.rs` | `transport` field changed `LocalCraftTransport` → `Box<dyn CraftTransport>`. Added `CraftController::with_transport(Box<dyn CraftTransport>)` and `CraftController::new_over_glider() -> Result<Self, String>`. `init_craft()` no longer reconstructs `self.transport` (see §3). |
| `Cargo.toml` | New `[[test]]` entry `craft_transport_phase_tests`, `required-features = ["craft"]`, matching the existing craft test entries. |
| `tests/craft_transport_phase_tests.rs` | **new.** 3 tests (below). |

No changes to `src/kernel/phase_transport/**` — the carrier itself is untouched, used exactly as documented.

---

## 2. Port / opcode mapping

`TransportController::init(4)` (M=4, same lattice as the sealed silicon evidence). The Avenue 2 carrier's physical velocity is fixed at `(-1,-1)`, so a single-lane `send()` can only reach cells that share `(r - c) mod m` with the source — i.e. reachability is confined to one diagonal. To let every craft endpoint reach every other endpoint directly, all four ports were placed on the lattice's main diagonal (`r == c`):

| Craft endpoint | Grid coord |
|---|---|
| `kernel` / `ground` (mission control, always the send-side) | `(1,1)` |
| `navigation` | `(2,2)` |
| `propulsion` | `(0,0)` |
| `power` | `(3,3)` |

This deviates from the illustrative `(0,3)` corner suggested for ground in the brief: `(0,3)` has `r-c = 1 mod 4` while the three subsystem corners `(2,2)/(0,0)/(3,3)` have `r-c = 0` — a packet from `(0,3)` toward any of them is off-diagonal and `TransportController::expected_arrival` would return `u64::MAX` (unreachable), so that exact corner set was not usable as-is. `(1,1)` was chosen instead, on the same diagonal as the other three. Documented at the top of `transport_phase.rs`.

Opcode: the craft layer's `opcode: &str` parameter is always a craft-command string (`"prop_arm"`, `"power_load"`, …), never a literal transport opcode. Mapping is `PacketOpcode::parse(opcode).unwrap_or(PacketOpcode::Data)` — recognized transport opcode words pass through, everything else (all real craft commands) becomes `PacketOpcode::Data`. The reject-path send in `controller.rs` (`b"reject"` payload, `cmd.as_str()` opcode) is handled by the same rule; only the payload differs, not the routing.

---

## 3. `PhaseTransportCraftAdapter` behavior

- `send(source, destination, opcode, payload)`: resolves both endpoint strings to grid coords, calls `ctl.send(src, dst, op, payload, 0)` on lane 0 (the non-ACK lane), ticks exactly `TransportController::expected_arrival(src, dst, 4)` generations so the carrier physically lands on the destination cell (this is what drives `TransportReceiver::receive` and sets the packet to `Received`), then `verify_trace(id)`. A `CraftPacketRecord` (`ack: false`) is pushed to a local mirror log. Returns the real carrier packet id.
- `acknowledge(packet_id)`: looks up the mirrored record to recover the original source/destination names, flips its `ack` flag, calls `ctl.acknowledge(packet_id)` (this fails with `TransportError::InvalidOpcode` unless the original packet already reached `Received` — which `send()` guarantees), ticks `TransportController::expected_arrival_dir(dst, src, 4, 1)` (ACK plane logical motion is `(+1,+1)`, not `(-1,-1)` — confirmed by reading `ack.rs`/`injector.rs`; the existing `FpgaGliderTransport::one_packet_smoke` uses `dir=-1` for its ACK eta too, but that only produces the correct tick count by coincidence for its specific symmetric `(2,2)↔(0,0)` pair on M=4, so it was not copied verbatim here), then `verify_trace(ack_id)`. Pushes a second mirrored record (`opcode: "ACK"`, `ack: true`, source/destination swapped).
- `records()`: returns the mirrored log, same shape `LocalCraftTransport` exposes.
- `TransportError` → `String` conversion is `.to_string()` throughout, as instructed.

## 3b. `CraftController` pluggability

`transport` is now `Box<dyn CraftTransport>`. `CraftTransport` gained a `Debug` supertrait bound only — the three method signatures from the brief are untouched. `CraftController::new()` now delegates to `with_transport(Box::new(LocalCraftTransport::new()))`, so the default path is byte-for-byte the same behavior as before.

One deliberate behavior change: `init_craft()` used to reconstruct `self.transport = LocalCraftTransport::new()` on every call, discarding any prior transport. That line is now removed — `init_craft()` resets mission/ledger/episode state but leaves the transport object alone, so a controller built via `with_transport`/`new_over_glider` keeps talking to the real carrier across repeated `init_craft()` calls instead of being silently swapped back to the local stand-in. Verified this is safe: no test anywhere (grepped `tests/`) reads `CraftController::transport`'s field type, log contents, or asserts on specific packet ids, and every existing test calls `init_craft()` at most once per fresh controller.

---

## 4. New tests (`tests/craft_transport_phase_tests.rs`, 3 tests)

1. `command_round_trips_through_glider_carrier` — direct adapter use: send `kernel→propulsion` `"prop_arm"`, assert the mirrored record's source/destination/opcode match what was sent and `ack=false`; acknowledge it, assert the reply record is `propulsion→kernel`, opcode `"ACK"`, `ack=true`, and the original record's `ack` flag flipped to `true`.
2. `every_craft_endpoint_pair_round_trips` — sends+acks `kernel→{navigation,propulsion,power}` sequentially on the same adapter instance (exercises lane reuse/BusyPort-avoidance), asserts 6 mirrored records total.
3. `craft_controller_arm_fire_over_glider_carrier` — full path through `CraftController::new_over_glider()`: `init_craft`, `PropArm`, `PropFire`, checks state transitions land on Φ1/Φ2 exactly as the existing `LocalCraftTransport`-backed `idle_arm_fire_succeeds` test expects, then asserts the `prop_arm`/`prop_fire` packets are present and acknowledged in `c.transport.records()` and exactly 2 `"ACK"` reply records exist.

---

## 5. Exact verification commands and pass counts

```
export CARGO_TARGET_DIR="$TEMP/kairos-target-wp1"

cargo test --features craft --lib
  -> 42 passed; 0 failed

cargo test --features craft \
  --test craft_tests --test craft_replay_tests --test craft_fdir_tests \
  --test phase_transport_tests --test craft_transport_phase_tests
  -> craft_fdir_tests:            5 passed; 0 failed
  -> craft_replay_tests:         19 passed; 0 failed
  -> craft_tests:                10 passed; 0 failed
  -> craft_transport_phase_tests: 3 passed; 0 failed
  -> phase_transport_tests:      18 passed; 0 failed
```

Additional regression sweep beyond the requested list (same command style, other craft-gated suites not explicitly listed in the brief):

```
cargo test --features craft \
  --test craft_fdir_partial_recovery_tests --test craft_enum_compat_tests --test craft_shell_tests
  -> craft_fdir_partial_recovery_tests: 5 passed; 0 failed
  -> craft_enum_compat_tests:           3 passed; 0 failed
  -> craft_shell_tests:                 2 passed; 0 failed
```

`cargo check --lib` (default feature set, no `craft`) also compiles clean — the craft module (and this adapter) stay fully behind `#[cfg(feature = "craft")]` at the parent `kernel::craft` boundary, so a non-craft build is unaffected.

**Total: 107 tests across lib + all 8 craft/phase_transport suites, 0 failures, 0 regressions.**

---

## 6. Friction

- **Diagonal-only reachability.** The single biggest gotcha: the carrier's fixed `(-1,-1)` velocity means only points on the same lattice diagonal are mutually reachable in one hop. The brief's example port table (`(2,2),(0,0),(3,3),(0,3)`) has three points on the diagonal and one (`(0,3)`, suggested for ground) off it. Caught this by hand-deriving `expected_arrival`'s invariant before wiring anything, so no wasted implementation cycle — but it's worth flagging since it's not obvious from the `TransportController` API surface alone (`expected_arrival` just silently returns `u64::MAX` for unreachable pairs, it doesn't error loudly).
- **ACK direction subtlety.** `FpgaGliderTransport::one_packet_smoke`'s ACK-eta call uses `expected_arrival` (`dir=-1`) rather than `expected_arrival_dir(..., 1)`, which only works because its specific `(2,2)`↔`(0,0)` pair is exactly `m/2` apart in both directions on M=4. Copying that pattern verbatim onto the new diagonal port table (where pairs are not all equidistant, e.g. `kernel(1,1)↔navigation(2,2)` is 1 step one way, 3 the other) would have produced a real, non-coincidental bug (wrong tick count → `acknowledge` verifying against an undelivered/wrong trace). Traced through `ack.rs`/`injector.rs` to confirm the ACK plane's *logical* motion is `(+1,+1)` and used `expected_arrival_dir(dst, src, m, 1)` explicitly instead.
- **`Box<dyn CraftTransport>` + `#[derive(Debug)]`.** Needed a `Debug` supertrait on `CraftTransport` for `CraftController`'s existing `#[derive(Debug)]` to keep compiling over the trait-object field — confirmed empirically with `cargo check` (Rust does synthesize `impl Debug for dyn Trait` once `Trait: Debug`) rather than assuming.
- **`init_craft()` resetting the transport.** Original code unconditionally rebuilt `LocalCraftTransport` on every `init_craft()`, which would have thrown away any real carrier wired in via `with_transport`. Removed that reset (see §3b) after confirming via grep that no existing test depends on transport state being wiped by `init_craft()`.

No blockers. No fabricated results — all numbers above are from actual `cargo test` runs in this session.
