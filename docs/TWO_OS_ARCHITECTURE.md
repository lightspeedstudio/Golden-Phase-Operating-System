# Two-OS Architecture — Base OS and Mission (Craft) OS

**Date:** 2026-07-24 · Status: formalized (the split already existed in code as
feature flags; this document names it and fixes the boundary).

One codebase, two products. The **only** thing that separates them is the
**craft/mission layer** — everything else (including SSH and the AI engine)
lives in the Base OS.

```
                 ┌──────────────────────────────────────────────┐
   MISSION OS →  │  craft/mission layer (nav·prop·power · FDIR · │  feature: craft/mission
  (Craft Main-   │  mission controls · glider-carried transport) │
    frame)       ├──────────────────────────────────────────────┤
                 │            === product boundary ===           │
   BASE OS    →  │  operator console · SSH · AI engine ·         │  default build
  (carried on    │  phase_transport · statechain (black box) ·   │
   the boards)   │  PhaseFS · VM + ISA · scheduler · memory ·    │
                 │  core GPL algebra (compose/reflect/topology)  │
                 └──────────────────────────────────────────────┘
                        ↓ same algebra, same traces ↓
                   QEMU RISC-V · iCE40 silicon (verified)
```

## 1. Golden Phase Base OS  (the utility / board OS)

The complete general-purpose Golden Phase OS. It is what runs **on the boards**
and is the substrate the Mission OS extends.

- **Contains:** core algebra, kernel (scheduler/memory/syscall), statechain
  black box (`assoc_parity` audit), PhaseFS, VM + ISA, phaselink/phasenet,
  `phase_transport` (glider carrier), the operator console/shell, **SSH remote
  operator**, and the **AI engine** (`kernel::ai`, ungated — always present).
- **Does NOT contain:** the craft/mission layer.
- **Build:** the default build. `cargo build` / `cargo test` /
  `cargo run --bin kairos_host`. SSH build: `--bin kairos_ssh`.
- **Board profiles (bare-metal):** `bm-minimum` (~390 KB), `bm-embedded`,
  `bm-full` — the images flashed to Board v1 / QEMU RISC-V.
- **Bins:** `kairos_host`, `kairos_ssh`, `kairos_boot`.
- **PZ7020 Utility board image (gold):**
  `cargo build --release --bin kairos_zynq --no-default-features --features "zynq-a9,utility" --target armv7a-none-eabi`
  packed with `fpga/zynq/release/PZ7020_G1P_PATH1_G64/` (`G1P+TNSR` at `0x40000000`, `G64S` at `0x40001000`). See `fpga/zynq/release/UTILITY_GOLD.txt`. Not Mission OS (`craft` stays off).

## 2. Golden Phase Mission OS  (the Craft Mainframe / ship computer)

Base OS **plus** the mission layer — the supervisor that monitors, governs, and
corrects a craft.

- **Adds (and only adds):** `src/kernel/craft/` (nav/prop/power state model,
  FDIR detect→isolate→recover→prove, quarantine, audited replay,
  `PhaseTransportCraftAdapter`), the `craft` shell commands
  (`src/userland/craft_cmd.rs`), and the mission scenarios.
- **Build:** `cargo build --features mission` (= `craft` + `ssh`), or the
  minimal mission surface with `--features craft`.
- **Bin / product:** `products/craft-mainframe/`.
- **Status:** FDIR + glider-carried transport proven in software and
  **hosted-sim over the sealed (silicon-verified) glider law** — not yet live
  on-silicon craft transport.

## 3. The boundary rule (enforced)

- All craft code lives under `src/kernel/craft/**` + `src/userland/craft_cmd.rs`
  + craft scenarios/tests, gated by `#[cfg(feature = "craft")]`.
- **Nothing in the Base OS may depend on `craft::`.** Verified 2026-07-24:
  `grep` for `craft::` outside the craft layer returns nothing; the default
  build compiles with zero craft code.
- Craft is **purely additive** — enabling it never changes Base behavior, only
  extends it. This keeps the Base OS small, certifiable, and independently
  shippable to the boards.

## 4. Test & size gates

| Gate | Base OS | Mission OS |
|------|---------|------------|
| Build | `cargo build` (default) — green | `cargo build --features mission` — green |
| Tests | `cargo test` (default) | `cargo test --features craft` (adds craft suites) |
| Boundary | no `craft::` outside craft layer | craft layer isolated + additive |
| Footprint target | `bm-minimum` ~390 KB on-board | hosted / ECP5-class board |

## 5. Why this split

- **Base OS** must be small and trustworthy enough to certify and carry on the
  silicon — a lawful compute + incorruptible log substrate. SSH and the AI
  engine are utility services, so they stay in Base.
- **Mission OS** is where craft-specific policy lives. Keeping it a strictly
  additive layer means the mainframe and the board OS never diverge on the
  algebra or the black box — the mission logic is the only delta.

## 6. Next

- On-silicon craft packet transport (Mission OS live UART) — today hosted-sim.
- Real (secret/asymmetric) checkpoint signing before the Mission OS faces an
  adversarial threat model (Avenue 6 finding: current key is public-derivable).
- comms/payload subsystems; nav-stuck / power-overload scenarios.
