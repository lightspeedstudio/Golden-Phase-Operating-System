# PhaseIndex — multidimensional index over PhaseFS inodes

Additive index partitioning `PhaseFs` inodes by `(state, Kairos-sector, Chronos-bucket)`, so a
lookup like "every Phi1 file written in Kairos-sector 2 during Chronos ticks 100..200" is a
handful of `BTreeMap` range lookups instead of a full filesystem walk. `PhaseFs` remains the sole
source of truth for inode contents — `PhaseIndex` is pure bookkeeping that must stay in sync via
explicit `insert`/`remove`/`relocate` calls.

Source: [`src/kernel/phase_index.rs`](../src/kernel/phase_index.rs) (`PhaseIndexController`).
Declared unconditionally in [`kernel::mod`](../src/kernel/mod.rs) (same convention as `phasenet`),
so it's available on every build, hosted or bare metal.

## Schema change

[`fs::Inode`](../src/fs/inode.rs) gained two fields, both `0` until first stamped:

```rust
pub chronos: u64,        // live StateClock.chronos_tick at last create/mutate
pub kairos_sector: u8,   // live StateClock.sector_id at last create/mutate
```

[`PhaseFs::stamp_phase_time`](../src/fs/phasefs.rs) stamps both from caller-supplied clock values.
`PhaseFs::inode(id)` / `PhaseFs::inode_id_for_path(path)` / `PhaseFs::iter_inodes()` give
`PhaseIndex` callers read access without exposing the inode map directly.

## `PhaseIndexController`

```rust
pub fn insert(&mut self, inode: InodeId, state: PhaseState, kairos_sector: u8, chronos: u64);
pub fn remove(&mut self, inode: InodeId) -> bool;
pub fn relocate(&mut self, inode: InodeId, state: PhaseState, kairos_sector: u8, chronos: u64);
pub fn query_range(&self, state: PhaseState, kairos_sector: u8, chronos_start: u64, chronos_end: u64) -> Vec<InodeId>;
```

Internally: `IndexKey { state, kairos_sector, chronos_bucket }` (Chronos ticks bucketed by
`CHRONOS_BUCKET_WIDTH = 16`) ordered lexicographically in a `BTreeMap<IndexKey, Vec<InodeId>>`,
plus a reverse `BTreeMap<InodeId, IndexKey>` so `remove`/`relocate` never need a linear scan.

**Correctness note:** `query_range` uses `BTreeMap::range(start_key..=end_key)`, which walks only
buckets that actually exist — `O(log n + k)` in the number of *populated* buckets touched, not in
`chronos_end - chronos_start`. An earlier draft looped bucket-by-bucket from `chronos_start` to
`chronos_end`; a query like `0..=u64::MAX` ("everything in this sector") would have taken ~10^18
iterations. Caught by a hanging `cargo test` run before it ever reached hardware.

## Syscall wiring

[`kernel::syscall`](../src/kernel/syscall.rs)'s `stamp_and_index(kernel, path)` helper reads the
live `StateClock` (`chronos_tick`, `sector_id`), calls `PhaseFs::stamp_phase_time`, reads the
inode's current `tag`, and `relocate`s it in `kernel.phase_index`. Called after every mutation:

| Syscall | PhaseIndex effect |
|---|---|
| `FS_MKDIR` | `stamp_and_index` after `fs.mkdir` succeeds |
| `FS_WRITE_FILE` | `stamp_and_index` after `fs.write_file` succeeds |
| `FS_LOCK` | `stamp_and_index` after `fs.lock_path` (relocates Phi1/Phi2 → Phi3) |
| `FS_UNLOCK` | `stamp_and_index` after `fs.unlock_path` (relocates Phi3 → Phi1/Phi2) |
| `FS_RM` | inode id captured *before* `fs.rm` (path won't resolve after), then `phase_index.remove` |

## Proof (hardware, 2026-08-12)

Full `mkdir` → `write` → `cat` → `stat` → `ls` → `lock` → `stat` → `unlock` → `stat` → `rm` → `ls`
sequence driven live over COM12 on PZ7020 hardware, through the real syscall path with
`PhaseIndex` wired in on every mutation:

```
gpos> mkdir /docs
gpos> write /docs/a.txt hello-phaseindex
wrote 16 bytes to /docs/a.txt
gpos> cat /docs/a.txt
hello-phaseindex
gpos> stat /docs/a.txt
Kairos1
gpos> ls /docs
a.txt
gpos> lock /docs/a.txt
gpos> stat /docs/a.txt
Kairos3
gpos> unlock /docs/a.txt
gpos> stat /docs/a.txt
Kairos1
gpos> rm /docs/a.txt
gpos> ls /docs

gpos>
```

`Kairos1 → Kairos3 → Kairos1` confirms `lock`/`unlock` relocate the index entry across phase-state
partitions correctly; the empty `ls /docs` after `rm` confirms `remove` dropped it. No crash, no
hang, on real silicon. Full transcript:
`fpga/zynq/evidence/kairos_zynq_boot/com12_phase_index_verify_20260812_023504.txt`.

## Test coverage

`cargo test --lib kernel::phase_index` (8 tests): insert/query, cross-state/sector/bucket
exclusion, remove (present + missing), relocate, multiple inodes sharing a bucket, idempotent
re-insert, and a query spanning multiple buckets.

`cargo test --lib kernel::syscall` adds 4 tests wiring `PhaseIndex` through the real
`dispatch()` path: `fs_mkdir_indexes_the_new_directory_in_phase_index`,
`fs_write_file_reindexes_on_the_current_clock`, `fs_rm_removes_the_inode_from_phase_index`,
`fs_lock_and_unlock_relocate_the_phase_index_entry`.
