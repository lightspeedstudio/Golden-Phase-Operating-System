// Copyright (c) 2024–2026 Richard Careaga, Metatron Research.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial

#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod ai;
pub mod anom;
pub mod authority;
pub mod boot_session;
#[cfg(all(feature = "captable", any(feature = "std", feature = "baremetal")))]
pub mod captable;
#[cfg(feature = "craft")]
pub mod craft;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod crypto;
pub mod curvature;
pub mod dma_contain;
pub mod event_log;
#[cfg(all(feature = "extmap", any(feature = "std", feature = "baremetal")))]
pub mod extmap;
pub mod fault_recovery;
pub mod fault_syndrome;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod hash;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod hid;
pub mod usb_hub;
#[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
pub mod identity;
#[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
pub mod ipc;
#[cfg(all(feature = "irqreg", any(feature = "std", feature = "baremetal")))]
pub mod irqreg;
pub mod memory;
#[cfg(feature = "mmu-app-isolation")]
pub mod aspace;
#[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
pub mod netsess;
pub mod network;
#[cfg(feature = "std")]
pub mod phase_benchmark;
pub mod phase_math;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod phase_transport;
#[cfg(all(target_arch = "arm", feature = "bm-net"))]
pub mod net_ip;
#[cfg(all(target_arch = "arm", feature = "ssh-bm"))]
pub mod net_ssh;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod phase_cache;
pub mod phase_index;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod phaselink;
pub mod phasenet;
pub mod phi_stability;
#[cfg(all(feature = "phasestack", any(feature = "std", feature = "baremetal")))]
pub mod phasestack;
#[cfg(all(feature = "phasesentinel", any(feature = "std", feature = "baremetal")))]
pub mod sentinel;
#[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
pub mod pmm;
pub mod phase_consensus;
pub mod process;
#[cfg(feature = "std")]
pub mod remote_shell;
#[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
pub mod sched2;
pub mod scheduler;
pub mod state_domain;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod state_proof;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod state_root;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod checkpoint_auth;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod statechain;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod statechain_checkpoint;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod statechain_watchdog;
#[cfg(all(feature = "cpu1", feature = "tensor"))]
pub mod phase_hash_parallel;
#[cfg(feature = "cpu1")]
pub mod spsc_mailbox;
#[cfg(feature = "tensor")]
pub mod tensor_ring;
#[cfg(feature = "tensor")]
pub mod tensor_state;
pub mod syscall;
pub mod trace;
#[cfg(all(feature = "txlog", any(feature = "std", feature = "baremetal")))]
pub mod txlog;

#[path = "kernel.rs"]
mod facade;
pub use facade::Kernel;
