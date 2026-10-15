# PZ7020 Hardware Hooks (G64 research)

Edition notes cite AMD DocNav / installed UG PDFs. Local design numbers from Bring-up V2 release reports (copied under `fpga/zynq/research/g64_phase_fabric_sidecar/baseline/`).

## Device

- Part: **xc7z020clg400-2** (Zynq-7000 XC7Z020, CLG400, speed -2)
- Board: PZ7020-SL-C
- Authority: utilization/timing reports + Vivado project — not the silkscreen name alone

## Resources (device vs current design)

| Resource | Device available | Used (G1P system_wrapper routed) | Free (approx) |
|----------|------------------|-----------------------------------|---------------|
| LUT | 53200 | 506 | ~52694 |
| FF | 106400 | 591 | ~105809 |
| BRAM tiles | 140 | 0 | 140 |
| DSP48E1 | 220 | 0 | 220 |
| BUFGCTRL | 32 | 1 | 31 |

Conservative sidecar guideline: ≤25% of **free** → ~13k LUT / ~26k FF / 35 BRAM / 55 DSP.

## Clocks

- Proven PL: `FCLK_CLK0` **100 MHz** (UG585 PS clocking; BD enables FCLK0)
- Research sweep may also try slower/faster periods **only** as OOC experiments; do not invent unsupported PS clocks without PCW reconfiguration evidence

## AXI / interconnect hooks (UG585)

| Interface | Baseline G1P design | G64 first experiment |
|-----------|---------------------|----------------------|
| M_AXI_GP0 | Yes → G1P @ 0x40000000 | Add sidecar @ proposed 0x40001000 |
| M_AXI_GP1 | Unused | Optional later |
| S_AXI_HP0–3 | Unused | After local proof only |
| S_AXI_ACP | Unused | Research after HP; not assumed better |
| IRQ_F2P | [0] = G1P (GIC 61) | Separate line after polled proof |

## DDR / coherency

- PS master to DDR is the existing OS path (ELF @ 0x00100000)
- PL DDR master **not** in first sidecar
- Cache coherency via ACP only if measured necessary (UG585 ACP chapter)

## Debug

- ILA/VIO available on 7-series (UG908) — optional on research BD
- Do not enable partial reconfiguration in first experiment

## Fixed-PS limitations

- Cannot transparently intercept Cortex-A9 instruction stream from PL
- Cannot replace UART0/DDR controller without PS reconfiguration risk
- G1P register map and IRQ assignment are frozen for this research wave

## References (cite exact local editions when building reports)

- DS190 Zynq-7000 Overview
- UG585 Zynq-7000 TRM
- UG821 Zynq Software Developers Guide
- UG474 7 Series CLB
- UG473 7 Series Memory Resources
- UG479 7 Series DSP48E1
- UG901 / UG904 / UG906 / UG835 / UG908 Vivado
- Board schematic/manual in FullOS hardware pack
- Local XSA: `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/PZ7020_G1P_BRINGUP_V2.xsa`
