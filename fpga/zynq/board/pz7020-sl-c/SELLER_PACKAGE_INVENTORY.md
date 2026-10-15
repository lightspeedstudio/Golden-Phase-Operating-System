# Seller package inventory (received)

**Staged copies:** `seller/` in this folder  
**Source roots on Desktop FullOS:**

- `01.芯片手册--datasheet`
- `02.软件和驱动--Software Tools and Drivers`
- `03.开机测试--Boot Test`
- `04.硬件相关--Hardware`
- `05.教程和源码--Course_Demos` ← **BSP/demo unlock**

## What you now have (real)

| Item | Path / note |
|------|-------------|
| **Schematic PDF** | `seller/Puzhi_PZ-StarLite_Schematic.pdf` |
| **Connector pin map XLSX** | `seller/PZ-Starlite_connectors_pins.xlsx` (JM1/JM2 PL pins) |
| **Stock BOOT.BIN** | `seller/BOOT.BIN` (+ `image.ub` in Boot Test — seller Linux; not Kairos) |
| **DDR / PHY / UART / QSPI datasheets** | under `01…datasheet` (MT41K256M16TW-107, RTL8211FD, CH340E, W25Q128, …) |
| **Course guides (PDF)** | FPGA / SDK / Linux / HLS / PYNQ + hardware user manual (copied into `seller/`) |
| **Course extract** | `seller/course/` (~4700 files from `1_FPGA.rar`, `2_SDK.rar`; Linux/HLS/PYNQ RARs not required for G1P) |

### Canonical BSP copies (use these for G1P)

| Item | Staged path | SHA-256 (first 16…) |
|------|-------------|---------------------|
| **ps7_init.*** + **system.hdf** + seller bit | `seller/ps7_hello_world/` | `ps7_init.c` `556ca51858dc41c7…` · `system.hdf` `5929763cb2be7f65…` |
| **PL LED/clk XDC** | `seller/xdc/led_twinkle.xdc` | `2c33bd924d8ab63e…` |
| **PL MDIO XDC** | `seller/xdc/mdio_rw_test.xdc` | (MDC **U14**, MDIO **U15**) |
| **PL UART loopback XDC** | `seller/xdc/uart_loopback_pl.xdc` | RX/TX **H16/H17** (expansion — **not** COM12) |

**Source of PS7 platform:**  
`course/2_SDK/2_SDK/1_hello_world/PZ_Uart.sdk/design_1_wrapper_hw_platform_0/`

### Seller PS7 fact sheet (from `ps7_init.html`)

| Item | Value |
|------|--------|
| Part | **xc7z020clg400-2** |
| DDR part (Vivado preset) | **MT41K256M16 RE-125** @ **533.33 MHz** (datasheet kit also has **TW-107** — same family; seal naming must match what we actually bring up) |
| UART0 | **MIO 10–11** → USB-UART **COM12** |
| QSPI | **MIO 1–6** |
| ENET0 | **MIO 16–27** + MDIO **MIO 52–53** |
| USB0 | **MIO 28–39**, reset **MIO 46** |
| SD0 | **MIO 40–45** |

### Seller PL pins (from course XDCs)

| Signal | Pin |
|--------|-----|
| sys_clk 50 MHz | **U18** |
| sys_rst_n | **U19** |
| led[0]/led[1] | **R19** / **V13** |
| key | **G14** (also **J15** in some labs) |
| PL eth MDIO | **U14** / **U15** |
| PL UART (demo) | **H16** / **H17** |

Note: LiteX listed PL UART **M19/M20**; seller demo uses **H16/H17**. Prefer **seller XDC** for Kairos PL demos.

## G1P *build* gate — cleared for seller materials

| Item | Status |
|------|--------|
| Seller Vivado `.xpr` labs | **Present** under `seller/course/1_FPGA/` |
| Seller **XDC** | **Present** → `seller/xdc/` |
| Seller **ps7_init** / **HDF** | **Present** → `seller/ps7_hello_world/` |
| Seller SDK hello / FSBL labs | **Present** under `seller/course/2_SDK/` |

Still needed for full G1P algebra seal / G2: expand beyond smoke compose(0,1) into frozen-vector sweep, then native Golden Phase OS port. First SD UART + GPL AXI smoke: **PASS** (`evidence/g1p_sd_smoke_v1/`).

## Immediate useful test (no our bitstream yet)

1. Format a microSD (FAT32).  
2. Copy seller `BOOT.BIN` (+ `image.ub` if required by their boot guide).  
3. Boot StarLite from SD.  
4. Open **COM12** 115200 — expect seller Linux/U-Boot banner.  

That proves the board + UART path. It is **not** a Golden Phase seal.

## Evidence rule

Do **not** treat seller `BOOT.BIN` as Kairos G1P.  
Do **not** use ALINX/ZC702 projects.  
Seller Course_Demos + schematic are wiring/PS7 authority for this board.
