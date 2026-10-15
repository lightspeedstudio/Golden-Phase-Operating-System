//! Cortex-A9 / ARMv7-A entry for Golden Phase OS on Zynq.

#![allow(dead_code)]

#[cfg(all(target_arch = "arm", feature = "mmu-app-isolation", not(feature = "cpu1")))]
compile_error!("mmu-app-isolation on Cortex-A9 requires the cpu1 feature");

pub mod mmu;

#[cfg(feature = "mmu-app-isolation")]
pub mod payloads;

#[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
pub mod isolation;

#[cfg(all(target_arch = "arm", not(feature = "mmu-app-isolation")))]
core::arch::global_asm!(include_str!("startup.S"));

#[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
core::arch::global_asm!(include_str!("startup_iso.S"));

#[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
core::arch::global_asm!(include_str!("user.S"));

#[cfg(all(feature = "cpu1", target_arch = "arm"))]
core::arch::global_asm!(include_str!("cpu1_entry.S"));
