//! BootSessionRoot — binds boot-time kernel context (public, not secret).
//!
//! Computed via PhaseHash v1's dedicated boot-chain hash
//! ([`crate::kernel::crypto::phase_hash::ph_boot_session_root`]), not a
//! generic tagged hash — this is a distinct construction from PH-S because
//! the boot session binds `(equation_root, init_pid, kernel_root, boot_tick)`,
//! not the filesystem-shaped `(previous, chronos, kairos_sector, metadata)`.

use crate::kernel::authority::StateHash;
use crate::kernel::event_log::ProcessId;

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn compute_boot_session_root(
    equation_root: &StateHash,
    init_pid: ProcessId,
    kernel_root: &StateHash,
    boot_tick: u64,
) -> StateHash {
    use crate::kernel::crypto::phase_hash::{ph_boot_session_root, BootSessionRootInput};
    ph_boot_session_root(&BootSessionRootInput {
        equation_root: *equation_root,
        init_pid,
        kernel_root: *kernel_root,
        boot_tick,
    })
}
