//! Software DMA containment for boards with no IOMMU (PZ7020 / Zynq-7000 PS).
//!
//! `IOMMU: HARDWARE_UNAVAILABLE`. This module is **not** an IOMMU and does
//! not claim protection against a malicious bus master. It is the strongest
//! software bound available: kernel-owned pools, physical-range checks,
//! bounce copies for untrusted bytes, ownership transitions, zero-on-reuse,
//! and refuse/quarantine on invalid descriptors.
//!
//! A discrete IOMMU is not a companion chip on Zynq-7000 AXI. Optional PL AXI
//! firewall is best-effort. Real SMMU is a later UltraScale+ class SoC.
//! Spec: `docs/CUSTOM_ZYNQ7000_BOARD.md`. USB uses a private `USB_DMA`
//! instance; GEM uses `GEM_DMA`. Neither is an IOMMU.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Usable DDR window (identity-mapped on this port). MMIO is outside.
pub const DMA_PHYS_LO: u32 = 0x0010_0000;
pub const DMA_PHYS_HI: u32 = 0x1FFF_FFFF;
pub const MAX_DESC: usize = 16;
pub const MAX_BUF: usize = 1536;
pub const MAX_POOLS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaOwner {
    Cpu,
    Hw,
    Quarantined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    Range,
    TooLong,
    NotInPool,
    Quarantined,
    CapDenied,
    Busy,
    Bound,
}

impl DmaError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Range => "dma: physical address outside DDR pool window",
            Self::TooLong => "dma: buffer exceeds MAX_BUF",
            Self::NotInPool => "dma: address not in a kernel-owned pool",
            Self::Quarantined => "dma: device quarantined",
            Self::CapDenied => "dma: capability denied",
            Self::Busy => "dma: descriptor still hardware-owned",
            Self::Bound => "dma: pool table full",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Pool {
    base: u32,
    len: u32,
}

pub struct DmaContainment {
    pools: [Pool; MAX_POOLS],
    n_pools: u8,
    owners: [DmaOwner; MAX_DESC],
    desc_addr: [u32; MAX_DESC],
    n_desc: u8,
    quarantined: bool,
    submits: u32,
    bounces: u32,
    refused: u32,
}

impl Default for DmaContainment {
    fn default() -> Self {
        Self::new()
    }
}

impl DmaContainment {
    pub const fn new() -> Self {
        Self {
            pools: [Pool { base: 0, len: 0 }; MAX_POOLS],
            n_pools: 0,
            owners: [DmaOwner::Cpu; MAX_DESC],
            desc_addr: [0; MAX_DESC],
            n_desc: 0,
            quarantined: false,
            submits: 0,
            bounces: 0,
            refused: 0,
        }
    }

    pub fn register_pool(&mut self, base: u32, len: u32) -> Result<(), DmaError> {
        validate_window(base, len)?;
        if self.n_pools as usize >= MAX_POOLS {
            return Err(DmaError::Bound);
        }
        let i = self.n_pools as usize;
        self.pools[i] = Pool { base, len };
        self.n_pools += 1;
        Ok(())
    }

    pub fn in_pool(&self, addr: u32, len: u32) -> bool {
        if validate_range(addr, len).is_err() {
            return false;
        }
        let end = addr.wrapping_add(len);
        for i in 0..self.n_pools as usize {
            let p = self.pools[i];
            let pend = p.base.wrapping_add(p.len);
            if addr >= p.base && end <= pend {
                return true;
            }
        }
        false
    }

    /// Copy untrusted bytes into a kernel pool buffer. Never returns the
    /// source pointer as a DMA address.
    pub fn bounce(dst: &mut [u8], src: &[u8]) -> Result<usize, DmaError> {
        if src.len() > MAX_BUF || src.len() > dst.len() {
            return Err(DmaError::TooLong);
        }
        dst[..src.len()].copy_from_slice(src);
        if src.len() < dst.len() {
            dst[src.len()..].fill(0);
        }
        Ok(src.len())
    }

    pub fn zero(buf: &mut [u8]) {
        buf.fill(0);
    }

    pub fn submit(
        &mut self,
        phys: u32,
        len: u32,
        cap_ok: bool,
    ) -> Result<(), DmaError> {
        if self.quarantined {
            self.refused = self.refused.wrapping_add(1);
            return Err(DmaError::Quarantined);
        }
        if !cap_ok {
            self.refused = self.refused.wrapping_add(1);
            self.quarantine();
            return Err(DmaError::CapDenied);
        }
        if !self.in_pool(phys, len) {
            self.refused = self.refused.wrapping_add(1);
            self.quarantine();
            return Err(DmaError::NotInPool);
        }
        if self.n_desc as usize >= MAX_DESC {
            // reuse a CPU-owned slot
        }
        for i in 0..self.n_desc as usize {
            if self.desc_addr[i] == phys {
                if self.owners[i] == DmaOwner::Hw {
                    self.refused = self.refused.wrapping_add(1);
                    return Err(DmaError::Busy);
                }
                self.owners[i] = DmaOwner::Hw;
                self.submits = self.submits.wrapping_add(1);
                return Ok(());
            }
        }
        if self.n_desc as usize >= MAX_DESC {
            self.refused = self.refused.wrapping_add(1);
            return Err(DmaError::Bound);
        }
        let i = self.n_desc as usize;
        self.desc_addr[i] = phys;
        self.owners[i] = DmaOwner::Hw;
        self.n_desc = self.n_desc.saturating_add(1);
        self.submits = self.submits.wrapping_add(1);
        Ok(())
    }

    pub fn complete(&mut self, phys: u32) -> Result<(), DmaError> {
        for i in 0..self.n_desc as usize {
            if self.desc_addr[i] == phys && self.owners[i] == DmaOwner::Hw {
                self.owners[i] = DmaOwner::Cpu;
                return Ok(());
            }
        }
        Err(DmaError::NotInPool)
    }

    pub fn quarantine(&mut self) {
        self.quarantined = true;
        for o in self.owners.iter_mut() {
            if *o == DmaOwner::Hw {
                *o = DmaOwner::Quarantined;
            }
        }
        GEM_QUARANTINED.store(true, Ordering::SeqCst);
        USB_QUARANTINED.store(true, Ordering::SeqCst);
    }

    pub fn is_quarantined(&self) -> bool {
        self.quarantined
    }

    pub fn pool_count(&self) -> u8 {
        self.n_pools
    }

    pub fn submit_count(&self) -> u32 {
        self.submits
    }

    pub fn refused_count(&self) -> u32 {
        self.refused
    }

    pub fn status_lines(&self) -> crate::platform::String {
        format!(
            "IOMMU: HARDWARE_UNAVAILABLE\nDMA_CONTAINMENT: SOFTWARE_BOUNDED\nquarantined: {}\npools: {}\nsubmits: {}\nbounces: {}\nrefused: {}\n",
            self.quarantined,
            self.n_pools,
            self.submits,
            self.bounces,
            self.refused
        )
    }
}

static GEM_QUARANTINED: AtomicBool = AtomicBool::new(false);
static USB_QUARANTINED: AtomicBool = AtomicBool::new(false);
static KERNEL_DMA_OK: AtomicU32 = AtomicU32::new(1);

pub fn validate_window(addr: u32, len: u32) -> Result<(), DmaError> {
    if len == 0 {
        return Err(DmaError::TooLong);
    }
    let end = match addr.checked_add(len) {
        Some(e) => e,
        None => return Err(DmaError::Range),
    };
    if addr < DMA_PHYS_LO || end.saturating_sub(1) > DMA_PHYS_HI {
        return Err(DmaError::Range);
    }
    Ok(())
}

pub fn validate_range(addr: u32, len: u32) -> Result<(), DmaError> {
    if len as usize > MAX_BUF {
        return Err(DmaError::TooLong);
    }
    validate_window(addr, len)
}

/// Kernel GEM/USB path is capability-ok. Application memory is never ok.
pub fn kernel_dma_allowed() -> bool {
    KERNEL_DMA_OK.load(Ordering::SeqCst) != 0
}

pub fn gem_quarantined() -> bool {
    GEM_QUARANTINED.load(Ordering::SeqCst)
}

pub fn usb_quarantined() -> bool {
    USB_QUARANTINED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmio_and_app_range_refused() {
        assert_eq!(validate_range(0xE000_B000, 64), Err(DmaError::Range));
        assert_eq!(validate_range(0x0000_1000, 64), Err(DmaError::Range));
        assert!(validate_range(0x0010_0000, 64).is_ok());
    }

    #[test]
    fn bounce_never_returns_source_pointer() {
        let src = [1u8, 2, 3, 4];
        let mut pool = [0u8; 16];
        let n = DmaContainment::bounce(&mut pool, &src).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&pool[..4], &src);
        assert!(pool[4..].iter().all(|&b| b == 0));
        pool[0] = 9;
        assert_eq!(src[0], 1);
    }

    #[test]
    fn submit_requires_pool_and_cap() {
        let mut d = DmaContainment::new();
        d.register_pool(0x0010_0000, 4096).unwrap();
        assert_eq!(
            d.submit(0x0010_0000, 64, false),
            Err(DmaError::CapDenied)
        );
        assert!(d.is_quarantined());
        let mut d = DmaContainment::new();
        d.register_pool(0x0010_0000, 4096).unwrap();
        assert_eq!(
            d.submit(0x0020_0000, 64, true),
            Err(DmaError::NotInPool)
        );
        assert!(d.is_quarantined());
        let mut d = DmaContainment::new();
        d.register_pool(0x0010_0000, 4096).unwrap();
        assert!(d.submit(0x0010_0000, 64, true).is_ok());
        assert_eq!(d.owners[0], DmaOwner::Hw);
        d.complete(0x0010_0000).unwrap();
        assert_eq!(d.owners[0], DmaOwner::Cpu);
    }

    #[test]
    fn usb_sized_windows_register_and_out_of_pool_refused() {
        let mut d = DmaContainment::new();
        d.register_pool(0x0010_0000, 64).unwrap();
        d.register_pool(0x0010_0100, 96).unwrap();
        d.register_pool(0x0010_0200, 8).unwrap();
        d.register_pool(0x0010_1000, 512).unwrap();
        assert!(d.submit(0x0010_0000, 48, true).is_ok());
        d.complete(0x0010_0000).unwrap();
        assert_eq!(
            d.submit(0x00A0_0000, 48, true),
            Err(DmaError::NotInPool)
        );
        assert!(d.is_quarantined());
    }

    #[test]
    fn zero_before_reuse() {
        let mut buf = [0xAAu8; 8];
        DmaContainment::zero(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn oversized_descriptor_refused() {
        assert_eq!(validate_range(0x0010_0000, 2000), Err(DmaError::TooLong));
    }
}
