//! PhaseNet firewall — verify frames before state is accepted.

use crate::kernel::authority::StateHash;
use crate::kernel::phasenet::link::PhaseLinkFrame;
use crate::platform::{HashSet, String, Vec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseNetVerdict {
    Accept,
    Quarantine,
    Drop,
}

#[derive(Debug, Clone)]
pub struct FirewallContext {
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub trusted_peer_addrs: HashSet<StateHash>,
    pub anom_protected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirewallFailure {
    EquationRoot,
    UnknownSender,
    BootSession,
    Sequence,
    FrameHash,
    ProofHash,
    ChainLink,
}

impl FirewallFailure {
    pub fn reason(&self) -> &'static str {
        match self {
            FirewallFailure::EquationRoot => "equation root mismatch",
            FirewallFailure::UnknownSender => "unknown sender",
            FirewallFailure::BootSession => "boot session mismatch",
            FirewallFailure::Sequence => "sequence not advancing",
            FirewallFailure::FrameHash => "frame hash invalid",
            FirewallFailure::ProofHash => "packet proof invalid",
            FirewallFailure::ChainLink => "previous frame hash mismatch",
        }
    }
}

pub struct FirewallResult {
    pub verdict: PhaseNetVerdict,
    pub failures: Vec<FirewallFailure>,
}

impl FirewallResult {
    pub fn is_accepted(&self) -> bool {
        self.verdict == PhaseNetVerdict::Accept
    }
}

#[cfg(feature = "std")]
pub fn evaluate_frame(
    frame: &PhaseLinkFrame,
    ctx: &FirewallContext,
    expected_sequence: u64,
    expected_prev_hash: &StateHash,
) -> FirewallResult {
    let mut failures = Vec::new();

    if frame.equation_root != ctx.equation_root {
        failures.push(FirewallFailure::EquationRoot);
    }
    if !ctx.trusted_peer_addrs.contains(&frame.src_addr)
        && frame.boot_session_root != ctx.boot_session_root
    {
        failures.push(FirewallFailure::UnknownSender);
    }
    if frame.boot_session_root != ctx.boot_session_root
        && !ctx.trusted_peer_addrs.contains(&frame.src_addr)
    {
        failures.push(FirewallFailure::BootSession);
    }
    if frame.sequence <= expected_sequence {
        failures.push(FirewallFailure::Sequence);
    }
    if !frame.verify_frame_hash() {
        failures.push(FirewallFailure::FrameHash);
    }
    if frame.previous_frame_hash != *expected_prev_hash {
        failures.push(FirewallFailure::ChainLink);
    }
    if let Ok(packet) = frame.packet() {
        if !packet.verify() || packet.proof_hash != frame.proof_hash {
            failures.push(FirewallFailure::ProofHash);
        }
        let _ = ctx.anom_protected;
    }

    let verdict = if failures.is_empty() {
        PhaseNetVerdict::Accept
    } else if failures.len() == 1
        && matches!(
            failures[0],
            FirewallFailure::BootSession | FirewallFailure::UnknownSender
        )
    {
        PhaseNetVerdict::Quarantine
    } else {
        PhaseNetVerdict::Drop
    };

    FirewallResult { verdict, failures }
}

#[cfg(not(feature = "std"))]
pub fn evaluate_frame(
    _frame: &PhaseLinkFrame,
    _ctx: &FirewallContext,
    _expected_sequence: u64,
    _expected_prev_hash: &StateHash,
) -> FirewallResult {
    FirewallResult {
        verdict: PhaseNetVerdict::Drop,
        failures: Vec::new(),
    }
}

pub fn render_verdict(verdict: PhaseNetVerdict) -> &'static str {
    match verdict {
        PhaseNetVerdict::Accept => "ACCEPT",
        PhaseNetVerdict::Quarantine => "QUARANTINE",
        PhaseNetVerdict::Drop => "DROP",
    }
}

pub fn render_failures(failures: &[FirewallFailure]) -> String {
    if failures.is_empty() {
        return "none".into();
    }
    failures
        .iter()
        .map(|f| f.reason())
        .collect::<Vec<_>>()
        .join(", ")
}
