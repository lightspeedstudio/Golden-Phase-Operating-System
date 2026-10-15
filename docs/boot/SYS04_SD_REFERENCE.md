<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 4 — working SD transfer reference

Extracted **before** the owned ADMA library was written. Numeric values are cited. This is not a generic SD tutorial.

**Working Stage 1 source:** `zynq_stage1/src/main.rs` → `SdCard::init` then `Fat32::mount` then `load_root_file(..., BOOT    BIN)`. SHA-256 `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`. File is not edited.

**Board-proven SD host:** `src/board/zynq7000/sd.rs` SHA-256 `b30aee86c2c3a35395f611324ff75b62027b00cb59b1141f811f5f40eb706792` (13441 bytes, System 1 lock). Polled **PIO CMD17** one 512-byte block. System 4 replaces **only** the transfer engine (CMD18 + ADMA2). Initialization through card select is copied from this file.

**FAT:** `src/board/zynq7000/fat.rs` — MBR partition 0 or superfloppy BPB, 512-byte sectors, 8.3 root, cluster chain. Working path is FAT32 (`root_ent_cnt == 0`). Owned library also accepts FAT16 when `root_ent_cnt != 0`.

**System 3 gate:** evidence commit `659a7744a4c689d72ce4cc3c7f9f54280a675a43` (`SYS03_COM12_PASS`). SDIO clock and MIO exist only after that PS7 sequence.

## Board / documents

| Item | Value | Source |
| --- | --- | --- |
| SD0 controller | `0xE0100000` | working `sd.rs`; UG585 SDIO |
| Card detect | not wired (`SD0_HAS_CARD_DETECT = false`) | `sd.rs`; `PCW_SD0_GRP_CD_ENABLE=0` |
| MIO | 40..45 | `ps7_config_dump.txt` `PCW_SD0_SD0_IO`; working `ps7.rs` `MIO_PIN_40`..`45` value `0x00001280` |
| SDIO clock | `SDIO_CLK_CTRL` `0xF8000150` mask `0x00003F33` value `0x00000A01` | working `clocks()` / XSA stream. DIVISOR0=10, CLKACT. IO PLL 1000 MHz → **100 MHz** SDIO base |
| Identify SDCLK | divisor 128 → `100 MHz / (2×128) = 390.625 kHz` | working `set_clock_div(128)`; SDHCI 2.00 8-bit divisor (UG585 Clock Control) |
| Data SDCLK | divisor 4 → `100 MHz / (2×4) = 12.5 MHz` | working `set_clock_div(4)` after CMD7 |
| Test card | USB `Mass Storage Device` serial `121220160204`, Disk 7, ~8 GiB, MBR, volume label `GPOS` | Windows `Get-Disk` / `Get-Volume` 2026-08-19; `docs/PZ7020_KNOWN_LIMITATIONS.md`; `CARD_LAYOUT.md` |

MIO is **not** reprogrammed by `SdCard::init`. It is inherited from BootROM + System 3 PS7 `MIO_OPS`.

## Working initialization (exact order from `SdCard::init`)

Software reset and host setup:

| Step | Access | Offset | Value | Notes |
| --- | --- | --- | --- | --- |
| 1 | W8 | `0x2F` SWRST | `0x01` | software reset all; poll bit 0 clear, **1_000_000** spins |
| 2 | W8 | `0x29` POWER | `0x0F` | 3.3 V + power on; `delay(50_000)` |
| 3 | W8 | `0x2E` TIMEOUT | `0x0E` | SDHCI timeout control. Data timeout = TMCLK × 2^(14+13) = 2^27 TMCLK (UG585 / SDHCI). Kept as working value |
| 4 | W16 | `0x34` NISTER | `0x01FF` | enable normal IRQs 0..8 |
| 5 | W16 | `0x36` EISTER | `0xF3FF` | enable error IRQs including ADMA Error bit 9 |
| 6 | W8 | `0x28` HOST | `0x00` | 1-bit, DMA off |
| 7 | clock | `0x2C` CLK | div **128** | stop SDCLK (`~0x0004`), `delay(1000)`; write `(128<<8)\|1 = 0x8001`; poll internal-clock stable bit 1 (**100_000** spins); enable SDCLK `\|0x0004` → `0x8005`; `delay(10_000)` |

Card identification (CMD inhibit wait **2_000_000** spins; each command clears NISTR/EISTR `0xFFFF`; atomic 32-bit write of Transfer Mode + Command at `0x0C`):

| Step | CMD | ARG | Command flags | XFER | Wait |
| --- | --- | --- | --- | --- | --- |
| 8 | CMD0 | 0 | `RESP_NONE` (0) | 0 | CMDCMP; then `delay(10_000)` |
| 9 | CMD8 | `0x000001AA` | R48\|CRC\|IDX (`0x1A`) | 0 | R7 check pattern `0xAA` or `NotSdV2` |
| 10 | CMD55 | RCA<<16 (0) | R48\|CRC\|IDX | 0 | |
| 11 | ACMD41 | `0x40300000` | **R48 only** (`0x02`) | 0 | repeat ≤1000 with `delay(50_000)` until OCR bit 31 ready. HCS bit 30 → `hcc`. **Do not add CRC/IDX** — working code does not |
| 12 | CMD2 | 0 | R136\|CRC (`0x09`) | 0 | CID |
| 13 | CMD3 | 0 | R48\|CRC\|IDX | 0 | RCA = RESP0[31:16] |
| 14 | CMD9 | RCA<<16 | R136\|CRC | 0 | CSD into RESP0..3 |
| 15 | CMD7 | RCA<<16 | **R48b**\|CRC\|IDX (`0x1B`) | 0 | **card select** |

`cmd_raw` always `wait_clear(CMDINHIBIT\|DATAINHIBIT, 2_000_000)` then `wait_ni(CMDCMP, 2_000_000)`. Error path: if NISTR ERR, write EISTR `0xFFFF` and NISTR ERR.

After CMD7 the working driver continues (still required, not “generic”):

| Step | Action |
| --- | --- |
| 16 | `set_clock_div(4)` → CLK `0x0401` then `0x0405` |
| 17 | ACMD6 arg `2` (4-bit), flags R48\|CRC\|IDX |
| 18 | W8 HOST `0x02` (DTW 4-bit) — **see correction** |
| 19 | CMD16 arg 512 |

Block size 512 is fixed. Addressing: `hcc` true → LBA; false → byte address `lba*512`.

## Working card capacity and addressing

Measured on this sacrificial card (`121220160204`) and the working CSD helper `src/fs/journal/capacity.rs`:

| Field | Value | Source |
| --- | --- | --- |
| CSD structure | v2 (SDHC/SDXC) | `sdhci_csd_sectors` test vector for this card: RESP1 `0x003BFF00`, RESP3 `0x00400000` → `device_sectors = 15728640` |
| Capacity | 15728640 × 512 = 8053063680 bytes | `CARD_LAYOUT.md`; `capacity.rs` `eight_gib_card_fits_measured_p1` |
| Addressing | block (HCC/CCS). ACMD41 `0x40300000` requests HCS; OCR bit 30 sets `hcc` | working `sd.rs` |
| Windows size | 8053063680 (`Get-Disk` Disk 7) | 2026-08-19 |

Working `read_block` uses PIO and **does** wait `NI_TRFCMP` after filling the buffer (best-effort `let _ = wait_ni(NI_TRFCMP)`). It never waits `NI_DMA` (bit 3). System 4 must keep that: wait Transfer Complete, poll ADMA Error, **never** wait DMA-INT.

## Real test card filesystem (parsed 2026-08-19)

`Get-Partition -DiskNumber 7` and raw BPB/root from `\\.\H:` (volume start = partition 0). PhysicalDrive7 MBR bytes were not readable without elevation; LBA fields match `CARD_LAYOUT.md` and `Get-Partition`.

### MBR (Disk 7)

| Slot | MbrType | Start LBA | Sectors | Offset bytes | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| 0 | 12 (`0x0C` FAT32 XINT13) | 2048 | 15704064 | 1048576 | Active; drive H: `GPOS` |
| 1 | 218 (`0xDA`) | 15706112 | 16384 | 8041529344 | PhaseFS journal region |
| 2 | empty | 0 | 0 | | |
| 3 | empty | 0 | 0 | | |

`p0_end = 15706112` equals p1 start. `device_sectors = 15728640` ≥ p1 end `15722496`.

### FAT BPB at partition 0 (volume LBA 0)

SHA-256 `773ce41142f4b279687a400d4eeec21a3d830da8dbb2133cd463b108855fb734` (512 bytes).

| BPB field | Offset | Value |
| --- | ---: | --- |
| Jump | 0 | `EB 58 90` |
| OEM | 3 | `MSDOS5.0` |
| Bytes/sector | 11 | 512 |
| Sec/cluster | 13 | **8** |
| Reserved sectors | 14 | **2158** |
| Num FATs | 16 | 2 |
| Root entries | 17 | **0** (FAT32) |
| TotSec16 | 19 | 0 |
| Media | 21 | `0xF8` |
| FATSz16 | 22 | 0 |
| TotSec32 | 32 | 15704064 |
| FATSz32 | 36 | **15305** |
| ExtFlags | 40 | 0 |
| FSVer | 42 | 0 |
| RootClus | 44 | **2** |
| FSInfo | 48 | 1 |
| BkBoot | 50 | 6 |
| FilSysType | 82 | `FAT32   ` |
| Signature | 510 | `55 AA` |

Volume-relative: `fat_lba = 2158`, `data_lba = 2158 + 2×15305 = 32768`. Disk LBA = 2048 + volume LBA.

### `BOOT.BIN` root 8.3 entry

Name `BOOT    BIN` (bytes `42 4F 4F 54 20 20 20 20 42 49 4E`). Attr `0x20`. First cluster **14**. Size **5992148**. Root cluster 2, sector 0, entry index 11.

Dirent SHA-256 `e1a1f02a2ba92a4011223245670a83af1f4a8ee8982c65d05a5c7acd0cb21ae1` (`docs/boot/SYS04_BOOT_DIRENT.bin`).

Cluster chain: **1463** clusters, **1** contiguous run. Volume LBA `32864`, length 11704 sectors. Not fragmented. First 16 KiB SHA-256 `2b64211bcaf760966053f03c4d7acf969b1c1a9dfa874b4fa3b8239e65814870` (`SYS04_BOOT_FIRST_16K.bin`). Bytes `0x20..0x28` = `AA995566` / `XLNX`. Same 16 KiB prefix as recovery `BOOT_RECOVERY_PLAINTEXT.BIN` `46bb693c…c713fb`.

Exact file length for the owned loader is the directory size **5992148**, not the padded cluster run.

## Timeouts (working, preserved)

| Wait | Spins / value | Citation |
| --- | --- | --- |
| SWRST clear | 1_000_000 | `sd.rs` |
| Inhibit clear | 2_000_000 | `sd.rs` |
| NISTR mask | 2_000_000 | `sd.rs` |
| Clock stable | 100_000 | `set_clock_div` |
| ACMD41 loops | 1000 × `delay(50_000)` | `sd.rs` |
| TIMEOUT reg | `0x0E` | `sd.rs`; SDHCI 2^(n+13) TMCLK |

`delay(n)` is `n` `spin_loop` iterations on the A9, not a documented millisecond. System 4 keeps these counts for init.

ADMA Transfer Complete wait uses the same **2_000_000** NISTR poll as working `wait_ni(NI_TRFCMP)`. At 12.5 MHz 4-bit, 16 KiB ≲ 3 ms; 2e6 APU loops is far above that. ADMA Error (EISTR bit 9) is checked every poll via existing `NI_ERR` handling.

## Reviewed correction vs working PIO host byte

| ID | Old (working PIO) | New | Reason | Document |
| --- | --- | --- | --- | --- |
| HOST after 4-bit | `0x02` DTW only | `0x12` DTW \| DMASEL=ADMA2 | 32-bit ADMA2 select is bits [4:3]=10b; 4-bit DTW is bit 1 | UG585 SDIO Host Control; SDHCI 2.00 |
| Transfer | CMD17 PIO, `NI_BRRR`, buffer register | CMD18, Block Count Enable, Auto CMD12, DMA Enable, ADMA2 descriptors | this system | UG585 SDIO Transfer Mode / ADMA |
| Complete wait | `NI_TRFCMP` (PIO also waits this) | `NI_TRFCMP`; if `NI_DMA` set, **clear and ignore**; fail on ADMA Error | working never waits DMA-INT; Pass A failed when DMA-INT was treated as complete | UG585 Normal Interrupt Status: bit 1 Transfer Complete, bit 3 DMA Interrupt (SDMA), Error bit 9 ADMA |
| PIO | `read_block` success path | `read_block_pio_degraded` only; success path must not call it | this system | — |

All CMD0/8/ACMD41/CMD2/3/9/7 flags, arguments, clock divisors, TIMEOUT, NISTER/EISTER, and CMD16 stay as in `sd.rs`.

## Owned transfer (System 4)

- Read-only FAT16/FAT32 root lookup of `BOOT    BIN`
- Exact directory file length
- Cluster-chain → contiguous LBA runs; each run is one ADMA2 descriptor; more than 8 runs → refuse (this card: 1 run)
- ADMA2 32-bit descriptors: Valid\|Tran, last has End. Length in bytes. Address 4-byte aligned
- CMD18, BCE, Auto CMD12, DMA Enable
- Read the first **16 KiB** (32 × 512) into DDR `0x02000000` (working Stage 1 `BOOT_SCRATCH`)
- UART:

```
(SD init after System 3 BOOTMODE SD PASS)
SD INIT PASS
SD FAT BOOT.BIN PASS bytes=<exact_file_length>
SD ADMA2 CMD18 PASS bytes=16384
SYS04 HOLD
```

`SD FAT BOOT.BIN PASS bytes=<exact_file_length>` prints the FAT directory `DIR_FileSize` of `BOOT    BIN` (offset 28 in the 8.3 entry), not a build-time constant. `5,992,148` is the System 3 image still on the card. After `BOOT_SYS04_ADMA.BIN` (5,997,268 bytes) is copied as `BOOT.BIN`, UART must show `bytes=5997268`.

`SD ADMA2 CMD18 PASS bytes=16384` prints only after FAT lookup, CMD18 response, Transfer Complete, ADMA Error register is zero, and two separate header reads succeed: `0xAA995566` at offset `0x20`, `0x584C4E58` (`XLNX`) at offset `0x24`. PIO is not used.

## Linked artifacts (COM12 not run)

See `docs/boot/SYS04_ARTIFACT_HASHES.txt`. Image `zynq_stage1/owned/build/BOOT_SYS04_ADMA.BIN` (does not overwrite System 3 recovery `BOOT_RECOVERY_PLAINTEXT.BIN`). Packed header: `0xAA995566` at `0x20`, `0x584C4E58` at `0x24`. COM12 **SYS04_COM12_PASS** on UART `com12_sys04.uart` SHA-256 `d298dc4bf6cc00031372577df32f9c6069a2b17f73c0f06b4e2a3fcef9f9e78b`. Do not start System 5 until that UART file is in the evidence commit.
