# Zynq Golden Phase Proof Ladder

**Board target:** PZ7020-SL-C / XC7Z020  
**Physical UART (G1P):** COM12 115200 8N1  
**Doctrine:** Math first, hardware second. **Golden Phase OS** is the bare-metal operating system itself (direct on Cortex-A9). Linux is not used.

Canonical silicon→app stack: [`fpga/zynq/GOLDEN_PHASE_ZYNQ_STACK.md`](../../fpga/zynq/GOLDEN_PHASE_ZYNQ_STACK.md).

```text
BootROM → stage-0 (FSBL temporary) → G1P bitstream → GP-OS kernel → native drivers → services → apps
```

`BOOT.BIN` target: `golden_phase_stage0.elf` + `system.bit` + `golden_phase_os.elf`.  
Early bring-up may use `zynq_fsbl.elf` + smoke/GP-OS ELF. Never `image.ub` / Linux / U-Boot for seals.

Every gate proves that the layer **preserves** the canonical Golden Phase algebra from `src/core/gphase_core.rs`. LED blinks, Linux boot, or peripheral smoke alone are not seals.

## Existing iCEBreaker claims (not Zynq)

Prior physical seals on **iCEBreaker V1.1a / COM24** include compose 16/16, reflect 4/4, topology 64/64, thresholds, parallel4, GPL VM, lattices N≤128, M=4 glider, serial M=8, PROP_ARM, and 61 evidence seals.

Canonical ancestor bitstream SHA-256:

`f499032615d2284b90fccfcf98207fae4f2dd2617868eba68549cc3102ae6dfc`

These prove algebra in **iCE40** gates. They do **not** prove Zynq AXI mapping, DDR, SD/FS, Ethernet, GP-OS↔PL, or large parallel XC7Z020 fabrics.

## Explicit Zynq non-claims (until sealed)

- G1P physical seal before COM12 smoke PASS on our `BOOT.BIN`
- AMD ZC702 / ALINX constraint substitution
- Simulation output as physical verification
- DDR / SD / Ethernet / GP-OS / M≥16 fabric seals before their gates
- Any Linux / PetaLinux / Buildroot / UIO / `image.ub` / U-Boot seal

## Gate status

| Gate | What is proved | Status |
|------|----------------|--------|
| **G0** | Host mathematical oracle (GPLV1) | **PASS** |
| **G1S** | Vendor-neutral RTL + AXI simulation | **PASS** |
| **G1P** | Physical PZ7020 — V1 smoke + V2 bring-up **PASS** (`evidence/g1p_bringup_v2/`) | **PASS** (GP-OS G2 still open) |
| **DDR-V2A** | Board DRAM qualification (bring-up checklist) | **STOPPED** — diverted from GP memory-system goal; not the kernel MM seal |
| **GP-MM-V1** | Phase-tag memory system on A9 (`release/PZ7020_GP_MEMORY_V1/`) | **PASS** — COM12 (`evidence/gp_memory_v1/`) |
| **GP-OS-V3** | Barebone GP-OS kernel (sched/timer/GIC/G1P) (`release/PZ7020_GP_OS_V3/`) | **PASS** — COM12 (`evidence/gp_os_v3/`) |
| **GP-OS-V4** | SD block I/O + PhaseFS persist (`release/PZ7020_GP_OS_V4/`) | **PASS** — COM12 (`evidence/gp_os_v4/`) |
| **GP-OS-V5** | GEM0 MDIO + MAC loopback GP frame (`release/PZ7020_GP_OS_V5/`) | **PASS** — COM12 (`evidence/gp_os_v5/`) |
| **GP-OS-V6** | PhaseNet-lite Ping/Pong over GEM (`release/PZ7020_GP_OS_V6/`) | **PASS** — COM12 (`evidence/gp_os_v6/`) |
| **GP-OS-V7** | MMU + phase-tag paging G3 (`release/PZ7020_GP_OS_V7/`) | **PACKAGED** — pending COM12 (`evidence/gp_os_v7/`) |
| **GP-OS-V8** | Multi-BD GEM + PHY wire path (`release/PZ7020_GP_OS_V8/`) | **PACKAGED** — pending COM12 |
| **GP-OS-V9** | SHA PhasePacket oracle (`release/PZ7020_GP_OS_V9/`) | **PACKAGED** — pending COM12 |
| **GP-OS-V10** | PhaseNet session transport (`release/PZ7020_GP_OS_V10/`) | **PACKAGED** — pending COM12 |
| **GP-OS-V11** | G6 SD→DDR→G1P→evidence→ETH (`release/PZ7020_GP_OS_V11/`) | **PACKAGED** — pending COM12 |
| **GP-OS-V12** | UART shell + apps (`release/PZ7020_GP_OS_V12/`) | **PACKAGED** — pending COM12 |
| **GP-OS-V13** | `golden_phase_stage0.elf` bootloader slot (`release/PZ7020_GP_OS_V13/`) | **PACKAGED** — pending COM12 |
| **GP-OS-V14** | Merge anchors `arch/armv7` + `board/zynq7000`; build via `zynq_a9_os` crate | **PACKAGED** — build path |
| **GP-OS-V15** | G7 stop at unproven size-2 (`release/PZ7020_GP_OS_V15/`) | **PACKAGED** — size-1 only |
| **GP-OS-V16 / META** | SSH + `GOLDEN PHASE OS META PASS` (`release/PZ7020_GP_OS_META/`) | **PHYSICALLY_VERIFIED** — COM12 `evidence/gp_os_meta/` |
| **G2** | Golden Phase OS kernel on Cortex-A9 (FSBL→bit→GP-OS; no Linux) | **PASS** (GP-OS-V3) |
| **G3** | Full memory manager / paging atop GP-MM | **PACKAGED** (GP-OS-V7; physical COM12 pending) |
| **G4** | SD + native filesystem | **PASS** (GP-OS-V4) |
| **G5** | Native Ethernet (MAC/PHY + transport) | **PACKAGED** (V8–V10; COM12 pending) |
| **G6** | GP-OS ↔ PL integration (beyond V2/V3 G1P smoke) | **PACKAGED** (GP-OS-V11) |
| **G7** | Larger fabric | **STOP** at size-1 (V15); size-2 unproven |

Linux is **not** a gate and is **never** part of this ladder.

**G1S ≠ G1P.** Simulation must set `PHYSICAL_BOARD_CLAIM=FALSE` and must never write `PHYSICALLY_VERIFIED.txt`.

Physical test tracker: [`fpga/zynq/evidence/PHYSICAL_TEST_REPORT.md`](../../fpga/zynq/evidence/PHYSICAL_TEST_REPORT.md).

Every later gate must preserve the **same frozen** `vectors.bin` digest from G0.

## Immutable oracle (G0)

Format: `fpga/zynq/oracle/FORMAT.md` (GPLV magic, 32-byte header, 48-byte records, 84 records).

Digest: SHA-256 of each record’s first 16 bytes only.

Source of truth: `gphase_core` only — no second compose implementation.

`VECTORS_SHA256=99eca89134347850322269f6c6e55d6ab3f7dd1cdaa1237988c6c5b94448183b`

## AXI-Lite register map (G1S / future G1P)

| Offset | Name |
|--------|------|
| 0x00 | ID = `0x47504C34` ("GPL4") |
| 0x04 | VERSION |
| 0x08 | INPUT_A |
| 0x0C | INPUT_B |
| 0x10 | INPUT_C |
| 0x14 | COMPOSE_AB (PL) |
| 0x18 | LEFT_RESULT (PL) |
| 0x1C | RIGHT_RESULT (PL) |
| 0x20 | PARITY (PL) |
| 0x24 | REFLECT_RESULT (PL) |
| 0x28 | TEST_COUNT (software-owned) |
| 0x2C | MISMATCH_COUNT (software-owned) |
| 0x30 | STATUS |
| 0x34 | START |
| 0x38 | CAPABILITIES |
| 0x3C | RESET_CAUSE / diagnostic |

PL result wires produce answers. Software compares against frozen vectors.

## Board package (G1P)

Seller materials are staged under `fpga/zynq/board/pz7020-sl-c/seller/` — see `SELLER_PACKAGE_INVENTORY.md`.

Bring-up artifacts: `fpga/zynq/release/PZ7020_G1P_SD_SMOKE_V1/` (`system.bit`, `.xsa`, **`BOOT.BIN`**, `zynq_fsbl.elf`, `g1p_smoke.elf`).  
Next: FAT32 SD with Kairos `BOOT.BIN` only (no `image.ub`), then COM12 capture. Do not claim G1P seal until UART PASS.

**Do not** substitute AMD ZC702 constraints. Seller Linux `BOOT.BIN`/`image.ub` is UART proof only, not a Golden Phase seal.

## Physical evidence (future)

Folders under `fpga/zynq/evidence/test_*` plus boot / g1p.  
Must include `PHYSICALLY_VERIFIED.txt`, UART capture, bitstream SHA, vectors SHA, Vivado util/timing/version, BSP manifest, program/capture commands — **only after real hardware PASS**.

## G2–G7 (blocked until G1P — Golden Phase OS, not Linux)

| Gate | Prove |
|------|-------|
| G2 | GP-OS kernel on A9 (stage-0→bit→`golden_phase_os.elf`); scheduler; reset/power cycle |
| G3 | DDR patterns + MMU/cache + GPL records + PL↔DDR |
| G4 | Native SD driver + GP filesystem + power-cycle persistence |
| G5 | Native GEM/PHY drivers + GPL Ethernet transport + fault detection |
| G6 | Full GP-OS ↔ PL (SD→DDR→PL→evidence→Ethernet) |
| G7 | Larger lanes / lattices (stop at first unproven size) |

Development order detail: `GOLDEN_PHASE_ZYNQ_STACK.md` (§ Development order).
