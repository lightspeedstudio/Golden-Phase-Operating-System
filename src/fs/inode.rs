//! Inode model — files and directories with phase tags.

use crate::core::gphase_core::PhaseState;
use crate::fs::block::BlockId;
use crate::platform::{String, Vec};

pub type InodeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inode {
    pub id: InodeId,
    pub name: String,
    pub kind: InodeKind,
    pub tag: PhaseState,
    pub blocks: Vec<BlockId>,
    pub parent: Option<InodeId>,
    /// Live `StateClock.chronos_tick` at the most recent create/mutate,
    /// stamped by `PhaseFs::stamp_phase_time`. `0` until first stamped.
    pub chronos: u64,
    /// Live `StateClock.sector_id` (Kairos-sector) at the most recent
    /// create/mutate, stamped by `PhaseFs::stamp_phase_time`.
    pub kairos_sector: u8,
}

impl Inode {
    pub fn new_file(id: InodeId, name: String, parent: Option<InodeId>) -> Self {
        Self {
            id,
            name,
            kind: InodeKind::File,
            tag: PhaseState::Phi1,
            blocks: Vec::new(),
            parent,
            chronos: 0,
            kairos_sector: 0,
        }
    }

    pub fn new_directory(id: InodeId, name: String, parent: Option<InodeId>) -> Self {
        Self {
            id,
            name,
            kind: InodeKind::Directory,
            tag: PhaseState::Phi2,
            blocks: Vec::new(),
            parent,
            chronos: 0,
            kairos_sector: 0,
        }
    }

    pub fn mark_deleted(&mut self) {
        self.tag = PhaseState::Phi0;
        self.blocks.clear();
    }
}
