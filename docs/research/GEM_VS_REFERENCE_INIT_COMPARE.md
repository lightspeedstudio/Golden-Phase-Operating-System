# GEM0 / RTL8211F — Reference Init Compare vs `gem.rs`

**Date:** 2026-08-01 (audit); cleanup applied same day — see [`PN_WIRE1_GEM_DRIVER_CLEANUP.md`](PN_WIRE1_GEM_DRIVER_CLEANUP.md)  
**Status:** audit complete; Gate A software defects corrected in tree — **Ethernet still NOT PASS**; U15 scope A/B/C still required  
**Scope:** line-level compare of manufacturer + Xilinx + seller sources against [`src/board/zynq7000/gem.rs`](../../src/board/zynq7000/gem.rs)  
**Does not claim wire TX/RX fixed.**

### Gate A follow-up (do not rewrite the audit gaps as “already fixed historically”)

| Defect | Audit finding | After Gate A |
|--------|---------------|--------------|
| NWCFG bit names | FCS_REMOVE/COPY_ALL swapped vs xemacps | Corrected to COPYALLEN/PAUSEEN/FCSREM |
| DMACR | `0x00040410` vs ~`0x00180700` | Constructed `0x00040700` for **256-byte** buffers (honest; not 1536) |
| RX OWN | status bit31 | addr word bit0 + SOF/EOF/len helpers |
| PHY `0xB100` | missing | **Still not** in live path (documented only) |
| `rgmii-id` delay | tried; PC=0; reverted | Still reverted |

---

## Sources actually opened this pass

| Source | Location | Depth |
|--------|----------|-------|
| RTL8211FD manufacturer datasheet | `01.芯片手册--datasheet/.../Giga ETH/RTL8211FD.pdf` (72 pp, extracted) | Reset, TXDLY/RXDLY straps, BMCR/Table 22 |
| Linux Realtek PHY driver | upstream `drivers/net/phy/realtek.c` (`rtl8211f_config_init`) | Full `rgmii-id` → page `0xd08` TX/RX delay policy |
| Xilinx Cadence GEM (`emacps_v3_9`) | Extracted from seller `3_Linux.part1.rar` → `xemacps.c` / `xemacps_hw.c` / `xemacps_hw.h` / `xemacps_control.c` | Reset, Start, NWCFG/DMACR/STARTTX bit defs |
| Seller DTS | Same RAR → `pcw.dtsi` / `system-top.dts` | `phy-mode = "rgmii-id"` on `&gem0` |
| Seller PL RTL8211 config | `1_FPGA.rar` → `mdio_ctrl.v` | Soft-reset `BMCR=0xB100`, page `0xd08` delay writes |
| Seller PS7 clocks/MIO | `fpga/zynq/board/pz7020-sl-c/seller/ps7_hello_world/ps7_init.c` | `GEM0_RCLK/CLK`, MIO16/52 |
| Prior board evidence | U-Boot `rgmii-id` COM12; RGMII delay experiment FAIL/REVERT | [`PN_WIRE1_RGMII_DELAY_COMPARE.md`](PN_WIRE1_RGMII_DELAY_COMPARE.md) |

Not available in-tree as a standalone Realtek Windows/SDK blob; Linux `realtek.c` is the public manufacturer-aligned driver used here.

---

## Match / mismatch matrix

| Area | Reference sequence | `gem.rs` today | Verdict |
|------|--------------------|----------------|---------|
| **SLCR GEM clocks** | `ps7_init`: `GEM0_RCLK_CTRL=0x1`, `GEM0_CLK_CTRL=0x00100801` | Not programmed in driver; live dump matches seller expected | **MATCH** (FSBL) |
| **MIO 16–27 / 52–53** | `ps7_init` ENET0 + MDIO | Same; MDIO + PHY ID proven | **MATCH** (FSBL) |
| **phy-mode** | DTS/`U-Boot`: `rgmii-id` | No phy-mode; delays not kept in `wire_init` | **GAP** (tried once → PC still 0 → reverted) |
| **RGMII TX delay** | Linux: page `0xd08` reg `0x11` bit8; PL `mdio_ctrl` writes `{7'h0,tx_delay,8'h09}` | Not in live path | **GAP** (experiment: `0009→0109`, no PC frames) |
| **RGMII RX delay** | Linux: page `0xd08` reg `0x15` bit3 ON for `rgmii-id` | Not in live path | Strap already had RX delay (`0019`) in experiment |
| **PHY hardware reset** | Datasheet: `PHYRSTB` ≥10 ms | GPIO reset still **unresolved** (ETH-0) | **UNRESOLVED** |
| **PHY soft reset** | PL: `BMCR=0xB100` then poll BMSR AN+link; datasheet BMCR.15 | **Absent** from `wire_init` | **GAP** |
| **GEM HW reset** | `XEmacPs_Reset` / `XEmacPs_ResetHw`: stop TX/RX, clear QBASE, NWCFG reset mask, clear stats/ISR | Partial: clear TXSR/RXSR/IDR, set MDEN; **no** full `ResetHw` | **PARTIAL** |
| **NWCFG speed/duplex** | xemacps: bit0=100, bit1=FD, bit10=1000 | `nwcfg_for_link` same | **MATCH** |
| **NWCFG copy-all / FCS** | xemacps: `COPYALLEN=bit4 (0x10)`, `FCSREM=bit17 (0x20000)`, `PAUSEEN=bit13 (0x2000)` | `NWCFG_FCS_REMOVE = 1<<4` (**actually COPYALL**), `NWCFG_COPY_ALL = 1<<13` (**actually PAUSEEN**); **true FCSREM never set** | **MISMATCH** (names/bits vs Xilinx) |
| **Measured NWCFG** | — | `0x000C2013` = MDC÷48 \| PAUSEEN \| COPYALL \| FD \| 100 | Explains accidental promiscuous ON; FCS strip OFF |
| **DMACR** | Zynq-7000 (GEM ver≤2): RXBUF≈1536 (`0x18<<16`) \| RXSIZE \| TXSIZE → ~`0x00180700` | Hard-coded `0x00040410` (RXBUF units=4→256 B, TXSIZE, INCR16) | **MISMATCH** |
| **TX descriptor** | Length \| LAST \| WRAP; clear USED; STARTTX | Same pattern in `wire_tx_raw` | **MATCH** (software path) |
| **TX start** | `XEMACPS_NWCTRL_STARTTX_MASK = 0x200` (bit9) | `NWCTRL_TXSTART = 1<<9` | **MATCH** |
| **TX enable** | `XEmacPs_Start`: set TXEN/RXEN after QBASE | `wire_init` sets `RXEN\|TXEN\|MDEN` after rings | **MATCH** (order OK) |
| **RX ownership** | UG585 / xemacps: RX ownership in **address word bit0**; used/length in status | `wire_rx_raw` polls **status bit31** / length | **MISMATCH** (known; phyloop already showed addr bit0 never flipped) |
| **Cache** | Xilinx apps: `Xil_DCacheFlush` before TX if D-cache on | Comment: D-cache off at startup; no clean/invalidate helpers in zynq7000 | **ASSUMED OK** if SCTLR.C stays 0 — not re-proven this pass |
| **DMA / TX complete** | TXSR COMPLETE / ownership return | Proven on COM12; PC still 0 | **GEM DMA OK ≠ wire OK** |

---

## Critical software findings (new depth)

### 1) NWCFG bit constants disagree with Xilinx `xemacps_hw.h`

```text
Xilinx:  COPYALLEN = 0x00000010   FCSREM = 0x00020000   PAUSEEN = 0x00002000
gem.rs:  "FCS_REMOVE" = 1<<4      "COPY_ALL" = 1<<13     (FCSREM never written)
```

So `wire_init` enables **copy-all + pause**, not **FCS remove + copy-all**.  
This is a real software defect vs the Cadence/Xilinx map. It is **not** proven to be the reason PC sees zero TX frames (TX path does not need FCSREM/COPYALL).

### 2) Official Realtek + seller DTS agree on `rgmii-id`

- `pcw.dtsi`: `&gem0 { phy-mode = "rgmii-id"; }`
- Linux `rtl8211f_config_init`: for `RGMII_ID` set TX delay bit8 + RX delay bit3 on page `0xd08`
- Already run on board: TX `0009→0109`, RX stayed `0019`; **PC still 0**; code reverted

Delay programming alone is **insufficient** as the sole fix.

### 3) PHY soft-reset missing vs seller PL sequence

`mdio_ctrl.v` always starts with `BMCR=0xB100` (soft reset + 100/FD pattern), then AN+link poll, then delay page.  
`gem.rs` never soft-resets the PHY in `wire_init`.

### 4) DMACR buffer-size policy differs from `XEmacPs_Reset`

Bare-metal uses a smaller RX buffer unit field and sets INCR16; Xilinx Zynq-7000 path uses 1536-byte RX buffers and (for GEM ver≤2) does not force INCR16.  
Relevant to RX sizing more than to “does TXC toggle.”

---

## Hypotheses for remaining wire failure (ordered)

| ID | Hypothesis | Needs |
|----|------------|-------|
| G | GEM never drives RGMII TXC during burst | U15 → **C** |
| H | Clock only; TX_CTL/TXD idle (GEM kick/clock-only) | U15 → **B** |
| I | Full RGMII TX activity leaves GEM; fault after PHY / magnetics / jack / PC | U15 → **A** |
| J | NWCFG/DMACR mismatches break RX or PCS loopback, not TX DMA | register decode + phyloop (secondary) |
| K | Missing PHY soft-reset / GPIO reset leaves PHY in bad RGMII state | soft-reset trial **after** scope class |

**Do not mark Ethernet fixed from UART `txburst: OK` alone.**

---

## Next actions (strict order)

1. **Physical:** [`PN_WIRE1_U15_SCOPE.md`](PN_WIRE1_U15_SCOPE.md) — `pn_wire1_u15_scope_run.py` → report **A / B / C**
2. **Software (only after A/B/C):**
   - If **C**: focus GEM output enable / MIO / NWCTRL TX path on silicon (clocks already match FSBL)
   - If **B**: focus TX BD length/LAST/USED + STARTTX vs data path
   - If **A**: focus PHY/media/PC; then consider BMCR soft-reset + (re)validate delay vs strap
3. **Safe software cleanup (does not claim wire PASS):** correct NWCFG constant names/bits to match `xemacps_hw.h`; optionally align DMACR RXBUF to 1536

---

## Acceptance for this audit

- [x] Seller Linux RAR opened; `xemacps` + `pcw.dtsi` extracted
- [x] Manufacturer datasheet pages for reset / TXDLY-RXDLY reviewed
- [x] Upstream Realtek `rgmii-id` sequence confirmed
- [x] Gaps vs `gem.rs` listed without inventing a wire fix
- [ ] U15 A/B/C still open
