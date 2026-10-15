# PZ7020 Port — Engineering Report

Exhaustive engineering summary of bringing the existing Golden Phase OS up on
real PZ7020-SL-C (Zynq-7020, Cortex-A9) hardware.

## 1. Original architecture before the PZ7020 port

Golden Phase OS (`kairos/src/`) is an architecture-neutral research OS:

- **core** — Kairos-State algebra (`gphase_core`), StateClock geometry.
- **kernel** — scheduler, memory (phase-tagged pages), syscall boundary, trace
  ring, phase math, PhaseNet/PhaseLink, and (hosted-only) statechain/AI.
- **userland** — the `gpos>` shell, command parser, and a large command
  registry dispatched through `execute_session`.
- **fs / vm** — PhaseFS and the GP VM (a RISC-V-style *guest* ISA interpreter).
- **runtime** — `Runtime::boot()`, the single boot entry that wires the hosted
  kernel + userland together.
- **boot** — a proven bare-metal target: RISC-V `virt` under QEMU
  (`boot/kernel_main.rs`), M-mode, serial shell. This established the pattern:
  *init the arch, then `Runtime::boot()`, then a serial shell loop.*

Two bare-metal ARM anchors already existed as **stubs**:
`src/arch/armv7/` (startup.S, linker.ld) and `src/board/zynq7000/`
(uart.rs, memory_map.rs, plus module declarations for `ps7_init`, `sd`, `fat`,
`kernel_main` that had never been written). The `zynq-a9` feature they implied
did not exist in `Cargo.toml`.

## 2. Rejected approach — the diagnostic-harness detour

An earlier line of work (`zynq_a9_os/`) was a **separate** bare-metal AArch32
crate that re-implemented its own kernel, its own `main`, and a **scripted
shell** (`shell::scripted_session()` in `zynq_a9_os/src/main.rs`) that
auto-played `help/status/compose/g1p/fs/net/exit` for a "seal" without any host
typing. That is a diagnostic/bring-up harness, **not** the Golden Phase OS, and
its scripted session prints predetermined output rather than dispatching real
interactive input through the OS userland.

This approach was **rejected** for the console goal: it would have produced a
"second Zynq OS" and a faked, non-interactive console. The scripted session was
removed from that crate, and the real port drives the **existing** OS userland
over live UART0 RX/TX instead.

## 3. Final corrected architecture

```
BootROM → FSBL → PS7 init → system.bit → kairos_zynq ELF in DDR →
Cortex-A9 startup → PZ7020 platform init → Runtime::boot() (existing OS) →
existing kernel/scheduler/memory/StateClock → existing userland → real UART shell
```

The port adds a platform layer and one ARM entry that funnels into the
**unchanged** `Runtime::boot()` and userland. See
`docs/PZ7020_ARCHITECTURE.md`.

## 4. Files added

| File | Role |
|------|------|
| `src/arch/armv7/startup.S` | Cortex-A9 reset: SVC/abort/undef/IRQ stacks, VBAR, BSS clear, `rust_main`; UART crash letters (`U/P/I/D!` + DFAR/DFSR) |
| `src/arch/armv7/linker.ld` | DDR layout at `0x00100000`, stacks, 16 MiB heap symbols |
| `src/board/zynq7000/ps7_init.rs` | PS7 bytecode interpreter (WRITE/MASKWRITE/MASKPOLL/MASKDELAY) — **stage-0 only** |
| `src/board/zynq7000/ps7_init_data.rs` | **Generated** silicon-3.0 op streams from the sealed `.xsa` |
| `src/board/zynq7000/sd.rs` | SD0 polled read driver (port of the board-proven `zynq_a9_os` read path) |
| `src/board/zynq7000/fat.rs` | Minimal FAT32 reader (MBR/BPB, 8.3 root, cluster chains) |
| `src/board/zynq7000/kernel_main.rs` | ARM mirror of `boot/kernel_main.rs`: heap → `Runtime::boot()` → splash → G1P report → polled tick → shell loop |
| `src/bin/kairos_zynq.rs` | ARM boot binary (mirror of `kairos_boot.rs`) |
| `zynq_stage1/*` | OCM SD loader crate (future FSBL-replacement path; builds, not run on HW) |
| `scripts/gen_ps7_init.py` | Deterministic `.xsa` → `ps7_init_data.rs` generator (records source/stream SHA) |
| `scripts/capture_kairos_com12.ps1` | Live COM12 capture harness (drives real `help`/`state`) |

## 5. Files changed (wiring only — no kernel logic altered)

| File | Change |
|------|--------|
| `Cargo.toml` | add `zynq-a9` feature + `kairos_zynq` bin |
| `src/lib.rs` | add `pub mod arch; pub mod board;` |
| `build.rs` | choose linker script by target (riscv64 vs armv7a) |
| `.cargo/config.toml` | add `armv7a-none-eabi` linker entry |
| `src/arch/mod.rs`, `src/board/mod.rs` | gate `armv7`/`zynq7000` on `zynq-a9`/`target_arch` |
| `src/boot/heap.rs` | `addr_of!` for the non-RISC-V heap-symbol externs |
| `src/kernel/mod.rs`, `src/userland/mod.rs`, `src/userland/commands/dispatch.rs` | gate `phase_transport`/`transport_cmd` behind `std` (with a bare-metal "hosted std build required" fallback) |
| `src/core/recursive_shell.rs`, `src/protocol/hybrid_frame.rs` | `no_std` float shims (`libm`) mirroring the existing `fsqrt` pattern |
| `src/userland/splash.rs` | arch label from `target_arch` (cosmetic; post-proof) |
| `src/arch/armv7/heap.rs` | **removed** — duplicate `#[global_allocator]` (kept the one in `boot/heap.rs`) |

## 6. Platform adapters added

UART0 (TX/RX, polled), SD0 (polled PIO read), FAT32 (read-only), PS7 init
(stage-0), a polled SCU global-timer tick source, and the G1P register window.
No MMU/cache enable is performed by the kernel path (the FSBL leaves a usable
flat mapping); `startup.S` installs the vector table and mode stacks only.

## 7. Feature flags used

- `zynq-a9` (new) = `baremetal` + the `arch/armv7` + `board/zynq7000` layer.
- Built `--no-default-features --features zynq-a9` for `armv7a-none-eabi`.
- The default hosted (`std`) build and the RISC-V `baremetal` build are
  unaffected and still compile.

## 8. Linker and memory layout

`src/arch/armv7/linker.ld`, `ENTRY(_start)`, single DDR region:

```
ddr : ORIGIN = 0x00100000, LENGTH = 0x0FF00000
.text (KEEP .text.boot) / .rodata / .data / .bss
_stack_top      (SVC, 64 KiB)
_irq_stack_top  (abort/undef/IRQ, 8 KiB)
__heap_start / __heap_end  (16 MiB, consumed by boot::heap)
```

Built kernel ELF: entry `0x00100060`, one `LOAD` segment at `0x00100000`,
file size `0x6a230` (~433 KiB), mem size `0x6a2c8`.

## 9. Reset / startup path

`startup.S` `_start`: set SVC stack → set abort/undef/IRQ mode stacks →
return to SVC with IRQs masked → set `VBAR` to the vector table → clear BSS →
`bl rust_main`. `rust_main` (`bin/kairos_zynq.rs`) calls
`board::zynq7000::kernel_main`. Faults print a single letter (`U`/`P`/`I`) or,
for data aborts, `D!` followed by DFAR/DFSR hex, then hang — so a crash is
diagnosable over UART.

## 10. DDR load address `0x00100000`

Fixed by the FSBL handoff convention (application above the 1 MiB OCM/reserved
window). The kernel ELF's single `LOAD` is placed there; the FSBL copies the
segment and jumps to `0x00100060`.

## 11. FSBL responsibilities

The sealed `zynq_fsbl.elf` (sha `8b71aa70…`) performs: PS7 initialization
(MIO/PLL/clocks/**DDR**/peripherals — the same register facts our
`ps7_init_data.rs` was generated from), programs the PL with `system.bit`, then
loads and jumps to the kairos_zynq application partition. In this release the
kernel does **not** re-run PS7 init.

## 12. system.bit loading

`system.bit` is the second BOOT.BIN partition; the FSBL configures the PL from
it before handoff, bringing the G1P AXI-Lite block live at `0x40000000`.

## 13. PS-to-PL mapping

G1P is on PS `M_AXI_GP0` at `0x40000000` (4 KiB). Register map in
`fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/REGISTER_MAP.md`: ID `0x47504C34`,
VERSION `0x00020000`, IRQ wired to `IRQ_F2P[0]` (GIC SPI 61). The kernel reads
ID/VERSION on boot and reports them.

## 14. UART0 TX and RX

`board/zynq7000/uart.rs`: polled. TX waits on `SR.TXEMPTY` then writes `FIFO`
(with `\n`→`\r\n`). RX: `read_byte_blocking` waits on `SR.RXEMPTY`; `read_line`
echoes input and handles backspace — mirroring the RISC-V console. Baud/MIO come
up as part of PS7 init (UART_FREQ 100 MHz, 115200 8-N-1).

## 15. Connection to the existing `Runtime::boot()`

`board::zynq7000::kernel_main` constructs `Runtime::new()` and calls
`runtime.boot()` — the identical entry the RISC-V path uses — then renders the
same `BootSplashInfo`/`render_boot_screen`. No boot logic is duplicated.

## 16. Connection to the existing userland shell

The shell loop builds a `ShellSession` over `runtime.hosted.{kernel,io,programs,
vms,session_state}` and calls `shell.execute_session(&mut session, &line)` —
the same dispatch used everywhere. Input comes from real UART0 RX; output is
flushed to UART0 TX. `help` and `state` execute through the unchanged
`commands::dispatch`.

## 17. StateClock initialization

Initialized inside `Runtime::boot()` (chronos + kairos clocks). Observed live:
`state` returned chronos tick 5 / φ 0.314 / x 1.996 and kairos tick 11 /
sector 5 / ε 0.314, dual-clock dilation 1.000.

## 18. Scheduler initialization

Initialized inside `Runtime::boot()`. Observed: init **pid 0** scheduled,
`processes: 1`, `running: 1`, current `active: pid 0 (init) Kairos2`.

## 19. Memory initialization

Existing phase-tagged page memory. Observed: `memory: 4 tagged pages mapped`
(splash) and `memory pages: 4` (`state`). Heap for `alloc` is the 16 MiB
`__heap_start..__heap_end` DDR region via `boot::heap` + `linked_list_allocator`.

## 20. Syscall boundary status

**Armed and exercised.** Splash: `syscall: user/kernel boundary armed`. Every
shell command runs through `syscall::ensure_running` / `syscall::write`, so a
working `help`/`state` is direct evidence the boundary functions on hardware.

## 21. PhaseFS status

Mounted at boot (`phasefs: /tmp mounted (2 inodes)`, observed). File operations
were **not** independently exercised on hardware → PARTIAL.

## 22. VM status

The GP VM (RISC-V-style **guest** ISA interpreter) reports ready
(`vm: RISC-V ISA loader ready`, observed). No guest program was run on hardware
→ PRESENT BUT UNTESTED. This is a guest ISA and says nothing about the host CPU
(ARM Cortex-A9).

## 23. Trace-ring status

Boot reported `trace: kernel event ring (4 records)` (observed). The `trace`
command was not run on hardware → PARTIAL.

## 24. Timer status

A polled SCU global-timer tick source drives `runtime.force_timer_tick()` every
~10 ms of timer time. The StateClock advanced (chronos 5 / kairos 11), which is
consistent with ticking, but the advance was not isolated from command-driven
`advance_chronos()` → PARTIAL (polled tick implemented; not independently
proven). No GIC timer interrupt is used.

## 25. G1P polling status

**ID/VERSION read verified** on hardware via polled MMIO reads of `0x40000000`/
`0x40000004`. IRQ 61 is *wired* but the kernel keeps IRQs masked (GIC not
enabled), so interrupt delivery is **not** proven.

## 26. Exact physical evidence

`fpga/zynq/evidence/kairos_zynq_boot/com12_capture_20260730_132633.txt`
(8468 bytes, sha `c52ff347…`). Contains the boot splash, awakening list, the
G1P report line, the `gpos>` prompt, the full `help` output, and the `state`
output. See `docs/PZ7020_EVIDENCE.md`.

## 27. Remaining engineering work

1. Enable the GIC and prove G1P IRQ 61 delivery (ISR + captured evidence).
2. Bring up and prove the `zynq_stage1` OCM loader + PCAP bitstream path
   (FSBL replacement) on hardware.
3. GEM Ethernet driver + PhaseNet-over-wire on this board.
4. Re-flash and re-capture with the corrected arch label.
5. Optional: MMU/cache enable for performance (currently flat, FSBL-provided).
