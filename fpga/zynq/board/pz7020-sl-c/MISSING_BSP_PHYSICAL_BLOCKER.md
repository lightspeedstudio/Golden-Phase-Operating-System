# PHYSICAL BLOCKER — PZ7020-SL-C BSP Missing

**Timestamp:** 2026-07-28  
**Gate stopped at:** G1P (physical PL mapping) — cannot proceed to G2–G7  
**Physical board claim:** FALSE  

## Inventory (what exists)

| Item | Status |
|------|--------|
| G0 oracle `vectors.bin` | PASS — SHA-256 `99eca89134347850322269f6c6e55d6ab3f7dd1cdaa1237988c6c5b94448183b` |
| G1S RTL + Icarus sim | PASS — `fpga/zynq/evidence/g1s_rtl_sim/` (`SIMULATION_VERIFIED.txt` only) |
| Vendor-neutral RTL | Present under `fpga/zynq/rtl/` |
| `fpga/zynq/board/pz7020-sl-c/` | Contains **only** `README_BLOCKED.md` |
| Seller XDC | **MISSING** |
| Seller schematic | **MISSING** |
| Seller Vivado example / `.xpr` | **MISSING** |
| `.xsa` / `.hdf` | **MISSING** |
| `ps7_init.c` / `.h` / `.tcl` | **MISSING** |
| FSBL ELF | **MISSING** |
| Bitstream | **MISSING** (not fabricated) |
| Vivado installation | **NOT FOUND** (`C:\Xilinx`, `C:\Program Files\Xilinx`, `D:\Xilinx`) |
| AMD ZC702 constraints | Not used (correct — must not substitute) |
| `kairos/fpga/board_top.xdc` | Artix-7 **example** only — **not** Starlite |
| `chipverify-project.zip` | `design.v` / `testbench.v` only — **not** Starlite BSP |
| Host COM12 | Port **opens** at 115200; idle (no Golden Phase banner) — **not** a seal |
| Vivado 2026.1 | Installer was downloading (~30% / ~25 GB) — toolchain not ready until Install + Final Processing finish |

## Searched locations

- `FullOS/` (including `kairos/fpga/zynq/`)
- `Desktop/`
- `Downloads/`
- `Documents/`

No PZ7020-SL-C / Starlite seller package, schematic, XDC, XSA, or `ps7_init*` found.

## Exact missing physical dependencies (required to continue)

1. Seller **XDC** for PZ7020-SL-C  
2. Seller **schematic** (or PDF with MIO / DDR / PHY callouts)  
3. Seller **Vivado example** or PS7 TCL  
4. **DDR part number** + DDR configuration  
5. **PS clock** configuration  
6. **UART MIO** mapping (host evidence path **COM12**)  
7. **SD MIO** wiring  
8. **Ethernet PHY model** + **PHY address**  
9. Reset / JTAG / boot-switch notes from **board manual**  
10. Working **Vivado** + **Bootgen** toolchain on the build machine  

## What was NOT done (by project law)

- No guessed MIO / DDR / PHY / pin map  
- No AMD ZC702 constraint substitution  
- No fabricated bitstream, BOOT.BIN, or COM12 capture seal  
- No `PHYSICALLY_VERIFIED.txt` for any G1P–G7 test  
- No Linux / PetaLinux / Buildroot  

## What is allowed after BSP arrives

1. Install seller files under `fpga/zynq/board/pz7020-sl-c/`  
2. Install Vivado + Bootgen  
3. Build PL from `fpga/zynq/rtl/` with **seller** PS7 + XDC only  
4. Port Golden Phase OS ARM target using BSP-derived memory map  
5. Build `BOOT.BIN` = FSBL + bitstream + GP-OS ELF  
6. Flash microSD via safe script; boot; run Tests 01→24 in order on **COM12**  

## Stop line

```text
STAGE=pl
RESULT=FAIL
FIRST_FAILURE=MISSING_PZ7020_SL_C_SELLER_BSP
NEXT_ACTION=Upload seller BSP (XDC, schematic, Vivado/PS7, DDR part, MIO map, PHY addr) into fpga/zynq/board/pz7020-sl-c/ and install Vivado
PHYSICAL_BOARD_CLAIM=FALSE
```
