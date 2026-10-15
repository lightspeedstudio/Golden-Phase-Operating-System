# Parallel Tensor Architecture — Research Report (T0)

**Date:** 2026-08-13
**Status:** T0 research only. Gates T1–T15 wait for a second, explicit approval.
**Product:** Golden Phase Base OS on PZ7020-SL-C. Not Linux. Not RISC-V host. Not a hosted thread simulation.

This document is the T0 deliverable. Nothing here is permission to invent AXI addresses, dual-core execution, speedup, or cryptographic strength. No `[cite: N]` markers.

---

## Audit snapshot (git)

| Tree | Branch | HEAD | Subject |
|------|--------|------|---------|
| Parent `FullOS` | `escher` | `4b7ab7ab3ffdba745c9ecc96d9a54d11cd6fc273` | `chore(kairos): bump submodule -- identity Gates 5-13 proven on PZ7020` |
| Nested `kairos` | `research/parallel-tensor` | `2c359fa6d3c6b05df1fe8a11379c03061522e035` | `identity: Gates 5-13 - ledger, store, sessions, mod tickets, PZ7020 proof` |

Committed baseline for this report is kairos `2c359fa`. The working tree is **dirty** (USB/GEM/fault/oracle/etc.) and contains **uncommitted** tensor files and a Path-1 bitstream. Those files are recorded in §16 as working-tree observations. They are **not** T0-approved gates and must not be mixed into unrelated dirt.

An earlier plan snapshot cited `main` `71e7097`. That was true at first audit; the live tree moved to `research/parallel-tensor` @ `2c359fa` with identity Gates 5–13 on HEAD.

---

## 1. Subsystem / file inventory

Cited against committed `2c359fa` unless a row says working-tree.

| Subsystem | Path / symbol | Notes |
|-----------|---------------|--------|
| Kernel facade | `src/kernel/kernel.rs` `pub struct Kernel` | Single exclusive `&mut` owner. Fields in §4. |
| Zynq bin entry | `src/bin/kairos_zynq.rs` `rust_main` | Calls `kernel_main::kernel_main()`. |
| Board runtime | `src/board/zynq7000/kernel_main.rs` `kernel_main` → `run_serial_shell` | Cooperative UART shell; `&mut runtime.hosted.kernel`. |
| ARM startup | `src/arch/armv7/startup.S` `_start` | `bl rust_main`. Hang is `wfi`. |
| Linker | `src/arch/armv7/linker.ld` | DDR `@ 0x00100000`. `_stack_top` 64 KiB SVC; `_irq_stack_top` 8 KiB. **No CPU1 stack.** |
| Scheduler (phase) | `src/kernel/scheduler.rs` `Scheduler` | Mutates `PhaseState`. |
| Scheduler (run) | `src/kernel/sched2.rs` `Sched2`, `RunState::{Ready,Running,Blocked,Exited}` | Never reads/writes `PhaseState`. `advance` at line 782. |
| StateClock | `src/stateclock/clock.rs`; geometry `src/stateclock/geometry.rs` `SECTOR_COUNT = 10` | Chronos + kairos. **Not** (M). |
| Phase algebra | `src/core/gphase_core.rs` `PhaseState::{Phi0..Phi3}`, `compose`, `reflect` | Host test `topology_64_triples`: **52 Same / 12 Reflection**. Compose table `[[0,1,2,3],[1,3,3,2],[2,3,3,1],[3,2,1,0]]`. |
| PhaseGuard (health) | `src/kernel/fault_syndrome.rs` `Health::{Valid,Suspect,Fault,Isolate}` | **No `Recover` variant.** Recovery is `fault_recovery` / snapshot restore. |
| PhaseGuard (research) | `src/stateclock/phaseguard_live.rs` | Feature `phaseguard-research`. |
| Statechain | `src/kernel/statechain.rs` `StatechainLedger` | Kernel field when `std` or `baremetal`. |
| Heap / PMM | `src/boot/heap.rs`; `src/kernel/pmm.rs` | Heap from `__heap_start`…`__heap_end` (16 MiB). |
| Identity | `src/kernel/identity/` `Identity` | Feature-gated. Board-proven 2026-08-13. |
| GIC | `src/board/zynq7000/gic.rs` Dist `0xF8F01000`, CPU `0xF8F00100` | SPI 61 (G1P), GEM 54, PPI 29. Target byte `0x01` (CPU0). **No ICDSGIR / SGI.** |
| irqreg | `src/kernel/irqreg.rs` | Feature `irqreg`. **Not a `Kernel` field.** Bounded seq ring, single-core. |
| AXI map | `src/board/zynq7000/memory_map.rs` | G1P `0x40000000`. G64 `0x40001000` labeled **proposed**. |
| G1P driver | `src/board/zynq7000/g1p.rs` | ID `GPL4` (`0x47504C34`), version `0x00020000`, `DEFAULT_TIMEOUT_SPINS = 1_000_000`. |
| G64 driver | `src/board/zynq7000/g64.rs` | Research sidecar; ID `G64S` (`0x47363453`). |
| Caps | `src/kernel/captable.rs` | `RIGHT_READ/WRITE/DELETE/GRANT`. Chronos expiry, delegation. `authorize` is the only allow path. |
| Hash | `src/kernel/crypto/phase_hash.rs` | `ph_boot_session_root`, PH-C/O/S/R. No `PH_tensor-proof` domain on HEAD. |
| Queues | `src/kernel/ipc.rs` | Bounded rings, generation/seq. Single-core. |
| MMU/cache | — | **Absent.** Docs: MMU/caches OFF. No SCTLR/TTBR bring-up in `startup.S`. |
| `TensorState` on HEAD | — | **Does not exist** in commit `2c359fa`. |

Build/packaging: `cargo build --release --bin kairos_zynq --target armv7a-none-eabi`; `bootgen` + `boot.bif` → `BOOT.BIN`. Capture scripts under `scripts/capture_*_com12.ps1`.

---

## 2. CPU1 boot-state audit

**Documented:** `README_PZ7020.md` — “Dual-core ARM Cortex-A9 (**this port uses core 0**)”. `docs/PZ7020_KNOWN_LIMITATIONS.md` — “Only Cortex-A9 core 0 is used; core 1 is not started”; “Flat memory, MMU/caches off”.

**Code (`startup.S`):**

- One `_start`. Sets SVC stack to `_stack_top`, abort/undef/IRQ to `_irq_stack_top`.
- Sets VBAR, clears BSS, `bl rust_main`.
- Never reads MPIDR.
- Never writes `0xFFFFFFF0`.
- Never issues `SEV` or `WFE`. Hang is `wfi`.

**SCU:** `kernel_main.rs` uses SCU **global timer** only (`0xF8F00200` / `0xF8F00204` / `0xF8F00208`) as a polled tick. No SCU control / filter / coherency enable.

**GIC SGI:** `gic.rs` programs Dist + CPU iface, SPI/PPI, CPU0 target. No `ICDSGIR` (`0xF00`), no `send_sgi`.

**Verdict:** CPU1 remains parked by BootROM/FSBL. Kairos does not establish a CPU1 reset vector or stack. UART text `CPU1 ONLINE` without per-core MPIDR counters would be a false claim.

Zynq-7000 AMP mechanism (UG585; **not implemented**): CPU0 writes CPU1 entry to `0xFFFFFFF0`, DSB, `SEV`. Alternative: SLCR `A9_CPU_RST_CTRL` at `0xF8000244`. This report records the mechanism; it does not implement it.

---

## 3. ARM memory-ordering design (hypothesis — not built)

### What exists today

| Primitive | Present? | Where |
|-----------|----------|--------|
| `DSB` / `DMB` | Yes | `src/board/zynq7000/usb.rs` `dsb()` / `dmb()` (USB MMIO only) |
| `ISB` | No | — |
| `SEV` / `WFE` | No | — |
| `WFI` | Yes | hang / idle |
| `LDREX`/`STREX` explicit | No | — |
| `core::sync::atomic` | Sparse | flags/counters (e.g. GEM `AtomicBool`); not a cross-core protocol |
| `cpsie i` / `cpsid i` | Yes | `gic.rs` |
| `alloc::sync::Arc` | Unused on bare metal | Hosted/`std` only. Wrong ownership model for `Kernel`. |

With MMU off, Cortex-A9 treats memory as Strongly-ordered. Exclusive monitors (`LDREX`/`STREX`) to Strongly-ordered or Device memory are **unpredictable** (ARM ARM). Rust `Atomic*` therefore **cannot** be the CPU0/CPU1 protocol until DDR is mapped Normal Inner/Outer Shareable and the SCU is enabled.

PS→PL AXI is **non-coherent GP0** (`C_USE_S_AXI_ACP=0`, HP disabled). MMIO already uses volatile access. If caches are later enabled, AXI buffers still need explicit maintenance or a Device/strongly-ordered mapping.

### Required before any lock-free CPU0/CPU1 claim

1. MMU identity map as Normal Inner/Outer Shareable, L1 (and L2 if used) on, SCU coherency enabled, **or**
2. An explicit non-atomic mailbox using DSB + `SEV`/`WFE`, **not** called lock-free.

Until (1) or (2) is board-proven, T5–T7 are **NO-GO**.

### Proposed ordering (if T5 is later approved)

Shared SPSC ring (irqreg/ipc seq protocol):

```text
Producer writes payload
    → Release-store producer index (or seq_j = n+1)
Consumer Acquire-loads producer index
    → Consumer may read payload
Consumer writes consumed marker
    → Release-store consumer index (seq_j = n+N)
Producer Acquire-loads consumer index before reuse
```

`ONLINE` handshake: CPU1 Release-stores `ONLINE`; CPU0 Acquire-loads it. `shutdown_generation`: Release on bump; Acquire on observe. Do not use `Relaxed` for payload visibility.

Do not claim “lock-free” unless every hot op has a stated progress guarantee, wraparound is safe, concurrency tests pass, **and** physical dual-core execution is proven.

---

## 4. Ownership map (every `Kernel` field)

There is **one** `&mut Kernel` on CPU0. No second-core owner exists. CPU1 column is a design hypothesis for after T5, not code.

Always present:

| Field | CPU0 today | Proposed CPU1 |
|-------|------------|----------------|
| `scheduler: Scheduler` | exclusive mut | no |
| `memory: Memory` | exclusive mut | no |
| `fs: PhaseFs` | exclusive mut | no |
| `phase_index` | exclusive mut | no |
| `network` | exclusive mut | no |
| `trace` | exclusive mut | no |
| `clock: StateClock` | exclusive mut | no |
| `authority: AuthorityState` | exclusive mut | no |
| `anom` | exclusive mut | no |
| `curvature` | exclusive mut | no |
| `phase_math` | exclusive mut | advisory hash/eval only |
| `state_domain` | exclusive mut | no |
| `phasenet` | exclusive mut | no |
| `phi_stability` | exclusive mut | no |

Feature / cfg gated:

| Field | Gate | CPU0 today | Proposed CPU1 |
|-------|------|------------|----------------|
| `physical_memory`, `page_frames` | `pmm` | exclusive mut | no |
| `phase_cache` | `std` or `baremetal` | exclusive mut | no |
| `captable` | `captable` | exclusive mut | no |
| `sched2` | `sched2` | exclusive mut | may **read** epoch snapshot for (M) |
| `ipc` | `ipc` | exclusive mut | no |
| `extmap` | `extmap` | exclusive mut | no |
| `txlog` | `txlog` | exclusive mut | no |
| `netsess` | `netsess` | exclusive mut | no |
| `identity` | `identity` | exclusive mut | no |
| `phaselink`, `statechain` | `std` or `baremetal` | exclusive mut; **commit owner** | advisory hash batch only |
| `object_protection`, `phase_benchmark`, `ai`, `ssh` | `std` | hosted only | n/a |
| `phaseguard`, `phaseguard_pending` | `phaseguard-research` | CPU0 policy | cannot self-trust |

Not on `Kernel`: `IrqRegistry` (board/GIC glue), UART, `G1p`, GIC distributor.

**Rule:** worker results **never** commit Root S. Path: request → evaluate (CPU1 or FPGA) → verify on CPU0 → commit.

Proposed shared objects (not built): bounded SPSC request/result rings (irqreg seq+generation, not `Arc`), `shutdown_generation`, heartbeat. Queue-full returns a structured error. No silent work loss. No reference into another CPU’s stack.

---

## 5. Tensor mathematical specification

### Canonical Layer-0 (frozen)

Source: `src/core/gphase_core.rs`.

\[
S=\{\Phi_0,\Phi_1,\Phi_2,\Phi_3\},\qquad |S|=4
\]

- 2-bit encodings `0..3` via `PhaseState::as_u8` / `from_u8`.
- `compose(a,b) = snap(fold(states[a]+states[b]))`.
- `reflect(x) = 3-x` ≡ `compose(Phi3, x)`.
- 16-entry table (test `compose_matches_table`):

```text
     Φ0 Φ1 Φ2 Φ3
Φ0    0  1  2  3
Φ1    1  3  3  2
Φ2    2  3  3  1
Φ3    3  2  1  0
```

- 64 ordered triples: **52 Same / 12 Reflection / 0 Error** (`topology_64_triples`).
- FPGA/software equivalence: G1P Layer-0 and G64 atlas contracts remain the authority for those maps. **Do not change them.**

### Additive kernel algebra

\[
\mathcal{A}_{\mathrm{kernel}}=S\times M\times V,\qquad |M|=4,\ |V|=16,\ |\mathcal{A}_{\mathrm{kernel}}|=256
\]

This is a **Cartesian product of sets**. It is **not** an algebraic tensor product (no bilinear map, no Kronecker product) unless a later gate defines one. The name “tensor” in this report means packed product state, not \(\otimes\) in the linear-algebra sense.

**(S)** bits 0–1: existing `PhaseState`. Operations only from `gphase_core`.

**(M)** bits 2–3 — **frozen:** `SchedulerPhase` = **sched2 dispatch-epoch modulo 4**.

- Source of truth: `Sched2::advance` (private-timer tick `now`, PPI 29).
- **Do not** reuse `StateClock.sector_id` (`SECTOR_COUNT = 10`) or `PhaseState`.
- Transition: \(m'=(m+\Delta_m)\bmod 4\) with \(\Delta_m=1\) **only** on `Sched2::advance`.
- A future `TensorState` stores a **snapshot** of that epoch. It is not a second clock.

**(V)** bits 4–7: compact class \(v=Q(\mathrm{CapabilitySet})\). See §6. Matching V **never** authorizes.

### Root projection and commuting condition

\[
\pi_S(s,m,v)=s
\]

For every supported tensor transition \(T_A\):

\[
\pi_S\bigl(T_A(x,o)\bigr)=T_S\bigl(\pi_S(x),\pi_O(o)\bigr)
\]

- \(T_S\) = existing `compose` / `reflect` on S.
- \(T_M\) = epoch increment (does not change S).
- \(T_V\) = Q-refresh (does not change S).

Upper-layer scheduler or capability changes must not silently mutate projected S.

### Packed representation (specified, not implemented on HEAD)

```text
TensorState(u32)
  PHASE_MASK = 0b0000_0011
  SCHED_MASK = 0b0000_1100
  CAPS_MASK  = 0b1111_0000
  USED_MASK  = 0x0000_00FF
```

- Reserved bits 8–31 must be zero.
- No public unchecked constructor in sealed builds.
- Inputs are real enum / range-checked values, not arbitrary `u8` holes (all 256 byte values are valid: 4×4×16).
- Canonical serialization: **little-endian**, payload = low byte (`raw as u8`) for hashes, Statechain, MMIO, storage.
- `from_raw` rejects `raw & !0xFF != 0`.
- Tests required at T2: all 256 round-trips; reserved bits; `unpack(pack(s,m,v))=(s,m,v)`; `project_root(pack(s,m,v))=s`; all four S values over all 64 (M,V) pairs.

### Security non-claim

Under the limited assumptions that all 256 states are equally probable, components are independent, and an attacker makes a blind random guess:

\[
P[\text{match }S]=\tfrac14=25\%,\qquad P[\text{match }(S,M,V)]=\tfrac1{256}\approx 0.390625\%
\]

This is **random state-match probability**. It is not cryptographic entropy, not resistance to an informed attacker, not authentication, not authorization, not integrity, and not a proof that S, M, V are statistically independent.

Integrity, if built:

```text
TensorProof = PH_tensor-proof(
    TensorState, Sequence, PreviousRoot,
    BootSessionRoot, AuthorityGeneration, Chronos)
```

Protected operations still require certificate verification, authenticated session, full `authorize`, Statechain continuity, PhaseGuard approval, and replay protection. That PH domain does not exist on HEAD.

Fault-detection hypothesis (unproven): a packed (S,M,V) mismatch can flag inconsistency earlier than S-only. That is consistency checking, not encryption.

---

## 6. Capability-class Q specification

Authoritative record: `src/kernel/captable.rs` — generational object + capability tables, `Rights`, subject, Chronos expiry, delegation parent chain, `authorize` / `delegate` / `grant`.

Live rights bits:

| Bit | Constant |
|-----|----------|
| 0 | `RIGHT_READ` |
| 1 | `RIGHT_WRITE` |
| 2 | `RIGHT_DELETE` |
| 3 | `RIGHT_GRANT` |

These four bits already span **16** values.

**Frozen Q for T2:**

\[
v = Q(\mathrm{CapabilitySet}) = \bigl(\mathrm{OR\ of\ calling\ subject's\ live\ rights}\bigr)\ \&\ 0x0F
\]

Empty set → \(v=0\). This is a **label / fast-rejection mask**, not a grant.

- `CapabilityTable::authorize` remains the only allow path.
- A matching V value alone must never authorize a protected operation.
- Identity `Capability` / `ModTicket` sit **above** captable and must not be folded into V without a later written equation.
- Object kind (File/Queue/Network), slot, generation, expiry, and owner remain in the table, not in the 4-bit class.

---

## 7. AXI address-map evidence

**PCB / Puzhi packs do not assign AXI bases.** Bases come from Vivado Address Editor → `.xsa` / `system.hwh`.

Rust (`memory_map.rs`):

| Constant | Value | Status |
|----------|-------|--------|
| `G1P_AXI_BASE` | `0x40000000` | Sealed BRINGUP_V2 HWH |
| `G64_AXI_BASE` | `0x40001000` | Labeled proposed; research sidecar |

Sealed BRINGUP_V2 (`fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/`):

| Parameter | Value |
|-----------|--------|
| Slave | `g1p_gpl_axi_lite_v2_0` |
| `C_BASEADDR` / `C_HIGHADDR` | `0x40000000`–`0x40000FFF` (4 KiB) |
| Master | `M_AXI_GP0` |
| `C_USE_M_AXI_GP1` | 0 |
| `C_USE_S_AXI_ACP` | 0 |
| `S_AXI_HP0..3` | 0 |
| FCLK0 | 100 MHz (`clk_fpga_0` period 10 ns) |
| IRQ | `IRQ_F2P[0]` level-high → GIC SPI 29 / ID 61 |
| `0x40001000` | **absent** |

G1P Layer-0 map (`REGISTER_MAP.md`): offsets `0x00`–`0x4C` (ID, VERSION, INPUT_A/B/C, COMPOSE, LEFT/RIGHT, PARITY, REFLECT, STATUS, COMMAND, CAPS, IRQ_*, ERROR_CLEAR).

Sealed hashes (`PZ7020_G1P_BRINGUP_V2/SHA256SUMS.txt`):

| File | SHA-256 |
|------|---------|
| `system.bit` | `bb758133f639986fce1d4035ef310571a15a04ced8e8da10f656493f5eb56f91` |
| `zynq_fsbl.elf` | `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` |
| `PZ7020_G1P_BRINGUP_V2.xsa` | `cf4a33161e9ced82be674aecd4f332e00a5b1f279dae8a83ab7193f3ad6996aa` |

STA (sealed, 2026-07-29): WNS **4.671 ns MET** at 100 MHz; WHS 0.053 ns; LUT **506** / 53200 (0.95%); FF **591** / 106400 (0.56%); BRAM/DSP **0**. Slack shows **room**, not a tensor critical path.

G64 research sidecar: second AXI-Lite slave at `0x40001000`–`0x40001FFF` in the G64 XSA only. Identity / ALLSYS / NETSESS `BOOT.BIN` load sealed `system.bit` `bb758133…`, not the G64 bit.

**Forbidden:** `0x40001000 + cpu_id * 0x1000`. That aliases G64 on CPU0 and invents `0x40002000` for CPU1.

No dedicated `report_cdc` deliverable. Sealed design is a single FCLK0 domain.

---

## 8. Accelerator Option A vs Option B

| | Option A — one shared AXI tensor engine | Option B — two logical register banks |
|--|------------------------------------------|----------------------------------------|
| What exists | Sealed production: **one** G1P bank at `0x40000000` | Research G64 sidecar: G1P + G64 on GP0. **Not** a tensor engine. Not in sealed BOOT.BIN |
| Address | Same 4 KiB window; extra offsets only after a new bitstream that still matches HWH | Second base only after a **new XSA** |
| Pros | Lower PL use; centralized verify; matches sealed interconnect | Simpler per-CPU ownership if truly duplicated |
| Cons | Arbitration / backpressure | More registers; still shares AXI unless duplicated; G64 already occupies `0x40001000` in research BD |

**Locked recommendation: Option A** unless a new Vivado BD assigns a real second address. Do not enable GP1 or treat `0x40001000` as tensor MMIO.

Proposed Option A protocol (adapt to real offsets after RTL; **not implemented on HEAD**):

```text
ARM writes request + sequence + tensor
    → barrier
    → COMMAND/VALID
FPGA latches, BUSY
    → evaluate S/M/V (parallel only if logic is independent)
    → result + status + matching sequence + DONE
ARM reads, verifies sequence/owner/status/π_S
    → CPU0 commits or rejects
```

Required: bounded timeout (G1P already has `DEFAULT_TIMEOUT_SPINS`), sequence match, error status, reset generation, stale-DONE reject, backpressure, software fallback, no infinite MMIO poll, volatile + ARM barriers.

Do not assume 1.5 ns critical path, 100–300+ MHz, or zero timing penalty. Those need synth, impl, STA, CDC, board clock, and physical measurement. Combinational dependence of V on S or M on a mux can lengthen the path.

---

## 9. Baseline performance

**Unmeasured on PZ7020 for tensor / kernel-split latency.** No sealed single-core ops/s, median/p95/p99, queue cost, AXI latency, or Statechain-commit latency for this architecture.

Existing board evidence that is **not** a tensor baseline:

| Image | ELF SHA-256 | BOOT.BIN SHA-256 | What it proves |
|-------|-------------|------------------|----------------|
| ALLSYS 20260813 | `2fe5e317946065983b0b9977f9c67db719135a5ccb80ab9666e365d4089ed323` | `bb210e0aceed7a602a37fd67a07bf2b63ce9f6f260732da3bedc151c13342c9e` | Eight-subsystem shell demos on COM12 |
| Identity Gate 13 | `9e00ab9ea3487d4533af23177f1f99c96593cf1625a28d29bb6cc609031f0f3a` | `14f2611e0dc2f6a0db0120d86fc008f84e2e5bae8bf04d56a57da5674f6c3fd3` | Login / mod tickets; bitstream still `bb758133…` |

Splash on those images reports `worker cores: 1`, `SCALAR` backend. Host fib numbers (if present in `HARDWARE_MEASUREMENTS.md`) are hosted Windows CPUs, not dual-core A9.

T13 cannot claim speedup until a frozen CPU0-only measurement exists on the same ELF class, clocks, and method.

---

## 10. Amdahl

\[
\mathrm{Speedup}_{\max}(2)=\frac{1}{(1-P)+P/2}
\]

\(P\) (parallel fraction) is **unknown**. \(N=2\) is the A9 maximum; FPGA assist is a different \(N\) and a different \(P\).

Efficiency \(=\mathrm{Speedup}/2\). Parallelism may raise throughput and worsen single-op latency. Report both when measured. **No speedup claim in this report.**

---

## 11. Risk register

| ID | Risk | Why it is real | Mitigation |
|----|------|----------------|------------|
| R1 | `LDREX`/`STREX` with MMU off | ARM unpredictable; atomics compile anyway | T5 blocked until MMU/SCU or DSB mailbox |
| R2 | Dirty working tree | Unrelated USB/GEM/fault edits + uncommitted tensor files | Tensor commits must not mix that dirt |
| R3 | No GIC SGI | `gic.rs` has no ICDSGIR | CPU1 IPI is new programming |
| R4 | PhaseGuard naming | `Health` has no `Recover` | Cite `Health`; restore via snapshot path |
| R5 | G64 vs sealed bit | `0x40001000` is research; sealed bit is `bb758133…` | Option A until a new XSA |
| R6 | `&mut Kernel` | Whole kernel is one exclusive owner | Split only after T3–T4; no `Arc` |
| R7 | irqreg not on `Kernel` | Lives beside GIC | Do not invent a second-core field |
| R8 | Identity vs tensor | Identity has its own ledger | Tensor must not become a second identity ledger |
| R9 | False `CPU1 ONLINE` | Easy to print from CPU0 | Require MPIDR + counters |
| R10 | Cache/AXI | GP0 non-coherent; caches off today | Explicit maintenance if caches go on |
| R11 | ABA / wrap | Rings use `u64` seq in irqreg | Keep generation+seq; test wrap |
| R12 | PhaseGuard Complete FT | Claimed on unmerged feature branch | `main` / this baseline does not claim C1–C12 |

---

## 12. Go / no-go

| Gate | Verdict now |
|------|-------------|
| T0 research doc | **GO** (this file) |
| T1 freeze baseline + reproduce current board image | **GO after approval** — must not mix dirty WT |
| T2 `TensorState` + 256 tests | **GO after approval** — additive; do not touch Layer-0 tables |
| T3–T4 CPU0 split + host queues | **CONDITIONAL GO** — keep `&mut Kernel` until split is proven; no `Arc` |
| T5–T7 CPU1 | **NO-GO** until MMU/SCU/atomics (or DSB mailbox) is implemented and board-proven |
| T8–T11 FPGA tensor | **NO-GO** until a Vivado-assigned map exists, Layer-0 contract is preserved, and a frozen CPU0 baseline is measured |
| T12–T15 integration / perf / seal | Blocked on T5 and T8 |

Working-tree Path-1 files (§16) do **not** change this table. T0 does not rubber-stamp uncommitted gates.

---

## 13. Gate-by-gate plan (T1–T15)

After every gate: files changed, math behavior, ownership/sync, tests, evidence class, failures, performance, commit status, whether physical hardware was used, whether any result was synthetic. Do not claim later gates early.

| Gate | Work | Expected files | Tests | Evidence class | Forbidden claims |
|------|------|----------------|-------|----------------|------------------|
| T1 | Freeze this spec; reproduce a hashed board image | evidence SHA; tests-as-spec census | 52/12 still holds | `MATH_REFERENCE_TEST` + `SEALED_BOOT_PROOF` | dual-core |
| T2 | `TensorState` newtype + serialize + π_S | e.g. `src/kernel/tensor_state.rs` | 256 round-trips; reserved bits; π_S | `HOST_UNIT_TEST` | AXI, CPU1, speedup, crypto |
| T3 | Document CPU0 commit vs evaluate **without** removing `&mut Kernel` | design + internal fns | commit path still exclusive | `HOST_UNIT_TEST` | SMP |
| T4 | Bounded SPSC on **host** using irqreg seq+generation | host queue module/tests | backpressure; no `Arc<Kernel>` | `HOST_CONCURRENCY_MODEL` | “works on A9” |
| T5 | CPU1 stack, MPIDR, `0xFFFFFFF0`, `SEV`, MMU/SCU **or** DSB mailbox | `startup.S`, `linker.ld`, cpu1 stub | cross-build then board | `BAREMETAL_CROSS_BUILD` then `ON_BOARD_CPU1_TEST` | ONLINE without MPIDR |
| T6 | Physical dual-core queue evidence | same | order, wrap, no loss | `ON_BOARD_DUAL_CORE_CONCURRENCY_TEST` | coherency if MMU still off |
| T7 | CPU1 software tensor worker; CPU0 verify+commit | kernel + board | CPU1 result ignored until CPU0 commit | `ON_BOARD_CPU1_TEST` | Root commit from CPU1 |
| T8 | Option A RTL + register protocol **or** new XSA | Vivado + `g1p.rs` | G1P 52/12 still holds | `RTL_SIMULATION` then `FPGA_SYNTHESIS_REPORT` / `FPGA_IMPLEMENTATION_REPORT` / `STATIC_TIMING_REPORT` | invented `0x40002000` |
| T9 | STA/CDC/util; board clock | reports preserved | WNS, LUT, FF, BRAM, DSP | `FPGA_*` | sim-only timing |
| T10 | AXI driver timeout/seq/fallback | `g1p.rs` | stale DONE, BUSY timeout | `ON_BOARD_FPGA_TENSOR_TEST` | G64 as tensor |
| T11 | CPU0/CPU1/FPGA integration | kernel + PL | SW↔HW vectors | `LIVE_KERNEL_PARALLEL_TEST` | “hardware proven” as a collapse |
| T12 | PhaseGuard: VALID allow; SUSPECT stop new privileged work + CPU0 verify; FAULT/ISOLATE disable CPU1/FPGA mutation; restore via snapshot | fault_syndrome policy | injection host then board | `HOST_UNIT_TEST` then board | `Health::Recover` exists |
| T13 | Frozen baseline vs parallel metrics | measurement + evidence md | same ELF class | `ON_BOARD_BASELINE` | Amdahl with unknown P |
| T14 | Sealed/research feature split | Cargo features default OFF | identity-off hash compare | `SEALED_BOOT_PROOF` | tensor is default |
| T15 | Exact-image PZ7020 evidence | SHA of ELF, BOOT.BIN, bitstream | UART + hashes | `ON_BOARD_*` | print-only PASS |

Fault policy (all later gates): worker/FPGA result is advisory until verified → reject invalid → record fault → retry on CPU0 reference path → repeated failure quarantines accelerator or CPU1 → Layer-0 kernel continues.

---

## 14. Exact non-claims

1. Dual-core execution is **not** happening in committed `2c359fa`.
2. There is **no** tensor accelerator in the **sealed** bitstream `bb758133…`.
3. `0x40001000` is **not** sealed tensor MMIO.
4. `1/256` is **not** cryptographic strength.
5. Host atomics / host threads are **not** Cortex-A9 SMP proof.
6. UART text without MPIDR counters is **not** `CPU1 ONLINE`.
7. G1P WNS 4.671 ns is **not** tensor timing.
8. PhaseGuard Complete FT (C1–C12) is **not** claimed on this baseline.
9. `Health` has **no** `Recover` variant.
10. `Sched2` epoch is **not** `StateClock.sector_id`.
11. Cartesian \(S\times M\times V\) is **not** an algebraic tensor product until bilinear maps are defined.
12. Matching V **never** grants a capability.
13. `alloc::sync::Arc` is **not** appropriate as the Kernel sharing model.
14. Uncommitted working-tree tensor files (§16) are **not** T0-approved, **not** a sealed image, and **not** CPU1 proof.
15. Splash `worker cores: 1` is the honest current parallel fraction for phase-math on bare metal.

---

## 15. Evidence-class glossary

| Class | Meaning |
|-------|---------|
| `MATH_REFERENCE_TEST` | Host test of Layer-0 tables / 64 triples. |
| `HOST_UNIT_TEST` | Hosted Rust test; no board. |
| `HOST_CONCURRENCY_MODEL` | Host threads/queues modeling a protocol. Not A9. |
| `BAREMETAL_CROSS_BUILD` | `armv7a-none-eabi` link succeeded. Not execution. |
| `RTL_SIMULATION` | Verilog sim. Not silicon. Not timing closure. |
| `FPGA_SYNTHESIS_REPORT` | Synthesis completed. |
| `FPGA_IMPLEMENTATION_REPORT` | Place-and-route completed. |
| `STATIC_TIMING_REPORT` | WNS/TNS for a named clock. |
| `ON_BOARD_CPU1_TEST` | PZ7020 proof CPU1 ran, with MPIDR counters. |
| `ON_BOARD_DUAL_CORE_CONCURRENCY_TEST` | Overlapping work + measurable per-core counters. |
| `ON_BOARD_FPGA_TENSOR_TEST` | Physical UART/MMIO vs exact bitstream hash. |
| `LIVE_KERNEL_PARALLEL_TEST` | Integrated kernel path on board. |
| `ON_BOARD_BASELINE` | Frozen single-core latency on a hashed ELF. |
| `SEALED_BOOT_PROOF` | Default-OFF image hash unchanged (or change explained). |
| `PHYSICAL_HARDWARE_TEST` / `ON_BOARD_SOFTWARE_TEST` | Existing house classes for prior subsystem gates. |

Never collapse these into “hardware proven.”

---

## 16. Working-tree observations (not T0 claims)

Present on disk at report time, **uncommitted**, mixed with unrelated dirty files. Recorded so they are not hidden. **T0 does not approve them.**

| Path | Observation | Allowed class if later reviewed |
|------|-------------|-------------------------------|
| `src/kernel/tensor_state.rs`, `tensor_ring.rs` | Untracked modules; `Cargo.toml` has `tensor = []`; `kernel/mod.rs` gates them | `HOST_UNIT_TEST` only after review |
| `tests/tensor_gate1_sealed_tests.rs`, `tensor_gate2_tests.rs` | Untracked | same |
| `fpga/zynq/rtl/gpl_axi_bringup_v2.v` | Modified vs HEAD | RTL, not silicon by itself |
| `fpga/zynq/sim/tb_gpl_axi_bringup_v2_tensor.v` | Untracked sim | `RTL_SIMULATION` |
| `fpga/zynq/release/PZ7020_G1P_PATH1_TENSOR/` | Untracked release: `system.bit` `e987711865e49fbf3b4a36b728d95d02e8410c8359db5ddc88c8df982f68c4f6`, ELF `c9367a4d…`, `BOOT.BIN` `8e94132b…`, FSBL still `8b71aa70…`; STA WNS **4.330 ns MET**; LUT 575 / FF 661 | `FPGA_IMPLEMENTATION_REPORT` / `STATIC_TIMING_REPORT` — **not** the sealed identity bit |
| `fpga/zynq/evidence/kairos_zynq_boot/com12_tensor_path1_20260813_115710.txt` | UART: `TNSR` at `0x40000000+0x100`, Layer-0 still `GPL4`; `compose 0x98*Phi1 -> 0x99 (CPU0 commit ok)`; `worker cores: 1` | physical UART for that **research** BOOT.BIN — **not** CPU1, **not** sealed `bb758133…` |

If a later session wants to accept Path 1, that is a **post-T0** gate review with clean commits, not this document claiming T8 done.

---

## Citations

- Kairos HEAD: `2c359fa6d3c6b05df1fe8a11379c03061522e035`
- Parent FullOS: `4b7ab7ab3ffdba745c9ecc96d9a54d11cd6fc273` on `escher`
- Math contract: `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md`
- Board port: `README_PZ7020.md`, `docs/PZ7020_KNOWN_LIMITATIONS.md`
- Identity Gate 13: `fpga/zynq/evidence/IDENTITY_STATUS_2026-08-13.md`
- Sealed G1P: `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/{SHA256SUMS.txt,timing_summary.rpt,utilization.rpt,REGISTER_MAP.md}`
- Phase algebra: `src/core/gphase_core.rs`
- GIC: `src/board/zynq7000/gic.rs`
- Startup: `src/arch/armv7/startup.S`, `linker.ld`
