//! Remote command authority policy.

use crate::kernel::remote_shell::error::RemoteShellError;
use crate::kernel::remote_shell::types::RemoteAuthority;
use crate::userland::parser::Command;

pub fn required_authority(cmd: &Command) -> RemoteAuthority {
    match cmd {
        Command::Help
        | Command::About
        | Command::Version
        | Command::State
        | Command::Clock
        | Command::ClockSkew
        | Command::Ps
        | Command::Trace
        | Command::Mem
        | Command::Log
        | Command::Dashboard
        | Command::Phase
        | Command::Topo
        | Command::Root
        | Command::Audit
        | Command::Watchdog
        | Command::Regs
        | Command::Isa
        | Command::Dump
        | Command::NetStatus
        | Command::PhiStabilityStatus
        | Command::PhaseBenchmarkStatus
        | Command::PhasePerfStatus
        | Command::PhaseLinkStatus
        | Command::PhaseNetStatus
        | Command::AnomStatus
        | Command::AnomList
        | Command::AnomVerify
        | Command::AnomObjectStatus { .. }
        | Command::StateModeQuery
        | Command::StateDomains
        | Command::StateMap { .. }
        | Command::StateView { .. }
        | Command::StateProof { .. }
        | Command::AiStatus
        | Command::AiJobs
        | Command::AiShow { .. }
        | Command::AiTrace { .. }
        | Command::AiTools
        | Command::SshStatus
        | Command::SshSessions
        | Command::SshSessionShow { .. }
        | Command::SshConfig
        | Command::SshKeyList
        | Command::SshHostKeyFingerprint
        | Command::SshHostKeyStatus
        | Command::SshAudit => RemoteAuthority::Viewer,
        Command::AnomSecurity { .. }
        | Command::AnomOff { .. }
        | Command::AnomProtect { .. }
        | Command::AnomOpen { .. }
        | Command::AnomClose { .. }
        | Command::AnomRekey { .. }
        | Command::AnomUnprotect { .. }
        | Command::AnomSealTest { .. } => RemoteAuthority::AnomAdministrator,
        Command::Checkpoint
        | Command::CheckpointAuthProvision
        | Command::CheckpointAuthSeal
        | Command::CheckpointAuthVerify
        | Command::SshHostKeyRegenerate
        | Command::SshKeyAdd { .. }
        | Command::SshKeyRemove { .. }
        | Command::SshKeyEnable { .. }
        | Command::SshKeyDisable { .. }
        | Command::SshSessionClose { .. }
        | Command::Kill { .. }
        | Command::FsjP6 { .. }
        | Command::FsjP7 { .. }
        | Command::FsjP8 { .. }
        | Command::FsjP10 { .. }
        | Command::FsjP11 { .. }
        | Command::FsjP1Reset => RemoteAuthority::Root,
        _ => RemoteAuthority::Operator,
    }
}

pub fn authorize_command(
    session_authority: RemoteAuthority,
    cmd: &Command,
) -> Result<(), RemoteShellError> {
    let needed = required_authority(cmd);
    if authority_allows(session_authority, needed) {
        Ok(())
    } else {
        Err(RemoteShellError::AuthorityDenied)
    }
}

fn authority_allows(have: RemoteAuthority, need: RemoteAuthority) -> bool {
    match have {
        RemoteAuthority::Root => true,
        RemoteAuthority::AnomAdministrator => matches!(
            need,
            RemoteAuthority::Viewer
                | RemoteAuthority::Operator
                | RemoteAuthority::AnomAdministrator
        ),
        RemoteAuthority::Operator => {
            matches!(need, RemoteAuthority::Operator | RemoteAuthority::Viewer)
        }
        RemoteAuthority::Viewer => need == RemoteAuthority::Viewer,
    }
}

pub fn command_category(cmd: &Command) -> &'static str {
    match cmd {
        Command::Unknown(_) | Command::Usage(_) => "unknown",
        Command::Exit => "session",
        Command::AnomSecurity { .. }
        | Command::AnomOff { .. }
        | Command::AnomProtect { .. }
        | Command::AnomOpen { .. }
        | Command::AnomClose { .. }
        | Command::AnomRekey { .. }
        | Command::AnomUnprotect { .. }
        | Command::AnomSealTest { .. }
        | Command::AnomStatus
        | Command::AnomList
        | Command::AnomVerify
        | Command::AnomObjectStatus { .. } => "anom",
        Command::SshStatus
        | Command::SshStart
        | Command::SshStop
        | Command::SshRestart
        | Command::SshSessions
        | Command::SshSessionShow { .. }
        | Command::SshSessionClose { .. }
        | Command::SshConfig
        | Command::SshKeyList
        | Command::SshKeyAdd { .. }
        | Command::SshKeyRemove { .. }
        | Command::SshKeyEnable { .. }
        | Command::SshKeyDisable { .. }
        | Command::SshHostKeyFingerprint
        | Command::SshHostKeyStatus
        | Command::SshHostKeyRegenerate
        | Command::SshAudit => "ssh",
        Command::Checkpoint
        | Command::CheckpointAuthStatus
        | Command::CheckpointAuthProvision
        | Command::CheckpointAuthSeal
        | Command::CheckpointAuthVerify
        | Command::CheckpointAuthHardware
        | Command::Audit => "statechain",
        _ => "operator",
    }
}
