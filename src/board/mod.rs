//! Board packages (seller-derived glue; replace per board).

/// Cadence GEM / Xilinx emacps bit helpers (host-testable; no MMIO).
pub mod gem_cadence_hw;
/// Zynq `BOOT.BIN` image headers (host-testable; no MMIO).
pub mod zynq_bootimg;
/// Stage 1 image-root signatures (host-testable).
pub mod zynq_boot_auth;

#[cfg(feature = "zynq-a9")]
pub mod zynq7000;
