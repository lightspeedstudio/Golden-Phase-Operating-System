//! Phase-tagged storage blocks.

use crate::core::gphase_core::PhaseState;
use crate::platform::{HashMap, String, Vec};
use core::fmt;

pub type BlockId = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseBlock {
    pub id: BlockId,
    pub tag: PhaseState,
    pub data: Vec<u8>,
    pub owner_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    NotFound(BlockId),
    Locked(BlockId),
    IllegalTagTransition {
        id: BlockId,
        from: PhaseState,
        to: PhaseState,
    },
    NotFree(BlockId),
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockError::NotFound(id) => write!(f, "block {id} not found"),
            BlockError::Locked(id) => write!(f, "block {id} is locked"),
            BlockError::IllegalTagTransition { id, from, to } => {
                write!(f, "block {id}: cannot transition {from:?} -> {to:?}")
            }
            BlockError::NotFree(id) => write!(f, "block {id} is not free"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BlockError {}

pub struct BlockStore {
    blocks: HashMap<BlockId, PhaseBlock>,
    next_id: BlockId,
}

impl Default for BlockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockStore {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn alloc(&mut self, owner_path: Option<String>) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;
        self.blocks.insert(
            id,
            PhaseBlock {
                id,
                tag: PhaseState::Phi0,
                data: Vec::new(),
                owner_path,
            },
        );
        id
    }

    pub fn get(&self, id: BlockId) -> Result<&PhaseBlock, BlockError> {
        self.blocks.get(&id).ok_or(BlockError::NotFound(id))
    }

    pub fn get_mut(&mut self, id: BlockId) -> Result<&mut PhaseBlock, BlockError> {
        self.blocks.get_mut(&id).ok_or(BlockError::NotFound(id))
    }

    pub fn tag(&mut self, id: BlockId, tag: PhaseState) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        if block.tag == tag {
            return Ok(());
        }
        if block.tag == PhaseState::Phi3 {
            return Err(BlockError::Locked(id));
        }
        if !legal_transition(block.tag, tag) {
            return Err(BlockError::IllegalTagTransition {
                id,
                from: block.tag,
                to: tag,
            });
        }
        block.tag = tag;
        Ok(())
    }

    pub fn lock(&mut self, id: BlockId) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        if block.tag == PhaseState::Phi0 {
            return Err(BlockError::NotFree(id));
        }
        block.tag = PhaseState::Phi3;
        Ok(())
    }

    pub fn unlock(&mut self, id: BlockId) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        if block.tag != PhaseState::Phi3 {
            return Err(BlockError::IllegalTagTransition {
                id,
                from: block.tag,
                to: PhaseState::Phi1,
            });
        }
        block.tag = PhaseState::Phi1;
        Ok(())
    }

    pub fn unlock_as_directory(&mut self, id: BlockId) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        if block.tag != PhaseState::Phi3 {
            return Err(BlockError::IllegalTagTransition {
                id,
                from: block.tag,
                to: PhaseState::Phi2,
            });
        }
        block.tag = PhaseState::Phi2;
        Ok(())
    }

    pub fn free(&mut self, id: BlockId) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        if block.tag == PhaseState::Phi3 {
            return Err(BlockError::Locked(id));
        }
        if block.tag != PhaseState::Phi1 && block.tag != PhaseState::Phi2 {
            return Err(BlockError::IllegalTagTransition {
                id,
                from: block.tag,
                to: PhaseState::Phi0,
            });
        }
        block.tag = PhaseState::Phi0;
        block.data.clear();
        block.owner_path = None;
        Ok(())
    }

    pub fn write_data(&mut self, id: BlockId, data: Vec<u8>) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        if block.tag == PhaseState::Phi3 {
            return Err(BlockError::Locked(id));
        }
        block.data = data;
        Ok(())
    }

    pub fn set_owner_path(
        &mut self,
        id: BlockId,
        owner_path: Option<String>,
    ) -> Result<(), BlockError> {
        let block = self.get_mut(id)?;
        block.owner_path = owner_path;
        Ok(())
    }

    #[cfg(feature = "phasefs-journal")]
    pub fn from_parts(blocks: HashMap<BlockId, PhaseBlock>, next_id: BlockId) -> Self {
        Self { blocks, next_id }
    }

    #[cfg(feature = "phasefs-journal")]
    pub fn next_id(&self) -> BlockId {
        self.next_id
    }

    #[cfg(feature = "phasefs-journal")]
    pub fn iter(&self) -> impl Iterator<Item = &PhaseBlock> {
        self.blocks.values()
    }

    pub fn phase_histogram(&self) -> [usize; 4] {
        let mut counts = [0usize; 4];
        for block in self.blocks.values() {
            counts[block.tag.as_u8() as usize] += 1;
        }
        counts
    }
}

fn legal_transition(from: PhaseState, to: PhaseState) -> bool {
    matches!(
        (from, to),
        (PhaseState::Phi0, PhaseState::Phi1)
            | (PhaseState::Phi0, PhaseState::Phi2)
            | (PhaseState::Phi1, PhaseState::Phi2)
            | (PhaseState::Phi1, PhaseState::Phi3)
            | (PhaseState::Phi2, PhaseState::Phi3)
            | (PhaseState::Phi1, PhaseState::Phi0)
            | (PhaseState::Phi2, PhaseState::Phi0)
            | (PhaseState::Phi3, PhaseState::Phi1)
            | (PhaseState::Phi3, PhaseState::Phi2)
    )
}
