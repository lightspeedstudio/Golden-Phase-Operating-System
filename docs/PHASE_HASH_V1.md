# PhaseHash v1 — domain-separated tuple-hash construction

`no_std`-safe, SHA-256-backed hash suite used for every content-addressed root in Golden Phase OS.
Source: [`src/kernel/crypto/phase_hash.rs`](../src/kernel/crypto/phase_hash.rs). Declared in
[`kernel::crypto`](../src/kernel/crypto/mod.rs), available under `any(feature = "std", feature =
"baremetal")` — same gate as the rest of `kernel::crypto`.

## Construction

`tuple_hash(domain, parts)` hashes:

```
MAGIC ("GP-PH1", 6 bytes)
|| u32_le(len(domain)) || domain
|| u32_le(count(parts))
|| { u32_le(len(part)) || part }  for each part
```

through a single SHA-256. The magic, explicit domain length, and explicit part count make every
encoding unambiguous — two distinct `(domain, parts)` inputs can never serialize to the same byte
string, so a collision in the pre-image would imply a SHA-256 collision. Proven by
`tuple_hash_is_sensitive_to_part_boundaries` / `tuple_hash_is_sensitive_to_part_count`: `("ab",
"c")` and `("a", "bc")` — and `("abc")` vs `("ab", "c")` — never collide even though naive
concatenation would make them look identical.

## The five hashes

| Hash | Function | Domain | Purpose |
|---|---|---|---|
| **PH-C** | `ph_content(data)` | `GPOS/PhaseHash/content/v1` | Raw content hash. |
| **PH-O** | `ph_orbit(state)` | `GPOS/PhaseHash/orbit/v1` | Kairos-State symmetry-orbit hash — invariant under `reflect()` (see below). |
| **PH-S** | `ph_state(&StateInput)` | `GPOS/PhaseHash/state/v1` | Chained per-object state hash: `previous` link + Chronos tick + Kairos-sector + metadata hash. |
| **PH-R** | `ph_root(leaves)` | `GPOS/PhaseHash/root/v1` | Root hash over an ordered list of leaf hashes. |
| **PH-R** (boot variant) | `ph_boot_image_root(fsbl, bitstream, kernel)` | `GPOS/PhaseHash/boot-image-root/v1` | Root unique to one authenticated (FSBL, bitstream, kernel) triad. |
| — | `ph_boot_session_root(&BootSessionRootInput)` | `GPOS/PhaseHash/boot-session-root/v1` | Dedicated boot-chain hash (see below) — deliberately **not** force-fit into PH-S, because its inputs are shaped differently. |

PH-O's orbit is built from [`gphase_core::reflect`](../src/core/gphase_core.rs), the reflection map
`rho(x) = 3 - x` on the four `PhaseState` values. `reflect` has no fixed points on `{0,1,2,3}`, so
every state has exactly one mirror: `{Phi0, Phi3}` and `{Phi1, Phi2}` are the two orbits.
`ph_reflection_orbit(state)` returns the canonical (discriminant-ordered) pair, so
`ph_orbit(s) == ph_orbit(reflect(s))` for every `s` — proven by
`ph_orbit_is_invariant_under_reflection`.

## `BootSessionRoot`

`BootSessionRoot` binds one physical boot: `(equation_root, init_pid, kernel_root, boot_tick)`. It
is the **only** consumer of `ph_boot_session_root`; every other subsystem that reads
`kernel.authority.boot_session_root` (ANOM keyring, PhaseLink, PhaseNet addressing) treats it as an
opaque input.

Computed in [`kernel::boot_session::compute_boot_session_root`](../src/kernel/boot_session.rs),
gated `any(feature = "std", feature = "baremetal")` — no always-zero stub branch. Called from
[`kernel::anom::boot::finalize_boot`](../src/kernel/anom/boot.rs) on **every** build that has a
statechain to bind (hosted `std` *and* bare metal) — this used to only run under `std`, which meant
`boot_session_root` was silently stuck at its zeroed `Default` on every real hardware boot.

## Construction-correctness tests

The original C++ reference vectors this construction was ported from were not recoverable after an
earlier full revert (pasted in chat, never saved to a file). Rather than guess at matching lost
vectors, `phase_hash.rs`'s tests instead prove the **construction itself** is correct: each test
manually builds the exact `tuple_hash` pre-image byte-for-byte with a bare `Sha256::digest`,
independent of `tuple_hash`, and asserts the two match
(`tuple_hash_matches_manually_built_preimage`). Domain separation, part-boundary sensitivity,
part-count sensitivity, orbit invariance, and per-field sensitivity of `ph_state` /
`ph_boot_session_root` are all covered — 12 tests total, `cargo test --lib
kernel::crypto::phase_hash`.

## Proof (hardware, 2026-08-12)

```
[ ROOTED] EquationRoot:        ee6fd313
[ SEALED] BootSessionRoot:     9602ac89
```

Real, non-zero, PhaseHash-computed values from the live boot splash on PZ7020 hardware — not the
all-zero stub the pre-existing `#[cfg(not(feature = "std"))]` path always returned on bare metal.
Evidence: `fpga/zynq/evidence/kairos_zynq_boot/com12_phase_index_verify_20260812_023504.txt`.
