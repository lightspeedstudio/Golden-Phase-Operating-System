//! Recompute and audit equation-rooted statechains.

use crate::kernel::hash::StateHash;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::state_root::{
    compute_equation_root, compute_kernel_state_root, compute_prime_log_commitment,
};
use crate::kernel::statechain::StatechainLedger;
use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatechainAuditReport {
    pub equation_ok: bool,
    pub chains_ok: bool,
    pub kernel_root_ok: bool,
    pub signed_checkpoints_ok: bool,
    pub stored_root: StateHash,
    pub recomputed_root: StateHash,
    pub tamper_detected: bool,
    pub message: String,
}

pub struct StatechainWatchdog {
    equation_root: StateHash,
    prime_log: StateHash,
}

impl Default for StatechainWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

impl StatechainWatchdog {
    pub fn new() -> Self {
        let equation = compute_equation_root();
        let prime = compute_prime_log_commitment(&equation);
        Self {
            equation_root: equation.hash,
            prime_log: prime.hash,
        }
    }

    pub fn verify_equation_root(&self, ledger: &StatechainLedger) -> bool {
        ledger.equation_root() == self.equation_root
    }

    pub fn verify_kernel_root(&self, ledger: &StatechainLedger) -> (bool, StateHash) {
        let recomputed = compute_kernel_state_root(
            &ledger.equation_root(),
            &ledger.heads_for_audit(SubsystemKind::Process),
            &ledger.heads_for_audit(SubsystemKind::Memory),
            &ledger.heads_for_audit(SubsystemKind::Filesystem),
            &ledger.heads_for_audit(SubsystemKind::Vm),
            &ledger.heads_for_audit(SubsystemKind::Syscall),
            &ledger.heads_for_audit(SubsystemKind::Driver),
            &ledger.heads_for_audit(SubsystemKind::Craft),
        );
        (ledger.kernel_root() == recomputed.hash, recomputed.hash)
    }

    pub fn verify_signed_checkpoints(&self, ledger: &StatechainLedger) -> bool {
        ledger.verify_checkpoints().is_ok()
    }

    pub fn audit(&self, ledger: &StatechainLedger) -> StatechainAuditReport {
        let equation_ok = self.verify_equation_root(ledger);
        let chains_ok = ledger.verify_all().is_ok();
        let (kernel_root_ok, recomputed_root) = self.verify_kernel_root(ledger);
        let signed_checkpoints_ok =
            ledger.checkpoints().is_empty() || self.verify_signed_checkpoints(ledger);
        let stored_root = ledger.kernel_root();
        let tamper_detected =
            !(equation_ok && chains_ok && kernel_root_ok && signed_checkpoints_ok);
        let message = if tamper_detected {
            "tamper or chain mismatch detected".into()
        } else {
            "statechain audit passed".into()
        };
        StatechainAuditReport {
            equation_ok,
            chains_ok,
            kernel_root_ok,
            signed_checkpoints_ok,
            stored_root,
            recomputed_root,
            tamper_detected,
            message,
        }
    }

    pub fn prime_log_root(&self) -> StateHash {
        self.prime_log
    }
}
