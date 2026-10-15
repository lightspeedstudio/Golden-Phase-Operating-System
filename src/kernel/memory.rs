//! Four-state page-tag memory model; access contention uses core compose.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::PageId;
use crate::kernel::phase_consensus::PhaseConsensus;
use crate::platform::{HashMap, String};
use core::fmt;

const ALLOC_TAG: PhaseState = PhaseState::Phi0;

struct Transition {
    from: PhaseState,
    to: PhaseState,
}

const TRANSITIONS: [(&str, Transition); 7] = [
    (
        "map_user",
        Transition {
            from: PhaseState::Phi0,
            to: PhaseState::Phi1,
        },
    ),
    (
        "map_kernel",
        Transition {
            from: PhaseState::Phi0,
            to: PhaseState::Phi2,
        },
    ),
    (
        "escalate",
        Transition {
            from: PhaseState::Phi1,
            to: PhaseState::Phi2,
        },
    ),
    (
        "lock_page",
        Transition {
            from: PhaseState::Phi2,
            to: PhaseState::Phi3,
        },
    ),
    (
        "unlock_page",
        Transition {
            from: PhaseState::Phi3,
            to: PhaseState::Phi2,
        },
    ),
    (
        "unmap_user",
        Transition {
            from: PhaseState::Phi1,
            to: PhaseState::Phi0,
        },
    ),
    (
        "unmap_kernel",
        Transition {
            from: PhaseState::Phi2,
            to: PhaseState::Phi0,
        },
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryError(pub String);

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MemoryError {}

pub struct Memory {
    tags: HashMap<PageId, PhaseState>,
    next_id: PageId,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    pub fn new() -> Self {
        Self {
            tags: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn alloc_page(&mut self) -> PageId {
        let page_id = self.next_id;
        self.next_id += 1;
        self.tags.insert(page_id, ALLOC_TAG);
        page_id
    }

    pub fn free_page(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        let tag = self.get_tag(page_id)?;
        if tag != ALLOC_TAG {
            return Err(MemoryError(format!(
                "cannot free page {page_id}: tag {:?}; unmap to Phi0 first",
                tag
            )));
        }
        self.tags.remove(&page_id);
        Ok(())
    }

    pub fn get_tag(&self, page_id: PageId) -> Result<PhaseState, MemoryError> {
        self.tags
            .get(&page_id)
            .copied()
            .ok_or_else(|| MemoryError(format!("no such page {page_id}")))
    }

    pub fn tag_report(&self) -> HashMap<PageId, PhaseState> {
        self.tags.clone()
    }

    fn apply(&mut self, op: &str, page_id: PageId) -> Result<(), MemoryError> {
        let transition = TRANSITIONS
            .iter()
            .find(|(name, _)| *name == op)
            .map(|(_, t)| t)
            .expect("known transition");
        let tag = self.get_tag(page_id)?;
        if tag != transition.from {
            return Err(MemoryError(format!(
                "{op} requires {:?}, but page {page_id} has {:?}",
                transition.from, tag
            )));
        }
        self.tags.insert(page_id, transition.to);
        Ok(())
    }

    pub fn map_user(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("map_user", page_id)
    }

    pub fn map_kernel(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("map_kernel", page_id)
    }

    pub fn escalate(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("escalate", page_id)
    }

    pub fn lock_page(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("lock_page", page_id)
    }

    pub fn unlock_page(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("unlock_page", page_id)
    }

    pub fn unmap_user(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("unmap_user", page_id)
    }

    pub fn unmap_kernel(&mut self, page_id: PageId) -> Result<(), MemoryError> {
        self.apply("unmap_kernel", page_id)
    }

    pub fn arbitrate(
        &self,
        page_id_a: PageId,
        page_id_b: PageId,
        consensus: &PhaseConsensus,
    ) -> Result<PhaseState, MemoryError> {
        let a = self.get_tag(page_id_a)?;
        let b = self.get_tag(page_id_b)?;
        match consensus.evaluate_compose(a, b) {
            Ok(ok) => Ok(ok.sw.compose),
            Err(_) => Err(MemoryError("phase consensus rejected".into())),
        }
    }
}
