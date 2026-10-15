# KairOS build safety

CPU spikes during preservation snapshots are usually caused by **overlapping heavy workloads**, not a single normal `cargo test`.

## What overloaded the system (2026-07-07)

Multiple agents/terminals ran at once:

| Workload | CPU impact |
|----------|------------|
| `export_graph_data` — fib(1_000_000) × 20 reps × 3 engines | **Extreme** (~10+ min, ~100% CPU) |
| `export_scale_compare_graph_data` — fib(1M) parallel sweep | **High** |
| `cargo test` (full suite) | **High** (compiles + runs all tests) |
| Baremetal `cargo build` / QEMU smoke | **Moderate** |

The graph benchmark alone finished in **~634 s** in one captured run. Running it **together** with full `cargo test` and baremetal builds can push CPU to **99%** and freeze the machine.

## Rules

1. **Never** run benchmark export + full `cargo test` + baremetal release build **simultaneously**.
2. **One workflow at a time** during preservation or release prep.
3. Set **`CARGO_BUILD_JOBS=2`** (or `1` on laptops) for routine work.
4. **`export_graph_data` / fib(1M)** — run **alone**, expect **10+ minutes** at full CPU.
5. Prefer **`scripts/export-benchmarks-safe.ps1`** (cleans existing `.raw.txt` output) over re-running heavy benchmarks.
6. Use **`scripts/test-safe.ps1`** for full tests (`--jobs 2`, `--test-threads=2`).

## Safe commands (run manually when ready)

```powershell
cd C:\Users\richa\OneDrive\Desktop\FullOS\kairos

# 1. Clean benchmark JSON from existing raw files (no re-run)
.\scripts\export-benchmarks-safe.ps1

# 2. Full test suite (limited parallelism)
.\scripts\test-safe.ps1

# 3. Only if raw files are missing or invalid — SLOW, run alone
.\scripts\export-benchmarks-safe.ps1 -ForceReRun
```

## Defer / cancel

- **Baremetal release builds** — defer until hosted tests and benchmark JSON are done.
- **`-ForceReRun` benchmarks** — only when `.raw.txt` files are missing or corrupt.
- **Parallel Cursor subagents** — avoid launching multiple cargo-heavy tasks on the same machine.

## Paths

| Resource | Path |
|----------|------|
| Safe benchmark export | `scripts/export-benchmarks-safe.ps1` |
| Safe test runner | `scripts/test-safe.ps1` |
| This guide | `docs/BUILD_SAFETY.md` |
| Snapshot benchmarks | `release_snapshot/benchmarks/` |
