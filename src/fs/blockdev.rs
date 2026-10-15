//! 512-byte block device used by the PhaseFS journal.
//!
//! Host tests use [`RamBlockDev`]. The Zynq path wraps `SdCard` in
//! `board::zynq7000::journal_sd::SdWindow`.

use core::fmt;

use crate::platform::Vec;

/// Typed SD/block failure. Does not change retry policy; callers still fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockIoDiag {
    pub op: u8,
    pub lba: u32,
    pub count: u32,
    pub present: u32,
    pub nistr: u16,
    pub eistr: u16,
    pub class: u8,
    pub reinit: u8,
    pub head: u32,
    pub tail: u32,
    pub generation: u64,
}

pub const IO_OP_READ: u8 = 0;
pub const IO_OP_WRITE: u8 = 1;
pub const IO_OP_FLUSH: u8 = 2;
pub const IO_OP_STATUS: u8 = 3;
pub const IO_CLASS_TIMEOUT: u8 = 0;
pub const IO_CLASS_CRC: u8 = 1;
pub const IO_CLASS_CONTROLLER: u8 = 2;
pub const IO_CLASS_MEDIA: u8 = 3;
pub const IO_CLASS_INHIBIT: u8 = 4;
pub const IO_CLASS_NOTREADY: u8 = 5;

fn op_name(op: u8) -> &'static str {
    match op {
        IO_OP_WRITE => "write",
        IO_OP_FLUSH => "flush",
        IO_OP_STATUS => "status",
        _ => "read",
    }
}

fn class_name(class: u8) -> &'static str {
    match class {
        IO_CLASS_CRC => "crc",
        IO_CLASS_CONTROLLER => "controller",
        IO_CLASS_MEDIA => "media",
        IO_CLASS_INHIBIT => "inhibit",
        IO_CLASS_NOTREADY => "notready",
        _ => "timeout",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDevError {
    OutOfRange,
    Io,
    Detail(BlockIoDiag),
    InjectedCrash,
}

impl fmt::Display for BlockDevError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockDevError::OutOfRange => write!(f, "lba out of range"),
            BlockDevError::Io => write!(f, "block i/o error"),
            BlockDevError::Detail(d) => write!(
                f,
                "block i/o op={} lba={} count={} present={:#010x} nistr={:#06x} eistr={:#06x} cmd_inh={} data_inh={} class={} reinit={} head={} tail={} gen={}",
                op_name(d.op),
                d.lba,
                d.count,
                d.present,
                d.nistr,
                d.eistr,
                (d.present & 1) != 0,
                (d.present & 2) != 0,
                class_name(d.class),
                d.reinit,
                d.head,
                d.tail,
                d.generation
            ),
            BlockDevError::InjectedCrash => write!(f, "injected crash"),
        }
    }
}

pub trait BlockDevice {
    fn read(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), BlockDevError>;
    fn write(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), BlockDevError>;
    fn flush(&mut self) -> Result<(), BlockDevError>;
}

/// In-memory 512-byte sector array. `flush` is a sequenced no-op for crash
/// injection (not a durability claim).
pub struct RamBlockDev {
    sectors: Vec<[u8; 512]>,
    writes: u32,
    crash_after_writes: Option<u32>,
}

impl RamBlockDev {
    pub fn new(nsectors: u32) -> Self {
        let mut sectors = Vec::new();
        for _ in 0..nsectors {
            sectors.push([0u8; 512]);
        }
        Self {
            sectors,
            writes: 0,
            crash_after_writes: None,
        }
    }

    pub fn nsectors(&self) -> u32 {
        self.sectors.len() as u32
    }

    pub fn set_crash_after_writes(&mut self, n: u32) {
        self.crash_after_writes = Some(n);
        self.writes = 0;
    }

    pub fn clear_crash_after_writes(&mut self) {
        self.crash_after_writes = None;
    }

    /// Overwrite one sector without counting toward crash injection (test
    /// corruption helper).
    pub fn corrupt(&mut self, lba: u32, byte: usize, xor: u8) {
        let sec = &mut self.sectors[lba as usize];
        sec[byte] ^= xor;
    }
}

impl BlockDevice for RamBlockDev {
    fn read(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), BlockDevError> {
        let sec = self
            .sectors
            .get(lba as usize)
            .ok_or(BlockDevError::OutOfRange)?;
        buf.copy_from_slice(sec);
        Ok(())
    }

    fn write(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), BlockDevError> {
        self.writes = self.writes.saturating_add(1);
        if let Some(limit) = self.crash_after_writes {
            if self.writes > limit {
                return Err(BlockDevError::InjectedCrash);
            }
        }
        let sec = self
            .sectors
            .get_mut(lba as usize)
            .ok_or(BlockDevError::OutOfRange)?;
        sec.copy_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), BlockDevError> {
        if let Some(limit) = self.crash_after_writes {
            if self.writes > limit {
                return Err(BlockDevError::InjectedCrash);
            }
        }
        Ok(())
    }
}
