//! Process address-space record and user-pointer checks.
//!
//! Hardware enforcement is the Cortex-A9 MMU. These checks are the
//! copy-in/copy-out gate; they do not replace page permissions.

use crate::arch::armv7::mmu::{
    add_len, invalid_desc, kernel_rw_page, l1_page_table, map_pages, page_index_in_section,
    ranges_overlap, section_index, user_may_read, user_may_write, user_rx_page, user_rw_page,
    FaultClass, MapError, L1_ENTRIES, L2_ENTRIES, PAGE_SIZE, SECTION_SIZE,
};
use crate::kernel::event_log::ProcessId;
use crate::kernel::hash::StateHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perm {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressSpace {
    pub process_id: ProcessId,
    pub package_id: StateHash,
    pub payload_root: StateHash,
    pub translation_table_root: u32,
    pub l2_phys: u32,
    pub code: Range,
    pub data: Range,
    pub stack: Range,
    pub guard: Range,
    pub asid: u8,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub base: u32,
    pub len: u32,
}

impl Range {
    pub fn contains(self, ptr: u32, len: u32) -> bool {
        let Some(end) = add_len(ptr, len) else {
            return false;
        };
        let Some(r_end) = add_len(self.base, self.len) else {
            return false;
        };
        ptr >= self.base && end <= r_end
    }

    pub fn overlaps(self, other: Range) -> bool {
        ranges_overlap(self.base, self.len, other.base, other.len)
    }
}

impl AddressSpace {
    pub fn permission_allows(&self, ptr: u32, len: u32, op: Perm) -> bool {
        match op {
            Perm::Read => {
                self.code.contains(ptr, len)
                    || self.data.contains(ptr, len)
                    || self.stack.contains(ptr, len)
            }
            Perm::Write => self.data.contains(ptr, len) || self.stack.contains(ptr, len),
            Perm::Execute => self.code.contains(ptr, len),
        }
    }

    pub fn range_valid(&self, ptr: u32, len: u32, op: Perm) -> bool {
        if len == 0 {
            return false;
        }
        if add_len(ptr, len).is_none() {
            return false;
        }
        if ptr == 0 {
            return false;
        }
        if self.guard.overlaps(Range { base: ptr, len }) {
            return false;
        }
        if ptr >= 0x4000_0000 {
            return false;
        }
        self.permission_allows(ptr, len, op)
    }
}

pub fn install_user_window(
    l1: &mut [u32],
    l2: &mut [u32],
    l2_phys: u32,
    space: &AddressSpace,
) -> Result<(), MapError> {
    if space.code.overlaps(space.data)
        || space.code.overlaps(space.stack)
        || space.data.overlaps(space.stack)
        || space.code.overlaps(space.guard)
        || space.data.overlaps(space.guard)
        || space.stack.overlaps(space.guard)
    {
        return Err(MapError::Overlap);
    }
    let sec = section_index(space.code.base);
    if section_index(space.data.base) != sec
        || section_index(space.stack.base) != sec
        || section_index(space.guard.base) != sec
    {
        return Err(MapError::CrossesSection);
    }
    if l1.len() != L1_ENTRIES || l2.len() != L2_ENTRIES {
        return Err(MapError::OutOfRange);
    }
    let sec_base = (sec as u32) * SECTION_SIZE;
    for i in 0..L2_ENTRIES {
        let va = sec_base + (i as u32) * PAGE_SIZE;
        l2[i] = kernel_rw_page(va);
    }
    prepare_user_l2(l2, space);
    l2[page_index_in_section(space.guard.base)] = invalid_desc();
    l1[sec] = l1_page_table(l2_phys, true);
    map_pages(
        l1,
        l2,
        l2_phys,
        space.code.base,
        space.code.base,
        space.code.len,
        user_rx_page(space.code.base),
        true,
    )?;
    map_pages(
        l1,
        l2,
        l2_phys,
        space.data.base,
        space.data.base,
        space.data.len,
        user_rw_page(space.data.base),
        true,
    )?;
    map_pages(
        l1,
        l2,
        l2_phys,
        space.stack.base,
        space.stack.base,
        space.stack.len,
        user_rw_page(space.stack.base),
        true,
    )?;
    let code_d = l2[page_index_in_section(space.code.base)];
    let data_d = l2[page_index_in_section(space.data.base)];
    if !user_may_read(code_d) || user_may_write(code_d) {
        return Err(MapError::WrongPerm);
    }
    if !user_may_write(data_d) {
        return Err(MapError::WrongPerm);
    }
    Ok(())
}

/// Invalidate the user window on this translation table.
/// Does not switch TTBR or flush TLB; Cortex-A9 callers must invalidate
/// TLB after this. Not a scheduler ASID switch.
pub fn unmap_user_window(l1: &mut [u32], l2: &mut [u32], space: &AddressSpace) {
    if l1.len() != L1_ENTRIES || l2.len() != L2_ENTRIES {
        return;
    }
    for r in [space.code, space.data, space.stack, space.guard] {
        let n = (r.len / PAGE_SIZE) as usize;
        for i in 0..n {
            let va = r.base + (i as u32) * PAGE_SIZE;
            l2[page_index_in_section(va)] = invalid_desc();
        }
    }
    l1[section_index(space.code.base)] = invalid_desc();
}

/// After filling L2 with kernel RW, `map_pages` rejects overlap. Clear the
/// user slots first so install can stamp user descriptors.
pub fn prepare_user_l2(l2: &mut [u32], space: &AddressSpace) {
    for r in [space.code, space.data, space.stack, space.guard] {
        let n = (r.len / PAGE_SIZE) as usize;
        for i in 0..n {
            let va = r.base + (i as u32) * PAGE_SIZE;
            l2[page_index_in_section(va)] = 0;
        }
    }
}

pub fn copy_l1(dst: &mut [u32], src: &[u32]) {
    dst.copy_from_slice(src);
}

pub fn classify_abort(fsr: u32, ifar: bool) -> FaultClass {
    let _ = ifar;
    crate::arch::armv7::mmu::fault_class(fsr)
}

pub fn next_asid(prev: u8) -> u8 {
    if prev == 255 {
        1
    } else {
        prev.saturating_add(1).max(1)
    }
}
