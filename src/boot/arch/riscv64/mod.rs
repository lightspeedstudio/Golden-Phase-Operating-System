//! RISC-V `virt` board support.

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
pub mod entry;
#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
#[cfg(all(feature = "baremetal", target_arch = "riscv64", feature = "bm-net"))]
pub mod phaselink_eth;
#[cfg(all(feature = "baremetal", target_arch = "riscv64", feature = "bm-net"))]
pub mod pnw1_peer;
pub mod timer;
pub mod trap;
pub mod uart;
#[cfg(all(feature = "baremetal", target_arch = "riscv64", feature = "bm-net"))]
pub mod virtio_net;

pub fn init() {
    uart::init();
}

pub fn late_init() {}
