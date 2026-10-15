//! `root` — Merkle roots for all subsystems.

use crate::kernel::Kernel;
use crate::platform::String;

#[cfg(feature = "std")]
use crate::kernel::state_root::{
    compute_equation_root, compute_prime_log_commitment, SubsystemKind,
};
#[cfg(feature = "std")]
use crate::userland::terminal_fmt::short_hex;

pub fn render(kernel: &Kernel) -> String {
    let mut out = String::new();

    #[cfg(feature = "std")]
    {
        let eq = compute_equation_root();
        let prime = compute_prime_log_commitment(&eq);
        out.push_str(&format!("EquationRoot:        {}…\n", short_hex(&eq.hash)));
        out.push_str(&format!(
            "PrimeLogCommitment:  {}…\n",
            short_hex(&prime.hash)
        ));
        out.push_str(&format!(
            "ProcessRoot:         {}…\n",
            short_hex(
                &kernel
                    .statechain
                    .subsystem_merkle_root(SubsystemKind::Process)
            )
        ));
        out.push_str(&format!(
            "MemoryRoot:          {}…\n",
            short_hex(
                &kernel
                    .statechain
                    .subsystem_merkle_root(SubsystemKind::Memory)
            )
        ));
        out.push_str(&format!(
            "FilesystemRoot:      {}…\n",
            short_hex(
                &kernel
                    .statechain
                    .subsystem_merkle_root(SubsystemKind::Filesystem)
            )
        ));
        out.push_str(&format!(
            "VmRoot:              {}…\n",
            short_hex(&kernel.statechain.subsystem_merkle_root(SubsystemKind::Vm))
        ));
        out.push_str(&format!(
            "SyscallRoot:         {}…\n",
            short_hex(
                &kernel
                    .statechain
                    .subsystem_merkle_root(SubsystemKind::Syscall)
            )
        ));
        out.push_str(&format!(
            "DriverRoot:          {}…\n",
            short_hex(
                &kernel
                    .statechain
                    .subsystem_merkle_root(SubsystemKind::Driver)
            )
        ));
        out.push_str(&format!(
            "KernelStateRoot:     {}…\n",
            short_hex(&kernel.statechain.kernel_root())
        ));
    }

    #[cfg(not(feature = "std"))]
    {
        let _ = kernel;
        out.push_str("root view requires hosted (std) build\n");
    }

    out
}
