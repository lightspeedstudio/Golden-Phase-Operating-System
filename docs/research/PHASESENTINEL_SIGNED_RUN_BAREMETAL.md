# PhaseSentinel signed onboard `app run` — Utility `BAREMETAL_CROSS_BUILD`

Classification: **BAREMETAL_CROSS_BUILD**. Not physical proof. Not a COM12 result. Do not flash from this note.

## Command

```
cargo build --release --bin kairos_zynq --no-default-features --features "zynq-a9,utility" --target armv7a-none-eabi
```

Target: `armv7a-none-eabi`. Linker: Xilinx `arm-none-eabi-gcc` (`.cargo/config.toml`).
`phasesentinel-research` is **not** in `utility`.

## ELF identity

| Field | Value |
|-------|--------|
| path | `target/armv7a-none-eabi/release/kairos_zynq` |
| size | 2510028 bytes |
| SHA-256 | `4B10F67324AC16F5A5256A4254036C6BF97E6D2DBE258DD86E29EEF3A383BCA3` |
| class | ELF32 little-endian |
| type | EXEC |
| machine | ARM (EM_ARM) |
| entry | `0x00100060` |
| ABI | Version5 EABI, soft-float |
| DT_NEEDED | none (no shared `std` / libstdc++) |

`rust_begin_unwind` is the rustc `no_std` panic entry, not `libstd`. No `std::io`, `std::fs`, `std::thread`, or `library/std` strings.

## Forbidden strings (must be absent)

| Needle | Count |
|--------|-------|
| `KAIROS_PHASESTACK_RESEARCH_CA_SEED_V1` | 0 |
| `phasestack-sign-dev` | 0 |
| `PHASESENTINEL-LIVE-INJECT` | 0 |
| `research_root_keypair` | 0 |
| `research_bump_generation` (nm) | 0 |

`trust-any-cert` is present as the parser **reject** string (same as Gate K).

## Sentinel launch symbols (must be present)

`arm-none-eabi-nm --defined-only`:

- `kairos::kernel::phasestack::run::launch`
- `kairos::kernel::phasestack::run::stop`
- `kairos::kernel::phasestack::run::revoke`
- `kairos::kernel::phasestack::run::live_view`
- `kairos::kernel::phasestack::run::record_launch_event`
- `kairos::kernel::sentinel::permit::PhaseSentinel::authorize_verified_ticket`
- `kairos::kernel::sentinel::permit::PhaseSentinel::consume_permit`

`inject_launch_nonce` is inlined into `run::launch`.

UART literals in the image: `SENTINEL_APP_OUTPUT:`, `launch_nonce=`, `vm_output=`, `sentinel=Allow`, `AppRegistryRoot`.

## Host tests (not this image)

`cargo test --features identity,phasesentinel,phasestack,captable --lib kernel::phasestack`: 122 passed (S0–S12 included).
`cargo test --features captable,phasestack --lib kernel::phasestack`: 108 passed (Sentinel off).

## Non-claims

Not ON_BOARD_HARDWARE_TEST. Not a BOOT.BIN. Not MMU isolation. Not a production CA.
