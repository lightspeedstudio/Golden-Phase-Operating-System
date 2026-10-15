# KairOS Φ-CORE — board family

**Native phase-computing development hardware.** Designed to run the **Golden Phase OS** and the
**KairOS phase kernel** at the silicon level. Two boards implement the same Φ-state (Kairos
States Φ₀–Φ₃) architecture: **Mikros** for constrained embedded use, **Makros** for the full
Golden Phase OS stack.

Status: **REV 0.1 — Design Phase** (vision / architecture capture, not built). Current dev work
runs on the PZ7020 (single Zynq-7000); this family is the product path beyond it.

---

## Mikros — REV 0.1

**Embedded Phase Computing Board.**

A compact embedded development board designed natively for phase computing. Implements the KairOS
Φ-state architecture in hardware, enabling four-state computation at the silicon level for
embedded and IoT applications.

**Specifications**

| | |
|---|---|
| Architecture | Phase-native ISA |
| States | Φ₀–Φ₃ (four-state) |
| Form Factor | Compact embedded |
| OS | KairOS / Golden Phase OS |
| Interface | GPIO, I²C, SPI, UART |
| Target | IoT, embedded, research |

**Capabilities**
- Native four-state phase computation in silicon
- KairOS phase kernel support
- Phase-coherent I/O interfaces
- Low-power phase-state management
- Research and development toolchain

**Architecture (single-chip):** the whole OS + phase fabric on one FPGA (PS + PL), plus DDR,
USB-C as the main port, Ethernet, and UART/JTAG for bring-up. Zero-class / minimal form factor —
full OS, no display.

---

## Makros — REV 0.1

**Full-Platform Phase Computing Board.**

A full-platform development board for phase computing research and application development.
Designed to run the complete Golden Phase OS stack with full peripheral support, high-speed
interfaces, and research-grade instrumentation.

**Specifications**

| | |
|---|---|
| Architecture | Phase-native full ISA |
| States | Φ₀–Φ₃ (four-state) |
| Form Factor | Full platform |
| OS | Golden Phase OS (full) |
| Interface | PCIe, USB, HDMI, Ethernet |
| Target | Desktop, research, server |

**Capabilities**
- Full Golden Phase OS support
- High-speed phase-coherent interfaces
- Research-grade instrumentation outputs
- Phase-native memory architecture
- Full development toolchain and SDK

**Architecture (multi-chip, video always offloaded):** same core as Mikros (FPGA + DDR +
Ethernet + USB) plus a **dedicated video FPGA + its own DDR** driving HDMI, scaling up to the
flagship configuration of **five FPGAs — one per Kairos State (Φ₀–Φ₃ = 4) + one dedicated
video** (see rationale below).

---

## Why it scales this way (the design principle)

The golden-phase compose algebra maps **naturally onto FPGA fabric** — the math *is* the silicon
layout. So the family scales by **giving the phase math more fabric**, and it keeps that fabric
**pure: graphics/video never share a chip with the algebra.** A GPU or shared video pipeline
would steal fabric and distort how the math fits — so **video always gets its own dedicated FPGA
+ its own DDR**, at every tier that has a display. The core system is never slowed by rendering.

Makros build-out, by fabric given to the math:

| Config | FPGAs | Video | Display | Note |
|---|---|---|---|---|
| Mikros | 1 | — | no | whole OS on one chip |
| Makros (base) | 2 | dedicated | HDMI | core + isolated video |
| Makros (flagship) | 5 | dedicated | HDMI | **1 FPGA per Kairos State** + video — "the math is the machine" |

## How this relates to today

- The **software** is proven on the PZ7020 (single Zynq-7000): boot, kernel, networking + SSH,
  USB U0, phase fabric (G1P/G64). That single-chip target ≈ the **Mikros** software.
- The board family is the **hardware productization** of that: same OS, more fabric per tier.
- Each board can become its own **research avenue** (see `docs/reports/AVENUE_*` and
  `docs/KAIROS_BOARD_V1_REQUIREMENTS.md`) — spec → block diagram → bring-up, proven one thing at a
  time on real silicon, the same read-vendor-part → bring-up → prove → document method as the OS.

## Open questions (to pin down before a BOM)

- FPGA part per board (Zynq-class for Mikros? larger for the Makros flagship? which video FPGA?).
- Mikros USB-C role (host, device, or dual-role) and the final embedded I/O set.
- Inter-chip fabric on the Makros flagship (how the 4 Kairos-State FPGAs + video interconnect).
- Video FPGA ↔ core interface (how the core hands frames to the dedicated video chip).
- PCIe scope on Makros (host/endpoint, lanes).
