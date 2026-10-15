# PN-WIRE-1 — GEM Driver Cleanup (Gate A/B/C)

**Branch:** `feature/native-ethernet`  
**Starting commit:** `451c8d1e72b6d479174bff3899d6a2650b31f7cd`  
**Status:** software defects corrected; **Ethernet still NOT PASS**; external TX / U15 A·B·C unresolved  
**Authority:** [`GEM_VS_REFERENCE_INIT_COMPARE.md`](GEM_VS_REFERENCE_INIT_COMPARE.md)

---

## What changed (Gate A)

### 1) NWCFG constants (xemacps_hw.h)

| Symbol | Old (wrong) | New (Xilinx) |
|--------|-------------|--------------|
| COPYALLEN | mislabeled as `FCS_REMOVE = 1<<4` | `0x00000010` |
| PAUSEEN | mislabeled as `COPY_ALL = 1<<13` | `0x00002000` (not set by wire init) |
| FCSREM | never set | `0x00020000` |

Wire NWCFG builder now sets **FCSREM | COPYALLEN** (+ speed/duplex/MDC).  
Measured pre-fix `0x000C2013` had COPYALL+PAUSEEN and lacked FCSREM.

**Does not claim this restores PC TX visibility.**

### 2) DMACR construction

| | Old | New |
|---|-----|-----|
| Literal | `0x00040410` | `0x00040700` via named fields |
| RXBUF | 4 units (256 B) | 4 units (256 B) — matches `WIRE_BUF_LEN` |
| RXSIZE | **clear** | **set** (`0x300`) |
| TXSIZE | set | set |
| INCR16 | set | **clear** (Zynq-7000 GEM ver≤2 path) |

Xilinx reset default ~`0x00180700` assumes **1536-byte** RX buffers (`24×64`).  
Our static rings are **256-byte** objects — we encode RXBUF for 256 and do **not** claim 1536.

Helpers: [`src/board/gem_cadence_hw.rs`](../../src/board/gem_cadence_hw.rs).

### 3) RX descriptor ownership

| Field | Layout (xemacps / UG585) | Old code | New code |
|-------|--------------------------|----------|----------|
| Ownership | **addr word bit0** (NEW/USED) | status bit31 | addr bit0 |
| WRAP | addr bit1 | addr bit1 | unchanged meaning |
| Buffer ptr | addr `[31:2]` | full word loosely | `ADDR_MASK` |
| SOF/EOF/len | status bits | len only (bit31 used as OWN) | SOF/EOF/len + reject 0 / oversized / missing EOF |

TX descriptor semantics unchanged.

### 4) PHY soft-reset `BMCR=0xB100` — documented, **not** in live path

Seller PL `mdio_ctrl.v` writes `0xB100` before link/delay.

| Bit | Meaning in `0xB100` |
|-----|---------------------|
| 15 | Software reset |
| 14 | Loopback = 0 |
| 13 | Speed100 = 1 |
| 12 | ANE = 0 (disabled) |
| 8 | Full duplex = 1 |
| 6 | Speed1000 = 0 |

So: **soft-reset + forced 100/full, AN off** — not the same as current negotiated 100/full with AN.

**Experiment policy (separate, reversible, later):**
1. Snapshot BMCR/BMSR/PHYSR
2. Write `0xB100`; poll until bit15 clears
3. Wait datasheet settle before further MDIO
4. Restore or re-AN; compare link
5. Do **not** combine with assumed `rgmii-id` delay re-add (prior delay trial FAIL/REVERT)

---

## Gate B — PHY loopback (after flash)

| Field | Result |
|-------|--------|
| BOOT | `BOOT_PZ7020_PN_WIRE1/BOOT.BIN` SHA256 `1943C8A367821241DF39F7BCC6B643CCF18305BF4499C8ADAD54D7D95BFACCB8` (copied to `H:\BOOT.BIN`) |
| Command | `net wire init` / `net wire phyloop` / `net wire status` |
| RX OWN before/after | **pending** — COM12 no `gpos>` until power-cycle after SD flash |
| RXSR / ISR / last_len | pending |
| Verdict | pending board run |

Honest rules:
- OWN flips + payload match → RX software defect was real
- OWN never flips → GEM↔PHY digital path still open
- Even if phyloop PASSes, external TX still unproven until PC sees the frame

## Gate C — External TX

| Field | Result |
|-------|--------|
| PC frames from `02:00:00:47:01:50` | pending |
| EtherType `0x88B5` | pending |

## Gate D — U15 scope

| Code | Status |
|------|--------|
| A / B / C | **unrecorded** — do not invent from UART |

Procedure: [`PN_WIRE1_U15_SCOPE.md`](PN_WIRE1_U15_SCOPE.md) — `python scripts/pn_wire1_u15_scope_run.py --port COM12` then `--letter A|B|C`.

---

## Preserved failed experiments

- RGMII delay-only (`rgmii-id` page `0xd08`): regs changed, PC=0, **reverted** — see [`PN_WIRE1_RGMII_DELAY_COMPARE.md`](PN_WIRE1_RGMII_DELAY_COMPARE.md)
- Prior phyloop NO MATCH with wrong RX OWN poll — see [`PN_WIRE1_PHY_LOOPBACK.md`](PN_WIRE1_PHY_LOOPBACK.md)

## Commits (local only; not pushed)

| SHA | Message |
|-----|---------|
| `8f146f0555a03335d729f1504dc3e25e2561098d` | `eth(zynq): correct GEM NWCFG and DMA register definitions` |
| `9c2f7c6ad2651868f91655fa1a7822ecf688f384` | `eth(zynq): fix RX descriptor ownership and parsing` |
