# PZ7020-SL-C / PZ7020-StarLite — open web findings

**Seller:** Puzhi Electronic Technology (Shanghai) — **not** ALINX.  
**Do not** use ALINX AX7020 BSP for this board.

## Official product pages

- EN: https://www.en.puzhi.com/Product/AMD-FPGA-Development-Board/Zynq-7000-SoC/PZ7020-StarLite  
- CN: https://puzhi.com/detail/125.html  
- Support: support@puzhitech.com  

**Official full BSP (XDC, Vivado demos, schematics ZIP):**  
Seller states materials are on **Google Drive / Yandex / Dropbox / 百度网盘 after purchase** — contact customer service.  
→ **Received locally** as `FullOS/05.教程和源码--Course_Demos` (plus folders 01–04). Staged under `seller/` — see `SELLER_PACKAGE_INVENTORY.md`.

## Open / third-party sources (partial — useful, not a full seller seal)

### 1. LiteX-Boards (BSD-2-Clause) — best public pin + PS7 config

- Platform: https://github.com/litex-hub/litex-boards/blob/master/litex_boards/platforms/puzhi_p7_starlite.py  
- Target: https://github.com/litex-hub/litex-boards/blob/master/litex_boards/targets/puzhi_p7_starlite.py  

Captured locally as references under `open_refs/` when mirrored.

### 2. Schematic fragments (Scribd)

- https://www.scribd.com/document/974652130/Puzhi-PZ-StarLite-Schematic  
- Title: PZ-StarLite schematic / power (Puzhi, ~Feb 2025, multi-sheet)

Use for cross-check only; prefer seller PDF after you get Drive access.

## Hardware facts extracted (cross-checked)

| Item | Value | Source |
|------|--------|--------|
| Device | **XC7Z020-2CLG400I** | Puzhi product page + LiteX |
| Package | CLG400 | LiteX `device = f"{variant}clg400-2"` |
| DDR | **512 MB**, **16-bit**, DDR3L | Product page + LiteX |
| DDR part (LiteX PS7) | **MT41K256M16 RE-125** | LiteX `PCW_UIPARAM_DDR_PARTNO` |
| PS crystal | **33.333333 MHz** | LiteX |
| APU freq (LiteX preset) | ~666.67 MHz | LiteX (product marketing says ~766 MHz — confirm in seller PS7) |
| QSPI | MIO **1..6**, x4, 200 MHz | LiteX |
| USB0 | MIO **28..39**, reset MIO **46** | LiteX + schematic labels |
| UART0 (PS, USB-UART / COM12) | **RX MIO10, TX MIO11** | Schematic labels `UART0_RX` / `UART0_TX` |
| SD0 | MIO **40..45** (CLK/CMD/DAT0–3) | Schematic + LiteX `add_sdio(0, "MIO 40 .. 45")` |
| PS GEM MDIO | **MDC MIO52, MDIO MIO53** | Schematic `PS_GPHY_MDC/MDIO` |
| PS Ethernet PHY | Gigabit on PS (RTL8211 family per docs/snippets) | Product + schematic `PS_GPHY_*` |
| PL Ethernet | Second GbE on PL RGMII (RTL8211F in LiteX PL pins) | LiteX `eth` / `eth_clocks` |
| PL 50 MHz clk | **U18** | LiteX |
| PL reset_n | **U19** (also tied to PS_PORB note) | LiteX |
| PL UART (seller demo) | **H16/H17** | Course `uart_loopback_pl.xdc` — expansion, **not** USB COM |
| PL UART (LiteX JM1) | TX **M19**, RX **M20** | LiteX — do not prefer over seller XDC |
| Host evidence UART | **COM12** 115200 8N1 | PS UART0 MIO10–11 via CH340 |

### PHY address

**Not fixed in public LiteX config.** Must come from seller schematic strap resistors or an MDIO scan (read non-`0xFFFF` ID across addresses 0–31).  
Do **not** invent a PHY address for seals.

## What this does / does not unlock

| Unlock | Status |
|--------|--------|
| Identify seller + chip + DDR part candidate | Yes |
| Draft PS7 UART/SD/QSPI/USB/MDIO MIOs | Yes (from LiteX + schematic labels) |
| Full official Vivado project / XDC / FSBL demo | **Yes** — Course_Demos staged; see inventory |
| Claim G1P physical seal from web scrapes alone | **No** |

## Next actions (Golden Phase stack — not Linux)

Canon: `fpga/zynq/GOLDEN_PHASE_ZYNQ_STACK.md`  
`BootROM → stage-0/FSBL → G1P bit → GP-OS → drivers → services → apps`

1. Finish Vitis Embedded `aarch32` → temporary `zynq_fsbl` + `g1p_smoke` + `BOOT.BIN`.  
2. COM12 SD smoke; then native GP-OS port (no `image.ub` / Linux).  
3. Seller `BOOT.BIN` only proved board/UART, not Golden Phase.

## Explicit non-substitutions

- ALINX **AX7020** GitHub (`alinxalinx/AX7020_*`) — different board.  
- AMD **ZC702** constraints — different board.  
- LiteX alone — not a substitute for seller bitstream/FSBL demos when sealing.
