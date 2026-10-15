# PZ7020 Build and Flash

End-to-end: source → kernel ELF → BOOT.BIN → FAT32 microSD → live console.

## Prerequisites

- Rust toolchain with the bare-metal ARM target:
  ```bash
  rustup target add armv7a-none-eabi
  ```
- **AMD/Xilinx Vitis 2026.1** (provides `arm-none-eabi-gcc` and `bootgen`).
  Put `arm-none-eabi-gcc` on `PATH`. `.cargo/config.toml` uses that name, not a
  machine-local `D:\Xilinx\...` path. Typical Windows install:
  `D:\Xilinx\2026.1\Vitis\gnu\aarch32\nt\gcc-arm-none-eabi\bin`.
- FSBL (`zynq_fsbl.elf`) and `system.bit` are **local board-pack inputs**, not
  required to clone or `cargo test` this tree. Obtain them from your sealed
  pack; match hashes in `fpga/zynq/evidence` / `SHA256SUMS.txt`. Do not
  commit vendor FSBL or bitstreams into a public history rewrite without an
  explicit policy decision.
- (Only to *regenerate* PS7 data) Python 3.

## 1. Build the Golden Phase OS kernel for the board

```bash
cargo build --release --bin kairos_zynq \
  --no-default-features --features zynq-a9 \
  --target armv7a-none-eabi
```

Output: `target/armv7a-none-eabi/release/kairos_zynq` (ELF, no extension).

Verify it is a Cortex-A9 image loading to DDR:

```bash
arm-none-eabi-readelf -l target/armv7a-none-eabi/release/kairos_zynq
# Machine: ARM ; Entry 0x100060 ; LOAD ... 0x00100000 ... RWE
```

## 2. (Optional) Build the OCM stage-1 loader

Future FSBL-replacement path only — **not** part of the proven boot chain.

```bash
cd zynq_stage1
cargo build --release --target armv7a-none-eabi
# -> zynq_stage1/target/armv7a-none-eabi/release/zynq_stage1
```

## 3. (Optional) Regenerate PS7 init data

Only when the `.xsa` changes. The generated file is checked in; a normal build
does **not** need Python or the `.xsa`.

```bash
python scripts/gen_ps7_init.py
# writes src/board/zynq7000/ps7_init_data.rs and prints .xsa + stream SHA-256
```

The generator fails hard on unknown opcodes and preserves original op order
(MASKWRITE/MASKPOLL/MASKDELAY included).

## 4. Package BOOT.BIN with bootgen

Create `boot.bif` (absolute paths recommended on Windows). Copy/rename the built
kernel ELF to end in `.elf` so bootgen treats it as an application partition:

```
the_ROM_image:
{
  [bootloader] <path>/zynq_fsbl.elf
  <path>/system.bit
  <path>/kairos_zynq.elf
}
```

```bash
bootgen -arch zynq -image boot.bif -w -o BOOT.BIN
```

Windows: invoke `D:\Xilinx\2026.1\Vitis\bin\bootgen.bat` (the `.bat` sets up the
Vitis environment the exe needs).

Expected: `[INFO] : Bootimage generated successfully`, ~4.6 MB `BOOT.BIN`.

### Partition order matters

`[bootloader]` **must** be the FSBL. `system.bit` **must** come before the
application ELF so the PL is configured before the kernel reads G1P.

## 5. Prepare the microSD (plain file copy only)

1. Format microSD as **FAT32**.
2. Remove any stale `BOOT.BIN` / `image.ub` from the root.
3. Copy `BOOT.BIN` to the **root** of the card.

**No raw-sector writes, no `diskpart`, no admin disk tooling.** The BootROM
reads `BOOT.BIN` as a normal FAT32 file.

Windows example (SD is drive `H:`):

```powershell
Copy-Item .\BOOT.BIN H:\BOOT.BIN -Force
# verify:
(Get-FileHash .\BOOT.BIN).Hash -eq (Get-FileHash H:\BOOT.BIN).Hash
```

Safely eject before removing the card so the write cache flushes.

## 6. Physical boot

1. Insert the microSD into the board.
2. Boot-mode jumpers → **SD**.
3. Connect USB-UART; **COM12 @ 115200 8-N-1**.
4. Power on / press **RESET**.

## 7. Capture the live console

```bash
powershell -ExecutionPolicy Bypass -File scripts\capture_kairos_com12.ps1 \
  -BootTimeoutSec 60 -CmdTimeoutSec 12
```

Press **RESET** when it says "press RESET now". The script captures the boot
banner until `gpos> `, then sends `help` and `state` and records the replies to
`fpga/zynq/evidence/kairos_zynq_boot/com12_capture_<timestamp>.txt`.

> **DTR/RTS note:** the capture script asserts DTR/RTS (matching the sealed
> `capture_com12.ps1`). If your USB-UART wiring ties DTR/RTS to the board reset,
> the board can be held in reset while the port is open — set both to `$false`
> in that case.

## 8. Reproducing the hashes

```bash
sha256sum BOOT.BIN
# 11272de87b28886e87a8e8023a2619e92f2c543536a09b4fc05102798a3ccd9e
sha256sum fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/system.bit
# bb758133f639986fce1d4035ef310571a15a04ced8e8da10f656493f5eb56f91
sha256sum target/armv7a-none-eabi/release/kairos_zynq
# b5d0ded50dd95f1e4a3f9ada6df368b7f93a86d57fd5cdf3813617897cf354f0  (as-proven build)
```

> The kernel ELF hash `b5d0ded5…` is the **as-proven** build (with the
> `baremetal-riscv64` splash label). After the cosmetic label fix the ELF hash
> changes; that corrected build has not yet been re-flashed/re-captured.

## Toolchain reference

| Tool | Path (this machine) |
|------|---------------------|
| Rust ARM linker | `D:\Xilinx\2026.1\Vitis\gnu\aarch32\nt\gcc-arm-none-eabi\bin\arm-none-eabi-gcc.exe` |
| bootgen | `D:\Xilinx\2026.1\Vitis\bin\bootgen.bat` |
| readelf | `D:\Xilinx\2026.1\Vitis\gnu\aarch32\nt\gcc-arm-none-eabi\bin\arm-none-eabi-readelf.exe` |
