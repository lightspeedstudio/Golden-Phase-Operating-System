# PZ7020 feature-completion seal (2026-07-30)

Branch: `feature/pz7020-completion`

## PHYSICALLY VERIFIED (COM12)

- Feature suite: `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_151124/`
  - G1P polled + selftest + IRQ 61; VM hello; PhaseFS RAM; phi-stability
- GEM ladder: `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_155025/`
  - GEM0 registers, MDIO, RTL8211F PHY ID, MAC local loopback PASS
  - Link DOWN = PARTIAL (no partner); external TX/RX = UNSUPPORTED

## Deferred (not this seal)

- Two-board PhaseNet peer over GEM0 cable
- PL second Ethernet / normal IP stack
- Hosted-only PhaseNet/SSH/AI

Research image: `BOOT_PZ7020_FEATURE_TEST.BIN` (GEM-era packaging under `BOOT_PZ7020_FEATURE_TEST/`).
