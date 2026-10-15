# board/zynq7000 — PZ7020-StarLite / PZ7020-SL-C

| Fact | Value |
|------|--------|
| Part | `xc7z020clg400-2` |
| UART | PS UART0 → COM12 115200 8N1 |
| G1P AXI | `0x40000000`, GIC SPI 61 |
| GEM0 | `0xE000B000`, MIO 16–27, MDIO 52–53, RTL8211 |
| SD0 | `0xE0100000` |
| GP-OS load | DDR `0x00100000` |
| Page pool | `0x00F00000` (64 × 4 KiB) |

Bitstream: frozen V2 `system.bit` until a G1P/G7 change requires a new bit.
OS sources: `kairos/zynq_a9_os/`.
