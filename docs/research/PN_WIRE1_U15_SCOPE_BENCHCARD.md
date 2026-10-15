# PN-WIRE-1 — U15 RGMII Scope Bench Card (A / B / C)

**Purpose:** one-sitting physical classification of the GEM0→U15 RGMII TX path.
**Companion to:** [`PN_WIRE1_U15_SCOPE.md`](PN_WIRE1_U15_SCOPE.md) (gate rules) — this card adds exact pins,
levels, trigger, and expected waveforms so the measurement is fast.
**Authority:** RTL8211F(D) datasheet Table 3 (RGMII pinout); StarLite schematic sheet 15 `PZ-StarLite--GPHY(PS)`.
**Does NOT claim wire PASS.** Records a letter only.

---

## 0. Preconditions (2 min)

- Board powered; **cable on the PS RJ45 (U15)** — confirm `net link` shows `UP` before scoping
  (link-up on GEM0 MDIO addr 0 = correct jack). **Not the PL/U16 jack.**
- **Force the link to 100 Mbps** if you can (PC NIC → 100/full). Reason: TXC becomes **25 MHz (40 ns period)**
  instead of 125 MHz — trivial for any scope, and the fault has already been shown to persist at 100 Mbps,
  so nothing is lost. If you must run gigabit, you need ≥500 MHz BW / ≥1 GSa/s.
- COM12 serial attached (the agent can fire the stimulus — see §4).

## 1. Chip & pins (U15 RTL8211F-CG, QFN-48)

Probe on the **PHY side** (U15 pins). Pins **15–20 are consecutive on one edge** — easy to fixture.

| Ch | Signal | U15 pin | Net |
|----|--------|---------|-----|
| 1  | **TXC**   | **20** | `PS_GPHY_GTX_CLK` |
| 2  | **TXCTL** | **19** | `PS_GPHY_TX_EN` |
| 3  | **TXD0**  | **18** | `PS_GPHY_TXD0` |
| (4)| TXD1      | 17 | `PS_GPHY_TXD1` |
| (5)| TXD2      | 16 | `PS_GPHY_TXD2` |
| (6)| TXD3      | 15 | `PS_GPHY_TXD3` |

**Minimum to classify A/B/C: 3 channels — TXC (20), TXCTL (19), TXD0 (18).** Add TXD1–3 only if a
logic analyzer with RGMII decode is on hand.

**Ground:** shortest possible lead to a GND near U15 — center pad, or the ground side of C109/C110
(the U15 decoupling caps). Long ground leads ring badly at these edge rates.

## 2. Electrical setup

- **I/O level: 1.8 V** (U15 RGMII rail is `VDD_1V8` / `DVDDRG`). **Confirm amplitude on the first
  TXC capture** — expect ~0 → ~1.8 V. If it reads ~2.5 V, adjust threshold and note it.
- Scope: DC coupling, **~0.5 V/div**, **logic threshold 0.9 V**, ≥100 MHz BW (100 Mbps case).
- Timebase: **20 µs/div** to see several frames; **100 µs/div** to see a whole `txburst 32` envelope
  (~160 µs of activity at 100 Mbps).

## 3. Trigger

- **Trigger source: TXCTL (Ch2, pin 19), rising edge, Normal mode, ~0.9 V.**
  TX_EN asserts at each frame start, so this catches the burst cleanly.
- Fallback if TXCTL is dead (that itself is the finding): trigger on **TXC (Ch1)** to check the clock alone.

## 4. Stimulus (agent / COM12)

Continuous repeating bursts so the scope re-triggers while you hold the probe:

```text
gpos> net wire init
gpos> net wire txburst 32      # repeat in a loop
```

The agent can drive this on a loop over COM12 (script: `scratchpad/scope_stimulus_loop.ps1`, or
`kairos/scripts/pn_wire1_u15_scope_run.py --port COM12`). Each `txburst 32` = 32 broadcast `0x88B5`
frames, src `02:00:00:47:01:50`. At 100 Mbps each 64-byte frame ≈ 5 µs on the wire.

## 5. Expected waveforms & letter

| LETTER | What you see | Meaning | Next branch (only after letter recorded) |
|--------|--------------|---------|------------------------------------------|
| **A** | **TXC toggling** (25 MHz @100M / 125 MHz @1G, continuous) **AND** TXCTL + TXD[0] show ~5 µs bursts in lockstep with each `txburst` | Data **is** leaving the GEM onto RGMII. Fault is **downstream of the RGMII TX pins**: check **TXDLY strap (pin 24)**, PHY delay page `0x0D08` regs `0x11`/`0x15`, PHY analog egress, magnetics/jack, or the RX-return side. | PHY/media branch: strap + delay + analog |
| **B** | **TXC toggles**, but **TXCTL and TXD stay idle/static** during the burst | GEM emits the clock but is **not driving the TX data lines**. TX BD / STARTTX / data-path. | GEM TX data-path branch |
| **C** | **No TXC** at all (flat) during the burst | GEM RGMII **TX clock not output**. MIO16 / NWCTRL TX enable / clock routing. (Would be surprising — SLCR/MIO16/22/52/53 all read correct.) | GEM MIO / clock branch |

Notes for reading:
- **TXC is continuous**, not bursty — it runs whenever the MAC clock is up, independent of frames.
  Bursts appear on **TXCTL/TXD**, aligned to each `txburst 32`.
- RGMII is **DDR**: data changes on both TXC edges. For A/B/C you only need presence/absence, not eye quality.
- Confirming TXC = **25 MHz** (or 125 MHz) also proves the MAC clock/speed is correct — a free bonus check.

## 6. Record the letter (closes the gate)

```text
python kairos/scripts/pn_wire1_u15_scope_run.py --port COM12 --letter A   # or B / C
```

Do **not** infer a letter from UART `txburst: OK` — that is not physical evidence.

## 7. Fast reference — what each letter rules in/out

- **A** ⇒ the GEM↔PHY *digital TX* is fine; the PCS-loopback NO-MATCH + zero-PC-frames must come from
  PHY delay/strap, analog TX, magnetics, or RX return. This is the **most likely** outcome given
  MAC-internal loopback passes but everything past the RGMII pins fails.
- **B** ⇒ firmware TX data path (not clock). Re-examine BD submit / `NWCTRL_TXSTART` / DMA on the wire ring.
- **C** ⇒ clock/pinmux, despite register reads matching. Scope MIO16 at the SoC side too.
