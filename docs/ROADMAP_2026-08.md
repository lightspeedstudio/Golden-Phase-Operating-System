# Golden Phase OS — Updated Roadmap (Alpha → Beyond)

**Date:** 2026-08-11 · **Supersedes:** `docs/ROADMAP.md` (2026-07-10, PDF-anchored)
**Product:** Golden Phase Operating System · **Kernel/HW line:** KairOS Φ-CORE
**Anchor:** `y⁴ − φy² + α = 0` → Kairos States Φ0–Φ3
**Status of this doc:** DRAFT for review — the concrete "next" tracks are proposals to refine together.

> **DRAFT.** This is a starting point written from the current, proven state. Refine the priorities
> after the hands-on feature test pass.

---

## The big shift since the last roadmap

The old roadmap said *"the algebra is real, the hardware is next."* **The hardware is now proven.**
Golden Phase OS boots bare-metal on the **PZ7020-StarLite (Zynq-7000, Cortex-A9)** to an interactive
`gpos>` shell — not a custom phase-plane board yet, but a genuine on-silicon OS. Crucially, the
**FPGA compose/reflect gate runs in the programmable logic** (G1P at `0x40000000`, software == hardware),
which is the PDF's Phase-4 "compose gate on silicon" goal, reached on a Zynq dev board.

## Where we are — proven on the PZ7020 (honest)

| Capability | Proof |
|---|---|
| Bare-metal boot → `gpos>` shell (no_std Rust) | live UART console |
| **FPGA phase engines** G1P / G64 (compose, reflect) | `g1p selftest` sw==hw MATCH |
| **Persistent filesystem** (SD / FAT32) | survives cold boot |
| **USB HID host** — keyboard enumerates + **drives the shell** | `usb kbd` + typed commands |
| **Bare-metal networking** (GEM0 + smoltcp) | ping / UDP / TCP, DHCP/DNS |
| **SSH** operator channel (no_std sunset, ed25519) | `ssh host "<cmd>"` |
| **Crypto / ANOM** (AES-GCM, ed25519, object seal) | `crypto selftest` PASS |
| **Statechain + watchdog** (Merkle ledger, audit) | `StatechainWatchdog::audit` |
| **Baremetal AI** control layer (AI-over-smoltcp) | `ai remote` (protocol proven; live needs a LAN server) |
| **Built-in `system selftest`** | **18/18 ALL PASS** on hardware |
| **Phase-state fault tolerance** (research) | phase-syndrome + TMR, `fault run` on the board |

**Milestone reached: "Basic Utility OS / Alpha"** — files persist, a USB keyboard drives the shell,
and one command self-verifies the whole machine.

---

## The forward roadmap (five tracks)

### Track A — Finish the OS surface (near-term, mostly software)
- **USB mouse** (≈ free after the keyboard HID path) and later **USB mass storage**.
- **Live `ai remote`** end-to-end against a real LAN inference server (mirror the SSH on-demand proof).
- **Layer-B all-commands smoke** — drive every command over UART via the harness, PASS/FAIL matrix.
- **Mission/Craft layer on silicon** — FDIR over the glider carrier + live UART packet transport
  (hosted-sim today).
- **Shell/UX polish** — history, line editing, `text edit` to SD, scripts persisted.

### Track B — Fault-tolerance research (continue the branch)
- **Wire the LIVE three estimates** (primary / PhaseGuard / checkpoint) into the kernel transition
  path continuously — today `fault` uses synthetic injection; make it a standing supervisor.
- **Real fault injection** into the running scheduler + measure detection/recovery rates.
- **Tune thresholds** (`T1/T2/T3`, `AGREE_TOL`, `Q_CAP`) against measured clean-workload distributions.
- A **fault-tolerance paper** — the phase-syndrome detector as the algebra's built-in redundancy (Thm 1).

### Track C — KairOS Φ-CORE custom hardware (the phase boards)
The insight: the phase algebra overlays an FPGA fabric cleanly; a GPU would fight the math, so **video
gets its own dedicated chip**. See `docs/BOARD_FAMILY.md`.
- **Mikros** (REV 0.1) — embedded phase board: 1 compute FPGA + **1 dedicated video FPGA (HDMI out)**,
  DDR, USB-C in. Interfaces: GPIO / I²C / SPI / UART.
- **Makros** (REV 0.1) — full platform: **5 FPGA (one per Kairos state Φ0–Φ3 + one dedicated video)**,
  PCIe / USB / HDMI / Ethernet.
- **Phase-plane research board** — the PDF's analog snap (103 mV) validation + 10-layer phase-plane
  stackup (Phase 5 GO/NO-GO gate before the 10-layer fab).

### Track D — Video / display
- **HDMI/video out** on the dedicated video path (the reason for the second FPGA on both boards).
- A phase-native framebuffer / terminal on the display.

### Track E — Papers, IP, honesty items
- **Provisional patent** (`docs/patent/`).
- **OSF papers** P2–P6 + whitepaper; fold in Avenues 1–9 (land the §3 octonion-cocycle retraction).
- **Real checkpoint signing** — Avenue 6 found the checkpoint key is derivable from public constants
  (integrity-only); add a secret/asymmetric signature for adversarial threat models.
- **PhaseGuard squeeze law** — still an **open hypothesis**: needs live physical squeeze samples to
  move from "observer proven" to "law sealed." Do not claim the law until measured.

---

## Suggested near-term order (proposal)
1. Hands-on feature test pass (you) → capture any bugs the self-test doesn't cover.
2. Track A quick wins: USB mouse, Layer-B smoke, live `ai remote`.
3. Track B: stand up the live fault-tolerance supervisor + tune on hardware.
4. Track C kickoff: freeze **Mikros REV 0.1** BOM + block diagram (2 FPGA + DDR + USB-C + HDMI).

## Standing laws (unchanged)
- One `compose` authority (`gphase_core`); no layer reimplements it.
- Read the vendor/spec, write no_std Rust, **prove on silicon**, commit with evidence.
- Never fake a pass or weaken a threshold to claim one (the honesty rule).
- Everything local; `copyright_*` untracked.
