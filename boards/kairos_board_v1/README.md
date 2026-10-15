# KairOS Board v1 — BSP skeleton

**Status:** Requirements frozen from measured builds (2026-07-07).  
**Full spec:** `docs/KAIROS_BOARD_V1_REQUIREMENTS.md`

## Target

- SoC: 64-bit RISC-V, ≥4 harts, M-mode boot
- ISA: `riscv64gc` (`riscv64gc-unknown-none-elf`)
- Kernel load: `0x80000000` (matches `kairos_boot` linker script)
- Flash image: `kairos_boot.bin` (557 KB full tier measured)

## Directory map

| File | Purpose |
|------|---------|
| `memory_map.txt` | RAM / MMIO / flash layout |
| `openocd.cfg` | JTAG debug (edit for your probe) |
| `gdb_init.gdb` | GDB connect helpers |

## Build flash image

```bash
cargo build --release --bin kairos_boot \
  --no-default-features \
  --features baremetal,bm-full \
  --target riscv64gc-unknown-none-elf
```

Output: see `release_snapshot/baremetal/tiers/full/kairos_boot.bin`

## QEMU reference (not this board)

```bash
qemu-system-riscv64 -machine virt -bios none -kernel kairos_boot.bin \
  -serial stdio -m 128M
```

Physical board port: replace UART/clock/PLIC with SoC-specific drivers under `src/boot/arch/`.
