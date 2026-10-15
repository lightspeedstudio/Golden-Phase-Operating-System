// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Stage 1 measurement using kernel PhaseHash (`ph_content` / `ph_boot_image_root`).
//!
//! This is the kernel math that belongs in the bootloader: domain-separated
//! hashes of the three BOOT.BIN payloads. Scheduler / StateClock / phase_math
//! SIMD do not run in OCM.

use crate::kernel::crypto::phase_hash::{ph_boot_image_root, ph_content};
use crate::kernel::hash::StateHash;

/// DDR mailbox below the 1 MiB application window (not OCM).
pub const BOOT_IMAGE_ROOT_ADDR: usize = 0x000F_0000;
pub const BOOT_IMAGE_ROOT_MAGIC: u32 = 0x4750_4231; // GPB1

#[derive(Clone, Copy)]
pub struct BootMeasure {
    pub stage1: StateHash,
    pub bitstream: StateHash,
    pub kernel: StateHash,
    pub image_root: StateHash,
}

pub fn measure(stage1: &[u8], bitstream: &[u8], kernel: &[u8]) -> BootMeasure {
    let s = ph_content(stage1);
    let b = ph_content(bitstream);
    let k = ph_content(kernel);
    BootMeasure {
        stage1: s,
        bitstream: b,
        kernel: k,
        image_root: ph_boot_image_root(&s, &b, &k),
    }
}

/// Write magic + BootImageRoot for the kernel to read later.
pub unsafe fn store(m: &BootMeasure) {
    let p = BOOT_IMAGE_ROOT_ADDR as *mut u8;
    core::ptr::write_volatile(p as *mut u32, BOOT_IMAGE_ROOT_MAGIC);
    core::ptr::copy_nonoverlapping(m.image_root.as_ptr(), p.add(4), 32);
}
