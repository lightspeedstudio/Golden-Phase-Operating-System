// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Handoff to the application in DDR (UG585: load address ≥ 1 MiB).

pub unsafe fn jump(entry: u32) -> ! {
    core::arch::asm!(
        "mcr p15, 0, {z}, c7, c5, 0",
        "mcr p15, 0, {z}, c7, c5, 6",
        "dsb",
        "isb",
        z = in(reg) 0u32,
        options(nostack),
    );
    let f: extern "C" fn() -> ! = core::mem::transmute(entry as usize);
    f()
}
