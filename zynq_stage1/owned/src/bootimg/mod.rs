// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Zynq-7000 `BOOT.BIN` layout (UG585). Independent of the packer.

pub mod parse;
pub mod frame;
#[cfg(feature = "std")]
pub mod pack;
#[cfg(feature = "std")]
pub mod stream;
#[cfg(feature = "std")]
pub mod bit;
