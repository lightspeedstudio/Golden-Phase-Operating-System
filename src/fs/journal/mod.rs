//! PhaseFS persistent write-ahead journal v1 (`feature = "phasefs-journal"`).
//!
//! RAM `txlog` is a separate software-reset journal. This module is the
//! SD/host WAL. Persist tries this WAL first and skips `txlog` when WAL succeeds.
//! Fallback is only [`wal_unavailable_falls_back_to_txlog`].
//! Not the same durability. See
//! `fpga/zynq/research/phasefs-persistent-journal-v1/`.

mod capacity;
mod format;
mod invariants;
mod partition;
mod p7;
mod p6_plan;
mod p9;
mod p10;
mod p11;
mod wal;

pub use capacity::sdhci_csd_sectors;
pub use invariants::{check_invariants, InvariantError};
pub use p6_plan::{
    classify_p6_evidence, classify_p6_transcript, P6Plan, P6Verdict, P6_PLAN_REL, P6_PLAN_SECTORS,
    P6_STAGE_BEFORE_REPLAY_APPLY,
};
pub use p7::{
    checksum_covered_offset, classify_p7_evidence, derive_abs_lba, wal_journal_rel, P7Fail,
    P7Report, P7Verdict, P7Window, P7_TOKEN,
};
pub use p9::{classify_p9_evidence, P9Verdict};
pub use p10::{
    classify_p10_boot, target_bytes as p10_target_bytes, target_path as p10_target_path, P10Fail,
    P10Verdict, P10_ANCHOR_BYTES, P10_ANCHOR_PATH, P10_READY, P10_TOKEN,
};
pub use p11::{
    classify_p11_boot, target_bytes as p11_target_bytes, target_path as p11_target_path, P11Fail,
    P11Verdict, P11_READY, P11_TOKEN,
};
pub use partition::{abs_journal_lba, validate_journal_window, MbrSlot, JOURNAL_PTYPE_DA};
pub use wal::{PhaseFsJournal, RecoverClass, RecoverReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalBackend {
    Absent,
    RamFallback,
    Sd {
        start_lba: u32,
        p1_sectors: u32,
        device_sectors: u32,
        formatted: bool,
    },
}

impl JournalBackend {
    pub fn is_sd(self) -> bool {
        matches!(self, Self::Sd { .. })
    }

    pub fn durable(self) -> bool {
        matches!(
            self,
            Self::Sd {
                formatted: true,
                ..
            }
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Absent => "NONE",
            Self::RamFallback => "RAM_FALLBACK",
            Self::Sd { formatted: true, .. } => "SD_PARTITION_1",
            Self::Sd {
                formatted: false, ..
            } => "SD_PARTITION_1_UNFORMATTED",
        }
    }
}

use crate::platform::{String, Vec};
use core::fmt;

pub const SECTOR: usize = 512;
pub const FORMAT_VERSION: u16 = 1;
pub const SB_MAGIC: [u8; 4] = *b"GPFJ";
pub const REC_MAGIC: [u8; 4] = *b"GPRC";
pub const CKPT_MAGIC: [u8; 4] = *b"GPFS";

pub const SUPERBLOCK_A: u32 = 0;
pub const SUPERBLOCK_B: u32 = 1;
pub const WAL_START: u32 = 2;
pub const WAL_SECTORS: u32 = 2048;
pub const CKPT0_START: u32 = 2050;
pub const CKPT_SECTORS: u32 = 2048;
pub const CKPT1_START: u32 = 4098;
pub const REGION_SECTORS: u32 = 6146;

pub const MAX_PAYLOAD: u32 = 4096;
pub const MAX_INODES: u32 = 64;
/// Checkpoint inode name / owner path. Must fit GPA store owners
/// `/phase/packages/{64-hex}/manifest.phase` (95 bytes).
pub const MAX_NAME: usize = 128;
pub const MAX_FILE_BYTES: usize = 4096;
pub const HEADER_LEN: usize = 128;

pub const OP_BEGIN: u16 = 1;
pub const OP_REDO_CREATE: u16 = 2;
pub const OP_REDO_WRITE: u16 = 3;
pub const OP_REDO_MKDIR: u16 = 4;
pub const OP_REDO_RENAME: u16 = 5;
pub const OP_REDO_REMOVE: u16 = 6;
pub const OP_REDO_LOCK: u16 = 7;
pub const OP_REDO_UNLOCK: u16 = 8;
pub const OP_REDO_STAMP: u16 = 9;
pub const OP_COMMIT: u16 = 10;
pub const OP_APPLIED: u16 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPoint {
    None = 0,
    AfterBegin = 1,
    AfterRedo = 2,
    AfterJournalFlush = 3,
    AfterCommit = 4,
    AfterCommitFlush = 5,
    DuringApply = 6,
    AfterApply = 7,
    AfterApplyFlush = 8,
    AfterApplied = 9,
    BeforeTailAdvance = 10,
    BeforeBegin = 11,
}

impl CrashPoint {
    pub fn from_u32(n: u32) -> Option<Self> {
        match n {
            0 => Some(CrashPoint::None),
            1 => Some(CrashPoint::AfterBegin),
            2 => Some(CrashPoint::AfterRedo),
            3 => Some(CrashPoint::AfterJournalFlush),
            4 => Some(CrashPoint::AfterCommit),
            5 => Some(CrashPoint::AfterCommitFlush),
            6 => Some(CrashPoint::DuringApply),
            7 => Some(CrashPoint::AfterApply),
            8 => Some(CrashPoint::AfterApplyFlush),
            9 => Some(CrashPoint::AfterApplied),
            10 => Some(CrashPoint::BeforeTailAdvance),
            11 => Some(CrashPoint::BeforeBegin),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

impl Default for CrashPoint {
    fn default() -> Self {
        CrashPoint::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedoOp {
    Create {
        path: String,
    },
    Write {
        path: String,
        data: Vec<u8>,
    },
    Mkdir {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Remove {
        path: String,
    },
    Lock {
        path: String,
    },
    Unlock {
        path: String,
    },
    Stamp {
        path: String,
        chronos: u64,
        kairos_sector: u8,
    },
}

impl RedoOp {
    pub fn op_type(&self) -> u16 {
        match self {
            RedoOp::Create { .. } => OP_REDO_CREATE,
            RedoOp::Write { .. } => OP_REDO_WRITE,
            RedoOp::Mkdir { .. } => OP_REDO_MKDIR,
            RedoOp::Rename { .. } => OP_REDO_RENAME,
            RedoOp::Remove { .. } => OP_REDO_REMOVE,
            RedoOp::Lock { .. } => OP_REDO_LOCK,
            RedoOp::Unlock { .. } => OP_REDO_UNLOCK,
            RedoOp::Stamp { .. } => OP_REDO_STAMP,
        }
    }

    pub fn encode_payload(&self) -> Result<Vec<u8>, JournalError> {
        match self {
            RedoOp::Create { path }
            | RedoOp::Mkdir { path }
            | RedoOp::Remove { path }
            | RedoOp::Lock { path }
            | RedoOp::Unlock { path } => {
                if path.len() > MAX_NAME * 4 {
                    return Err(JournalError::PayloadTooBig);
                }
                Ok(path.as_bytes().to_vec())
            }
            RedoOp::Write { path, data } => {
                if path.len() > u16::MAX as usize || data.len() > MAX_FILE_BYTES {
                    return Err(JournalError::PayloadTooBig);
                }
                let mut buf = Vec::new();
                buf.extend_from_slice(&(path.len() as u16).to_le_bytes());
                buf.extend_from_slice(path.as_bytes());
                buf.extend_from_slice(data);
                if buf.len() > MAX_PAYLOAD as usize {
                    return Err(JournalError::PayloadTooBig);
                }
                Ok(buf)
            }
            RedoOp::Rename { from, to } => {
                if from.len() > u16::MAX as usize || to.len() > u16::MAX as usize {
                    return Err(JournalError::PayloadTooBig);
                }
                let mut buf = Vec::new();
                buf.extend_from_slice(&(from.len() as u16).to_le_bytes());
                buf.extend_from_slice(from.as_bytes());
                buf.extend_from_slice(&(to.len() as u16).to_le_bytes());
                buf.extend_from_slice(to.as_bytes());
                Ok(buf)
            }
            RedoOp::Stamp {
                path,
                chronos,
                kairos_sector,
            } => {
                let mut buf = Vec::new();
                buf.extend_from_slice(&(path.len() as u16).to_le_bytes());
                buf.extend_from_slice(path.as_bytes());
                buf.extend_from_slice(&chronos.to_le_bytes());
                buf.push(*kairos_sector);
                Ok(buf)
            }
        }
    }

    pub fn decode_payload(op: u16, payload: &[u8]) -> Result<RedoOp, JournalError> {
        fn utf8(b: &[u8]) -> Result<String, JournalError> {
            core::str::from_utf8(b)
                .map(|s| s.into())
                .map_err(|_| JournalError::BadPayload)
        }
        match op {
            OP_REDO_CREATE => Ok(RedoOp::Create {
                path: utf8(payload)?,
            }),
            OP_REDO_MKDIR => Ok(RedoOp::Mkdir {
                path: utf8(payload)?,
            }),
            OP_REDO_REMOVE => Ok(RedoOp::Remove {
                path: utf8(payload)?,
            }),
            OP_REDO_LOCK => Ok(RedoOp::Lock {
                path: utf8(payload)?,
            }),
            OP_REDO_UNLOCK => Ok(RedoOp::Unlock {
                path: utf8(payload)?,
            }),
            OP_REDO_WRITE => {
                if payload.len() < 2 {
                    return Err(JournalError::BadPayload);
                }
                let n = u16::from_le_bytes([payload[0], payload[1]]) as usize;
                if payload.len() < 2 + n {
                    return Err(JournalError::BadPayload);
                }
                Ok(RedoOp::Write {
                    path: utf8(&payload[2..2 + n])?,
                    data: payload[2 + n..].to_vec(),
                })
            }
            OP_REDO_RENAME => {
                if payload.len() < 2 {
                    return Err(JournalError::BadPayload);
                }
                let n = u16::from_le_bytes([payload[0], payload[1]]) as usize;
                if payload.len() < 4 + n {
                    return Err(JournalError::BadPayload);
                }
                let m = u16::from_le_bytes([payload[2 + n], payload[3 + n]]) as usize;
                if payload.len() != 4 + n + m {
                    return Err(JournalError::BadPayload);
                }
                Ok(RedoOp::Rename {
                    from: utf8(&payload[2..2 + n])?,
                    to: utf8(&payload[4 + n..])?,
                })
            }
            OP_REDO_STAMP => {
                if payload.len() < 2 {
                    return Err(JournalError::BadPayload);
                }
                let n = u16::from_le_bytes([payload[0], payload[1]]) as usize;
                if payload.len() != 2 + n + 8 + 1 {
                    return Err(JournalError::BadPayload);
                }
                let mut c = [0u8; 8];
                c.copy_from_slice(&payload[2 + n..2 + n + 8]);
                Ok(RedoOp::Stamp {
                    path: utf8(&payload[2..2 + n])?,
                    chronos: u64::from_le_bytes(c),
                    kairos_sector: payload[2 + n + 8],
                })
            }
            _ => Err(JournalError::BadPayload),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    Unformatted,
    Checksum,
    BadMagic,
    BadVersion,
    TornRecord,
    JournalFull,
    PayloadTooBig,
    BadPayload,
    InjectedCrash,
    Io,
    IoDetail(crate::fs::blockdev::BlockIoDiag),
    MediaUnavailable,
    Persist,
    Fs,
    NoPartition,
    PartitionOverlap,
    PartitionTooSmall,
    PartitionOutOfRange,
    BadPartition,
    CapacityUnknown,
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalError::Unformatted => write!(f, "journal unformatted"),
            JournalError::Checksum => write!(f, "journal checksum"),
            JournalError::BadMagic => write!(f, "journal bad magic"),
            JournalError::BadVersion => write!(f, "journal bad version"),
            JournalError::TornRecord => write!(f, "torn journal record"),
            JournalError::JournalFull => write!(f, "journal full"),
            JournalError::PayloadTooBig => write!(f, "payload too big"),
            JournalError::BadPayload => write!(f, "bad payload"),
            JournalError::InjectedCrash => write!(f, "injected crash"),
            JournalError::Io => write!(f, "journal i/o"),
            JournalError::IoDetail(d) => write!(
                f,
                "journal i/o op={} lba={} count={} present={:#010x} nistr={:#06x} eistr={:#06x} cmd_inh={} data_inh={} class={} reinit={} head={} tail={} gen={}",
                match d.op {
                    crate::fs::blockdev::IO_OP_WRITE => "write",
                    crate::fs::blockdev::IO_OP_FLUSH => "flush",
                    crate::fs::blockdev::IO_OP_STATUS => "status",
                    _ => "read",
                },
                d.lba,
                d.count,
                d.present,
                d.nistr,
                d.eistr,
                (d.present & 1) != 0,
                (d.present & 2) != 0,
                match d.class {
                    crate::fs::blockdev::IO_CLASS_CRC => "crc",
                    crate::fs::blockdev::IO_CLASS_CONTROLLER => "controller",
                    crate::fs::blockdev::IO_CLASS_INHIBIT => "inhibit",
                    crate::fs::blockdev::IO_CLASS_NOTREADY => "notready",
                    crate::fs::blockdev::IO_CLASS_MEDIA => "media",
                    _ => "timeout",
                },
                d.reinit,
                d.head,
                d.tail,
                d.generation
            ),
            JournalError::MediaUnavailable => write!(f, "media unavailable"),
            JournalError::Persist => write!(f, "checkpoint persist"),
            JournalError::Fs => write!(f, "phasefs"),
            JournalError::NoPartition => write!(f, "no journal partition"),
            JournalError::PartitionOverlap => write!(f, "journal partition overlaps another"),
            JournalError::PartitionTooSmall => write!(f, "journal partition too small"),
            JournalError::PartitionOutOfRange => write!(f, "journal partition out of range"),
            JournalError::BadPartition => write!(f, "journal partition type refused"),
            JournalError::CapacityUnknown => write!(f, "sd capacity unknown"),
        }
    }
}

impl From<crate::fs::blockdev::BlockDevError> for JournalError {
    fn from(value: crate::fs::blockdev::BlockDevError) -> Self {
        match value {
            crate::fs::blockdev::BlockDevError::InjectedCrash => JournalError::InjectedCrash,
            crate::fs::blockdev::BlockDevError::Detail(d) => JournalError::IoDetail(d),
            crate::fs::blockdev::BlockDevError::OutOfRange | crate::fs::blockdev::BlockDevError::Io => {
                JournalError::Io
            }
        }
    }
}

/// Persist records RAM `txlog` only when the WAL cannot start.
///
/// `Ok(())` (WAL applied) must not fall back — that would dual-write a
/// RAM journal after durable media already committed. Other WAL errors
/// (`Checksum`, `Io`, …) must not fall back either: they are failures,
/// not “use RAM instead.”
pub fn wal_unavailable_falls_back_to_txlog(result: Result<(), JournalError>) -> bool {
    matches!(
        result,
        Err(JournalError::NoPartition) | Err(JournalError::MediaUnavailable)
    )
}

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static IO_HEAD: AtomicU32 = AtomicU32::new(0);
static IO_TAIL: AtomicU32 = AtomicU32::new(0);
static IO_GEN: AtomicU64 = AtomicU64::new(0);

pub fn set_journal_io_ctx(head: u32, tail: u32, generation: u64) {
    IO_HEAD.store(head, Ordering::Relaxed);
    IO_TAIL.store(tail, Ordering::Relaxed);
    IO_GEN.store(generation, Ordering::Relaxed);
}

pub fn journal_io_ctx() -> (u32, u32, u64) {
    (
        IO_HEAD.load(Ordering::Relaxed),
        IO_TAIL.load(Ordering::Relaxed),
        IO_GEN.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests;
