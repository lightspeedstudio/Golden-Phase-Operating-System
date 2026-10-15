# ARMv7-A MMU isolation audit (Gate 1)

**Branch:** `feature/armv7-mmu-app-isolation-v1`  
**Parent:** `8ae130c` (kairos `main` after USB HID fast-forward). Plan text named `09f120c`; that is the pre-HID parent. This audit is measured from `8ae130c`.  
**Image:** Utility ELF only. **Not flashed. Not isolation. Not a board proof.**

Build:

```
cargo build --release --bin kairos_zynq --no-default-features --features "zynq-a9,utility" --target armv7a-none-eabi
```

Linker: Xilinx `arm-none-eabi-gcc` (`.cargo/config.toml`).

| Field | Value |
|-------|--------|
| ELF SHA-256 | `494F393B8D3A208F2228C945464485558F39A1E10DA9D3B40DADE0BCA0BEE4BE` |
| Size | 2 550 428 bytes |
| Class | ELF32 LE EXEC, EABI5 soft-float |
| Entry | `0x00100060` |
| LOAD | VA/PA `0x00100000`, FileSiz `0x1e347c`, MemSiz `0x1ea118`, **Flg RWE** |

Identity MMU ≠ application isolation. This document records what exists so page tables can be built without guessing.

## 1. Physical address map (nm + readelf)

| Symbol / region | Address | Notes |
|-----------------|---------|--------|
| LOAD / DDR image | `0x00100000` | FSBL handoff |
| `.note.gnu.build-id` | `0x00100000`–`0x00100024` | |
| `vectors` | `0x00100040` | VBAR; `.text.boot` |
| `_start` | `0x00100060` | same as ELF entry |
| `.text` | `0x00100040` size `0x1afae4` | ends `0x002afb24` |
| `rust_main` | `0x001002dc` | |
| `.rodata` | `0x002afb40` size `0x031dd8` | |
| `.data` | `0x002e1920` size `0x001b5c` | |
| `__bss_start` | `0x002e4000` | |
| `__bss_end` | `0x002ea118` | |
| `_stack_top` | `0x002fa120` | CPU0 SVC 64 KiB |
| `_irq_stack_top` | `0x002fc120` | abort/undef/IRQ **share** 8 KiB |
| `_mmu_l1` | `0x00300000` | 16 KiB, 16 KiB aligned |
| `_cpu1_stack_top` | `0x00308000` | |
| `_cpu1_irq_stack_top` | `0x00309000` | |
| `__heap_start` | `0x00309000` | 16 MiB `boot::heap` |
| `__heap_end` | `0x01309000` | PMM base (already 4 KiB aligned) |
| DDR high | `0x1FFFFFFF` | `memory_map::DDR_HIGH` |
| Linker window | `0x00100000` + `0x0FF00000` | ends `0x10000000`; heap/PMM extend past that in the **logical** DDR map to `0x20000000` |

PMM (`zynq7000_usable_region`): `__heap_end` .. `DDR_HIGH+1` = `0x01309000` .. `0x20000000`.

### 1 MiB section overlap (why L2 is required)

| 1 MiB section | Contents |
|---------------|----------|
| `0x00100000` | vectors + most of `.text` |
| `0x00200000` | rest of `.text` **and** all `.rodata`, `.data`, `.bss`, SVC/IRQ stacks |
| `0x00300000` | `_mmu_l1`, CPU1 stacks, start of heap |
| `0x00300000`–`0x01300000` | heap |
| `0x01300000`–`0x1FFFFFFF` | PMM |

Kernel text and kernel data share section `0x00200000`. A 1 MiB section cannot be both privileged-RX and privileged-RW+XN. Gate 2+ must use **L2 small pages** for the kernel image, not section AP alone.

Linker bounds **were** determined from this ELF. Gate 1 does not stop.

## 2. Exception vectors and CPU modes

[`src/arch/armv7/startup.S`](../../src/arch/armv7/startup.S):

- VBAR = `vectors` (`0x00100040` on this ELF).
- Reset → SVC, `sp = _stack_top`.
- Abort/Undef/IRQ mode stacks all `_irq_stack_top` (shared).
- FIQ: no stack; vector hangs.
- After boot: **SVC (0x13)**. IRQs unmasked after GIC init (`cpsie i`).
- **USR/SYS are not used.** No `leave_user` / `subs pc, lr` user return.

| Vector | Today |
|--------|--------|
| Undef | UART `U`, hang |
| SVC | **hang** |
| Prefetch abort | UART `P`, hang |
| Data abort | UART `D!` + DFAR/DFSR hex, hang |
| IRQ | `srsdb` / `kairos_board_irq_dispatch` / `rfeia` |
| FIQ | hang |

Syscall path is Rust `kernel::syscall::dispatch` (Phi2→Phi3), **not** ARM `svc`.

## 3. Current MMU (identity, not isolation)

[`src/board/zynq7000/cpu1.rs`](../../src/board/zynq7000/cpu1.rs) `enable_mmu_scu_cpu0` (`feature = "cpu1"`, pulled by `utility`):

- Short-descriptor L1, 4096 × 1 MiB identity sections.
- DDR `0`..`DDR_HIGH` : Normal Inner/Outer WBWA, shareable, **AP=0b11 full**.
- Above DDR: Shareable Device + XN, **AP=0b11 full**.
- TTBCR = 0 (N=0, **TTBR0 only**; TTBR1 unused). Kernel is at **low** DDR so TTBR1 high-half does not fit.
- DACR = `0xFFFFFFFF` — **all domains Manager; AP bits are ignored.**
- No ASID / CONTEXTIDR / PXN programming.
- UART: `MMU: CPU0 identity (DDR Normal-S WBWA, MMIO Device); SCU on`

CPU1 ([`cpu1_entry.S`](../../src/arch/armv7/cpu1_entry.S)) loads TTBR0 from `CPU1_MAILBOX` (same table), DACR Manager, SCTLR M|C|I|Z.

## 4. MMIO

| Device | Base |
|--------|------|
| UART0 | `0xE0000000` |
| USB0 | `0xE0002000` |
| GEM0 | `0xE000B000` |
| G1P | `0x40000000` |
| G64S | `0x40001000` |
| SLCR | `0xF8000000` |
| SCU | `0xF8F00000` |
| GIC CPU | `0xF8F00100` |
| GIC dist | `0xF8F01000` |
| Global timer | `0xF8F00200` |
| CPU1 BootROM vector | `0xFFFFFFF0` |

## 5. DMA / USB / framebuffer

GEM `WIRE_*` rings and USB `CTRL_QH` / qTDs / setup/data buffers are **`static mut` in kernel BSS**, not PMM. Cache clean/invalidate exists in `gem.rs`. **No framebuffer allocator.** DMA masters can write those BSS buffers regardless of user AP; v1 does **not** claim DMA protection.

## 6. Process, PhaseStack, MemoryEnvelope, Sentinel

- `Process` is a phase-state record (no TTBR, no stacks, no register file).
- PhaseStack launch runs **in-kernel PhaseVM**. Native ELF execute returns `NativeIsolationUnavailable`; `mmu_wx_status() == Blocked`.
- No `MemoryEnvelope` type. `ExecEnvelope.mem_bytes` is a logical VM size.
- `sentinel_contain_task`: `kill_process`, captable close, optional sched2 exit, identity session revoke, disposition, statechain. Docs: quarantine/isolate **do not claim address-space containment**. String `MMU_VIOLATION` is absent.

## 7. Cache / SMP

CPU0: SCU enable, ACTLR.SMP|FW, L1 D-cache invalidate, I-cache+TLB invalidate, then SCTLR M|C|Z|I. CPU1 copies that after reading mailbox TTBR0. Isolated apps in v1 are **CPU0-only**. CPU1 must not keep a TTBR that identity-maps the PMM user region.

## 8. Non-claims (this audit)

Not user-mode. Not Client DACR. Not per-process TTBR. Not W^X. Not guard pages. Not SVC syscalls. Not hardware proof of isolation.

## 9. Gate 1 result

Linker symbols and reserved regions **are determined**. Proceeding to Model A (per-process TTBR0, TTBCR.N=0, L2 for kernel image and user windows, DACR Client on CPU0) is safe from an address-map standpoint. Utility without `mmu-app-isolation` stays the identity+Manager fallback.

## 10. C10 root cause -- kernel `.data` mapped read-only (fixed 2026-08-16)

`BOOT_MMU_ISOLATION_20260815` reset-looped on the board with:

```
D!002e5f90/0000080f
MMU abort class=PERMISSION far=002e5f90 fsr=0000080f
C10_WATCHDOG_RESET
```

`arm-none-eabi-nm` on that exact ELF
(`B30D943511FC4684814C31D228603EF394D5F69B7D594C6CF1CA4AEB695233F2`):

| symbol | address |
| --- | --- |
| `_text_end` | `0x002b2dd4` |
| `_rodata_start` | `0x002b2de0` |
| `_rodata_end` | `0x002e51f8` |
| `_data_start` | `0x002e5200` |

`_rodata_end` and `_data_start` are 8 bytes apart, so both live in page
`0x002e5000`. `mmu::page_kind` classified a page from its **base address**:
`0x002e5000 >= _rodata_start && 0x002e5000 < _rodata_end` matched first, so the
whole page -- including live `.data` at `0x002e5200..0x002e5fff` -- was mapped
`kernel_ro_page` (AP2=1, AP[1:0]=0b01, privileged read-only; descriptor
`0x002e565f`). The FAR `0x002e5f90` sits in that page, so the **first ordinary
kernel write to a `.data` static** took a permission fault. FSR `0x80f` decodes
as WnR=1 (write), status `0xF` = page permission fault -- consistent.

Nothing about user mode, DACR or the abort policy was involved. The first
kernel abort was then classified `WatchdogReset`, which hit SLCR `0xF8000200`
and produced the PL reconfiguration blink loop.

`BOOT_MMU_ISOLATION_C10HALT_*` had the same defect
(`_rodata_end == _data_start == 0x002e64c0`, page `0x002e6000`), which is why it
stalled inside `report_g1p` instead of reboot-looping -- `HaltIsolated` stopped
the SLCR storm without addressing the map.

### Fix

1. `mmu::page_kind` now classifies each page by **overlap**, testing the
   writable window first, so no page containing a writable byte can be mapped
   read-only.
2. `linker_iso.ld` page-aligns the `.rodata` and `.data` output sections
   (`ALIGN(4096)` on the section header -- a bare `. = ALIGN()` between sections
   is ignored under `> ddr` region allocation), so the windows are disjoint per
   page and W^X is exact rather than merely safe.
3. `isolation::enable` and `mmuiso status` print the live layout and
   `aligned=YES/NO`, so the board reports its own map.

Post-fix layout (`AFCC7C9F060C5307060C8DA3625585E3A76F2C8AD93035D06D6A4F2EBF4A0DCE`):
`_text_end=0x002b2fb8`, `_rodata_start=0x002b3000`,
`_rodata_end=_data_start=0x002e5000` -- all boundaries page-aligned.

### Regression cover

`tests/mmu_isolation_host.rs`, 29 passing. The faulting ELF is kept as the
fixture `MEASURED_ISO_20260815` with `ISO_20260815_ABORT_FAR = 0x002e5f90`.
Negative control: restoring the old base-address classification fails
`shared_rodata_data_page_is_writable` and `kernel_data_is_writable_and_xn` with
`desc 0x002e565f`, and passes with the fix.

**Still host-only. Not Cortex-A9 proof -- COM12 pending.**

## 11. Hardware result -- full isolation matrix PASS (A05 replacement 2026-08-16)

PZ7020 COM12, source `d0b69e1d8ee3ef43e3912a3731ada7023e9e55fd`,
`--features "zynq-a9,utility"`. BOOT SHA-256
`2d636c78622af6c618402f0bc4462f625cbccef793a0da1f549e82c9c4d0fa9a`
(ELF `b61a70f07f54b983e63840a51e5ee781abfa1e9d9d132bf6363a563defbee179`),
gold PATH1_G64 `system.bit`
`d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829`,
FSBL `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874`.
Raw UART `com12_mmuiso_matrix_20260816_114324.uart` SHA-256
`b15168ef9ba8396717cb98b8d0defeadf1e00990639f82726edbe57840b6547a`
(9193 bytes) in
`fpga/zynq/evidence/kairos_zynq_boot/BOOT_A05_REPLACE_20260816/`.
Host sidecar `*.host.jsonl` is not UART. A03: `ORDERED_SINGLE_RUN`
run_id `c77a4558`. One `mmuiso matrix`, ten PASS rows in `payloads::matrix`
order, one `mmuiso matrix done`. `gpos>` is transport only.

Frozen GOAL5 `com12_matrix.txt` SHA-256
`23cea7b111eed5ebe678f8b855345b7029f5b7073713d7f84ede8b3f12465f45`
stays REVIEW_REQUIRED (A03 `DUPLICATE_OR_MISSING_DONE`). Do not re-parse it.

`mmuiso matrix` -- **10/10 PASS** (A05 replacement UART).

| Probe | FAR | FSR | Decode | Result |
| --- | --- | --- | --- | --- |
| legal-read | -- | -- | `LEGAL_OK` via SVC, `rc=0` | PASS |
| code-read | -- | -- | no abort, `rc=0` | PASS |
| kernel-read | `00100000` | `0000000f` | permission, page, read | PASS |
| kernel-write | `00100000` | `0000080f` | permission, page, write | PASS |
| peer-read | `1fffa000` | `0000000f` | permission, page, read | PASS |
| peer-write | `1fffa000` | `0000080f` | permission, page, write | PASS |
| mmio-write | `e0000030` | `0000080d` | permission, **section**, write | PASS |
| code-write | `1ff75000` | `0000080f` | permission, page, write | PASS |
| execute-data | `1ff82000` | `0000000f` | **prefetch**, permission | PASS |
| stack-overflow | `1ff87ffc` | `00000007` | **translation** (unmapped) | PASS |

LAYOUT `text=00100040..002d6c84 ro=002d7000..0030d000 data=0030d000..00315098`.
`PAGE_ALIGNED_BOUNDARIES: YES`. `DMA_PROTECTION: NO`.

### Goals

| # | Goal | Status |
| --- | --- | --- |
| 1 | Apps cannot R/W kernel memory | PASS (kernel-read, kernel-write) |
| 2 | Apps cannot R/W another app's memory | PASS (peer-read, peer-write) |
| 3 | Apps cannot access raw MMIO | PASS (mmio-write) |
| 4 | W^X for app memory | PASS (execute-data) |
| 5 | App code RO+X | PASS (code-read + code-write) |
| 6 | Guard pages | PASS (stack-overflow) |
| 7 | Real ARM abort, never `RECOVERED` | PASS (fault probes ISOLATED) |

### Fixes this required

1. `mmu::page_kind` overlap classification (section 10) -- kernel `.data` was
   mapped read-only.
2. `linker_iso.ld` page-aligned `.rodata`/`.data` output sections (section 10).
3. **L1 table alignment.** The PMM base is `__heap_end` = `0x01321000`, which
   is `= 0x1000 (mod 0x4000)`, and the buddy allocator returns
   `base + k * 16384`, so `alloc(4)` could never yield a 16 KiB-aligned table.
   With `TTBCR.N == 0` TTBR0 takes the base from bits[31:14] and reads
   bits[13:0] as attributes, so `switch_ttbr` walked a table 4 KiB below the
   real one -- garbage descriptors, an untranslatable abort vector, and a
   silent double-fault hang with no output. Fixed by over-allocating
   `L1_ALLOC_PAGES` (8) and carving an aligned table via `align_up_l1`,
   with `l1_base_ok` failing closed instead of hanging.
4. **Prefetch-abort PC.** `abort_pabt` ran `bl uart_crash_putc` before
   `sub r2, lr, #4`; the `bl` clobbers `LR_abt`, so the reported PC was the
   address of the `bl` itself (`0010010c`). Banked state is now captured
   before the call.
5. Duplicate raw FIFO stores removed from both abort vectors (`DD!` -> `D!`),
   and `run_matrix` now prints status *after* the run so `LAST_FAULT` is live.

### Non-claims (unchanged)

No DMA or IOMMU protection -- GEM/USB rings are kernel BSS and DMA masters
ignore AP bits. Apps are CPU0-pinned; there is no SMP TLB shootdown. Host
tests (35 passing in `tests/mmu_isolation_host.rs`) are not Cortex-A9 proof.
The table above is the A05 replacement COM12 matrix only. Not DMA. Not IOMMU.
GOAL5 `com12_matrix.txt` remains REVIEW_REQUIRED.
