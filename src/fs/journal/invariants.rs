//! Post-recovery PhaseFS invariant checker.

use crate::core::gphase_core::PhaseState;
use crate::fs::inode::InodeKind;
use crate::fs::phasefs::PhaseFs;
use crate::platform::{HashMap, HashSet};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantError {
    DanglingDirent,
    MissingParent,
    DuplicateDirent,
    DanglingBlock,
    DualBlockOwner,
    FreeBlockReferenced,
    KindMismatch,
    LockMismatch,
}

impl fmt::Display for InvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvariantError::DanglingDirent => write!(f, "dirent names missing inode"),
            InvariantError::MissingParent => write!(f, "inode parent missing or not a directory"),
            InvariantError::DuplicateDirent => write!(f, "inode owned by two dirents"),
            InvariantError::DanglingBlock => write!(f, "inode references missing/free block"),
            InvariantError::DualBlockOwner => write!(f, "block owned by two inodes"),
            InvariantError::FreeBlockReferenced => write!(f, "Phi0 block referenced"),
            InvariantError::KindMismatch => write!(f, "inode kind mismatch"),
            InvariantError::LockMismatch => write!(f, "locked inode with unlocked block"),
        }
    }
}

pub fn check_invariants(fs: &PhaseFs) -> Result<(), InvariantError> {
    let mut owned: HashSet<u32> = HashSet::new();
    let mut block_owner: HashMap<u32, u32> = HashMap::new();

    for inode in fs.iter_inodes() {
        match inode.kind {
            InodeKind::Directory => {}
            InodeKind::File => {}
        }
        if inode.id != fs.root_id() {
            let parent = inode.parent.ok_or(InvariantError::MissingParent)?;
            let p = fs.inode(parent).ok_or(InvariantError::MissingParent)?;
            if p.kind != InodeKind::Directory {
                return Err(InvariantError::MissingParent);
            }
            let ents = fs.dir_entries(parent).ok_or(InvariantError::MissingParent)?;
            let hits = ents.iter().filter(|e| e.inode == inode.id).count();
            if hits != 1 {
                return Err(InvariantError::MissingParent);
            }
        }
        if let Some(ents) = fs.dir_entries(inode.id) {
            if inode.kind != InodeKind::Directory {
                return Err(InvariantError::KindMismatch);
            }
            for e in ents {
                if fs.inode(e.inode).is_none() {
                    return Err(InvariantError::DanglingDirent);
                }
                if !owned.insert(e.inode) {
                    return Err(InvariantError::DuplicateDirent);
                }
            }
        }
        for &bid in &inode.blocks {
            let tag = fs.block_phase(bid).ok_or(InvariantError::DanglingBlock)?;
            if tag == PhaseState::Phi0 {
                return Err(InvariantError::FreeBlockReferenced);
            }
            if block_owner.insert(bid, inode.id).is_some() {
                return Err(InvariantError::DualBlockOwner);
            }
            if inode.tag == PhaseState::Phi3 && tag != PhaseState::Phi3 {
                return Err(InvariantError::LockMismatch);
            }
        }
    }
    Ok(())
}
