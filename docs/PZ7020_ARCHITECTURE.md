# PZ7020 Architecture

## One operating system

There is **one** Golden Phase OS. The Zynq/PZ7020 code is a **platform-support
layer** that starts that OS on this board and gives it a console. It is not a
second operating system, not a fork of the kernel, and contains no kernel
policy.

## Full boot / initialization stack

```
BootROM
  → FSBL (zynq_fsbl.elf)
      → PS7 initialization  (MIO / PLL / clocks / DDR / peripherals)
      → system.bit          (PL configured; G1P live @ 0x40000000)
      → kairos_zynq ELF loaded into DDR @ 0x00100000
  → Cortex-A9 startup       (arch/armv7/startup.S: stacks, VBAR, BSS clear)
  → PZ7020 platform init    (board/zynq7000: UART0, heap region, timer, G1P window)
  → Runtime::boot()         ── existing Golden Phase OS ──
      → existing kernel
      → existing scheduler / memory / StateClock
      → existing syscall boundary / PhaseFS / GP VM / trace ring
      → existing userland
  → real UART shell (gpos>)
```

## Layer separation

### Golden Phase OS core — `kairos/src/`

Architecture-neutral. Unchanged by this port.

| Module | Responsibility |
|--------|----------------|
| `core/` | Kairos-State algebra (`gphase_core`), recursive shell math |
| `stateclock/` | chronos + kairos clocks, geometry, atlas |
| `kernel/` | scheduler, memory (phase-tagged pages), syscall, trace, phase math, PhaseNet/PhaseLink |
| `fs/` | PhaseFS |
| `vm/` | GP VM — **RISC-V-style guest ISA interpreter** (not the host CPU) |
| `runtime/` | `Runtime::boot()` — the single OS boot entry |
| `userland/` | `gpos>` shell, parser, command registry, splash |
| `boot/` | existing RISC-V `virt` QEMU bare-metal path (parallel, unchanged) |

### PZ7020 platform support

Board bring-up and console only.

| Component | File(s) | Responsibility |
|-----------|---------|----------------|
| CPU startup | `src/arch/armv7/startup.S` | reset vector, mode stacks, VBAR, BSS clear, crash letters |
| Link/memory layout | `src/arch/armv7/linker.ld` | DDR `0x00100000`, stacks, heap symbols |
| MMU expectation | (flat, FSBL-provided) | kernel path does not enable MMU/cache |
| UART0 | `src/board/zynq7000/uart.rs` | polled PS UART0 TX/RX console |
| GIC | (not enabled in kernel path) | IRQs masked; polled console + polled timer |
| Timer | `src/board/zynq7000/kernel_main.rs` | polled SCU global-timer tick source |
| SD | `src/board/zynq7000/sd.rs` | SD0 polled PIO read (stage-0 loader) |
| GEM | (not in this build) | Ethernet — unsupported on this bare-metal build |
| G1P | `src/board/zynq7000/memory_map.rs`, `kernel_main.rs` | AXI-Lite window `0x40000000`, ID/VERSION read + report |
| Boot packaging | `boot.bif` + `bootgen` | FSBL + system.bit + kernel → BOOT.BIN |
| PS7 init | `src/board/zynq7000/ps7_init.rs` + `ps7_init_data.rs` | **stage-0 only**; not run when the FSBL boots |

## The GP VM guest ISA (disambiguation)

The kernel's VM is the **GP VM RISC-V ISA interpreter** — a *guest* instruction
set executed in software by the OS. The physical processor is an **ARM
Cortex-A9**. The boot line `vm: RISC-V ISA loader ready` and the corrected
splash label `baremetal-armv7-a-cortex-a9` describe two different things: the
guest ISA and the host CPU respectively. Nothing in this port implies the
silicon is RISC-V.

## Two boot entries, one OS

| Path | Entry | Target | Status |
|------|-------|--------|--------|
| RISC-V QEMU | `boot::kernel_main` (`bin/kairos_boot.rs`) | `riscv64gc-unknown-none-elf` | pre-existing, unchanged |
| PZ7020 ARM | `board::zynq7000::kernel_main` (`bin/kairos_zynq.rs`) | `armv7a-none-eabi` | this port, physically verified |

Both entries do the same three things — init the arch, call `Runtime::boot()`,
run a serial shell loop — against the same kernel and userland.

## Future FSBL-replacement path (not yet on hardware)

```
BootROM → zynq_stage1 (OCM): ps7_init (from ps7_init_data.rs) → PCAP system.bit
        → SD/FAT32 read KAIROS.BIN → DDR 0x00100000 → jump → Golden Phase OS
```

`zynq_stage1` builds for `armv7a-none-eabi` and reuses the same board modules,
but has **not** been run on hardware; PCAP bitstream loading is not yet
implemented. Until it lands, the proven FSBL chain above is the release path.
