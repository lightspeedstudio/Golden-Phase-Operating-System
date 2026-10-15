# PN-WIRE-1 — U15 RGMII Scope Gate (A / B / C)

**Status:** physical letter **UNRECORDED** until operator enters A/B/C  
**Hard stop:** no GEM/handshake/PHY soft-reset/RGMII-delay software until letter recorded  
**Does not claim wire PASS** from UART `txburst: OK`

## Why this gate exists

Handshake and PC capture already showed `PEER_HEARD_REQ=no` / `board_mac=0`.  
GEM TX-complete does not prove RGMII pins toggle. Next classification is **physical only**.

## Stimulus (agent / COM12)

```text
python kairos/scripts/pn_wire1_u15_scope_run.py --port COM12
```

Board commands:

```text
gpos> net wire init
gpos> net wire txburst 32
```

Evidence: `kairos/fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_u15_scope_<timestamp>/`

## Probe (operator)

Chip: **U15** `RTL8211F-CG` sheet `PZ-StarLite--GPHY(PS)` — **not U16**.

| Signal | Net (schematic) |
|--------|-----------------|
| TXC | `PS_GPHY_GTX_CLK` / PHY `TXC` |
| TXCTL | `PS_GPHY_TX_EN` / PHY `TXCTL` |
| TXD[3:0] | `PS_GPHY_TXD0..3` |

## Letter table

| LETTER | Observation | SOFTWARE_BRANCH (only after letter) |
|--------|-------------|-------------------------------------|
| **A** | TXC + TXCTL + TXD all burst | After PHY/media/PC; then BMCR soft-reset / delay |
| **B** | TXC toggles; TXCTL/TXD idle | TX BD / STARTTX / data-path |
| **C** | No TXC | GEM MIO / NWCTRL TX output |

Record after scoping:

```text
python kairos/scripts/pn_wire1_u15_scope_run.py --port COM12 --letter A
```

(or B / C). Do **not** invent a letter from UART logs alone.

## Current record

| Field | Value |
|-------|--------|
| Evidence | `kairos/fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_u15_scope_20260801_225149/` |
| UART stimulus | **OK** — two × `net wire txburst 32` |
| `LETTER` | **UNRECORDED** |
| `SOFTWARE_BRANCH` | **NONE_UNTIL_LETTER** |

Until operator enters `LETTER=A|B|C` (via `--letter`), no GEM/PHY software branch.
