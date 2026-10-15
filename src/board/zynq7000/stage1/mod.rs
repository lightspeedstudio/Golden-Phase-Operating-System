// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Golden Phase Stage 1 — owned First Stage Boot Loader (OCM).
//!
//! Not Xilinx FSBL. License: LightSpeed Studio LLC / Golden Phase.

pub mod auth;
#[cfg(target_arch = "arm")]
pub mod boot_mode;
#[cfg(target_arch = "arm")]
pub mod handoff;
pub mod measure;
#[cfg(target_arch = "arm")]
pub mod mmio;
#[cfg(target_arch = "arm")]
pub mod pcap;
#[cfg(target_arch = "arm")]
pub mod ps7;
#[cfg(target_arch = "arm")]
pub mod wdt;
