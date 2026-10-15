//! Circular WAL, checkpoint slots, crash injection, recovery.

use super::format::{
    decode_record_header, decode_superblock, encode_record, encode_superblock, record_sectors,
    verify_record, RawRecord, Superblock,
};
use super::p6_plan::{
    decode_plan, encode_plan, P6Plan, P6_PLAN_REL, P6_STAGE_BEFORE_REPLAY_APPLY,
};
use super::p7::{
    checksum_covered_offset, derive_abs_lba, sector_hash, wal_journal_rel, P7Fail, P7Report,
    P7Window, P7_TOKEN,
};
use super::p10::{
    target_bytes as p10_target_bytes, target_path as p10_target_path, P10Fail, P10_ANCHOR_BYTES,
    P10_ANCHOR_PATH, P10_TOKEN,
};
use super::p11::{
    target_bytes as p11_target_bytes, target_path as p11_target_path, P11Fail, P11_TOKEN,
};
use super::{
    CrashPoint, JournalError, RedoOp, CKPT0_START, CKPT1_START, CKPT_SECTORS, OP_APPLIED, OP_BEGIN,
    OP_COMMIT, OP_REDO_CREATE, OP_REDO_STAMP, SUPERBLOCK_A, SUPERBLOCK_B,
    WAL_SECTORS, WAL_START,
};
use crate::fs::blockdev::BlockDevice;
use crate::fs::persist;
use crate::fs::phasefs::PhaseFs;
use crate::kernel::hash::StateHash;
use crate::platform::{String, Vec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverClass {
    Clean,
    Recovered,
    RejectedCorrupt,
    Isolated,
}

impl RecoverClass {
    pub fn as_str(self) -> &'static str {
        match self {
            RecoverClass::Clean => "CLEAN",
            RecoverClass::Recovered => "RECOVERED",
            RecoverClass::RejectedCorrupt => "REJECTED_CORRUPT",
            RecoverClass::Isolated => "ISOLATED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanStop {
    End,
    Corrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoverReport {
    pub class: RecoverClass,
    pub replayed: bool,
    pub discarded: bool,
    pub generation: u64,
    pub fs_root: StateHash,
    pub txn_id: u64,
}

impl Default for RecoverReport {
    fn default() -> Self {
        Self {
            class: RecoverClass::Clean,
            replayed: false,
            discarded: false,
            generation: 0,
            fs_root: [0u8; 32],
            txn_id: 0,
        }
    }
}

pub struct PhaseFsJournal<D: BlockDevice> {
    pub dev: D,
    head: u32,
    tail: u32,
    used: u32,
    generation: u64,
    next_txn_id: u64,
    live_slot: u8,
    checkpoint_generation: u64,
    fs_root: StateHash,
    prev_record_root: StateHash,
    crash: CrashPoint,
    active_sb: u8,
    pub p6_lines: Vec<String>,
    last_op: u16,
}

impl<D: BlockDevice> PhaseFsJournal<D> {
    pub fn set_crash(&mut self, crash: CrashPoint) {
        self.crash = crash;
    }

    pub fn crash(&self) -> CrashPoint {
        self.crash
    }

    pub fn into_device(self) -> D {
        self.dev
    }

    pub fn fs_root(&self) -> StateHash {
        self.fs_root
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn checkpoint_generation(&self) -> u64 {
        self.checkpoint_generation
    }

    pub fn next_txn_id(&self) -> u64 {
        self.next_txn_id
    }

    pub fn wal_cursor(&self) -> (u32, u32, u32) {
        (self.head, self.tail, self.used)
    }

    fn maybe_crash(&self, at: CrashPoint) -> Result<(), JournalError> {
        if self.crash == at {
            Err(JournalError::InjectedCrash)
        } else {
            Ok(())
        }
    }

    fn p6_say(&mut self, line: &str) {
        self.p6_lines.push(String::from(line));
        #[cfg(all(feature = "baremetal", target_arch = "arm"))]
        {
            crate::board::zynq7000::journal_sd::emit_p6_line(line);
        }
    }

    fn load_p6_plan(&mut self) -> Result<Option<P6Plan>, JournalError> {
        let sec = match self.read_sec(P6_PLAN_REL) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        if sec.iter().all(|b| *b == 0) {
            return Ok(None);
        }
        match decode_plan(&sec) {
            Ok(p) => Ok(Some(p)),
            Err(_) => Ok(None),
        }
    }

    fn store_p6_plan(&mut self, p: &P6Plan) -> Result<(), JournalError> {
        let enc = encode_plan(p);
        self.write_sec(P6_PLAN_REL, &enc)?;
        self.flush()?;
        Ok(())
    }

    pub fn arm_p6(&mut self, run_id: u64) -> Result<u64, JournalError> {
        let p = P6Plan::armed(run_id, self.generation, self.next_txn_id);
        self.store_p6_plan(&p)?;
        self.p6_say(&format!(
            "P6_ARMED run={run_id} gen={} txn={}",
            p.generation,
            p.txn_id
        ));
        Ok(run_id)
    }

    fn p6_run_id(&mut self) -> Option<u64> {
        self.load_p6_plan().ok().flatten().map(|p| p.run_id)
    }

    fn p6_plan_matches_txn(&mut self, txn_id: u64) -> bool {
        match self.load_p6_plan() {
            Ok(Some(p)) => p.armed == 1 && p.remaining == 1 && p.txn_id == txn_id,
            _ => false,
        }
    }

    fn try_p6_recovery_crash(&mut self, txn_id: u64) -> Result<(), JournalError> {
        let Some(p) = self.load_p6_plan()? else {
            return Ok(());
        };
        if p.armed != 1 || p.remaining != 1 {
            return Ok(());
        }
        if p.stage != P6_STAGE_BEFORE_REPLAY_APPLY {
            return Ok(());
        }
        if p.txn_id != txn_id || p.generation != self.generation {
            return Ok(());
        }
        self.p6_say(&format!(
            "P6_RECOVERY_BEGIN run={} gen={}",
            p.run_id,
            p.generation
        ));
        self.store_p6_plan(&p.consumed())?;
        self.p6_say(&format!(
            "P6_RECOVERY_CRASH_FIRED run={} stage=BeforeReplayApply",
            p.run_id
        ));
        // Exact TEST_MATRIX P6 fire: After REDO decode, before apply_redo.
        Err(JournalError::InjectedCrash)
    }

    fn wal_lba(rel: u32) -> u32 {
        WAL_START + (rel % WAL_SECTORS)
    }

    fn ckpt_base(slot: u8) -> u32 {
        if slot == 0 {
            CKPT0_START
        } else {
            CKPT1_START
        }
    }

    fn read_sec(&mut self, lba: u32) -> Result<[u8; 512], JournalError> {
        let mut buf = [0u8; 512];
        self.dev.read(lba, &mut buf)?;
        Ok(buf)
    }

    fn write_sec(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), JournalError> {
        self.dev.write(lba, buf)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), JournalError> {
        self.dev.flush()?;
        Ok(())
    }

    fn load_superblocks(dev: &mut D) -> Result<(Superblock, u8), JournalError> {
        let mut a = [0u8; 512];
        let mut b = [0u8; 512];
        dev.read(SUPERBLOCK_A, &mut a)?;
        dev.read(SUPERBLOCK_B, &mut b)?;
        let da = decode_superblock(&a);
        let db = decode_superblock(&b);
        match (da, db) {
            (Ok(sa), Ok(sb)) => {
                if sa.generation >= sb.generation {
                    Ok((sa, 0))
                } else {
                    Ok((sb, 1))
                }
            }
            (Ok(sa), Err(_)) => Ok((sa, 0)),
            (Err(_), Ok(sb)) => Ok((sb, 1)),
            (Err(_), Err(_)) => Err(JournalError::Unformatted),
        }
    }

    fn from_super(dev: D, sb: Superblock, active_sb: u8) -> Self {
        Self {
            dev,
            head: sb.head,
            tail: sb.tail,
            used: sb.used_sectors,
            generation: sb.generation,
            next_txn_id: sb.next_txn_id,
            live_slot: sb.live_slot,
            checkpoint_generation: sb.checkpoint_generation,
            fs_root: sb.fs_root,
            prev_record_root: sb.prev_record_root,
            crash: CrashPoint::None,
            active_sb,
            p6_lines: Vec::new(),
            last_op: 0,
        }
    }

    pub fn attach(mut dev: D) -> Result<Self, JournalError> {
        let (sb, active) = Self::load_superblocks(&mut dev)?;
        Ok(Self::from_super(dev, sb, active))
    }

    pub fn format(mut dev: D, fs: &PhaseFs) -> Result<Self, JournalError> {
        let bytes = persist::encode(fs).map_err(|_| JournalError::Persist)?;
        let root = persist::content_root(fs);
        write_checkpoint_bytes(&mut dev, 0, &bytes)?;
        let dead = [0u8; 512];
        for i in 0..WAL_SECTORS {
            dev.write(WAL_START + i, &dead)?;
        }
        let sb = Superblock {
            generation: 1,
            head: 0,
            tail: 0,
            used_sectors: 0,
            live_slot: 0,
            next_txn_id: 1,
            checkpoint_generation: 1,
            fs_root: root,
            prev_record_root: [0u8; 32],
        };
        let enc = encode_superblock(&sb);
        dev.write(SUPERBLOCK_A, &enc)?;
        dev.write(SUPERBLOCK_B, &dead)?;
        dev.flush()?;
        Ok(Self::from_super(dev, sb, 0))
    }

    fn current_super(&self) -> Superblock {
        Superblock {
            generation: self.generation,
            head: self.head,
            tail: self.tail,
            used_sectors: self.used,
            live_slot: self.live_slot,
            next_txn_id: self.next_txn_id,
            checkpoint_generation: self.checkpoint_generation,
            fs_root: self.fs_root,
            prev_record_root: self.prev_record_root,
        }
    }

    fn write_new_superblock(&mut self) -> Result<(), JournalError> {
        self.generation = self.generation.saturating_add(1);
        let other = if self.active_sb == 0 {
            SUPERBLOCK_B
        } else {
            SUPERBLOCK_A
        };
        let enc = encode_superblock(&self.current_super());
        self.write_sec(other, &enc)?;
        self.active_sb = 1 - self.active_sb;
        Ok(())
    }

    fn write_record_raw(&mut self, rec: RawRecord) -> Result<u32, JournalError> {
        let bytes = encode_record(&rec)?;
        let nsec = (bytes.len() / 512) as u32;
        if self.used.saturating_add(nsec) > WAL_SECTORS {
            return Err(JournalError::JournalFull);
        }
        for i in 0..nsec {
            let rel = (self.head + i) % WAL_SECTORS;
            let mut sec = [0u8; 512];
            let off = (i as usize) * 512;
            sec.copy_from_slice(&bytes[off..off + 512]);
            self.write_sec(Self::wal_lba(rel), &sec)?;
        }
        self.head = (self.head + nsec) % WAL_SECTORS;
        self.used += nsec;
        self.prev_record_root = rec.prev_root;
        // After writing, prev_record_root for the *next* record is checksum of this one.
        // Store chain tip as SHA of encoded header+payload (without requiring extra hash type).
        let payload_len = rec.payload.len();
        let mut tip = [0u8; 32];
        tip.copy_from_slice(&super::format::sha256(&bytes[..128 + payload_len]));
        self.prev_record_root = tip;
        Ok(nsec)
    }

    fn append(&mut self, op_type: u16, txn_id: u64, seq: u64, payload: Vec<u8>) -> Result<u32, JournalError> {
        let rec = RawRecord {
            op_type,
            txn_id,
            record_seq: seq,
            target: 0,
            payload,
            prev_root: self.prev_record_root,
        };
        let nsec = self.write_record_raw(rec)?;
        self.last_op = op_type;
        Ok(nsec)
    }

    fn load_checkpoint_slot(&mut self, slot: u8) -> Result<PhaseFs, JournalError> {
        let mut bytes = Vec::new();
        let base = Self::ckpt_base(slot);
        for i in 0..CKPT_SECTORS {
            let lba = base
                .checked_add(i)
                .ok_or(JournalError::PartitionOutOfRange)?;
            let sec = self.read_sec(lba)?;
            bytes.extend_from_slice(&sec);
        }
        persist::decode(&bytes).map_err(|_| JournalError::Persist)
    }

    fn write_checkpoint_slot(&mut self, slot: u8, bytes: &[u8], crash_after_first: bool) -> Result<(), JournalError> {
        if bytes.len() > (CKPT_SECTORS as usize) * 512 {
            return Err(JournalError::Persist);
        }
        let base = Self::ckpt_base(slot);
        let mut nwritten = 0u32;
        let mut off = 0usize;
        while off < bytes.len() || nwritten == 0 {
            let mut sec = [0u8; 512];
            if off < bytes.len() {
                let take = core::cmp::min(512, bytes.len() - off);
                sec[..take].copy_from_slice(&bytes[off..off + take]);
                off += take;
            }
            let lba = base
                .checked_add(nwritten)
                .ok_or(JournalError::PartitionOutOfRange)?;
            self.write_sec(lba, &sec)?;
            nwritten += 1;
            if crash_after_first {
                return Err(JournalError::InjectedCrash);
            }
            if off >= bytes.len() {
                break;
            }
        }
        Ok(())
    }

    fn scan_records(&mut self) -> Result<(Vec<RawRecord>, ScanStop), JournalError> {
        let mut rel = self.tail;
        let mut scanned = 0u32;
        let mut out = Vec::new();
        loop {
            if scanned >= WAL_SECTORS {
                break;
            }
            let first = self.read_sec(Self::wal_lba(rel))?;
            let header = match decode_record_header(&first) {
                Ok(h) => h,
                Err(JournalError::TornRecord) | Err(JournalError::BadMagic) => {
                    return Ok((out, ScanStop::End));
                }
                Err(JournalError::Checksum)
                | Err(JournalError::BadVersion)
                | Err(JournalError::PayloadTooBig) => {
                    return Ok((out, ScanStop::Corrupt));
                }
                Err(e) => return Err(e),
            };
            let (op_type, txn_id, seq, target, payload_len, _ph, prev) = header;
            if txn_id < self.next_txn_id {
                if txn_id == 0 {
                    return Ok((out, ScanStop::Corrupt));
                }
                return Ok((out, ScanStop::End));
            }
            let nsec = record_sectors(payload_len);
            if nsec == 0 || scanned.saturating_add(nsec) > WAL_SECTORS {
                return Ok((out, ScanStop::Corrupt));
            }
            let mut raw = Vec::new();
            raw.extend_from_slice(&first);
            for i in 1..nsec {
                let sec = self.read_sec(Self::wal_lba((rel + i) % WAL_SECTORS))?;
                raw.extend_from_slice(&sec);
            }
            let payload = if payload_len == 0 {
                Vec::new()
            } else {
                raw[128..128 + payload_len as usize].to_vec()
            };
            if verify_record(&first, &payload).is_err() {
                return Ok((out, ScanStop::Corrupt));
            }
            if txn_id != self.next_txn_id {
                return Ok((out, ScanStop::Corrupt));
            }
            let expect_seq = out.len() as u64;
            if seq != expect_seq {
                return Ok((out, ScanStop::Corrupt));
            }
            if out.is_empty() && op_type != OP_BEGIN {
                return Ok((out, ScanStop::Corrupt));
            }
            out.push(RawRecord {
                op_type,
                txn_id,
                record_seq: seq,
                target,
                payload,
                prev_root: prev,
            });
            rel = (rel + nsec) % WAL_SECTORS;
            scanned += nsec;
        }
        Ok((out, ScanStop::End))
    }

    pub fn run_transaction(&mut self, fs: &mut PhaseFs, ops: &[RedoOp]) -> Result<u64, JournalError> {
        self.maybe_crash(CrashPoint::BeforeBegin)?;
        let txn_id = self.next_txn_id;
        let mut seq = 0u64;
        self.append(OP_BEGIN, txn_id, seq, Vec::new())?;
        self.maybe_crash(CrashPoint::AfterBegin)?;
        seq += 1;
        for op in ops {
            let payload = op.encode_payload()?;
            self.append(op.op_type(), txn_id, seq, payload)?;
            seq += 1;
        }
        self.maybe_crash(CrashPoint::AfterRedo)?;
        self.flush()?;
        self.maybe_crash(CrashPoint::AfterJournalFlush)?;
        self.append(OP_COMMIT, txn_id, seq, Vec::new())?;
        seq += 1;
        self.maybe_crash(CrashPoint::AfterCommit)?;
        self.flush()?;
        if self.p6_plan_matches_txn(txn_id) {
            let run = self.p6_run_id().unwrap_or(0);
            self.p6_say(&format!("P6_TRANSACTION_COMMITTED run={run}"));
        }
        self.maybe_crash(CrashPoint::AfterCommitFlush)?;

        for op in ops {
            fs.apply_redo(op).map_err(|_| JournalError::Fs)?;
        }
        let bytes = persist::encode(fs).map_err(|_| JournalError::Persist)?;
        let inactive = 1 - self.live_slot;
        let during = self.crash == CrashPoint::DuringApply;
        self.write_checkpoint_slot(inactive, &bytes, during)?;
        self.maybe_crash(CrashPoint::AfterApply)?;
        self.flush()?;
        self.maybe_crash(CrashPoint::AfterApplyFlush)?;

        let root = persist::content_root(fs);
        let mut applied = Vec::new();
        applied.push(inactive);
        applied.extend_from_slice(&(self.checkpoint_generation.saturating_add(1)).to_le_bytes());
        applied.extend_from_slice(&root);
        self.append(OP_APPLIED, txn_id, seq, applied)?;
        self.maybe_crash(CrashPoint::AfterApplied)?;
        self.live_slot = inactive;
        self.checkpoint_generation = self.checkpoint_generation.saturating_add(1);
        self.fs_root = root;
        self.next_txn_id = txn_id.saturating_add(1);
        self.tail = self.head;
        self.used = 0;
        self.last_op = 0;
        self.maybe_crash(CrashPoint::BeforeTailAdvance)?;
        self.write_new_superblock()?;
        self.flush()?;
        self.crash = CrashPoint::None;
        Ok(txn_id)
    }

    pub fn p7_inject(
        &mut self,
        run_id: u64,
        token: &str,
        window: P7Window,
    ) -> Result<P7Report, P7Fail> {
        if token != P7_TOKEN {
            return Err(P7Fail::WrongToken);
        }
        if window.device_sectors == 0 {
            return Err(P7Fail::CapacityUnknown);
        }
        if window.p1_sectors == 0 {
            return Err(P7Fail::MissingPartition);
        }
        self.p6_say(&format!("P7_ARMED run={run_id}"));
        let path = format!("/tmp/p7_torn_{run_id}.txt");
        let data = format!("p7-must-not-appear-{run_id}").into_bytes();
        let op = RedoOp::Write { path, data };
        let payload = op.encode_payload().map_err(|_| P7Fail::Io)?;
        let offset = checksum_covered_offset(payload.len() as u32)?;
        let txn_id = self.next_txn_id;
        self.append(OP_BEGIN, txn_id, 0, Vec::new())
            .map_err(|_| P7Fail::Io)?;
        self.append(op.op_type(), txn_id, 1, payload)
            .map_err(|_| P7Fail::Io)?;
        self.flush().map_err(|_| P7Fail::Io)?;
        if self.used == 0 {
            return Err(P7Fail::NoWalSector);
        }
        if self.last_op == OP_COMMIT || self.last_op == OP_APPLIED {
            return Err(P7Fail::CommittedTail);
        }
        self.write_new_superblock().map_err(|_| P7Fail::Io)?;
        self.flush().map_err(|_| P7Fail::Io)?;
        let last_rel = (self.head + WAL_SECTORS - 1) % WAL_SECTORS;
        let journal_rel = wal_journal_rel(last_rel);
        let abs = derive_abs_lba(window, journal_rel)?;
        self.p6_say(&format!(
            "P7_UNCOMMITTED_WRITTEN run={run_id} lba={abs}"
        ));
        let mut sec = self
            .read_sec(Self::wal_lba(last_rel))
            .map_err(|_| P7Fail::Io)?;
        let before = sector_hash(&sec);
        sec[offset] ^= 0xFF;
        self.write_sec(Self::wal_lba(last_rel), &sec)
            .map_err(|_| P7Fail::Io)?;
        self.flush().map_err(|_| P7Fail::Io)?;
        let mut back = self
            .read_sec(Self::wal_lba(last_rel))
            .map_err(|_| P7Fail::Io)?;
        let after = sector_hash(&back);
        let changed = back[offset] != (sec[offset] ^ 0xFF);
        for b in sec.iter_mut() {
            *b = 0;
        }
        for b in back.iter_mut() {
            *b = 0;
        }
        if before == after || !changed {
            return Err(P7Fail::ReadbackMismatch);
        }
        let hex = |h: &[u8; 32]| {
            let mut s = String::new();
            for b in h {
                s.push_str(&format!("{b:02x}"));
            }
            s
        };
        self.p6_say(&format!(
            "P7_CORRUPTION_VERIFIED run={run_id} lba={abs} offset={offset} before={} after={}",
            hex(&before),
            hex(&after)
        ));
        Ok(P7Report {
            run_id,
            abs_lba: abs,
            offset,
            before,
            after,
        })
    }

    /// Negative-test helper: try to corrupt whatever is currently last, without a new txn.
    pub fn p7_corrupt_existing_tail(
        &mut self,
        token: &str,
        window: P7Window,
    ) -> Result<P7Report, P7Fail> {
        if token != P7_TOKEN {
            return Err(P7Fail::WrongToken);
        }
        if self.used == 0 {
            return Err(P7Fail::NoWalSector);
        }
        if self.last_op == OP_COMMIT || self.last_op == OP_APPLIED {
            return Err(P7Fail::CommittedTail);
        }
        let last_rel = (self.head + WAL_SECTORS - 1) % WAL_SECTORS;
        let abs = derive_abs_lba(window, wal_journal_rel(last_rel))?;
        let _ = abs;
        Err(P7Fail::CommittedTail)
    }

    /// Fill WAL past one wrap (same pattern as host `wraparound_then_recover`), then
    /// commit `/tmp/p8_wrap_<run_id>.txt`. Does not SLCR.
    pub fn p8_wrap_commit(
        &mut self,
        fs: &mut PhaseFs,
        run_id: u64,
    ) -> Result<(u32, u32, u32), JournalError> {
        const P8_ITERS: u32 = 520;
        let (head0, _, _) = self.wal_cursor();
        self.p6_say(&format!("P8_ARMED run={run_id} head={head0}"));
        let _ = self.run_transaction(
            fs,
            &[RedoOp::Mkdir {
                path: "/tmp".into(),
            }],
        );
        for i in 0..P8_ITERS {
            let path = format!("/p8f{i}.txt");
            self.run_transaction(
                fs,
                &[RedoOp::Write {
                    path: path.clone(),
                    data: b"n".to_vec(),
                }],
            )?;
            if i > 0 {
                let prev = format!("/p8f{}.txt", i - 1);
                self.run_transaction(fs, &[RedoOp::Remove { path: prev }])?;
            }
            if i % 50 == 0 {
                let (h, t, u) = self.wal_cursor();
                self.p6_say(&format!("P8_FILL i={i} head={h} tail={t} used={u}"));
            }
        }
        let wrap_path = format!("/tmp/p8_wrap_{run_id}.txt");
        let wrap_data = format!("p8-wrap-{run_id}").into_bytes();
        self.run_transaction(
            fs,
            &[RedoOp::Write {
                path: wrap_path,
                data: wrap_data,
            }],
        )?;
        let (head1, tail1, used1) = self.wal_cursor();
        let wrapped = head1 != head0;
        self.p6_say(&format!(
            "P8_WRAPPED run={run_id} head0={head0} head={head1} tail={tail1} used={used1} wrapped={wrapped}"
        ));
        if !wrapped {
            return Err(JournalError::Io);
        }
        Ok((head0, head1, used1))
    }

    /// Persist BEGIN + redo for the P10 target, then flush. Does not write COMMIT
    /// and does not apply the target to `fs`.
    pub fn p10_stage_uncommitted(
        &mut self,
        fs: &mut PhaseFs,
        run_id: u64,
        token: &str,
    ) -> Result<(), P10Fail> {
        if token != P10_TOKEN {
            return Err(P10Fail::WrongToken);
        }
        let _ = self.run_transaction(
            fs,
            &[RedoOp::Mkdir {
                path: "/tmp".into(),
            }],
        );
        if fs.read_file(P10_ANCHOR_PATH).is_err() {
            self.run_transaction(
                fs,
                &[RedoOp::Write {
                    path: P10_ANCHOR_PATH.into(),
                    data: P10_ANCHOR_BYTES.to_vec(),
                }],
            )
            .map_err(|_| P10Fail::Io)?;
        }
        let txn_id = self.next_txn_id;
        let mut seq = 0u64;
        self.append(OP_BEGIN, txn_id, seq, Vec::new())
            .map_err(|_| P10Fail::Io)?;
        seq += 1;
        let op = RedoOp::Write {
            path: p10_target_path(run_id),
            data: p10_target_bytes(run_id),
        };
        let payload = op.encode_payload().map_err(|_| P10Fail::Io)?;
        self.append(op.op_type(), txn_id, seq, payload)
            .map_err(|_| P10Fail::Io)?;
        self.flush().map_err(|_| P10Fail::Io)?;
        self.p6_say(&format!(
            "P10_UNCOMMITTED_WRITTEN run={run_id} txn={txn_id}"
        ));
        Ok(())
    }

    /// Full COMMIT of the P11 target, including checkpoint apply. Does not SLCR.
    pub fn p11_commit(
        &mut self,
        fs: &mut PhaseFs,
        run_id: u64,
        token: &str,
    ) -> Result<u64, P11Fail> {
        if token != P11_TOKEN {
            return Err(P11Fail::WrongToken);
        }
        let _ = self.run_transaction(
            fs,
            &[RedoOp::Mkdir {
                path: "/tmp".into(),
            }],
        );
        let txn = self
            .run_transaction(
                fs,
                &[RedoOp::Write {
                    path: p11_target_path(run_id),
                    data: p11_target_bytes(run_id),
                }],
            )
            .map_err(|_| P11Fail::Io)?;
        self.p6_say(&format!("P11_COMMITTED run={run_id} txn={txn}"));
        Ok(txn)
    }

    pub fn recover(&mut self, fs: &mut PhaseFs) -> Result<RecoverReport, JournalError> {
        super::set_journal_io_ctx(self.head, self.tail, self.generation);
        let (records, stop) = self.scan_records()?;
        let mut report = RecoverReport {
            class: RecoverClass::Clean,
            replayed: false,
            discarded: false,
            generation: self.generation,
            fs_root: self.fs_root,
            txn_id: 0,
        };
        if records.is_empty() {
            *fs = self.load_checkpoint_slot(self.live_slot)?;
            self.fs_root = persist::content_root(fs);
            report.fs_root = self.fs_root;
            if stop == ScanStop::Corrupt {
                report.class = RecoverClass::RejectedCorrupt;
                self.tail = self.head;
                self.used = 0;
                self.last_op = 0;
                self.write_new_superblock()?;
                self.flush()?;
                report.discarded = true;
                report.generation = self.generation;
            }
            return Ok(report);
        }

        let txn_id = records[0].txn_id;
        report.txn_id = txn_id;
        let has_commit = records.iter().any(|r| r.op_type == OP_COMMIT && r.txn_id == txn_id);
        let applied = records.iter().find(|r| r.op_type == OP_APPLIED && r.txn_id == txn_id);

        if stop == ScanStop::Corrupt && !has_commit {
            *fs = self.load_checkpoint_slot(self.live_slot)?;
            self.fs_root = persist::content_root(fs);
            report.class = RecoverClass::RejectedCorrupt;
            report.replayed = false;
            report.discarded = true;
            report.fs_root = self.fs_root;
            self.tail = self.head;
            self.used = 0;
            self.last_op = 0;
            self.write_new_superblock()?;
            self.flush()?;
            report.generation = self.generation;
            return Ok(report);
        }

        if let Some(app) = applied {
            if app.payload.len() < 41 {
                *fs = self.load_checkpoint_slot(self.live_slot)?;
                self.fs_root = persist::content_root(fs);
                report.class = RecoverClass::RejectedCorrupt;
                report.fs_root = self.fs_root;
                return Ok(report);
            }
            let slot = app.payload[0];
            let ckpt_gen = u64::from_le_bytes(app.payload[1..9].try_into().unwrap());
            if slot > 1 {
                *fs = self.load_checkpoint_slot(self.live_slot)?;
                self.fs_root = persist::content_root(fs);
                report.class = RecoverClass::RejectedCorrupt;
                report.fs_root = self.fs_root;
                return Ok(report);
            }
            *fs = self.load_checkpoint_slot(slot)?;
            self.live_slot = slot;
            self.checkpoint_generation = ckpt_gen;
            self.fs_root = persist::content_root(fs);
            self.finish_scan(&records)?;
            self.next_txn_id = txn_id.saturating_add(1);
            self.write_new_superblock()?;
            self.flush()?;
            report.class = RecoverClass::Clean;
            report.fs_root = self.fs_root;
            report.generation = self.generation;
            return Ok(report);
        }

        if !has_commit {
            *fs = self.load_checkpoint_slot(self.live_slot)?;
            self.fs_root = persist::content_root(fs);
            self.finish_scan(&records)?;
            self.write_new_superblock()?;
            self.flush()?;
            report.class = RecoverClass::Isolated;
            report.discarded = true;
            report.fs_root = self.fs_root;
            report.generation = self.generation;
            return Ok(report);
        }

        *fs = self.load_checkpoint_slot(self.live_slot)?;
        let mut redos = Vec::new();
        for r in &records {
            if r.txn_id != txn_id {
                continue;
            }
            if (OP_REDO_CREATE..=OP_REDO_STAMP).contains(&r.op_type) {
                redos.push(RedoOp::decode_payload(r.op_type, &r.payload)?);
            }
        }
        self.try_p6_recovery_crash(txn_id)?;
        for op in &redos {
            fs.apply_redo(op).map_err(|_| JournalError::Fs)?;
        }
        let bytes = persist::encode(fs).map_err(|_| JournalError::Persist)?;
        let inactive = 1 - self.live_slot;
        self.write_checkpoint_slot(inactive, &bytes, false)?;
        self.flush()?;
        self.live_slot = inactive;
        self.checkpoint_generation = self.checkpoint_generation.saturating_add(1);
        self.fs_root = persist::content_root(fs);
        self.finish_scan(&records)?;
        self.next_txn_id = txn_id.saturating_add(1);
        self.write_new_superblock()?;
        self.flush()?;
        report.class = RecoverClass::Recovered;
        report.replayed = true;
        report.fs_root = self.fs_root;
        report.generation = self.generation;
        Ok(report)
    }

    fn finish_scan(&mut self, records: &[RawRecord]) -> Result<(), JournalError> {
        let mut nsec = 0u32;
        for r in records {
            nsec += record_sectors(r.payload.len() as u32);
        }
        self.tail = (self.tail + nsec) % WAL_SECTORS;
        self.head = self.tail;
        self.used = 0;
        self.last_op = 0;
        Ok(())
    }
}

fn write_checkpoint_bytes<D: BlockDevice>(dev: &mut D, slot: u8, bytes: &[u8]) -> Result<(), JournalError> {
    if bytes.len() > (CKPT_SECTORS as usize) * 512 {
        return Err(JournalError::Persist);
    }
    let base = if slot == 0 { CKPT0_START } else { CKPT1_START };
    let mut off = 0usize;
    let mut i = 0u32;
    while off < bytes.len() {
        let mut sec = [0u8; 512];
        let take = core::cmp::min(512, bytes.len() - off);
        sec[..take].copy_from_slice(&bytes[off..off + take]);
        dev.write(
            base.checked_add(i).ok_or(JournalError::PartitionOutOfRange)?,
            &sec,
        )?;
        off += take;
        i += 1;
    }
    Ok(())
}

impl PhaseFsJournal<crate::fs::blockdev::RamBlockDev> {
    pub fn format_ram(fs: &PhaseFs) -> Result<Self, JournalError> {
        let dev = crate::fs::blockdev::RamBlockDev::new(
            super::REGION_SECTORS + super::p6_plan::P6_PLAN_SECTORS,
        );
        Self::format(dev, fs)
    }
}
