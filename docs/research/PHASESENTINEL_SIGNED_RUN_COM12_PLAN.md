# PhaseSentinel signed onboard `app run` — COM12 plan

Classification of this document: **test plan**. Physical boot 2026-08-15 is recorded in `fpga/zynq/evidence/kairos_zynq_boot/BOOT_PHASESENTINEL_SIGNED_RUN_20260815/STATUS.md` (ON_BOARD_HARDWARE_TEST). Do not merge or push until review.

## Image

```
cargo build --release --bin kairos_zynq --no-default-features --features "zynq-a9,utility" --target armv7a-none-eabi
```

- FSBL: proven Utility FSBL (`fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf`)
- Bitstream: `PZ7020_G1P_PATH1_G64/system.bit` (record full SHA-256)
- ELF: this branch's `kairos_zynq` (record full SHA-256)
- Sealed Utility must **not** include `phasesentinel-research`

`BAREMETAL_CROSS_BUILD` is not physical proof. Cross-build ELF SHA-256 (this branch, not flashed): `4B10F67324AC16F5A5256A4254036C6BF97E6D2DBE258DD86E29EEF3A383BCA3`. See `PHASESENTINEL_SIGNED_RUN_BAREMETAL.md`.

## Offline packages

Sign on the host with `phasestack_sign pack-hello <root.sk> <out.gpa>`.
Do not sign on the board. The board image contains only `ROOT_PUBLIC_KEY_PIN`.

`hello_app()` now emits the 16-byte kernel-injected launch nonce via `EmitMem`.
Re-pack `hello.gpa` with the operator seed before the physical run. The committed 962-byte `hello.gpa` is the previous Print/Phi1 package and still verifies against the public pin (Gate J). It will **not** satisfy `vm_output == launch_nonce`.

## Positive path (one sealed-image boot)

```
app registry
app verify hello.gpa
app install hello.gpa
app run hello-a7f3c91e
app status hello-a7f3c91e
app stop hello-a7f3c91e
app revoke hello-a7f3c91e
app run hello-a7f3c91e
```

Required:

| command | expected |
|---------|----------|
| app registry | empty or known; AppRegistryRoot printed |
| app verify | signature verifies |
| app install | Installed |
| app run | sentinel=Allow, real pid, SENTINEL_APP_OUTPUT, vm_output == launch_nonce |
| app status | Installed/Running fields, gen, payload root |
| app stop | stopped; process retired |
| app revoke | revoked; granted=0 |
| app run | `PackageRevoked` or `Revoked` before spawn |
| shell | continues |

Legal phases Phi0→Phi1→Phi2→Phi3 (then Phi0 on stop).

## Negative packages (same sealed boot)

1. One-byte modified payload → `PayloadRootMismatch` or `CanonicalMismatch`
2. Unknown signer → `SignerUnknown` or `UnknownCert`
3. Retired research CA → `SignerRetired` or `RetiredSigner`
4. Over-capability (`granted` not ⊆ `requested ∩ policy`) → `CapabilityEscalation`
5. Unsigned → signature/certificate refusal

None of these may reach PhaseVM execute.

## Research-image only (not the sealed claim)

Build with extra `phasesentinel-research`. Unlock token `PHASESENTINEL-LIVE-INJECT`.

6. Stale ticket: bump registry generation then run → `RegistryGenerationMismatch`
7. Permit replay → `PermitConsumed` or `NonceReplayed`

Never add this feature to `utility`.

## Evidence (after a real COM12, later commit)

Unedited COM12 transcript, this plan, parsed matrix, commands, source commit, evidence commit, ELF, BOOT.BIN, system.bit, FSBL identity, every `.gpa`, public pin, SHA-256 manifest.

Each row: command, expected, observed, pid, registry generation, manifest root, payload root, Sentinel decision, classification.

## Non-claims

Production CA, secure boot, MMU sandbox, native ELF, third-party ABI, physical key extraction, universal app security, all HID/net/storage paths, production readiness.
