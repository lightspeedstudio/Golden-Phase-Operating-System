# PZ7020 Feature Audit — 2026-07-30

Evidence-driven audit of every Golden Phase OS feature exposed on the PZ7020.

- **Build under audit:** `--no-default-features --features zynq-a9 --target armv7a-none-eabi`
  (`zynq-a9` = `baremetal`; **no** `std`, **no** `ssh`, **no** `bm-net`).
- **Physical evidence baseline:** `fpga/zynq/evidence/kairos_zynq_boot/com12_capture_20260730_132633.txt`
  (sha `c52ff347c03df0d242b5cb8399c07d356f2e9ea056c7dac37d74c9200d98c09c`).
- **Method:** classification is derived from the actual source + compile-time
  gates (authoritative for *which handler runs on bare metal*). Physical status
  is separate: only `help` and `state` have a live capture so far. Everything
  else marked "PRESENT BUT UNTESTED" awaits a harness run
  (`scripts/pz7020_test_harness.ps1` + `pz7020_test_plan_default.txt`).

## Classification legend

`PHYSICALLY VERIFIED` · `SOFTWARE VERIFIED` · `PRESENT BUT UNTESTED` · `PARTIAL`
· `UNSUPPORTED` · `HOSTED ONLY` · `STUB — MUST NOT CLAIM PASS` · `FAILED`

> On bare metal, `std`/`ssh`-gated commands fall through to an honest
> "hosted std build required" / hosted message — they are **HOSTED ONLY**, not
> stubs claiming success and not failures.

---

## Phase 1 — Shell command inventory (38 commands from the physical `help`)

| Command | Source handler | Gate (effect on zynq-a9) | Expected bare-metal | Classification | Test cmd | Evidence |
|---------|----------------|--------------------------|---------------------|----------------|----------|----------|
| help | `userland/commands/help.rs` | none → runs | prints catalog | **PHYSICALLY VERIFIED** | `help` | capture 132633 |
| about | `userland/commands/about.rs` | none → runs | Zen text | PRESENT BUT UNTESTED | `about` | pending |
| clear | `userland/commands/clear.rs` | none → runs | clears screen | PRESENT BUT UNTESTED (denylisted in harness) | `clear` | pending |
| version | `userland/commands/version.rs` + `splash.rs` | none → runs | version + **ARM** target label | PRESENT BUT UNTESTED | `version` | pending |
| state | `userland/commands/state_cmd.rs` | none → runs | StateClock/process view | **PHYSICALLY VERIFIED** | `state` | capture 132633 |
| clock | `userland/clock_cmd.rs` | none → runs | StateClock view/controls | PRESENT BUT UNTESTED | `clock` | pending |
| phasenet | `userland/phasenet_cmd.rs` | `std` → **fallback** | "hosted std build required" | **HOSTED ONLY** | `phasenet` | pending |
| phaselink | `userland/phaselink_cmd.rs` (std) / `phaselink_baremetal_cmd.rs` (bm-net) | std off + bm-net off → **fallback** | hosted/needs-bm-net message | **HOSTED ONLY** | `phaselink` | pending |
| phaseperf | `userland/phaseperf_cmd.rs` | `std` → **fallback** | hosted message | **HOSTED ONLY** | `phaseperf` | pending |
| phi-stability | `userland/phi_stability_cmd.rs` | none → runs | stress status/run | PRESENT BUT UNTESTED | `phi-stability` | pending |
| phase-benchmark | `userland/phase_benchmark_cmd.rs` | `std` → **fallback** | hosted message | **HOSTED ONLY** | `phase-benchmark` | pending |
| net | `userland/net_cmd.rs` | none → runs (no GEM driver) | no-device/discovery | PARTIAL / UNSUPPORTED (no GEM) | `net` | pending |
| regs | `userland/commands/regs` (via `dispatch`) | none → runs | VM regfile if a program ran | PRESENT BUT UNTESTED | `regs` | pending |
| ps | `userland/ps.rs` | none → runs | process list | PRESENT BUT UNTESTED | `ps` | pending |
| whoami | `userland/commands/whoami_cmd.rs` | none → runs | authority/visibility | PRESENT BUT UNTESTED | `whoami` | pending |
| host | `userland/commands/host_cmd.rs` | none → runs | machine identity/devices | PRESENT BUT UNTESTED | `host` | pending |
| spawn | `userland/commands/spawn_cmd.rs` | none → runs | create named process | PRESENT BUT UNTESTED | `spawn t` | pending |
| kill | `userland/kill.rs` | none → runs (**destructive**) | stop pid | PRESENT BUT UNTESTED (harness denylist) | `kill <pid>` | pending |
| trace | `userland/commands/trace_cmd.rs` | none → runs | kernel trace events | PRESENT BUT UNTESTED | `trace` | pending |
| isa | `userland/commands/isa.rs` | none → runs | ISA reference table | PRESENT BUT UNTESTED | `isa` | pending |
| run | `userland/run.rs` / `run_paste.rs` | none → runs (**paste mode**) | run pasted/named program | PRESENT BUT UNTESTED (needs paste seq) | `run` + program + `.end` | pending |
| dump | `userland/dump.rs` | none → runs | combined state/ps/regs/trace | PRESENT BUT UNTESTED | `dump` | pending |
| mem | `userland/mem.rs` | none → runs | page phase tags | PRESENT BUT UNTESTED | `mem` | pending |
| log | `userland/log.rs` | none → runs | topology reflection log | PRESENT BUT UNTESTED | `log` | pending |
| exit | `dispatch` `ShellAction::Exit` | none (**destructive**: ends shell) | leaves shell | PRESENT BUT UNTESTED (harness denylist) | `exit` | pending |
| dashboard | `userland/dashboard.rs` | none → runs | operator dashboard | PRESENT BUT UNTESTED | `dashboard` | pending |
| top | `userland/top.rs` | none → runs | live phase monitor (N frames) | PRESENT BUT UNTESTED | `top 1` | pending |
| watchdog | `userland/watchdog_view.rs` | none → runs | statechain watchdog view | PRESENT BUT UNTESTED | `watchdog` | pending |
| phase | `userland/terminal_cmd.rs` | none → runs | subsystem phase counts | PRESENT BUT UNTESTED | `phase` | pending |
| topo | `userland/terminal_cmd.rs` + `topo_view.rs` | none → runs | topology events | PRESENT BUT UNTESTED | `topo` | pending |
| curvature | `userland/curvature_cmd.rs` | none → runs | curvature watchdog | PRESENT BUT UNTESTED | `curvature` | pending |
| root | `userland/terminal_cmd.rs` + `root_view.rs` | none → runs (content stub) | state roots (stub) | STUB — MUST NOT CLAIM PASS | `root` | pending |
| audit | `userland/terminal_cmd.rs` + `audit_cmd.rs` | none → runs (content hosted) | statechain audit (hosted/stub) | HOSTED ONLY / STUB | `audit` | pending |
| checkpoint | `userland/terminal_cmd.rs` + `checkpoint_cmd.rs` | none → runs (crypto hosted) | "requires hosted crypto" | HOSTED ONLY / STUB | `checkpoint` | pending |
| ai | `userland/ai_cmd.rs` | `std` → **fallback** | "ai: hosted std build required" | **HOSTED ONLY** | `ai` | pending |
| ssh | `userland/ssh_cmd.rs` | `ssh` → **fallback** | hosted message | **HOSTED ONLY** | `ssh` | pending |
| anom | `userland/commands/... anom_cmd` | none → runs | ANOM security mode | PRESENT BUT UNTESTED | `anom` | pending |

## Phase 1 — Boot-time subsystem inventory

| Subsystem | Source module | Zynq dependency | Boot line (observed) | Classification |
|-----------|---------------|-----------------|----------------------|----------------|
| StateClock | `stateclock/` | none | "chronos + kairos clocks" | **PHYSICALLY VERIFIED** (`state`) |
| gphase_core | `core/gphase_core.rs` | none | "compose / reflect algebra" | **PHYSICALLY VERIFIED** (phase values in `state`) |
| scheduler | `kernel/scheduler.rs` | none | "Phi0-Phi3 phase graph" | **PHYSICALLY VERIFIED** (pid 0, 1 proc) |
| memory | `kernel/memory.rs` | DDR | "4 tagged pages mapped" | **PHYSICALLY VERIFIED** (4 pages) |
| syscall boundary | `kernel/syscall.rs`, `userland/syscall.rs` | none | "user/kernel boundary armed" | **PHYSICALLY VERIFIED** (dispatch works) |
| PhaseFS | `fs/` | (RAM; SD not wired to fs) | "/tmp mounted (2 inodes)" | PARTIAL (mount only) |
| VM | `vm/` | none | "RISC-V ISA loader ready" | PRESENT BUT UNTESTED |
| trace ring | `kernel/trace.rs` | none | "kernel event ring (4 records)" | PARTIAL |
| init process | `userland/init.rs`, `runtime/loader.rs` | none | "pid 0 scheduled (1 proc)" | **PHYSICALLY VERIFIED** |
| timer | `board/zynq7000/kernel_main.rs` (SCU global timer) | SCU timer | "preempt + chronos tick" | PARTIAL (polled; not isolated) |
| PhaseFS journal | `fs/` | none | "phase records read-only" | PARTIAL (read-only) |
| StateClock atlas | `stateclock/` | none | "10-sector map (sec 5, k=4)" | **PHYSICALLY VERIFIED** (`state` sector 5) |
| authority/ANOM | `kernel/anom/`, `userland/...anom` | none | "authority: root / standby" | PARTIAL |
| PhaseLink | `kernel/phaselink/` | (bm-net for wire) | "PhaseLink codec: ready" | HOSTED ONLY / needs bm-net |
| PhaseNet | `kernel/phasenet/` | (GEM/loopback) | "loopback/UART" | HOSTED ONLY |
| Phase Math | `kernel/phase_math/` | none | "scalar equivalence: OK" | PARTIAL (boot self-check only) |
| PHI-STABILITY | `kernel/phi_stability/` | none | "engine: ready / fixed-1024" | PRESENT BUT UNTESTED |
| G1P | `board/zynq7000/kernel_main.rs` | PL @ 0x40000000 | "id GPL4 … version 0x00020000" | **PHYSICALLY VERIFIED** (ID/VERSION) |
| SD | `board/zynq7000/sd.rs` | SD0 | (used by stage-1, not kernel) | PRESENT BUT UNTESTED |
| GIC | (not enabled in kernel path) | GIC | — | UNSUPPORTED (this build) |
| GEM | (no driver in `kairos_zynq`) | GEM0/PHY | "Ethernet attachment: none" | UNSUPPORTED (this build) |

---

## Phase 2 — Architecture proof (dependency graph)

The Zynq entry funnels into the **real** OS. Exact paths:

```
src/bin/kairos_zynq.rs
  rust_main()  ->  kairos::board::zynq7000::kernel_main::kernel_main()
        src/board/zynq7000/kernel_main.rs
          boot::heap::init()                         (src/boot/heap.rs)
          Runtime::new(); runtime.boot()             (src/runtime/host.rs)  <-- REAL OS
            init_memory(): kernel.alloc_page() x4    (src/kernel/memory.rs)
            mount_phasefs(): kernel.fs.mkdir("/tmp") (src/fs/)
            Loader::boot_init(&mut hosted)           (src/runtime/loader.rs)
          splash::render_boot_screen(...)            (src/userland/splash.rs)
          report_g1p(): read 0x40000000/…4           (board window)
          run_serial_shell():
            uart::read_line()                        (src/board/zynq7000/uart.rs)  <-- REAL UART RX
            shell.execute_session(&mut session,line) (src/userland/shell.rs)       <-- REAL SHELL
              commands::dispatch::execute(...)       (src/userland/commands/dispatch.rs) <-- REAL REGISTRY
                -> state_cmd / help / scheduler / memory / gphase_core / vm / …
```

**Verified facts:**

- The ARM entry calls the same `Runtime::boot()` used by the hosted and RISC-V
  builds — no board-specific boot logic. (`src/runtime/host.rs:93`.)
- The real scheduler/memory/gphase_core/VM are reached via `Runtime` →
  `Hosted` → `Kernel`; the board layer imports them, it does not reimplement
  them. (`src/board/zynq7000/kernel_main.rs` imports `crate::runtime::Runtime`,
  `crate::userland::shell`, `crate::userland::syscall`.)
- UART **input** reaches the real parser: `uart::read_line()` →
  `shell.execute_session` → `parser` → `commands::dispatch`.
- The board layer contains **only** platform code: `uart.rs`, `sd.rs`,
  `fat.rs`, `ps7_init.rs`, `ps7_init_data.rs`, `memory_map.rs`, `kernel_main.rs`.
  No scheduler/memory/fs/shell/algebra replacements.

**Duplicate/replacement modules (NOT in the working build):**

- `zynq_a9_os/` and `zynq_a9_mm/` are **separate standalone workspaces** with
  their own toy scheduler/memory/process — bring-up harnesses from earlier work.
  They are **not** dependencies of the `kairos` crate and are **not** compiled
  into `kairos_zynq`. The `kairos_zynq` image links `kairos::board::*` only.
  → **Quarantined by non-use.** Not deleted (owner's artifacts; deletion is
  out of scope for this audit and destructive). Documented here so they are not
  mistaken for the OS.

---

## Phase 3 — Test harness

- `scripts/pz7020_test_harness.ps1` — opens COM12, waits for `gpos>`, sends one
  plan command at a time over **real UART RX**, waits for the prompt to return,
  timestamps start/end, detects panic/abort/reset/hang/lost-prompt, refuses a
  destructive denylist unless `-AllowDestructive`, and writes per-run
  `transcript.txt` + `results.json` + `report.md` + `SHA256SUMS.txt`.
- `scripts/pz7020_test_plan_default.txt` — safe, read-only, prompt-returning
  command plan in Phase-4 order (no `run`/`kill`/`rm`/`exit`/`write`/`clear`).

No input is fabricated inside the kernel; all input travels over PS UART0 RX.

---

## Phase 4 — Physical test status (pending a board session)

Only `help` and `state` are physically verified (2026-07-30 capture). The
harness + default plan are ready; running them on the board will populate the
remaining rows with real transcripts and SHA-256s. Special cases:

- **C (VM `run`)** — needs a paste sequence (`run` → program lines → `.end`);
  not in the default plan to avoid a false "hang". A dedicated VM plan will feed
  a minimal program and check `regs`/halt. The guest ISA is RISC-V-style; the
  **host CPU is ARM Cortex-A9** (independent facts).
- **G (G1P IRQ 61 / GIC)** — the GIC is **not** enabled in the kernel path; no
  ISR path exists yet. Implementing GIC init + IRQ 61 + ISR + EOI is real new
  platform work, to be done **board-in-the-loop** so delivery is proven by an
  interrupt-count capture, never from register config alone.
- **H (GEM Ethernet)** — no GEM driver in `kairos_zynq`; **UNSUPPORTED** here.
  Register/MDIO/PHY/loopback/TX-RX bring-up is a separate effort; classify
  UNSUPPORTED/FAILED honestly until a capture exists.
- **I (PhaseNet/PhaseLink)** — HOSTED ONLY on this build; software loopback must
  never be labelled physical Ethernet.
- **J/K (checkpoint/statechain/ssh/ai)** — HOSTED ONLY / STUB on bare metal.
  Never print PASS/VERIFY for these here.

---

## Bugs found

- **Cosmetic arch label** (`src/userland/splash.rs`): non-hosted build printed
  `baremetal-riscv64` on ARM hardware. **Fixed** (commit `d3478e1`) to select
  from `target_arch`. Software-verified; re-flash/re-capture pending.

No functional bugs found in the OS core during this audit; the boot path and the
`help`/`state` commands work as designed on hardware.

## Fixes made

| Fix | Commit | Root cause | Evidence |
|-----|--------|------------|----------|
| Arch label reflects ARM | `d3478e1` | hardcoded `baremetal-riscv64` | ELF string `baremetal-armv7-a-cortex-a9`; re-capture pending |

## Remaining work

1. Run `pz7020_test_harness.ps1` on the board; capture transcripts/JSON/MD for
   every safe command; promote rows to PHYSICALLY VERIFIED / FAILED per result.
2. Dedicated VM `run` plan (paste a minimal program; verify `regs`/halt).
3. GIC + G1P IRQ 61 delivery (board-in-the-loop), with interrupt-count evidence.
4. GEM Ethernet bring-up (register → MDIO/PHY → loopback → frame TX/RX) or a
   documented UNSUPPORTED.
5. Re-flash + re-capture the label-corrected build.

## Physical run — 2026-07-30 16:27:09 (`run_20260730_162709`)

Evidence dir: `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_162709/`
transcript sha256 `3dd8e00adbddf4c8953cdb2b9b1ee36c974a7a32b243e2cb93cc84fbe771a5eb`.
Kernel: g1p/GIC build (BOOT.BIN sha `e17a3ac5…`, kernel ELF sha `7061492d…`).
Driven over real PS UART0 RX by `scripts/pz7020_test_harness.ps1` +
`pz7020_test_plan_feature.txt`. The GIC-enabling kernel booted to `gpos>` —
**the proven console is preserved.**

| Feature | Captured output | New classification |
|---------|-----------------|--------------------|
| Boot (GIC-enabled kernel) | reached `gpos>` | **PHYSICALLY VERIFIED** |
| G1P identity (`g1p info`) | `id=0x47504C34 version=0x00020000 caps=0x000000FF` | **PHYSICALLY VERIFIED** |
| G1P compose (`g1p exec`) | 4 cases, `sw_compose==hw_compose` MATCH | **PHYSICALLY VERIFIED** (HW==`gphase_core`) |
| G1P reflect (`g1p reflect`) | 0→3,1→2,2→1,3→0 MATCH | **PHYSICALLY VERIFIED** |
| G1P selftest | 22 checks (full compose table + reflect + invalid-input + invalid-opcode) → PASS | **PHYSICALLY VERIFIED** |
| **G1P IRQ 61 / GIC** (`g1p irq-test 4`) | `expected=4 observed=4 exec_fails=0 missed=0 PASS` | **PHYSICALLY VERIFIED** (ISR count driven only by the HW IRQ handler) |
| VM (`isa`/`run hello`/`regs`) | `ran hello on pid 1`; `regs pc=2 halted=true A=Phi2` | **PHYSICALLY VERIFIED** (real exec + reg change + halt) |
| PhaseFS RAM (`write`/`cat`/`stat`/`ls` on `/tmp`) | wrote 22 bytes; `cat` returns content; `stat` Kairos1; `ls` lists it | PARTIAL → **RAM read/write PHYSICALLY VERIFIED** (SD persistence not tested) |
| StateClock advancement | boot sector 5 → `state` tick 74 / sector 2 | **PHYSICALLY VERIFIED** (live advance + sector change) |
| phi-stability (`phi-stability`/`status`) | status renders (0 tests run) | PARTIAL (status only; stress run not exercised) |

### IRQ verification rationale (Phase 4G)

`G1P_IRQ_COUNT` is incremented **only** inside `kairos_board_irq_dispatch`
(`src/board/zynq7000/gic.rs`), the handler entered from the CPU IRQ vector in
`src/arch/armv7/startup.S`. `g1p irq-test 4` clears the count, enables the G1P
`IRQ_ENABLE`, fires 4 real EXEC ops, and waits (bounded) for the count to rise
each time. `observed=4 missed=0` therefore means the CPU took and dispatched 4
real IRQ 61 exceptions through the GIC — not register-config inference. Captured
over COM12, so Phase 4G is satisfied.

### Not advanced by this run (still open)

- **GEM Ethernet (H)** — `net` ladder present (`gem.rs`) but not exercised in
  this plan; remains UNSUPPORTED/UNTESTED until a `net probe/phy/link/loopback`
  capture exists. Software loopback must never be labelled physical Ethernet.
- **PhaseFS SD persistence** — `/tmp` is RAM; persistence across reset untested.
- **phi-stability stress run** — only status shown; `phi-stability run N` not run.
- **Hosted-only** (phasenet/phaselink/phaseperf/phase-benchmark/ssh/ai) and
  **stub** (root/audit/checkpoint) remain HOSTED ONLY / STUB — never PASS here.

## Global honesty rules (enforced in this audit)

One Golden Phase OS; no second toy OS; no scripted shell; no fabricated input;
no hardcoded success; no self-comparison presented as external verification; no
PASS for hosted-only or stub code; **no physical claim without a physical
capture.**
