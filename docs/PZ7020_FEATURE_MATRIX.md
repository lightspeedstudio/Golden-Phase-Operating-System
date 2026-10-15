# PZ7020 Feature Matrix

## State legend

| State | Meaning |
|-------|---------|
| **PHYSICALLY VERIFIED** | Proven on the board in the 2026-07-30 COM12 capture |
| **SOFTWARE VERIFIED** | Builds/runs correctly in software (compile/host), not proven on this board |
| **PRESENT BUT UNTESTED** | Code exists and initializes, but the function was not exercised on HW |
| **PARTIAL** | Some aspect proven on HW; the rest not isolated/exercised |
| **UNSUPPORTED** | Not implemented in this bare-metal build path |
| **HOSTED ONLY** | Requires the hosted `std` build; stubbed on bare metal by design |
| **STUB — MUST NOT CLAIM PASS** | Placeholder that reports itself as stub; never to be reported as passing |

Evidence path for all "PHYSICALLY VERIFIED"/"PARTIAL" rows:
`fpga/zynq/evidence/kairos_zynq_boot/com12_capture_20260730_132633.txt`.

## Boot / platform

| Feature | Owning module | Platform dependency | State | Test procedure | Evidence | Remaining work |
|---------|---------------|---------------------|-------|----------------|----------|----------------|
| microSD boot (BootROM) | Zynq BootROM + `boot.bif` | SD, FAT32 | PHYSICALLY VERIFIED | Flash BOOT.BIN, RESET | capture (boot output) | — |
| FSBL PS7 init | sealed `zynq_fsbl.elf` | Zynq PS | PHYSICALLY VERIFIED | boot | capture (DDR/console live) | — |
| system.bit PL load | FSBL + `system.bit` | PL | PHYSICALLY VERIFIED | boot; read G1P | G1P line | — |
| Our `ps7_init.rs` / `ps7_init_data.rs` | `board/zynq7000/ps7_init*` | PS (stage-0) | PRESENT BUT UNTESTED | run via `zynq_stage1` | — | prove on HW (FSBL did PS7 in proof) |
| ps7 data generator | `scripts/gen_ps7_init.py` | none (host) | SOFTWARE VERIFIED | `python scripts/gen_ps7_init.py` | generated file + SHA | — |
| Cortex-A9 startup | `arch/armv7/startup.S` | Cortex-A9 | PHYSICALLY VERIFIED | boot reaches `rust_main` | capture (splash) | — |
| DDR load @ 0x00100000 | `arch/armv7/linker.ld` | DDR | PHYSICALLY VERIFIED | boot | capture | — |
| Heap (`alloc`) | `boot/heap.rs` | DDR | PHYSICALLY VERIFIED | boot (`Runtime` uses alloc) | capture (shell works) | — |
| `zynq_stage1` OCM loader | `zynq_stage1/` | OCM, SD, FAT | SOFTWARE VERIFIED | build; run as bootloader | build only | run + capture on HW |
| PCAP bitstream load | (not implemented) | devcfg/PCAP | UNSUPPORTED | — | — | implement + prove |

## Console

| Feature | Owning module | Platform dependency | State | Test procedure | Evidence | Remaining work |
|---------|---------------|---------------------|-------|----------------|----------|----------------|
| UART0 TX | `board/zynq7000/uart.rs` | PS UART0 | PHYSICALLY VERIFIED | any boot output | capture | — |
| UART0 RX | `board/zynq7000/uart.rs` | PS UART0 | PHYSICALLY VERIFIED | type `help`/`state` | capture | — |
| Interactive `gpos>` shell | `userland/shell.rs` + `kernel_main.rs` | UART0 | PHYSICALLY VERIFIED | type commands | capture | — |
| `help` command | `userland/commands/help.rs` | — | PHYSICALLY VERIFIED | type `help` | capture | — |
| `state` command | `userland/commands/state_cmd.rs` | — | PHYSICALLY VERIFIED | type `state` | capture | — |
| Crash-letter fault report | `arch/armv7/startup.S` | UART0 | PRESENT BUT UNTESTED | trigger a fault | — | fault-inject test |

## Kernel / OS core

| Feature | Owning module | Platform dependency | State | Test procedure | Evidence | Remaining work |
|---------|---------------|---------------------|-------|----------------|----------|----------------|
| `Runtime::boot()` | `runtime/host.rs` | — | PHYSICALLY VERIFIED | boot | capture (awakening list) | — |
| Scheduler + init pid 0 | `kernel/scheduler.rs`, `userland/init.rs` | — | PHYSICALLY VERIFIED | `state` | capture (pid 0, 1 proc) | — |
| Memory (tagged pages) | `kernel/memory.rs` | DDR | PHYSICALLY VERIFIED | `state` | capture (4 pages) | — |
| StateClock (chronos+kairos) | `stateclock/` | — | PHYSICALLY VERIFIED | `state` | capture (tick 5/11) | — |
| Syscall boundary | `kernel/syscall.rs` | — | PHYSICALLY VERIFIED | any command | capture (dispatch works) | — |
| Polled timer tick | `board/zynq7000/kernel_main.rs` | SCU global timer | PARTIAL | observe clock advance | capture (tick advanced) | isolate tick vs cmd-driven; add GIC timer IRQ |
| PhaseFS | `fs/` | — | PARTIAL | `fs`/file ops | capture (mount line only) | exercise file I/O on HW |
| GP VM (RISC-V guest ISA) | `vm/` | — | PRESENT BUT UNTESTED | `run` a program, `regs` | boot "ready" only | run guest program on HW |
| Trace ring | `kernel/trace.rs` | — | PARTIAL | `trace` | boot "4 records" only | run `trace` on HW |
| Phase Math (scalar) | `kernel/phase_math/` | — | PARTIAL | `phaseperf` | boot "scalar equivalence: OK" | run `phaseperf`/`verify` on HW |
| PhaseLink codec | `kernel/phaselink/` | — | PRESENT BUT UNTESTED | `phaselink loopback` | boot "ready" only | run loopback on HW |
| PhaseNet | `kernel/phasenet/` | (loopback) | PRESENT BUT UNTESTED | `phasenet` | boot "loopback/UART" | run on HW |

## PL / peripherals

| Feature | Owning module | Platform dependency | State | Test procedure | Evidence | Remaining work |
|---------|---------------|---------------------|-------|----------------|----------|----------------|
| G1P ID/VERSION read | `board/zynq7000/kernel_main.rs` | PL @ 0x40000000 | PHYSICALLY VERIFIED | boot G1P line | capture | — |
| G1P EXEC/SELFTEST ops | (G1P AXI regs) | PL | PRESENT BUT UNTESTED | write COMMAND, read results | — | drive ops from shell |
| G1P IRQ 61 via GIC | GIC (not enabled) | PL + GIC | STUB — MUST NOT CLAIM PASS | enable GIC, fire IRQ, ISR | — | enable GIC; prove ISR |
| SD0 read driver | `board/zynq7000/sd.rs` | SD0 | PRESENT BUT UNTESTED | used by `zynq_stage1` | — | prove via stage-1 on HW |
| FAT32 reader | `board/zynq7000/fat.rs` | SD | PRESENT BUT UNTESTED | used by `zynq_stage1` | — | prove via stage-1 on HW |
| GEM Ethernet | (not in this build) | GEM0/PHY | UNSUPPORTED | — | — | add driver; prove on HW |

## Hosted-only features (bare metal = stub by design)

| Feature | Owning module | State | Note |
|---------|---------------|-------|------|
| SSH remote shell | `hosted/ssh/`, `userland/ssh_cmd` | HOSTED ONLY | needs `std`/tokio/russh |
| Statechain ledger | `kernel/statechain*` | HOSTED ONLY | boot: "hosted-only; baremetal stub" |
| Signed checkpoint | `kernel/statechain_checkpoint` | STUB — MUST NOT CLAIM PASS | boot: "requires hosted crypto" |
| EquationRoot / BootSessionRoot / boot manifest | `kernel/…root` | STUB — MUST NOT CLAIM PASS | boot lines say "stub" |
| AI bridge | `kernel/ai/`, `userland/ai_cmd` | HOSTED ONLY | management network, `std` |
| phase_transport / `transport` cmd | `kernel/phase_transport`, `userland/transport_cmd` | HOSTED ONLY | gated behind `std`; bare metal prints "hosted std build required" |

## Global honesty rule

Any row marked **STUB — MUST NOT CLAIM PASS** or **HOSTED ONLY** must never be
reported as a passing bare-metal feature. The kernel itself prints these as
`stub`/`hosted-only` on boot; documentation and any future automation must
preserve that distinction.
