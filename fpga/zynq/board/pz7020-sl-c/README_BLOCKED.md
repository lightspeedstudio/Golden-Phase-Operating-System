# PZ7020-SL-C board support — PARTIAL (seller materials arrived)

## Status

`GATE=G1P_ZYNQ_PHYSICAL`  
`STATUS=PARTIAL_BSP` — schematic + datasheets + stock BOOT.BIN present; **Vivado project / XDC / PS7 export still missing**  
`PHYSICAL_BOARD_CLAIM=FALSE` (no Kairos seal yet)  
`UART_EVIDENCE_PORT=COM12 115200 8N1`

## Docs in this folder

- [`SELLER_PACKAGE_INVENTORY.md`](SELLER_PACKAGE_INVENTORY.md) — what arrived  
- [`OPEN_SOURCE_FINDINGS.md`](OPEN_SOURCE_FINDINGS.md) — LiteX cross-check  
- [`MISSING_BSP_PHYSICAL_BLOCKER.md`](MISSING_BSP_PHYSICAL_BLOCKER.md) — original blocker (superseded in part)  
- [`seller/`](seller/) — staged schematic, pin xlsx, BOOT.BIN  

## Have now

- [x] Schematic PDF  
- [x] Connector pin spreadsheet  
- [x] DDR / PHY / UART / Flash datasheets  
- [x] Seller stock `BOOT.BIN` (+ Linux `image.ub`)  
- [x] CH340 UART driver  

## Still need (ask Puzhi for demo pack / 例程)

- [ ] Vivado project (`.xpr`) or TCL to recreate PS7  
- [ ] XDC / constraints  
- [ ] `.xsa` or `ps7_init` / FSBL sources  
- [ ] Bare-metal Hello World (preferred over Linux for our path)

## Hard prohibitions

- Do **not** substitute AMD ZC702 or ALINX AX7020 projects.  
- Do **not** claim Golden Phase physical PASS from seller Linux `BOOT.BIN`.  
