# Zynq-7000 Golden Phase stack (canonical)

**Board package (this tree):** PZ7020-StarLite / PZ7020-SL-C  
**OS:** Golden Phase OS is the bare-metal operating system itself — it runs directly on the Cortex-A9 with nothing underneath it. Linux is never used and never a seal target.

## Boot layers (silicon → apps)

```text
Zynq-7000 silicon
  → 1. AMD BootROM
  → 2. Golden Phase stage-0 loader / temporary FSBL
  → 3. FPGA bitstream (G1P hardware kernel)
  → 4. Golden Phase OS kernel (Cortex-A9, no Linux)
  → 5. Golden Phase native hardware drivers
  → 6. Golden Phase native system services
  → 7. Golden Phase applications and interface
```

| Layer | Role |
|-------|------|
| **1. BootROM** | Fixed in silicon. Finds `BOOT.BIN` on SD and loads stage-0. Cannot replace. |
| **2. Stage-0** | Board init only: DDR, clocks, MIO, UART, SD, GIC, PL config. **Not** an OS. Early bring-up may use AMD `zynq_fsbl.elf`; later replace/minimize as `golden_phase_stage0.elf`. |
| **3. Bitstream** | PL: Golden Phase execution units, phase-state, AXI, mailboxes, watchdog, HW scheduler pieces, fault detect. |
| **4. GP-OS kernel** | This is Golden Phase OS: the bare-metal OS itself on the A9 (no Linux, no host runtime). Scheduler, memory/pages, IRQs, syscalls, GP VM, legality, watchdog, fault recovery, HW IPC. |
| **5. Drivers** | Native: UART, DDR/MMU, SD, timers, GIC, GEM, PHY, GPIO, I²C, SPI, USB, G1P. Direct MMIO — not Linux drivers. |
| **6. Services** | Later: FS, TCP/IP, DHCP/DNS, SSH, logging, update, shell, remote mgmt — all inside GP-OS. |
| **7. Apps** | Golden Phase applications and interface. |

## Correct `BOOT.BIN`

**Target:**

```text
BOOT.BIN
├── [bootloader] golden_phase_stage0.elf
├── system.bit
└── golden_phase_os.elf
```

**Temporary (current bring-up):**

```text
BOOT.BIN
├── [bootloader] zynq_fsbl.elf
├── system.bit
└── g1p_smoke.elf   → evolves into golden_phase_os.elf
```

FSBL does **not** make the system Linux. We do **not** ship or seal:

- `image.ub`, Linux kernel, rootfs, Linux drivers/services, U-Boot

Seller `BOOT.BIN` + `image.ub` only proved board + UART (COM12). That is not a Golden Phase seal.

## Portability

| Layer | Scope |
|-------|--------|
| **Common GP-OS** | Kernel, scheduler, memory/process model, VM, phase algebra, syscalls, FS, net, SSH, G1P register protocol, most driver logic, apps |
| **Zynq-7000 arch** | Cortex-A9 startup, GIC, UART/SD/GEM/timers, cache/MMU, AXI |
| **Board package** | DDR size/timings, clocks, MIO, UART/SD/Eth PHY+reset, GPIO, package/XDC, memory map, bitstream top |

```text
Golden Phase OS common core
  + Zynq-7000 architecture layer
  + PZ7020-StarLite board package
```

Future custom board = same core + Zynq arch + **new board package** (not a whole OS rewrite).

## Development order (this stack)

1. Custom SD boot + UART message (`g1p_smoke` / early GP-OS)
2. DDR test  
3. Timer + interrupt test  
4. SD-card driver  
5. G1P FPGA communication  
6. Golden Phase scheduler + memory system  
7. Native filesystem  
8. Ethernet MAC + PHY  
9. Native TCP/IP  
10. Native SSH  
11. Full GP applications and interface  

`g1p_smoke` is **only** first hardware bring-up. It leads into the native GP-OS port, not Linux.

## Current engineering status (host)

- **V1 smoke PASS** (preserved): `release/PZ7020_G1P_SD_SMOKE_V1/`
- **V2 bring-up PASS** (preserved): `release/PZ7020_G1P_BRINGUP_V2/` — G1P, vectors, illegal, timer, GIC, PL IRQ
- G1P AXI base remains **`0x40000000`**; fabric IRQ SPI **29** → GIC ID **61** (from SDT `pl.dtsi`)
- **V2A board DDR memtest** — **STOPPED** (bring-up diversion). Goal is Golden Phase **memory system** / kernel MM, not re-sealing StarLite DRAM hardware.
- **GP-MM-V1 PASS** (COM12): `release/PZ7020_GP_MEMORY_V1/` + `evidence/gp_memory_v1/` — V2 FSBL + V2 bit + `kairos_zynq_mm.elf` (alloc/map/escalate/lock/illegal/compose). Source: `kairos/zynq_a9_mm/`.
- **GP-OS-V3 PASS** (COM12): `release/PZ7020_GP_OS_V3/` + `evidence/gp_os_v3/` — V2 FSBL + V2 bit + `kairos_zynq_os.elf` (memory + scheduler + timer/GIC + G1P). Source: `kairos/zynq_a9_os/`.
- **GP-OS-V4 PASS** (COM12): `release/PZ7020_GP_OS_V4/` + `evidence/gp_os_v4/` — SD0 PIO + PhaseFS-on-disk at LBA 8192. Source: `kairos/zynq_a9_os/`.
- **GP-OS-V5 PASS** (COM12): `release/PZ7020_GP_OS_V5/` + `evidence/gp_os_v5/` — GEM0 MDIO + MAC local loopback GP ethertype frame. Source: `kairos/zynq_a9_os/`.
- **GP-OS-V6 PASS** (COM12): `release/PZ7020_GP_OS_V6/` + `evidence/gp_os_v6/` — PhaseNet-lite StateUpdate + Ping/Pong over GEM (`PN01`, ethertype `0x88B5`). MAC re-init between Ping and Pong required on 1-BD ring.
- **GP-OS-V7 PACKAGED** (physical seal pending): `release/PZ7020_GP_OS_V7/` — Cortex-A9 short-descriptor MMU + phase-tag L2 paging (G3). DDR Normal Non-cacheable for GEM DMA safety. Source: `kairos/zynq_a9_os/`.
- Next: COM12 physical seal for V7; then wire-link peer / full SHA PhasePacket as needed.
