# PN-WIRE-1 — PHY PCS loopback isolation (next gate)

**Status:** board run **NO MATCH** (2026-08-01) — pre Gate A RX ownership fix  
**Evidence:** `fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_phyloop_20260801_182011/`  
**Not a wire PASS** — diagnostic only  
**Authority:** RTL8211F(D) datasheet Table 22 (BMCR); StarLite schematic  

### Gate A note (2026-08-01)

Prior phyloop dumps treated RX “USED” as **status bit31**. Xilinx/UG585 ownership is **address word bit0**.  
Gate A corrects software parsing ([`PN_WIRE1_GEM_DRIVER_CLEANUP.md`](PN_WIRE1_GEM_DRIVER_CLEANUP.md)).  
Re-run `net wire phyloop` after flash before claiming the earlier NO MATCH was purely electrical.  
**External TX remains unproven** until PC sees `02:00:00:47:01:50`.

### Run result (instrumented `D8BDE552…`, log `debug-2a2f53.log`)

| Check | Value |
|-------|--------|
| BMCR loopback | wrote/read `0x6100` |
| TX complete | yes (`TXSR=0x21`, no underrun/HRESP) |
| RX MATCH | **NO** (`last_len=0`) |
| SLCR vs seller PS7 | **all match** (GEM0 clocks + MIO16/22/52/53) |
| RX armed | RXEN=true, RXQBASE set, BD USED=false (HW owns) before/after |
| RXSR/ISR | `0` before and after — no RX event |
| Interpretation | Not SLCR mismatch / not unarmed RX / not MAC filter reject; RGMII electrical or PHY loopback speed latch still open |

#### Hypothesis scorecard (session `2a2f53`)

| ID | Hypothesis | Result | Evidence |
|----|------------|--------|----------|
| A | SLCR clock/MIO ≠ seller | **REJECTED** | `rclk_ok=gclk_ok=mio16_ok=mio22_ok=mdio_ok=true` |
| B | RX DMA not armed | **REJECTED** | `RXEN=true`, RXBD status `0` (HW-owned) unchanged |
| C | RXSR/ISR show reject/activity | **REJECTED** | `RXSR=0 ISR=0 any_rx=false` |
| D | BMCR loopback not programmed | **REJECTED** as “bit not set”; **open** on Note-1 speed latch | readback `0x6100`; datasheet Note 1: 0.12/0.13/0.6/0.8 need reset/restart/PWD |
| E | RX frames rejected by MATCH filter | **REJECTED** | `any_rx=false` |
| F | Forced 100/full not latched (Note 1) | **REJECTED** as root cause | Note-1 run: `latched=0x2100` speed100/duplex OK, `matched_speed_bits=true`, still `NO_MATCH` / `RXSR=0` / RX addr bit0 still 0 |

### Note-1 run (`BOOT 8F10DD72…`, log `debug-2a2f53.log`)

- `bmcr_note1`: force/latched `0x2100`, ane=false, speed100=true, duplex=true
- `bmcr_lb_after_note1`: loop=true, matched_speed_bits=true
- TX complete; RX BD addr words still even (UG585 ownership bit0 = HW still owns) → **no RX DMA write**
- Still not SLCR / unarmed RX / filter / missing Note-1 latch

**Next decisive check:** scope/LA on U15 RGMII `TXC` / `TX_CTL` / `TXD` during `net wire txburst` — procedure [`PN_WIRE1_U15_SCOPE.md`](PN_WIRE1_U15_SCOPE.md) / `scripts/pn_wire1_u15_scope_run.py`.

### Scope gate (not software-complete)

UART `txburst` software evidence (2026-08-01): two × `net wire txburst 32` OK (`debug-2a2f53.log` burst1/burst2).

| Code | Physical observation | Status |
|------|----------------------|--------|
| **A** | TXC + TX_CTL + TXD[3:0] burst activity | **unrecorded** |
| **B** | TXC toggles; TX_CTL/TXD idle | **unrecorded** |
| **C** | No TXC during burst | **unrecorded** |

Do **not** claim GEM RGMII pin activity or Ethernet fix from UART logs alone.

## Proven so far (external TX still unproven)

GEM TX complete, no underrun/HRESP, MAC loopback OK, link 100/full, RGMII delay experiment no PC frames.

## Jack / PHY path (schematic)

| Path | Schematic | Chip | MDIO |
|------|-----------|------|------|
| **PS GEM0 (this test)** | sheet `PZ-StarLite--GPHY(PS)` | **U15** `RTL8211F-CG` nets `PS_GPHY_*` | PS MDC/MDIO **MIO 52–53** → PHY addr **0** |
| PL second Ethernet | PL GPHY sheet | **U16** `RTL8211F-CG` nets `PL_GPHY_*` | PL MDIO — **out of scope** |

Cable must be on the **PS** RJ45 (U15 / `PS_GPHY`), not the PL jack. Link UP on GEM0 MDIO addr 0 is necessary but confirm the physical plug matches U15.

## Test A — PCS digital loopback (implemented)

Datasheet BMCR `0x00` bit **0.14 Loopback**:

> Enable PCS loopback mode — RGMII transmit data routed to RGMII receive data path.

Sequence in `gem::wire_phy_pcs_loopback`:

1. `net wire init` (MAC loopback off, multi-BD wire path)
2. Snapshot BMCR/BMSR + PHYSR page `0xA43` reg `0x1A` (MDI plug / ALDPS only)
3. Write BMCR `0x6100` = Loopback | Speed100 | FullDuplex (AN off)
4. Drain RX, TX one broadcast `0x88B5`, poll RX for matching src/ethertype
5. Restore prior BMCR

| Result | Meaning |
|--------|---------|
| RX MATCH | GEM↔PHY RGMII works; fault after PHY digital (analog/magnetics/jack/cable/PC) |
| RX NO MATCH | Still GEM external ↔ RTL8211F RGMII boundary (or RX return) |

Shell:

```text
gpos> net wire init
gpos> net wire phyloop
```

## Test B — remote/cable loopback

**Unsupported** in the RTL8211F(D) datasheet register map reviewed here (no remote/MDI loopback bit in BMCR/PHYCR1/PHYCR2). Do not invent vendor addresses.

## PHY counters

No transmit packet / transmit error counter is defined in the Clause-22 / page `0xA43` registers used for this diagnostic. Report **PHY TX count = unknown**. Available status used: BMCR isolate/pwd/loop, BMSR link, PHYSR ALDPS/MDI plug.

## Scope aid

```text
gpos> net wire txburst 32
```

Reuses `wire_tx_raw` only (same broadcast PNW1 frame). For probing RGMII TXC / TX_CTL / TXD during repeats.

## Decisive table (after phyloop run)

| GEM TX complete | PHY TX count | PCS loopback RX | PC sees frame | Meaning |
|-----------------|--------------|-----------------|---------------|---------|
| yes | unknown | MATCH | no | post-PHY (analog/jack/cable/PC) |
| yes | unknown | NO MATCH | no | GEM↔PHY RGMII boundary |
| yes | unknown | MATCH | yes | then debug host REP / board RX |

## Out of scope

PhaseNet, PN01, descriptor-ring redesign, IP stack, PL Ethernet.
