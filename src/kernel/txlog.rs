//! Transaction and recovery log (`feature = "txlog"`), subsystem 7 of the
//! eight kernel-subsystems build program — see
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` §0.7/§9 and
//! `fpga/zynq/evidence/TXLOG_STATUS_<date>.md`.
//!
//! Makes multi-step `PhaseFs` mutations (`write_file`/`mkdir`/`rm`, batched)
//! atomic against the failure class actually tested here: a transaction
//! interrupted mid-execution, or a software reset that leaves a transaction
//! record behind for `recover_one`/`recover_all` to resolve. This module does
//! **not** claim power-loss durability — `PhaseFs` and this journal are both
//! RAM-only (like `captable`/`irqreg`/`sched2`/`ipc`/`extmap`); a real power
//! interruption loses the journal too, so only a *software*-reset class of
//! fault (the live process/kernel state — including this journal — survives;
//! only the caller's control flow was interrupted) is exercised or claimed.
//!
//! ## Mathematics (per the build contract's Part V §7)
//! - Transaction state machine: `FREE -> PREPARED -> WRITING -> VERIFIED ->
//!   COMMITTED`, with a `FAILED -> ROLLED_BACK or ISOLATED` branch —
//!   [`TxState`].
//! - `T = (id, R_old, R_candidate, operations, t, k, g)` — [`TxRecord`]:
//!   `id`, `root_old`, `root_candidate`, `ops`, `tick`, `kairos_sector`,
//!   `generation`.
//! - Journal chain `J_n = PH-S(J_{n-1}, t_n, k_n, PH-C(T_n))`: realized
//!   through the **real, live** event chain — `StatechainLedger::
//!   record_transition` (`ObjectStatechain::append`/`compute_event_hash`),
//!   not `phase_hash::ph_state`, per the math contract's explicit decision
//!   (§5 discrepancy: the live chain uses `state_proof`, not `ph_state`).
//!   Every Prepare/Commit/Rollback/Isolate step appends one real ledger
//!   event, keyed `(ObjectKind::File, tx_id)` — see [`append_ledger_event`].
//! - Commit predicate: `Commit(T) = [allWritesComplete] ∧ [allPHCVerified] ∧
//!   [PH-R(newLeaves) = R_candidate] ∧ [authorityValid] ∧
//!   [orderingOrFlushRequirementMet]`. Concretely: every op in `ops` applied
//!   without error (`allWritesComplete`), the post-write `PhaseFs` content at
//!   every touched path re-hashes (via
//!   [`crate::kernel::crypto::phase_hash::ph_content`]/`ph_root`) to exactly
//!   `root_candidate` (`allPHCVerified` + `PH-R` match), the ledger accepted
//!   the append (`authorityValid` — `record_transition`'s internal
//!   `verify_proof`/`verify_event_hash` checks), and the ledger's own
//!   monotonic sequence numbering enforces ordering. Only then is `Committed`
//!   reached and `R_accepted <- R_candidate` published (the commit ledger
//!   event).
//! - Atomicity target: `R_afterRecovery ∈ {R_old, R_candidate}`, never a
//!   mixed root — [`Txlog::recover_one`] only ever finishes a transaction
//!   forward (to `R_candidate`) from a **proven** `Verified` state, or rolls
//!   it back (to `R_old`) from anything earlier; it never guesses.
//! - Idempotence: `Recover(Recover(X)) = Recover(X)` — `recover_one` on an
//!   already-terminal (`Committed`/`RolledBack`/`Isolated`) record is a
//!   no-op that returns the same outcome without touching `PhaseFs` or the
//!   ledger again.
//!
//! ## Phase/kernel integration
//! - `PhaseFs` remains authoritative for namespace/inodes and content; this
//!   module only wraps its real `write_file`/`mkdir`/`rm` calls in a batched,
//!   undo-tracked transaction. It never bypasses `PhaseFs`'s own mutation
//!   API — every apply and every undo goes through `write_file`/`mkdir`/`rm`
//!   exactly as a caller would (math contract §7's recovery rule: "always
//!   attempt restore through the real mutation API," reusing the same
//!   pattern as `kernel::fault_recovery::recover`).
//! - Persist (`FS_WRITE_FILE`/`FS_MKDIR`/`FS_RM`) calls
//!   [`Txlog::run_transaction`] when `phasefs-journal` is off, or when
//!   board WAL cannot start (`NoPartition`/`MediaUnavailable`). WAL
//!   success skips this ring. Software-reset atomicity, not SD durability.
//!   Feature off: persist is a direct `PhaseFs` call, identical to before
//!   this module existed.
//! - Reuses the real `StatechainLedger`/`ObjectStatechain`/
//!   `StateTransitionProof` machinery (see module docs above) — no second,
//!   weaker signature or hash chain is invented.
//! - PH-C ([`crate::kernel::crypto::phase_hash::tuple_hash`] over the
//!   content-hash domain) binds each op's index/kind/presence/path/data into
//!   one leaf; PH-R ([`crate::kernel::crypto::phase_hash::ph_root`]) combines
//!   a transaction's ordered leaves into `root_old`/`root_candidate`.
//! - `PhaseCache`/`PhaseIndex` invalidation for the touched paths remains
//!   `kernel::syscall`'s job (already wired for `FS_WRITE_FILE`/`FS_MKDIR`/
//!   `FS_RM`) — this module does not duplicate it.
//!
//! ## Complexity targets (per the build contract)
//! - Append (`run_transaction`/`recover_one`): amortized `O(1)` — bounded by
//!   [`TXLOG_MAX_OPS_PER_TX`] ops per transaction, not by total record count.
//! - Recovery (`recover_all`): `O(number of populated records)` — the
//!   journal is a bounded ring ([`TXLOG_MAX_RECORDS`]), never an unbounded
//!   scan.
//!
//! ## Non-goals (explicit — see also `TXLOG_STATUS_<date>.md`)
//! - **No power-loss durability claim.** See the module summary above; this
//!   is a RAM-only journal over a RAM-only filesystem.
//! - **No persistent SD-backed write-ahead log.** `TxRecord`s live in a
//!   bounded in-memory ring, like `extmap`'s block pool.
//! - **No optional `y^2/y^4` dual-degree brace.** The math contract permits
//!   this only where Gate 0 locates the canonical `y` coordinate in the live
//!   kernel equation; Gate 0 did not, so it is not used here.
//! - This subsystem consumes no G64 operation and no `PhaseState` reflection
//!   quotient; its only `PhaseState` usage is the same fixed `Phi1 -> Phi2`
//!   pair every existing filesystem-mutation ledger event already uses
//!   (`kernel::kernel::Kernel::record_fs_event`) — not phase mathematics.

use crate::fs::{FsError, PhaseFs};
use crate::kernel::crypto::phase_hash::{ph_root, tuple_hash, DOMAIN_CONTENT};
use crate::kernel::hash::StateHash;
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::statechain::{ChainError, StatechainLedger};
use core::fmt;

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;

/// Maximum ops in one transaction. Bounded, per the contract's complexity
/// targets — never an unbounded batch.
pub const TXLOG_MAX_OPS_PER_TX: usize = 8;

/// Bounded ring capacity for [`Txlog`]'s evidence log. The oldest record is
/// evicted once full — same bounded-ring discipline as `irqreg`'s deferred
/// event ring.
pub const TXLOG_MAX_RECORDS: usize = 64;

/// One batched `PhaseFs` mutation. `write_file`/`mkdir`/`rm`, matching the
/// three operations named in the build contract's subsystem-7 title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxOp {
    WriteFile {
        path: crate::platform::String,
        data: Vec<u8>,
    },
    Mkdir {
        path: crate::platform::String,
    },
    Rm {
        path: crate::platform::String,
    },
}

impl TxOp {
    fn kind_byte(&self) -> u8 {
        match self {
            TxOp::WriteFile { .. } => 0,
            TxOp::Mkdir { .. } => 1,
            TxOp::Rm { .. } => 2,
        }
    }

    fn path(&self) -> &str {
        match self {
            TxOp::WriteFile { path, .. } => path,
            TxOp::Mkdir { path } => path,
            TxOp::Rm { path } => path,
        }
    }
}

/// `FREE -> PREPARED -> WRITING -> VERIFIED -> COMMITTED`, with
/// `FAILED -> ROLLED_BACK or ISOLATED`. `Free` is only the logical
/// pre-allocation state; no [`TxRecord`] is ever stored while `Free` (a
/// record is only pushed into the log once it reaches `Prepared`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxState {
    Free,
    Prepared,
    Writing,
    Verified,
    Committed,
    Failed,
    RolledBack,
    Isolated,
}

/// Captured *before* mutating, so rollback can restore the exact prior
/// content through the real `PhaseFs` mutation API — never a raw memory
/// copy of internal `PhaseFs` state.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UndoOp {
    CreatedFile {
        path: crate::platform::String,
    },
    OverwroteFile {
        path: crate::platform::String,
        previous: Vec<u8>,
    },
    CreatedDir {
        path: crate::platform::String,
    },
    RemovedFile {
        path: crate::platform::String,
        previous: Vec<u8>,
    },
    RemovedDir {
        path: crate::platform::String,
    },
}

/// `T = (id, R_old, R_candidate, operations, t, k, g)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxRecord {
    pub id: u64,
    pub state: TxState,
    pub ops: Vec<TxOp>,
    pub root_old: StateHash,
    pub root_candidate: StateHash,
    pub tick: u64,
    pub kairos_sector: u8,
    pub generation: u32,
    undo: Vec<UndoOp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxlogError {
    /// A transaction with zero ops was requested.
    Empty,
    /// More than [`TXLOG_MAX_OPS_PER_TX`] ops were requested in one batch.
    TooManyOps,
    /// `recover_one`/`record`/`verify_record` referenced an id not present
    /// in the bounded ring (evicted, or never existed).
    UnknownTx,
    Fs(FsError),
    Chain(ChainError),
}

impl fmt::Display for TxlogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxlogError::Empty => write!(f, "transaction has no ops"),
            TxlogError::TooManyOps => write!(f, "too many ops in one transaction"),
            TxlogError::UnknownTx => write!(f, "unknown transaction id"),
            TxlogError::Fs(e) => write!(f, "{e}"),
            TxlogError::Chain(e) => write!(f, "ledger chain error: {e:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TxlogError {}

/// Controlled fault kinds for [`Txlog::run_transaction_with_fault`] — each
/// corrupts (or interrupts) one real step of a real transaction, mirroring
/// `kernel::fault_recovery`'s "corrupt one real field, then prove
/// detection/recovery" pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectedFault {
    /// Stop applying ops after exactly `n` of them have been written
    /// (`n = 0` stops before any write at all, right after `Prepared`).
    StopAfterOp(u32),
    /// After `Verified` recomputes the real post-write root, flip a byte of
    /// the *stored* `root_candidate` before the commit comparison.
    CorruptCandidateRoot,
    /// After `Verified` recomputes the real post-write root, flip a byte of
    /// that *computed* root before the commit comparison.
    CorruptActualRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxOutcome {
    Committed,
    RolledBack,
    Isolated,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TxlogStats {
    pub records: usize,
    pub committed: u64,
    pub rolled_back: u64,
    pub isolated: u64,
    pub recoveries: u64,
    pub faults_injected: u64,
}

/// PH-C leaf for op `idx`: binds the op's position, kind, presence, path,
/// and data into one hash, via the real tuple-hash construction
/// (`crate::kernel::crypto::phase_hash::tuple_hash`), domain-separated the
/// same as every other PH-C use in the repository.
fn leaf_hash(idx: u32, kind_byte: u8, present: bool, path: &str, data: &[u8]) -> StateHash {
    tuple_hash(
        DOMAIN_CONTENT,
        &[
            &idx.to_le_bytes(),
            &[kind_byte],
            &[present as u8],
            path.as_bytes(),
            data,
        ],
    )
}

/// The leaf this op's *target* state hashes to, independent of the live
/// `PhaseFs` (used both to build `root_candidate` at `Prepared` time, and to
/// re-derive it later from a possibly-tampered `TxRecord.ops` for
/// `Txlog::verify_intent`).
fn candidate_leaf_from_op(idx: u32, op: &TxOp) -> StateHash {
    match op {
        TxOp::WriteFile { path, data } => leaf_hash(idx, 0, true, path, data),
        TxOp::Mkdir { path } => leaf_hash(idx, 1, true, path, &[]),
        TxOp::Rm { path } => leaf_hash(idx, 2, false, path, &[]),
    }
}

/// Snapshot the real pre-mutation `PhaseFs` state at `op`'s path: the undo
/// record needed to reverse it, the `old` leaf (pre-tx state), and the
/// `candidate` leaf (this op's target state). Returns an error for any op
/// whose preconditions already fail against live `PhaseFs` state (e.g.
/// `mkdir` on an existing path) — such a transaction is rejected before
/// `Prepared`, exactly like a single `PhaseFs` call would reject it.
fn snapshot_before(
    fs: &mut PhaseFs,
    idx: u32,
    op: &TxOp,
) -> Result<(UndoOp, StateHash, StateHash), TxlogError> {
    let candidate = candidate_leaf_from_op(idx, op);
    match op {
        TxOp::WriteFile { path, .. } => match fs.read_file(path) {
            Ok(prev) => {
                let old = leaf_hash(idx, 0, true, path, &prev);
                Ok((
                    UndoOp::OverwroteFile {
                        path: path.clone(),
                        previous: prev,
                    },
                    old,
                    candidate,
                ))
            }
            Err(FsError::NotFound(_)) => {
                let old = leaf_hash(idx, 0, false, path, &[]);
                Ok((UndoOp::CreatedFile { path: path.clone() }, old, candidate))
            }
            Err(e) => Err(TxlogError::Fs(e)),
        },
        TxOp::Mkdir { path } => match fs.stat(path) {
            Ok(_) => Err(TxlogError::Fs(FsError::AlreadyExists(path.clone()))),
            Err(FsError::NotFound(_)) => {
                let old = leaf_hash(idx, 1, false, path, &[]);
                Ok((UndoOp::CreatedDir { path: path.clone() }, old, candidate))
            }
            Err(e) => Err(TxlogError::Fs(e)),
        },
        TxOp::Rm { path } => match fs.read_file(path) {
            Ok(prev) => {
                let old = leaf_hash(idx, 2, true, path, &prev);
                Ok((
                    UndoOp::RemovedFile {
                        path: path.clone(),
                        previous: prev,
                    },
                    old,
                    candidate,
                ))
            }
            Err(FsError::NotFound(_)) => Err(TxlogError::Fs(FsError::NotFound(path.clone()))),
            Err(FsError::NotFile(_)) => {
                // Exists, but is a directory (PhaseFs::rm dispatches on kind).
                let old = leaf_hash(idx, 2, true, path, &[]);
                Ok((UndoOp::RemovedDir { path: path.clone() }, old, candidate))
            }
            Err(e) => Err(TxlogError::Fs(e)),
        },
    }
}

/// The leaf for `op`'s path as `PhaseFs` *actually* holds it right now —
/// used after real writes (must match `root_candidate`) and after real
/// undo (must match `root_old`). Never trusts a cached/remembered value.
fn actual_leaf_after(fs: &mut PhaseFs, idx: u32, op: &TxOp) -> Result<StateHash, TxlogError> {
    let kind = op.kind_byte();
    let path = op.path();
    match fs.read_file(path) {
        Ok(data) => Ok(leaf_hash(idx, kind, true, path, &data)),
        Err(FsError::NotFound(_)) => Ok(leaf_hash(idx, kind, false, path, &[])),
        Err(FsError::NotFile(_)) => Ok(leaf_hash(idx, kind, true, path, &[])),
        Err(e) => Err(TxlogError::Fs(e)),
    }
}

fn apply_op(fs: &mut PhaseFs, op: &TxOp) -> Result<(), FsError> {
    match op {
        TxOp::WriteFile { path, data } => fs.write_file(path, data),
        TxOp::Mkdir { path } => fs.mkdir(path),
        TxOp::Rm { path } => fs.rm(path),
    }
}

/// Reverse one op through the real `PhaseFs` mutation API. Errors are
/// tolerated by callers (best-effort: an op that was never actually applied
/// yields a harmless "nothing to undo" error here) — [`Txlog::rollback`]
/// never trusts this alone; it always re-verifies the resulting root.
fn undo_one(fs: &mut PhaseFs, undo: &UndoOp) -> Result<(), FsError> {
    match undo {
        UndoOp::CreatedFile { path } => fs.rm(path),
        UndoOp::OverwroteFile { path, previous } => fs.write_file(path, previous),
        UndoOp::CreatedDir { path } => fs.rm(path),
        UndoOp::RemovedFile { path, previous } => fs.write_file(path, previous),
        UndoOp::RemovedDir { path } => fs.mkdir(path),
    }
}

/// Append one real ledger event for transaction `tx_id`, keyed
/// `(ObjectKind::File, tx_id as u32)` so every transaction gets its own
/// append-only chain of Prepare/Commit-or-Rollback-or-Isolate events. Reuses
/// `StatechainLedger::record_transition` unmodified — the same call
/// `Kernel::record_fs_event` already makes for single-op filesystem
/// mutations (same `Phi1 -> Phi2` pair; not phase mathematics, see module
/// docs "Non-goals").
fn append_ledger_event(
    ledger: &mut StatechainLedger,
    tx_id: u64,
    operation: &str,
    root: StateHash,
) -> Result<(), TxlogError> {
    let mut payload = tx_id.to_le_bytes().to_vec();
    payload.extend_from_slice(&root);
    ledger
        .record_transition(
            ObjectKind::File,
            tx_id as u32,
            SubsystemKind::Filesystem,
            PhaseState::Phi1,
            PhaseState::Phi2,
            operation,
            None,
            &payload,
        )
        .map(|_| ())
        .map_err(TxlogError::Chain)
}

/// Bounded, RAM-only multi-op transaction log over `PhaseFs`, tied to the
/// real `StatechainLedger`. See module docs for the full contract.
#[derive(Debug)]
pub struct Txlog {
    next_id: u64,
    next_generation: u32,
    records: Vec<TxRecord>,
    stats: TxlogStats,
}

impl Default for Txlog {
    fn default() -> Self {
        Self::new()
    }
}

impl Txlog {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            next_generation: 0,
            records: Vec::new(),
            stats: TxlogStats::default(),
        }
    }

    pub fn stats(&self) -> TxlogStats {
        TxlogStats {
            records: self.records.len(),
            ..self.stats
        }
    }

    pub fn record(&self, id: u64) -> Option<&TxRecord> {
        self.records.iter().find(|r| r.id == id)
    }

    pub fn records(&self) -> &[TxRecord] {
        &self.records
    }

    fn record_mut(&mut self, id: u64) -> Option<&mut TxRecord> {
        self.records.iter_mut().find(|r| r.id == id)
    }

    fn push_record(&mut self, record: TxRecord) {
        if self.records.len() >= TXLOG_MAX_RECORDS {
            self.records.remove(0);
        }
        self.records.push(record);
    }

    /// Run one batched, atomic transaction against the real `fs`/`ledger`.
    /// No fault injected — see [`Txlog::run_transaction_with_fault`] for the
    /// fault-injection entry point used by this module's own tests.
    pub fn run_transaction(
        &mut self,
        fs: &mut PhaseFs,
        ledger: &mut StatechainLedger,
        tick: u64,
        kairos_sector: u8,
        ops: Vec<TxOp>,
    ) -> Result<(u64, TxOutcome), TxlogError> {
        self.run_transaction_with_fault(fs, ledger, tick, kairos_sector, ops, None)
    }

    pub fn run_transaction_with_fault(
        &mut self,
        fs: &mut PhaseFs,
        ledger: &mut StatechainLedger,
        tick: u64,
        kairos_sector: u8,
        ops: Vec<TxOp>,
        fault: Option<InjectedFault>,
    ) -> Result<(u64, TxOutcome), TxlogError> {
        if ops.is_empty() {
            return Err(TxlogError::Empty);
        }
        if ops.len() > TXLOG_MAX_OPS_PER_TX {
            return Err(TxlogError::TooManyOps);
        }

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);

        // ---- PREPARED: snapshot "before" state; compute R_old / R_candidate.
        let mut undo = Vec::with_capacity(ops.len());
        let mut old_leaves = Vec::with_capacity(ops.len());
        let mut new_leaves = Vec::with_capacity(ops.len());
        for (idx, op) in ops.iter().enumerate() {
            let (u, old_leaf, candidate_leaf) = snapshot_before(fs, idx as u32, op)?;
            undo.push(u);
            old_leaves.push(old_leaf);
            new_leaves.push(candidate_leaf);
        }
        let root_old = ph_root(&old_leaves);
        let root_candidate = ph_root(&new_leaves);

        self.push_record(TxRecord {
            id,
            state: TxState::Prepared,
            ops: ops.clone(),
            root_old,
            root_candidate,
            tick,
            kairos_sector,
            generation,
            undo,
        });
        append_ledger_event(ledger, id, "txlog_prepare", root_candidate)?;

        // ---- WRITING: execute every op, in order, through the real PhaseFs API.
        if let Some(r) = self.record_mut(id) {
            r.state = TxState::Writing;
        }
        let stop_after = match fault {
            Some(InjectedFault::StopAfterOp(n)) => Some(n as usize),
            _ => None,
        };
        let mut real_error: Option<FsError> = None;
        let mut interrupted = false;
        for (idx, op) in ops.iter().enumerate() {
            if stop_after == Some(idx) {
                interrupted = true;
                self.stats.faults_injected += 1;
                break;
            }
            if let Err(e) = apply_op(fs, op) {
                real_error = Some(e);
                break;
            }
        }

        if interrupted || real_error.is_some() {
            if let Some(r) = self.record_mut(id) {
                r.state = TxState::Failed;
            }
            let outcome = self.rollback(fs, ledger, id)?;
            if let Some(e) = real_error {
                // A real `PhaseFs` error always propagates to the caller —
                // rollback is safety cleanup, not a substitute success. This
                // keeps single-op transaction wrapping (see
                // `kernel::syscall`'s FS_WRITE_FILE/FS_MKDIR/FS_RM handlers)
                // byte-for-byte equivalent to a direct `PhaseFs` call: a
                // failing op still surfaces its own `FsError`.
                return Err(TxlogError::Fs(e));
            }
            // Fault-injected stop (test-only): no real error exists, so the
            // rollback/isolate outcome itself is the caller-visible result.
            return Ok((id, outcome));
        }

        // ---- VERIFIED: recompute PH-C leaves from the real post-write fs.
        if let Some(r) = self.record_mut(id) {
            r.state = TxState::Verified;
        }
        let mut actual_leaves = Vec::with_capacity(ops.len());
        for (idx, op) in ops.iter().enumerate() {
            actual_leaves.push(actual_leaf_after(fs, idx as u32, op)?);
        }
        let mut root_actual = ph_root(&actual_leaves);
        if fault == Some(InjectedFault::CorruptActualRoot) {
            root_actual[0] ^= 0xFF;
            self.stats.faults_injected += 1;
        }
        if fault == Some(InjectedFault::CorruptCandidateRoot) {
            if let Some(r) = self.record_mut(id) {
                r.root_candidate[0] ^= 0xFF;
            }
            self.stats.faults_injected += 1;
        }
        let expected_candidate = self.record(id).ok_or(TxlogError::UnknownTx)?.root_candidate;

        if root_actual != expected_candidate {
            // Commit predicate failed (PH-R mismatch): never a false commit.
            let outcome = self.rollback(fs, ledger, id)?;
            return Ok((id, outcome));
        }

        // ---- COMMITTED: publish R_accepted <- R_candidate via the real ledger.
        append_ledger_event(ledger, id, "txlog_commit", root_actual)?;
        if let Some(r) = self.record_mut(id) {
            r.state = TxState::Committed;
        }
        self.stats.committed += 1;
        Ok((id, TxOutcome::Committed))
    }

    /// Best-effort undo of every op's undo record (tolerating individual
    /// errors for ops that were never actually applied), then re-verify the
    /// real resulting `PhaseFs` root against `root_old` before claiming
    /// `RolledBack` — never a false recovery (math contract §7: "Unrecoverable
    /// state must isolate, never a false RECOVERED").
    fn rollback(
        &mut self,
        fs: &mut PhaseFs,
        ledger: &mut StatechainLedger,
        id: u64,
    ) -> Result<TxOutcome, TxlogError> {
        let record = self.record(id).ok_or(TxlogError::UnknownTx)?.clone();
        for u in record.undo.iter().rev() {
            let _ = undo_one(fs, u);
        }

        let mut reverify_ok = true;
        let mut leaves = Vec::with_capacity(record.ops.len());
        for (idx, op) in record.ops.iter().enumerate() {
            match actual_leaf_after(fs, idx as u32, op) {
                Ok(leaf) => leaves.push(leaf),
                Err(_) => {
                    reverify_ok = false;
                    break;
                }
            }
        }
        let restored_root = if reverify_ok {
            ph_root(&leaves)
        } else {
            [0u8; 32]
        };

        if reverify_ok && restored_root == record.root_old {
            append_ledger_event(ledger, id, "txlog_rollback", restored_root)?;
            if let Some(r) = self.record_mut(id) {
                r.state = TxState::RolledBack;
            }
            self.stats.rolled_back += 1;
            Ok(TxOutcome::RolledBack)
        } else {
            append_ledger_event(ledger, id, "txlog_isolate", restored_root)?;
            if let Some(r) = self.record_mut(id) {
                r.state = TxState::Isolated;
            }
            self.stats.isolated += 1;
            Ok(TxOutcome::Isolated)
        }
    }

    /// Reopen/recover transaction `id` after a (simulated) software reset.
    /// `expected_generation` must match the record's real `generation`
    /// (math contract §6's generation-stamped identity, reused here exactly
    /// as `captable` uses it for handles) — a mismatch is isolated, never
    /// falsely recovered. Idempotent: calling this again on an already
    /// terminal record (`Committed`/`RolledBack`/`Isolated`) is a no-op that
    /// returns the same outcome without touching `fs`/`ledger` again.
    pub fn recover_one(
        &mut self,
        fs: &mut PhaseFs,
        ledger: &mut StatechainLedger,
        id: u64,
        expected_generation: u32,
    ) -> Result<TxOutcome, TxlogError> {
        let (state, generation) = {
            let r = self.record(id).ok_or(TxlogError::UnknownTx)?;
            (r.state, r.generation)
        };

        if generation != expected_generation {
            if let Some(r) = self.record_mut(id) {
                r.state = TxState::Isolated;
            }
            self.stats.isolated += 1;
            return Ok(TxOutcome::Isolated);
        }

        match state {
            TxState::Committed => Ok(TxOutcome::Committed),
            TxState::RolledBack => Ok(TxOutcome::RolledBack),
            TxState::Isolated => Ok(TxOutcome::Isolated),
            TxState::Verified => {
                // All writes were already applied and had matched
                // `root_candidate` at Verified time; only the COMMITTED
                // ledger append was lost. Re-verify the *real* fs (never
                // trust the stale marker) before finishing forward.
                let record = self.record(id).ok_or(TxlogError::UnknownTx)?.clone();
                let mut leaves = Vec::with_capacity(record.ops.len());
                for (idx, op) in record.ops.iter().enumerate() {
                    leaves.push(actual_leaf_after(fs, idx as u32, op)?);
                }
                let actual = ph_root(&leaves);
                self.stats.recoveries += 1;
                if actual == record.root_candidate {
                    append_ledger_event(ledger, id, "txlog_recover_commit", actual)?;
                    if let Some(r) = self.record_mut(id) {
                        r.state = TxState::Committed;
                    }
                    self.stats.committed += 1;
                    Ok(TxOutcome::Committed)
                } else {
                    self.rollback(fs, ledger, id)
                }
            }
            TxState::Prepared | TxState::Writing | TxState::Failed | TxState::Free => {
                self.stats.recoveries += 1;
                self.rollback(fs, ledger, id)
            }
        }
    }

    /// Recover every non-terminal record currently in the log, oldest first.
    pub fn recover_all(
        &mut self,
        fs: &mut PhaseFs,
        ledger: &mut StatechainLedger,
    ) -> Vec<(u64, TxOutcome)> {
        let pending: Vec<(u64, u32)> = self
            .records
            .iter()
            .filter(|r| {
                !matches!(
                    r.state,
                    TxState::Committed | TxState::RolledBack | TxState::Isolated
                )
            })
            .map(|r| (r.id, r.generation))
            .collect();
        let mut out = Vec::with_capacity(pending.len());
        for (id, generation) in pending {
            if let Ok(outcome) = self.recover_one(fs, ledger, id, generation) {
                out.push((id, outcome));
            }
        }
        out
    }

    /// Cross-check a terminal record's claimed `state` against the real
    /// `PhaseFs` content at its ops' paths, right now. A tampered/corrupted
    /// `state` marker (e.g. flipped to `Committed` by
    /// [`Txlog::corrupt_state_for_audit_test`] on a record that was actually
    /// rolled back) never passes this check, because it recomputes the root
    /// from live `PhaseFs` state — it does not just read the marker back.
    pub fn verify_record(&self, fs: &mut PhaseFs, id: u64) -> Result<bool, TxlogError> {
        let record = self.record(id).ok_or(TxlogError::UnknownTx)?.clone();
        let expected = match record.state {
            TxState::Committed => record.root_candidate,
            TxState::RolledBack | TxState::Isolated => record.root_old,
            _ => return Ok(false),
        };
        let mut leaves = Vec::with_capacity(record.ops.len());
        for (idx, op) in record.ops.iter().enumerate() {
            leaves.push(actual_leaf_after(fs, idx as u32, op)?);
        }
        Ok(ph_root(&leaves) == expected)
    }

    /// Cross-check a record's stored `root_candidate` against the candidate
    /// leaves *recomputed from its currently-stored `ops`* — independent of
    /// `PhaseFs`. Detects a corrupted/tampered intent (op path/data changed
    /// after `Prepared` computed `root_candidate` from the original ops).
    pub fn verify_intent(&self, id: u64) -> Result<bool, TxlogError> {
        let record = self.record(id).ok_or(TxlogError::UnknownTx)?;
        let leaves: Vec<StateHash> = record
            .ops
            .iter()
            .enumerate()
            .map(|(idx, op)| candidate_leaf_from_op(idx as u32, op))
            .collect();
        Ok(ph_root(&leaves) == record.root_candidate)
    }

    /// Test-only: leave a transaction behind in a non-terminal state,
    /// simulating a software reset that interrupted `run_transaction` before
    /// it could reach a terminal outcome. Mirrors `PhaseFs::
    /// set_pending_journal_for_test`'s identical "explicit test-only crash
    /// point" precedent. `applied_count` ops are actually executed through
    /// the real `PhaseFs` API before this returns; the rest are left
    /// unapplied. `mark_verified` additionally marks the record `Verified`
    /// (only meaningful when `applied_count == ops.len()`).
    #[cfg(any(test, debug_assertions))]
    pub fn simulate_interrupted_transaction_for_test(
        &mut self,
        fs: &mut PhaseFs,
        tick: u64,
        kairos_sector: u8,
        ops: Vec<TxOp>,
        applied_count: usize,
        mark_verified: bool,
    ) -> Result<u64, TxlogError> {
        if ops.is_empty() {
            return Err(TxlogError::Empty);
        }
        if ops.len() > TXLOG_MAX_OPS_PER_TX || applied_count > ops.len() {
            return Err(TxlogError::TooManyOps);
        }

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);

        let mut undo = Vec::with_capacity(ops.len());
        let mut old_leaves = Vec::with_capacity(ops.len());
        let mut new_leaves = Vec::with_capacity(ops.len());
        for (idx, op) in ops.iter().enumerate() {
            let (u, old_leaf, candidate_leaf) = snapshot_before(fs, idx as u32, op)?;
            undo.push(u);
            old_leaves.push(old_leaf);
            new_leaves.push(candidate_leaf);
        }
        let root_old = ph_root(&old_leaves);
        let root_candidate = ph_root(&new_leaves);

        for op in ops.iter().take(applied_count) {
            apply_op(fs, op).map_err(TxlogError::Fs)?;
        }

        let state = if applied_count == 0 {
            TxState::Prepared
        } else if mark_verified && applied_count == ops.len() {
            TxState::Verified
        } else {
            TxState::Writing
        };

        self.push_record(TxRecord {
            id,
            state,
            ops,
            root_old,
            root_candidate,
            tick,
            kairos_sector,
            generation,
            undo,
        });
        Ok(id)
    }

    /// Test-only audit setter: corrupt one op's stored intent/data after
    /// `Prepared` computed `root_candidate` from the *original* ops. Used to
    /// prove [`Txlog::verify_intent`] detects the tamper. Mirrors
    /// `StatechainLedger`'s `#[doc(hidden)] ..._for_audit_test` setters.
    #[doc(hidden)]
    #[cfg(any(test, debug_assertions))]
    pub fn corrupt_op_data_for_audit_test(&mut self, id: u64, op_index: usize, new_data: Vec<u8>) {
        if let Some(r) = self.record_mut(id) {
            if let Some(TxOp::WriteFile { data, .. }) = r.ops.get_mut(op_index) {
                *data = new_data;
            }
        }
    }

    /// Test-only audit setter: overwrite a record's `state` marker directly,
    /// bypassing the real state machine. Used to prove
    /// [`Txlog::verify_record`] never blindly trusts a corrupted commit
    /// marker.
    #[doc(hidden)]
    #[cfg(any(test, debug_assertions))]
    pub fn corrupt_state_for_audit_test(&mut self, id: u64, new_state: TxState) {
        if let Some(r) = self.record_mut(id) {
            r.state = new_state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> (PhaseFs, StatechainLedger, Txlog) {
        (PhaseFs::new(), StatechainLedger::new(), Txlog::new())
    }

    // ── HOST_UNIT_TEST: commit path ─────────────────────────────────────

    #[test]
    fn single_write_transaction_commits_and_content_matches() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let (id, outcome) = txlog
            .run_transaction(&mut fs, &mut ledger, 1, 0, ops)
            .unwrap();
        assert_eq!(outcome, TxOutcome::Committed);
        assert_eq!(fs.read_file("/a.txt").unwrap(), b"hello");
        assert_eq!(txlog.record(id).unwrap().state, TxState::Committed);
        assert!(txlog.verify_record(&mut fs, id).unwrap());
        assert!(txlog.verify_intent(id).unwrap());
    }

    #[test]
    fn three_op_write_mkdir_rm_transaction_commits_atomically() {
        let (mut fs, mut ledger, mut txlog) = env();
        fs.write_file("/old.txt", b"stale").unwrap();
        let ops = vec![
            TxOp::WriteFile {
                path: "/new.txt".into(),
                data: b"fresh".to_vec(),
            },
            TxOp::Mkdir {
                path: "/newdir".into(),
            },
            TxOp::Rm {
                path: "/old.txt".into(),
            },
        ];
        let (_id, outcome) = txlog
            .run_transaction(&mut fs, &mut ledger, 5, 1, ops)
            .unwrap();
        assert_eq!(outcome, TxOutcome::Committed);
        assert_eq!(fs.read_file("/new.txt").unwrap(), b"fresh");
        assert!(fs.stat("/newdir").is_ok());
        assert!(matches!(
            fs.read_file("/old.txt"),
            Err(FsError::NotFound(_))
        ));
    }

    // ── HOST_FAULT_INJECTION: interrupted writes roll back ──────────────

    #[test]
    fn fault_stop_before_any_write_rolls_back_to_old_root() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let (id, outcome) = txlog
            .run_transaction_with_fault(
                &mut fs,
                &mut ledger,
                1,
                0,
                ops,
                Some(InjectedFault::StopAfterOp(0)),
            )
            .unwrap();
        assert_eq!(outcome, TxOutcome::RolledBack);
        assert!(matches!(fs.read_file("/a.txt"), Err(FsError::NotFound(_))));
        assert_eq!(txlog.record(id).unwrap().state, TxState::RolledBack);
        assert!(txlog.verify_record(&mut fs, id).unwrap());
    }

    #[test]
    fn fault_stop_after_first_of_two_ops_rolls_back_both() {
        let (mut fs, mut ledger, mut txlog) = env();
        fs.write_file("/existing.txt", b"original").unwrap();
        let ops = vec![
            TxOp::WriteFile {
                path: "/existing.txt".into(),
                data: b"overwritten".to_vec(),
            },
            TxOp::Mkdir {
                path: "/never_created".into(),
            },
        ];
        let (id, outcome) = txlog
            .run_transaction_with_fault(
                &mut fs,
                &mut ledger,
                2,
                0,
                ops,
                Some(InjectedFault::StopAfterOp(1)),
            )
            .unwrap();
        assert_eq!(outcome, TxOutcome::RolledBack);
        // op0 actually executed (overwrite), then was undone.
        assert_eq!(fs.read_file("/existing.txt").unwrap(), b"original");
        assert!(matches!(
            fs.stat("/never_created"),
            Err(FsError::NotFound(_))
        ));
        assert_eq!(txlog.record(id).unwrap().state, TxState::RolledBack);
    }

    #[test]
    fn real_fs_error_mid_transaction_rolls_back_prior_ops() {
        // `NotEmpty` can only be discovered by the real `PhaseFs::rm` call at
        // apply time (the prepare-time snapshot only checks presence/kind,
        // not emptiness) -- a genuine mid-`Writing` failure, not a
        // precondition rejected up front.
        let (mut fs, mut ledger, mut txlog) = env();
        fs.mkdir("/nonempty").unwrap();
        fs.write_file("/nonempty/child.txt", b"x").unwrap();
        let ops = vec![
            TxOp::WriteFile {
                path: "/created.txt".into(),
                data: b"data".to_vec(),
            },
            TxOp::Rm {
                path: "/nonempty".into(), // fails: NotEmpty
            },
        ];
        let err = txlog
            .run_transaction(&mut fs, &mut ledger, 3, 0, ops)
            .unwrap_err();
        assert_eq!(err, TxlogError::Fs(FsError::NotEmpty("/nonempty".into())));
        assert!(matches!(
            fs.read_file("/created.txt"),
            Err(FsError::NotFound(_))
        ));
        assert!(
            fs.stat("/nonempty").is_ok(),
            "directory must survive the failed rm + rollback"
        );
        // The transaction was still safely rolled back internally even
        // though the real error propagates to the caller.
        let record = txlog.records().last().unwrap();
        assert_eq!(record.state, TxState::RolledBack);
    }

    #[test]
    fn corrupt_candidate_root_forces_rollback_not_false_commit() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let (id, outcome) = txlog
            .run_transaction_with_fault(
                &mut fs,
                &mut ledger,
                1,
                0,
                ops,
                Some(InjectedFault::CorruptCandidateRoot),
            )
            .unwrap();
        assert_eq!(outcome, TxOutcome::RolledBack);
        assert!(matches!(fs.read_file("/a.txt"), Err(FsError::NotFound(_))));
        let _ = id;
    }

    #[test]
    fn corrupt_actual_root_forces_rollback_not_false_commit() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let (_id, outcome) = txlog
            .run_transaction_with_fault(
                &mut fs,
                &mut ledger,
                1,
                0,
                ops,
                Some(InjectedFault::CorruptActualRoot),
            )
            .unwrap();
        assert_eq!(outcome, TxOutcome::RolledBack);
        assert!(matches!(fs.read_file("/a.txt"), Err(FsError::NotFound(_))));
    }

    // ── HOST_FAULT_INJECTION: reopen/recover after a simulated reset ────

    #[test]
    fn recover_from_prepared_rolls_back_and_is_idempotent() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let id = txlog
            .simulate_interrupted_transaction_for_test(&mut fs, 1, 0, ops, 0, false)
            .unwrap();
        let gen = txlog.record(id).unwrap().generation;

        let first = txlog.recover_one(&mut fs, &mut ledger, id, gen).unwrap();
        assert_eq!(first, TxOutcome::RolledBack);
        assert!(matches!(fs.read_file("/a.txt"), Err(FsError::NotFound(_))));

        let stats_after_first = txlog.stats();
        let second = txlog.recover_one(&mut fs, &mut ledger, id, gen).unwrap();
        assert_eq!(second, first, "Recover(Recover(X)) = Recover(X)");
        assert_eq!(
            txlog.stats(),
            stats_after_first,
            "idempotent: no re-mutation"
        );
    }

    #[test]
    fn recover_from_writing_partial_rolls_back_both_ops() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![
            TxOp::WriteFile {
                path: "/a.txt".into(),
                data: b"one".to_vec(),
            },
            TxOp::WriteFile {
                path: "/b.txt".into(),
                data: b"two".to_vec(),
            },
        ];
        // Only the first op actually ran before the simulated reset.
        let id = txlog
            .simulate_interrupted_transaction_for_test(&mut fs, 2, 0, ops, 1, false)
            .unwrap();
        assert_eq!(fs.read_file("/a.txt").unwrap(), b"one");
        assert!(matches!(fs.read_file("/b.txt"), Err(FsError::NotFound(_))));

        let gen = txlog.record(id).unwrap().generation;
        let outcome = txlog.recover_one(&mut fs, &mut ledger, id, gen).unwrap();
        assert_eq!(outcome, TxOutcome::RolledBack);
        assert!(matches!(fs.read_file("/a.txt"), Err(FsError::NotFound(_))));
        assert!(matches!(fs.read_file("/b.txt"), Err(FsError::NotFound(_))));
    }

    #[test]
    fn recover_from_verified_commits_forward_and_is_idempotent() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        // All ops actually applied and match the candidate root, but the
        // reset happened before the COMMITTED ledger append.
        let id = txlog
            .simulate_interrupted_transaction_for_test(&mut fs, 1, 0, ops, 1, true)
            .unwrap();
        assert_eq!(txlog.record(id).unwrap().state, TxState::Verified);

        let gen = txlog.record(id).unwrap().generation;
        let first = txlog.recover_one(&mut fs, &mut ledger, id, gen).unwrap();
        assert_eq!(first, TxOutcome::Committed);
        assert_eq!(fs.read_file("/a.txt").unwrap(), b"hello");

        let stats_after_first = txlog.stats();
        let second = txlog.recover_one(&mut fs, &mut ledger, id, gen).unwrap();
        assert_eq!(
            second,
            TxOutcome::Committed,
            "Recover(Recover(X)) = Recover(X)"
        );
        assert_eq!(txlog.stats(), stats_after_first, "idempotent: no re-commit");
    }

    #[test]
    fn recover_with_wrong_generation_isolates_never_falsely_recovers() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let id = txlog
            .simulate_interrupted_transaction_for_test(&mut fs, 1, 0, ops, 1, true)
            .unwrap();
        let real_gen = txlog.record(id).unwrap().generation;
        let outcome = txlog
            .recover_one(&mut fs, &mut ledger, id, real_gen.wrapping_add(1))
            .unwrap();
        assert_eq!(outcome, TxOutcome::Isolated);
        // The real write is untouched either way -- isolation makes no claim
        // about fs content, it only refuses to trust the stale handle.
        assert_eq!(fs.read_file("/a.txt").unwrap(), b"hello");
    }

    // ── HOST_UNIT_TEST: tamper detection on historical records ──────────

    #[test]
    fn corrupted_commit_marker_detected_by_verify_record() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let (id, outcome) = txlog
            .run_transaction_with_fault(
                &mut fs,
                &mut ledger,
                1,
                0,
                ops,
                Some(InjectedFault::StopAfterOp(0)),
            )
            .unwrap();
        assert_eq!(outcome, TxOutcome::RolledBack);
        assert!(txlog.verify_record(&mut fs, id).unwrap());

        // Corrupt the marker to claim Committed, even though the real fs
        // content still matches the ROLLED-BACK (old) state.
        txlog.corrupt_state_for_audit_test(id, TxState::Committed);
        assert!(
            !txlog.verify_record(&mut fs, id).unwrap(),
            "a corrupted commit marker must never verify"
        );
    }

    #[test]
    fn corrupted_intent_detected_by_verify_intent() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops = vec![TxOp::WriteFile {
            path: "/a.txt".into(),
            data: b"hello".to_vec(),
        }];
        let (id, outcome) = txlog
            .run_transaction(&mut fs, &mut ledger, 1, 0, ops)
            .unwrap();
        assert_eq!(outcome, TxOutcome::Committed);
        assert!(txlog.verify_intent(id).unwrap());

        txlog.corrupt_op_data_for_audit_test(id, 0, b"tampered".to_vec());
        assert!(
            !txlog.verify_intent(id).unwrap(),
            "corrupted intent must never verify against the original root_candidate"
        );
    }

    // ── HOST_PROPERTY_TEST: bounds ───────────────────────────────────────

    #[test]
    fn empty_transaction_rejected() {
        let (mut fs, mut ledger, mut txlog) = env();
        let err = txlog
            .run_transaction(&mut fs, &mut ledger, 0, 0, Vec::new())
            .unwrap_err();
        assert_eq!(err, TxlogError::Empty);
    }

    #[test]
    fn oversized_transaction_rejected() {
        let (mut fs, mut ledger, mut txlog) = env();
        let ops: Vec<TxOp> = (0..(TXLOG_MAX_OPS_PER_TX + 1))
            .map(|i| TxOp::WriteFile {
                path: format!("/f{i}"),
                data: vec![b'x'],
            })
            .collect();
        let err = txlog
            .run_transaction(&mut fs, &mut ledger, 0, 0, ops)
            .unwrap_err();
        assert_eq!(err, TxlogError::TooManyOps);
    }

    #[test]
    fn bounded_ring_evicts_oldest_record() {
        let (mut fs, mut ledger, mut txlog) = env();
        let mut first_id = None;
        for i in 0..(TXLOG_MAX_RECORDS + 4) {
            let ops = vec![TxOp::WriteFile {
                path: format!("/r{i}"),
                data: vec![b'x'],
            }];
            let (id, _) = txlog
                .run_transaction(&mut fs, &mut ledger, i as u64, 0, ops)
                .unwrap();
            if i == 0 {
                first_id = Some(id);
            }
        }
        assert!(
            txlog.record(first_id.unwrap()).is_none(),
            "oldest record must be evicted"
        );
        assert_eq!(txlog.stats().records, TXLOG_MAX_RECORDS);
    }
}
