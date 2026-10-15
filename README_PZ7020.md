# Golden Phase OS — PZ7020-SL-C Bare-Metal Port

> **One OS, one kernel.** This is not a second operating system. It is the
> existing Golden Phase OS (`kairos/src/`) running on real Zynq-7000 hardware
> through a thin PZ7020 platform-support layer. The kernel, `Runtime::boot()`,
> scheduler, memory, StateClock, and userland command registry are unchanged.

## Executive summary

The Golden Phase OS kernel boots and runs bare-metal on a **PZ7020-SL-C**
(Zynq-7020, Cortex-A9), presenting its normal interactive `gpos>` shell over
the PS UART0 serial console (COM12). On **2026-07-30** a live COM12 capture
recorded the full boot, a working `help` command, a working `state` command,
and the G1P PL block responding at `0x40000000` with its expected ID and
version. That capture is the physical proof of this port.

**Utility OS gold (2026-08-14, on `main`):** one bitstream with G1P+TNSR at `0x40000000` and G64S at `0x40001000`. COM12 same-boot PASS (TNSR T10 4/4, G64 compose 16/16, reflect 4/4, atlas 52/12/0). Pack `fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN` — see `fpga/zynq/release/UTILITY_GOLD.txt`.

## Current physical verification status

**PHYSICALLY VERIFIED (live COM12 capture 2026-07-30):**

- microSD boot via the on-chip BootROM
- proven FSBL boot path (PS7 init + PL configuration)
- `system.bit` PL bitstream loaded (G1P responds)
- Cortex-A9 bare-metal kernel execution from DDR `0x00100000`
- existing `Runtime::boot()` and kernel/userland initialization
- existing scheduler + init process (pid 0)
- existing memory subsystem (4 tagged pages)
- existing StateClock (chronos + kairos)
- existing VM / userland command registry
- real PS UART0 **TX** (all boot + command output)
- real PS UART0 **RX** (typed input echoed and executed)
- live interactive `gpos>` shell
- typed `help` → full command catalog
- typed `state` → live StateClock / process output
- G1P ID `0x47504C34`, version `0x00020000`, mapped at `0x40000000`

**NOT yet physically proven** (see `docs/PZ7020_KNOWN_LIMITATIONS.md`):

- G1P interrupt 61 actually firing through the GIC
- the custom `zynq_stage1` loader replacing the FSBL
- custom PCAP bitstream loading
- GEM Ethernet operation
- hosted SSH
- hosted cryptographic checkpoint / statechain features

## Board and console

| Item | Value |
|------|-------|
| Board | PZ7020-SL-C (PicoZed / StarLite family) |
| SoC | Xilinx Zynq-7020, `xc7z020clg400-2` |
| CPU | Dual-core ARM Cortex-A9 (this port uses core 0) |
| Console | PS UART0 @ `0xE0000000` → COM12 |
| Serial | 115200 baud, 8 data bits, no parity, 1 stop (8-N-1) |
| Boot media | FAT32 microSD |
| DDR load address | `0x00100000` |
| G1P PL block | `0x40000000` (AXI-Lite), GIC SPI 61 |
| TNSR (gold) | `0x40000000+0x100` inside G1P window |
| G64S (gold) | `0x40001000` (AXI-Lite), polled |

## Boot-chain diagram

```
SD BootROM
  └─> proven FSBL (zynq_fsbl.elf)
        ├─ PS7 initialization (MIO / PLL / clocks / DDR / peripherals)
        ├─ loads system.bit into the PL  (G1P fabric goes live)
        ├─ loads kairos_zynq ELF into DDR @ 0x00100000
        └─ jumps to the Cortex-A9 kernel entry (_start)
              └─> arch/armv7 startup.S  (stacks, VBAR, BSS clear)
                    └─> rust_main → board::zynq7000::kernel_main
                          └─> boot::heap::init
                          └─> Runtime::boot()            [existing OS]
                          └─> boot splash over UART0
                          └─> G1P ID/VERSION read + report
                          └─> polled global-timer tick source
                          └─> real UART0 RX/TX shell loop [existing userland]
                                └─> gpos>  (help, state, …)
```

## Repository directory map

```
kairos/
├─ src/                         THE Golden Phase OS (kernel/runtime/userland)
│  ├─ arch/armv7/               Cortex-A9 startup.S + linker.ld  (platform)
│  ├─ board/zynq7000/           PZ7020 board support             (platform)
│  │   ├─ ps7_init.rs           PS7 bytecode interpreter (stage-0 only)
│  │   ├─ ps7_init_data.rs      GENERATED from the sealed .xsa
│  │   ├─ sd.rs                 SD0 polled read driver
│  │   ├─ fat.rs                minimal FAT32 reader
│  │   ├─ uart.rs               PS UART0 polled TX/RX
│  │   ├─ memory_map.rs         board addresses
│  │   └─ kernel_main.rs        ARM entry → Runtime::boot() → shell
│  ├─ bin/kairos_zynq.rs        ARM boot binary (mirror of kairos_boot.rs)
│  ├─ boot/                     existing RISC-V QEMU boot (unchanged path)
│  ├─ runtime/ kernel/ userland/ core/ fs/ vm/ …  existing OS layers
├─ zynq_stage1/                 OCM SD loader (future FSBL-replacement path)
├─ scripts/
│  ├─ gen_ps7_init.py           deterministic ps7_init_data.rs generator
│  └─ capture_kairos_com12.ps1  live COM12 capture harness
├─ fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/   sealed HW export (.xsa), FSBL, system.bit
└─ fpga/zynq/evidence/kairos_zynq_boot/       COM12 proof capture
```

## Relationship: `kairos/src` vs the Zynq platform layer

`kairos/src/` **is** the operating system — algebra core, kernel, scheduler,
memory, StateClock, PhaseFS, the GP VM, and the userland shell. It is
architecture-neutral and already ran on RISC-V under QEMU.

`src/arch/armv7/` and `src/board/zynq7000/` are the **platform-support layer**:
CPU startup, linker/memory layout, MMU expectations, UART, timer, SD, FAT, and
the G1P register window. They exist only to *start the OS on this board and give
it a console*. They contain no kernel logic and do not constitute a second OS.
`board::zynq7000::kernel_main` is the ARM mirror of the existing RISC-V
`boot::kernel_main` — it calls the same `Runtime::boot()` and the same userland
`execute_session` dispatch.

## Toolchain requirements

- **Rust** with target `armv7a-none-eabi`
  (`rustup target add armv7a-none-eabi`)
- **AMD/Xilinx Vitis 2026.1** providing:
  - `arm-none-eabi-gcc` (used as the Rust linker for the ARM target)
  - `bootgen` (packages `BOOT.BIN`)
- **Python 3** (only to *regenerate* `ps7_init_data.rs`; not needed for a
  normal build — the generated file is checked in)

## Build commands

Build the Golden Phase OS kernel image for the board:

```bash
cargo build --release --bin kairos_zynq \
  --no-default-features --features zynq-a9 \
  --target armv7a-none-eabi
```

Output ELF: `target/armv7a-none-eabi/release/kairos_zynq`.

(Optional) Build the OCM stage-1 loader — future FSBL-replacement path only:

```bash
cd zynq_stage1
cargo build --release --target armv7a-none-eabi
```

## BOOT.BIN packaging

Package the proven FSBL + `system.bit` + your kernel ELF with `bootgen`
(`boot.bif`):

```
the_ROM_image:
{
  [bootloader] zynq_fsbl.elf
  system.bit
  kairos_zynq.elf     ← rename the built kairos_zynq ELF to *.elf
}
```

```bash
bootgen -arch zynq -image boot.bif -w -o BOOT.BIN
```

The FSBL here is the sealed `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf`
(sha256 `8b71aa70…`, matching `zynq_stage0/IDENTITY.txt`).

## microSD preparation

1. Format the microSD as **FAT32**.
2. Copy `BOOT.BIN` to the **root** of the card (plain file copy — no raw-sector
   writes, no `diskpart`, no admin disk tools).
3. Remove any stale `BOOT.BIN` / `image.ub` from a previous image first.

## Physical boot procedure

1. Insert the microSD into the board.
2. Set the boot-mode jumpers to **SD**.
3. Connect USB-UART; open **COM12** at **115200 8-N-1**.
4. Power on / press **RESET**.

## Expected serial output

The Golden Phase boot splash, then the kernel awakening list
(`stateclock`, `gphase_core`, `scheduler`, `memory`, `syscall`, `phasefs`,
`vm`, `trace`, `init`, `timer`, …), then:

```
G1P: id GPL4 (0x47504C34) version 0x00020000 @ 0x40000000, IRQ 61 wired (polled; GIC not enabled)
gpos>
```

## Interactive shell test

At `gpos>` type `help` (command catalog) and `state` (live StateClock /
process view). Both are served by the existing userland — see
`docs/PZ7020_EVIDENCE.md` for the exact captured transcript.

## G1P register verification

On boot the kernel reads `0x40000000` (ID) and `0x40000004` (VERSION) through
the FSBL-configured PL and reports the values. Expected: ID `0x47504C34`
("GPL4"), VERSION `0x00020000` — matching
`fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/REGISTER_MAP.md`. A mismatch prints an
explicit `UNEXPECTED …` line rather than a stub success.

## Evidence files and hashes

| Artifact | Path | SHA-256 |
|----------|------|---------|
| COM12 capture | `fpga/zynq/evidence/kairos_zynq_boot/com12_capture_20260730_132633.txt` | `c52ff347c03df0d242b5cb8399c07d356f2e9ea056c7dac37d74c9200d98c09c` |
| BOOT.BIN | (built via bootgen) | `11272de87b28886e87a8e8023a2619e92f2c543536a09b4fc05102798a3ccd9e` |
| system.bit | `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/system.bit` | `bb758133f639986fce1d4035ef310571a15a04ced8e8da10f656493f5eb56f91` |
| kairos_zynq ELF (as flashed) | (built to `target/…/release/kairos_zynq`) | `b5d0ded50dd95f1e4a3f9ada6df368b7f93a86d57fd5cdf3813617897cf354f0` |

## Proven feature list

See `docs/PZ7020_FEATURE_MATRIX.md`. Summary: UART TX/RX, interactive shell,
`help`, `state`, scheduler/init, memory tagging, StateClock, and G1P ID/version
read are **PHYSICALLY VERIFIED**.

## Unproven feature list

G1P IRQ-through-GIC, `zynq_stage1`/PCAP boot, GEM Ethernet, hosted SSH, and
hosted crypto (statechain/checkpoint) are **not** physically proven. Several are
hosted-only or stubbed on bare metal by design.

## Known splash-label correction

The physically-proven build printed `(baremetal-riscv64)` — a hardcoded label,
now corrected to select from `target_arch` (`arm` →
`baremetal-armv7-a-cortex-a9`). The correction is **software-verified**
(compiles; string present in the ARM ELF) but the corrected build has **not**
been re-flashed/re-captured. The `vm: RISC-V ISA loader` line refers to the
**GP VM guest ISA interpreter**, not the host CPU (which is ARM Cortex-A9).

## Troubleshooting

| Symptom | Likely cause | Action |
|---------|--------------|--------|
| No UART output at all | RESET not pressed; wrong COM port; DTR/RTS holding reset | Confirm COM12, press RESET during capture window |
| Crash letter `D!…` | data abort (bad address) with DFAR/DFSR hex | decode fault address; check load layout |
| Crash letter `U`/`P`/`I` | undefined / prefetch abort / unexpected IRQ | inspect kernel entry / vectors |
| Boots but no `gpos>` | splash printed but shell loop not reached | check `Runtime::boot()` return / heap init |
| G1P `UNEXPECTED …` | `system.bit` not loaded or wrong bitstream | verify BOOT.BIN partition order |

See `docs/PZ7020_RECOVERY.md` for full recovery steps.

## Recovery procedure

The flash is a plain FAT32 file copy, so recovery is: re-format the card FAT32
and copy a known-good `BOOT.BIN` to the root. No raw sectors are involved and
the on-card user data region is never touched. See `docs/PZ7020_RECOVERY.md`.

## Release / tag information

- **Branch:** `zynq-pz7020-port`
- **Preservation commit:** `7b7d75490c06d38d9b45c1540f53aaed0db1e846`
  — "PZ7020 bare-metal console port physically verified"
- **Annotated tag:** `pz7020-console-proof-20260730`

See `docs/PZ7020_CHANGELOG.md`.
