//! Zynq-7000 / PZ7020-SL-C platform modules.

pub mod fat;
pub mod g1p;
pub mod g64;
#[cfg(target_arch = "arm")]
pub mod gem;
#[cfg(all(target_arch = "arm", feature = "bm-net"))]
pub mod gem_phy;
#[cfg(target_arch = "arm")]
pub mod gic;
#[cfg(target_arch = "arm")]
pub mod kernel_main;
#[cfg(all(target_arch = "arm", feature = "cpu1"))]
pub mod cpu1;
pub mod memory_map;
#[cfg(target_arch = "arm")]
pub mod phaselink_eth;
pub mod ps7_init;
pub mod ps7_init_data;
#[cfg(all(target_arch = "arm", any(feature = "ssh-bm", feature = "identity")))]
pub mod rng;
pub mod sd;
#[cfg(all(target_arch = "arm", feature = "phasefs-journal"))]
pub mod journal_sd;
pub mod uart;
pub mod stage1;
#[cfg(all(target_arch = "arm", feature = "usb"))]
pub mod usb;
#[cfg(all(target_arch = "arm", feature = "usb"))]
pub mod usb_hid;
