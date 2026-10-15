# PZ7020 Physical Evidence

## Evidence file

| Field | Value |
|-------|-------|
| Path | `fpga/zynq/evidence/kairos_zynq_boot/com12_capture_20260730_132633.txt` |
| Capture date/time | 2026-07-30, 13:26:33 local (filename timestamp) |
| File size | 8468 bytes |
| SHA-256 | `c52ff347c03df0d242b5cb8399c07d356f2e9ea056c7dac37d74c9200d98c09c` |
| Board | PZ7020-SL-C (Zynq-7020, Cortex-A9) |
| Console | COM12, 115200 8-N-1 |
| Capture tool | `scripts/capture_kairos_com12.ps1` |

The capture was produced by opening COM12, pressing the board **RESET**, letting
the boot banner scroll to the `gpos> ` prompt, then having the harness send
`help` and `state` over real UART0 RX and record the kernel's replies.

## Observed evidence (directly in the transcript)

### Boot success
Boot splash rendered, followed by the kernel awakening list. Box-drawing/φ glyphs
appear as `?` in the file because the capture was decoded through a non-UTF-8
console code page; the byte stream itself is the kernel's real UART output.
Awakening lines observed:

```
[ AWAKE ] stateclock: chronos + kairos clocks
[ AWAKE ] gphase_core: compose / reflect algebra
[ AWAKE ] scheduler: Phi0-Phi3 phase graph
[ AWAKE ] memory: 4 tagged pages mapped
[ AWAKE ] syscall: user/kernel boundary armed
[ AWAKE ] phasefs: /tmp mounted (2 inodes)
[ AWAKE ] vm: RISC-V ISA loader ready
[ AWAKE ] trace: kernel event ring (4 records)
[ AWAKE ] init: pid 0 scheduled (1 proc)
[ AWAKE ] timer: preempt + chronos tick
```

### G1P ID / version
```
G1P: id GPL4 (0x47504C34) version 0x00020000 @ 0x40000000, IRQ 61 wired (polled; GIC not enabled)
```
- G1P **ID** = `0x47504C34` ("GPL4")
- G1P **VERSION** = `0x00020000`
- Mapped at `0x40000000`
- Matches `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/REGISTER_MAP.md`.

### `help` command (real dispatch)
Typed `help` returned the full command catalog (help, about, clear, version,
state, clock, phasenet, phaselink, phaseperf, phi-stability, phase-benchmark,
net, regs, ps, whoami, host, spawn, kill, trace, isa, run, dump, mem, log, exit,
dashboard, top, watchdog, phase, topo, curvature, root, audit, checkpoint, ai,
ssh, anom). This is the existing userland registry, not a scripted list.

### `state` command (live values)
```
GPOS StateClock
chronos:  tick: 5   phi: 0.314   x: 1.996
kairos:   tick: 11  sector_id: 5 epsilon: 0.314
dual clock: proper_tick: 5  state_tick: 5  dilation_factor: 1.000  skew: 0
kernel phase:
  current: active: pid 0 (init) Kairos2
  processes: 1   running: 1   memory pages: 4
```

## What the evidence proves (observation)

- microSD boot; proven FSBL path; `system.bit` loaded (G1P responds).
- Cortex-A9 bare-metal execution of the Golden Phase OS from DDR `0x00100000`.
- `Runtime::boot()` completed; scheduler + init pid 0; memory 4 tagged pages;
  StateClock live; syscall boundary functional (commands dispatch).
- Real UART0 **TX** (all output) and **RX** (typed `help`/`state` executed).
- Live interactive `gpos>` shell.
- G1P ID `0x47504C34` and VERSION `0x00020000` at `0x40000000`.

## Observation vs inference (explicit)

| Statement | Type |
|-----------|------|
| Bytes for splash/`help`/`state`/G1P appeared on COM12 | **Observation** |
| G1P ID/VERSION read back the expected constants | **Observation** |
| `help`/`state` executed via the real userland dispatch | **Observation** (interactive input was typed, not scripted) |
| Scheduler/memory/StateClock initialized correctly | **Observation** (their live values printed) |
| The polled global timer (not command flow) advanced the clock | **Inference** — not isolated in this capture |
| PhaseFS file ops work | **Not shown** — only the boot-time mount line was observed |
| GP VM executes guest programs correctly on HW | **Not shown** — no program was run |

## Explicitly NOT proven by this capture

- G1P **interrupt 61** firing through the GIC (IRQs were masked; GIC not enabled).
- The custom **`zynq_stage1`** loader replacing the FSBL.
- Custom **PCAP** bitstream loading.
- **GEM Ethernet** operation (boot reported "Ethernet attachment: none").
- **Hosted SSH**.
- **Hosted cryptographic** checkpoint / statechain (reported "hosted-only / stub").
- PhaseFS file I/O, GP VM program execution, `trace` contents (commands not run).

## Related artifacts and hashes

| Artifact | SHA-256 |
|----------|---------|
| COM12 capture | `c52ff347c03df0d242b5cb8399c07d356f2e9ea056c7dac37d74c9200d98c09c` |
| BOOT.BIN (as flashed) | `11272de87b28886e87a8e8023a2619e92f2c543536a09b4fc05102798a3ccd9e` |
| system.bit | `bb758133f639986fce1d4035ef310571a15a04ced8e8da10f656493f5eb56f91` |
| kairos_zynq ELF (as flashed) | `b5d0ded50dd95f1e4a3f9ada6df368b7f93a86d57fd5cdf3813617897cf354f0` |
| FSBL (zynq_fsbl.elf) | `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` |
