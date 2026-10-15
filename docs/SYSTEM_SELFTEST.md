# `system selftest` — built-in full-suite self-test

`system selftest` (aliases: `selftest`, `self_test`) runs a built-in health check of every
Golden Phase OS feature area on the real board. It prints one line per area
(`[ PASS ]` / `[ FAIL ] ` / `[ N/A  ]`), a final tally, and — on the board — **saves the full
log to the SD card as `SELFTEST.LOG`** so every run leaves durable evidence.

Source: `src/userland/commands/selftest_cmd.rs`. Wired in `parser.rs` (`Command::SystemSelftest`),
`commands/dispatch.rs`, and `commands/mod.rs`.

## Design principles

- **Reuse, don't reimplement.** Each check calls an *existing* verify function — it never
  re-derives the logic it is testing.
- **Honest N/A.** An area whose verify hook is absent on this build (e.g. a std-only transport
  on baremetal) reports `N/A`, not a fake PASS. N/A areas are explicit **build-out candidates**.
- **The suite is the to-do list.** A red or N/A names exactly which feature is not yet real on
  the board — each becomes its own focused build-out.

## Areas and the verify function each reuses

| Area | Reuses |
|---|---|
| Phase Math | `gphase_core::reflect/compose` + `phase_math::reference::verify_tables()` |
| StateClock / Dual-Clock | `StateClock::snapshot()` (chronos+kairos monotonic) |
| Phi-Stability | `phi_stability::proof::trajectory_root()` |
| State-Domain VM | `state_domain::build_usb_domain()` + `domain_root` stability |
| Statechain Integrity | `state_root::merkle_root()` |
| ANOM Privacy (crypto) | `kernel::crypto::selftest()` (AES-256-GCM + ed25519 + tamper) |
| PhaseLink & PhaseNet | `phaselink::merkle::phaselink_root()` |
| Secure Remote Op (SSH) | `crypto::sign` ed25519 host-key gen + sign/verify + tamper |
| PhaseFS Filesystem | `fs_sd::save`/`cat` SD round-trip (board) |
| Networking (Ethernet) | `board::zynq7000::gem::probe_registers()` (board) |
| USB Host (HID) | `board::zynq7000::usb::probe()` (board) |
| Engineering Terminal (G1P) | `G1p::exec` compose vs `gphase_core::compose` (board) |
| Memory & Scheduler | heap allocation probe |
| AI Control Layer | N/A on baremetal — build-out: AI-over-smoltcp |
| Phase Transport | N/A — build-out: baremetal verify hook |
| RISC-V ISA VM | N/A — build-out: `isa` load+run self-check |

Board-only checks (USB/GEM/G1P/SD) report `N/A` on the host build so the compile gate stays green.

## Proof (hardware, 2026-08-10)

```
=== Golden Phase OS — system selftest ===
[ PASS ] Phase Math: reflect involution + compose/topology tables match core
[ PASS ] StateClock / Dual-Clock: chronos=0 kairos=0 (dual-clock snapshot monotonic)
[ PASS ] Phi-Stability: trajectory_root deterministic ...
[ PASS ] State-Domain VM: usb domain: 10 states, domain_root ... stable
[ PASS ] Statechain Integrity: merkle root deterministic ...
[ PASS ] ANOM Privacy (crypto): aead=ok ed25519_sign=ok reject_tamper=ok => PASS
[ PASS ] PhaseLink & PhaseNet: phaselink_root deterministic ...
[ PASS ] Secure Remote Op (SSH): ed25519 host-key gen + sign/verify + tamper-reject
[ PASS ] PhaseFS Filesystem: SD write+read round-trip (persistent)
[ PASS ] Networking (Ethernet): GEM0 NWCTRL=0x00000010 (controller present)
[ PASS ] USB Host (HID): host controller up @ 0xE0002000; no device
[ PASS ] Engineering Terminal (G1P): G1P compose sw=3 hw=3 MATCH
[ N/A  ] AI Control Layer: ABSENT on baremetal (std::net) — build-out: AI-over-smoltcp
[ N/A  ] Phase Transport: verify hook pending on baremetal — build-out candidate
[ N/A  ] RISC-V ISA VM: isa load+run self-check pending — build-out candidate
[ PASS ] Memory & Scheduler: heap allocator operational (dynamic alloc OK)
system selftest: 13/13 subsystems PASS — ALL PASS
fs: saved SELFTEST.LOG (1302 bytes) to SD
```

Evidence transcript: `fpga/zynq/evidence/kairos_zynq_boot/selftest_20260810_174747.txt`.

## Run it

On the board over COM12: type `system selftest` at the `gpos>` prompt. The results print live
and `SELFTEST.LOG` lands on the SD card (readable afterward with `fs cat SELFTEST.LOG` or by
mounting the card).
