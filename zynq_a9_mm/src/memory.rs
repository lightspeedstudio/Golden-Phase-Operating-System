//! Four-state page-tag memory — same rules as `kairos::kernel::memory`.

use crate::gphase::{compose, PhaseState};

pub type PageId = u32;

const MAX_PAGES: usize = 64;
const ALLOC_TAG: PhaseState = PhaseState::Phi0;

struct Transition {
    from: PhaseState,
    to: PhaseState,
}

const TRANSITIONS: [(&str, Transition); 7] = [
    ("map_user", Transition { from: PhaseState::Phi0, to: PhaseState::Phi1 }),
    ("map_kernel", Transition { from: PhaseState::Phi0, to: PhaseState::Phi2 }),
    ("escalate", Transition { from: PhaseState::Phi1, to: PhaseState::Phi2 }),
    ("lock_page", Transition { from: PhaseState::Phi2, to: PhaseState::Phi3 }),
    ("unlock_page", Transition { from: PhaseState::Phi3, to: PhaseState::Phi2 }),
    ("unmap_user", Transition { from: PhaseState::Phi1, to: PhaseState::Phi0 }),
    ("unmap_kernel", Transition { from: PhaseState::Phi2, to: PhaseState::Phi0 }),
];

pub struct Memory {
    tags: [Option<PhaseState>; MAX_PAGES],
    next_id: PageId,
}

impl Memory {
    pub const fn new() -> Self {
        Self {
            tags: [None; MAX_PAGES],
            next_id: 0,
        }
    }

    pub fn alloc_page(&mut self) -> PageId {
        let page_id = self.next_id;
        self.next_id += 1;
        let idx = page_id as usize;
        assert!(idx < MAX_PAGES);
        self.tags[idx] = Some(ALLOC_TAG);
        page_id
    }

    pub fn free_page(&mut self, page_id: PageId) -> Result<(), ()> {
        let tag = self.get_tag(page_id)?;
        if tag != ALLOC_TAG {
            return Err(());
        }
        self.tags[page_id as usize] = None;
        Ok(())
    }

    pub fn get_tag(&self, page_id: PageId) -> Result<PhaseState, ()> {
        self.tags
            .get(page_id as usize)
            .copied()
            .flatten()
            .ok_or(())
    }

    fn apply(&mut self, op: &str, page_id: PageId) -> Result<(), ()> {
        let transition = TRANSITIONS
            .iter()
            .find(|(name, _)| *name == op)
            .map(|(_, t)| t)
            .expect("known");
        let tag = self.get_tag(page_id)?;
        if tag != transition.from {
            return Err(());
        }
        self.tags[page_id as usize] = Some(transition.to);
        Ok(())
    }

    pub fn map_user(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("map_user", page_id)
    }
    pub fn map_kernel(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("map_kernel", page_id)
    }
    pub fn escalate(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("escalate", page_id)
    }
    pub fn lock_page(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("lock_page", page_id)
    }
    pub fn unlock_page(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("unlock_page", page_id)
    }
    pub fn unmap_user(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("unmap_user", page_id)
    }
    pub fn unmap_kernel(&mut self, page_id: PageId) -> Result<(), ()> {
        self.apply("unmap_kernel", page_id)
    }

    pub fn arbitrate(&self, a: PageId, b: PageId) -> Result<PhaseState, ()> {
        Ok(compose(self.get_tag(a)?, self.get_tag(b)?))
    }
}
