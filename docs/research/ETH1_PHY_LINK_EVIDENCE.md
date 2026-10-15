# ETH-1 — PHY / MDIO Cable-Link Evidence

**Branch:** `feature/native-ethernet`  
**Software commit:** `d6e527e`  
**Gate:** ETH-1 (PHY ID + cable link + speed/duplex)  
**Scope:** no external TX/RX  
**BOOT:** `5847C0A969B4AD575576BE8622E167F33C92F20416113866F6310693F19DFE84`

## Verdict

| Layer | Status |
|-------|--------|
| ETH-1 software (`d6e527e`) | **READY** |
| `BOOT.BIN` (Xilinx link + correct ELF) | **DONE** |
| Flash + COM12 `gpos>` | **PASS** |
| ETH-1 physical | **PASS** (2026-08-01) |

> ETH-1 PASS: RTL8211F on GEM0 MDIO addr 0, cable DOWN/UP observed with speed/duplex decode. No external TX/RX / PhaseNet wire yet.

Do **not** use `BOOT_PZ7020_FEATURE_TEST`, `BOOT_BAD_portable_gcc.BIN`, or `BOOT_WRONG_ELF_no_eth1_strings.BIN` as ETH-1 evidence.

---

## Physical checks

| Check | Result | Evidence |
|-------|--------|----------|
| PHY ID `0x001C` / `0xC916` addr 0 | **PASS** | `eth1_boot_test_20260801_152042.txt` |
| Cable disconnected → link DOWN | **PASS** | `gem link: phy=0 DOWN bmsr=0x7989 speed=n/a duplex=n/a` — `eth1_down_poll_20260801_152151.txt` |
| Cable connected → link UP | **PASS** | `gem link: phy=0 UP bmsr=0x79AD speed=100 duplex=full` — `eth1_up_poll_20260801_152209.txt` |
| Negotiated speed | **PASS** | `speed=100` |
| Negotiated duplex | **PASS** | `duplex=full` |
| MAC loopback (local only) | **PASS** | ethertype `0x88B5` in `net status` |

---

## Artifacts

Directory: [`fpga/zynq/evidence/kairos_zynq_boot/BOOT_PZ7020_ETH1_d6e527e/`](../../fpga/zynq/evidence/kairos_zynq_boot/BOOT_PZ7020_ETH1_d6e527e/)

| File | SHA-256 / note |
|------|----------------|
| `kairos_zynq.elf` | `70FCBB862F23EF280EDE1C11CDE8AA40C1E7C11D03340CEBEEC8F8333393E7CE` |
| `BOOT.BIN` | `5847C0A969B4AD575576BE8622E167F33C92F20416113866F6310693F19DFE84` (4 663 768 bytes) |
| `boot_eth1.bif` | FSBL + `system.bit` + ETH-1 ELF |

Built with Xilinx Vitis 2026.1 `arm-xilinx-eabi-gcc` 13.4.0 + `bootgen -arch zynq -w`.

---

## Explicitly not started

- PN-WIRE-1 / external GEM TX/RX
- PhaseNet over wire
- ARP / IP stack
- Push / merge to main
