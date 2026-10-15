// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Fail-closed Stage 1 policy for a mission-ready boot.

use crate::board::zynq_boot_auth::{self, SIG_LEN, STAGE1_ROOT_PUBKEY};
use crate::board::zynq_bootimg::{self, BootImgError, PartitionHeader};
use crate::kernel::dma_contain::{DMA_PHYS_HI, DMA_PHYS_LO};
use crate::kernel::hash::StateHash;

use super::measure::BootMeasure;

pub fn ddr_span(addr: u32, len: u32) -> bool {
    match addr.checked_add(len) {
        Some(end) if end > addr => addr >= DMA_PHYS_LO && (end - 1) <= DMA_PHYS_HI,
        _ => false,
    }
}

pub fn check_app(p: &PartitionHeader) -> Result<(), BootImgError> {
    if !ddr_span(p.load_addr, p.byte_len() as u32) {
        return Err(BootImgError::BadRange);
    }
    if p.exec_addr < zynq_bootimg::APP_LOAD_MIN {
        return Err(BootImgError::BadRange);
    }
    Ok(())
}

pub fn check_pcap_src(ptr: u32, len: u32) -> Result<(), BootImgError> {
    if !ddr_span(ptr, len) {
        return Err(BootImgError::BadRange);
    }
    Ok(())
}

pub fn verify_measure(m: &BootMeasure, sig: &[u8; SIG_LEN]) -> Result<(), BootImgError> {
    if zynq_boot_auth::verify_image_root(&STAGE1_ROOT_PUBKEY, &m.image_root, sig) {
        Ok(())
    } else {
        Err(BootImgError::BadSignature)
    }
}

pub fn root_hex(root: &StateHash, out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, b) in root.iter().enumerate() {
        out[i * 2] = HEX[(b >> 4) as usize];
        out[i * 2 + 1] = HEX[(b & 0xF) as usize];
    }
}
