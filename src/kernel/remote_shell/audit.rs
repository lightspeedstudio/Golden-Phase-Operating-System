//! SSH statechain audit records (bounded public commitments).

use crate::core::gphase_core::PhaseState;
use crate::kernel::remote_shell::types::SSH_STATECHAIN_OBJECT_ID;
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;

pub fn record_ssh_event(kernel: &mut Kernel, event: &str, detail: &[u8]) {
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        SSH_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        PhaseState::Phi2,
        PhaseState::Phi3,
        event,
        None,
        detail,
    );
}

pub fn redact_command(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("anom") && (lower.contains("-p") || lower.contains("passphrase")) {
        return "anom:<redacted>".into();
    }
    if lower.starts_with("ai ask ") {
        return "ai ask:<redacted>".into();
    }
    let head = line.split_whitespace().next().unwrap_or("cmd");
    if line.len() > 64 {
        format!("{head}:<truncated>")
    } else {
        line.into()
    }
}
