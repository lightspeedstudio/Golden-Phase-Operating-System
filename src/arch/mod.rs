//! CPU architecture ports for bare-metal Golden Phase OS.

#[cfg(any(
    all(feature = "zynq-a9", target_arch = "arm"),
    feature = "mmu-app-isolation"
))]
pub mod armv7;
