# PN-WIRE-1 — RGMII / PHY init compare (before edit)

**Date:** 2026-08-01  
**Gate:** TX-leaves-GEM0 failed visually (GEM COMPLETE, PC sees 0 frames)  
**Scope:** compare seller/U-Boot/Linux vs bare-metal — **no code changes in this note**  
**Evidence run:** `fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_txgate_20260801_173412/`

## Comparison table

| Setting | Seller / U-Boot / Linux | Bare metal (Kairos GEM) | Difference |
|---------|-------------------------|-------------------------|------------|
| PHY reset | PL demo: BMCR soft-reset `0xB100` then link poll (`mdio_ctrl.v`). PS GPIO reset: **unresolved** in ETH-0. | No PHY soft-reset; no GPIO reset in `gem.rs` | Bare metal relies on power-on / FSBL only |
| RGMII mode | U-Boot: `interface rgmii-id` (`seller_boot_smoke` COM12). DTB: `phy-mode` = `rgmii-id` (`seller/image.ub`) | Implicit RGMII via MIO; **no** phy-mode / delay policy | Missing `rgmii-id` delay programming |
| TX delay (2 ns) | Linux/U-Boot Realtek: page `0xd08` reg `0x11` bit8 **ON** for `rgmii-id` / `rgmii-txid`. PL synth IP: `tx_delay_en=1` | **Not written** | **Missing** |
| RX delay (2 ns) | Linux Realtek: page `0xd08` reg `0x15` bit3 **ON** for `rgmii-id`. PL synth IP: `rx_delay_en=0` (PL-only path ≠ PS) | **Not written** | **Missing vs PS `rgmii-id`** |
| PHY page/register | Page select `0x1F` → `0x0d08`; delay regs `0x11` / `0x15` (not `0x1d`/`0x1e`) | Page `0x1F` only for PHYSR page `0xA43` / reg `0x1A` (speed/duplex read) | Delay page never selected |
| GEM clock 100M | FSBL/`ps7_init`: `GEM0_CLK_CTRL` → IO PLL, **125 MHz** source (`0xF8000140` = `0x00100801`); speed via MAC/PHY AN | Same FSBL clocks; `NWCFG` bit0 = 100 Mbps (`0x000C2013` measured) | No material clock gap found |
| MIO 16–27 | ENET0 RGMII (`SELLER_PACKAGE_INVENTORY.md`); `ps7_init` MIO programming | Same FSBL/`ps7_init_data.rs` | Match (FSBL) |
| MDC/MDIO 52–53 | PS_GPHY MDC/MDIO; MDIO works (PHY ID + link) | Same; MDIO proven | Match (FSBL) |
| PHY strap status | Straps (RXD0/RXD1) set power-on delay defaults; Linux **overrides** from `phy-mode` | Left at strap / reset defaults | Unknown until page `0xd08` is read |
| Vendor delay write | Seller Linux attaches `RTL8211F` under `rgmii-id` (implies driver delay config). PL lab writes `0xd08`/`0x11`/`0x15` explicitly | **No** vendor delay write | **Primary gap** |

### Sources

- U-Boot banner: `fpga/zynq/evidence/seller_boot_smoke/com12_capture_20260729_125652.txt`
- DTB string `rgmii-id`: `fpga/zynq/board/pz7020-sl-c/seller/image.ub`
- PL delay sequence: `…/19_eth_udp_loop/RTL8211_Config_IP/src/mdio_ctrl.v` (soft reset + page `0xd08`)
- PL synth params: `RTL8211_Config_IP_0.v` → `tx_delay_en=1`, `rx_delay_en=0` (**PL MAC path only**)
- Bare metal: `src/board/zynq7000/gem.rs` (no delay / no SLCR / no PHY reset)
- Clocks/MIO: `src/board/zynq7000/ps7_init_data.rs` + seller inventory
- Linux Realtek policy: `rgmii-id` → TX+RX delay bits on page `0xd08`

**Do not treat PL `tx=1/rx=0` as the PS GEM policy.** PS seller stack says `rgmii-id` → both delays on (Linux Realtek).

## Smallest missing init (candidate)

Match PS `rgmii-id` with the Realtek page sequence only (no PhaseNet / ring / RX changes):

```text
1. phy_write(addr, 0x1F, 0x0D08)          # page select
2. read  reg 0x11; set bit8 (TXDLY); write back
3. phy_write(addr, 0x1F, 0x0D08)
4. read  reg 0x15; set bit3 (RXDLY); write back
5. phy_write(addr, 0x1F, 0x0000)          # page 0
6. optional: dump 0x11/0x15 before+after for evidence
7. net wire ping debug + unfiltered PC sniff
```

If TX still invisible after both delays ON, next single-variable trial is TX-only (`rgmii-txid`), then soft-reset BMCR before delay write — still without touching PhaseNet.

## Pass condition (this sub-gate)

PC capture sees **one** frame with `src=02:00:00:47:01:50` (broadcast `0x88B5` preferred).

Full PN-WIRE-1 still requires REP RX later; not this step.

## Experiment result — FAIL / REVERTED

Evidence: `fpga/zynq/evidence/kairos_zynq_boot/pn_wire1_rgmii_20260801_175605/`

| Check | Result |
|-------|--------|
| reg 0x11 | `0009` → `0109` (TXDLY enabled) |
| reg 0x15 | `0019` → `0019` (RXDLY already set) |
| page restored | yes (`0x0000`) |
| link UP 100/full | yes |
| ownership returned | yes |
| TX complete | yes |
| underrun/HRESP | no |
| PC frames from `02:00:00:47:01:50` | **0** |

**Verdict:** `rgmii-id` PHY delay programming did not make a frame visible on the PC.  
**Action taken:** delay init **reverted** from `gem.rs` / `net_cmd.rs`. Stopped — no further GEM/PhaseNet changes in this step.
