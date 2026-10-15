# Kairos Zynq Golden Phase Bring-up

Math first, hardware second. **Golden Phase OS** is the bare-metal OS itself (direct on Cortex-A9). Linux is not a seal path.

Stack doctrine: [`GOLDEN_PHASE_ZYNQ_STACK.md`](GOLDEN_PHASE_ZYNQ_STACK.md)  
Proof ladder: [`docs/reports/ZYNQ_PROOF_LADDER.md`](../../docs/reports/ZYNQ_PROOF_LADDER.md).

## Boot package (correct)

```text
BOOT.BIN = [bootloader] stage0.elf  +  system.bit  +  golden_phase_os.elf
```

Early bring-up may temporarily use `zynq_fsbl.elf` + `g1p_smoke.elf`. Never seal with `image.ub` / Linux / U-Boot.

## Generate G0 (host oracle)

From the `kairos` crate root:

```powershell
cargo run --bin gpl_oracle -- --update-frozen
cargo run --bin gpl_oracle -- --check
```

Artifacts land in `oracle/`:

- `vectors.bin`
- `manifest.txt`
- `digests.sha256`
- `FORMAT.md`

CI must regenerate into a temp directory and **byte-compare** against committed files. Use `--update-frozen` only when intentionally refreshing.

```powershell
.\scripts\generate_oracle.ps1
```

## Verify frozen artifacts

```powershell
cargo test --test gpl_oracle_tests
cargo run --bin gpl_oracle -- --check
```

## Run G1S (RTL simulation)

Requires Icarus Verilog (`iverilog` + `vvp`) on PATH.

```powershell
.\scripts\test_g1s.ps1
```

Evidence is written to `evidence/g1s_rtl_sim/` as **`SIMULATION_VERIFIED.txt`** (never `PHYSICALLY_VERIFIED.txt`). Output must include `PHYSICAL_BOARD_CLAIM=FALSE`.

## G1P / board package

Seller BSP for **PZ7020-StarLite** is under [`board/pz7020-sl-c/`](board/pz7020-sl-c/) (`SELLER_PACKAGE_INVENTORY.md`).

Vivado release dir (bitstream + XSA):

```text
fpga/zynq/release/PZ7020_G1P_SD_SMOKE_V1/
```

Portable layers:

- **Common GP-OS** — kernel, algebra, most drivers/services/apps  
- **Zynq-7000 arch** — A9, GIC, UART/SD/GEM/timers, AXI  
- **Board package** — DDR/MIO/PHY/XDC for StarLite (swap package for a new board rev)

## Evidence locations

| Gate | Path |
|------|------|
| G0 | `evidence/g0_math_oracle/` |
| G1S | `evidence/g1s_rtl_sim/` |
| G1P | `evidence/g1p_zynq_map/` + release XSA/bit (physical seal after COM12 PASS) |
