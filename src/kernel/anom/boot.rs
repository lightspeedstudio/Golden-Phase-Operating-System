//! Boot-time authority initialization — always root/transparent.

use crate::kernel::anom::mode::AnomStatus;
use crate::kernel::authority::{AuthorityMode, VisibilityMode};
use crate::kernel::Kernel;

pub fn finalize_boot(kernel: &mut Kernel, init_pid: u32) {
    kernel.authority.authority = AuthorityMode::Root;
    kernel.authority.visibility = VisibilityMode::Transparent;
    kernel.anom.mark_boot_completed();

    // Real BootSessionRoot on every build that has a statechain to bind
    // (hosted std AND bare metal) — never left as the zeroed Default.
    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let equation_root = kernel.statechain.equation_root();
        let kernel_root = kernel.statechain.kernel_root();
        let boot_tick = kernel.clock.proper_tick;
        kernel.authority.boot_session_root = crate::kernel::boot_session::compute_boot_session_root(
            &equation_root,
            init_pid,
            &kernel_root,
            boot_tick,
        );
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    {
        let equation_root = kernel.statechain.equation_root();
        kernel.anom.set_status(AnomStatus::Available);
        kernel.init_state_domain();
        kernel.phaselink.init(
            equation_root,
            kernel.authority.boot_session_root,
            "local",
        );
        kernel.phasenet.init(
            equation_root,
            kernel.authority.boot_session_root,
            "kairos-local",
        );
        super::protocol::load_root_history(kernel);
        let bsr = kernel.authority.boot_session_root;
        let eq = kernel.statechain.equation_root();
        let mut registry = core::mem::take(&mut kernel.object_protection.registry);
        registry.load_from_fs(&mut kernel.fs, &bsr, &eq, &kernel.statechain);
        #[cfg(feature = "std")]
        registry.load_all(&bsr, &eq, &kernel.statechain);
        kernel.object_protection.registry = registry;
    }

    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        let _ = init_pid;
        kernel.anom.set_status(AnomStatus::BaremetalStub);
    }
}
