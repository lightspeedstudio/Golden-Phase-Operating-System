//! QEMU boot layer — serial console, timer, traps, and `kernel_main`.

pub mod arch;
pub mod memory_map;
pub mod serial;

#[cfg(feature = "baremetal")]
pub mod heap;
#[cfg(feature = "baremetal")]
pub mod kernel_main;

#[cfg(feature = "baremetal")]
pub use kernel_main::kernel_main;
