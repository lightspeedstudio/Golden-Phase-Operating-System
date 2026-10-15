//! ANOM entry/exit safety policy — Audited Non-Observable Mode.

use crate::kernel::authority::StateHash;
use crate::kernel::authority::{AuthorityMode, AuthorityState, VisibilityMode};
use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    BootIncomplete,
    AlreadyAnom,
    NotAnom,
    BootSessionUnavailable,
    EquationRootUnavailable,
    StatechainUnavailable,
    PassphraseRejected,
    CryptoUnavailable,
}

impl PolicyError {
    pub fn reason(&self) -> &'static str {
        match self {
            PolicyError::BootIncomplete => "boot not completed",
            PolicyError::AlreadyAnom => "already in ANOM mode",
            PolicyError::NotAnom => "not in ANOM mode",
            PolicyError::BootSessionUnavailable => "missing boot session root",
            PolicyError::EquationRootUnavailable => "equation root unavailable",
            PolicyError::StatechainUnavailable => "statechain unavailable",
            PolicyError::PassphraseRejected => "passphrase rejected",
            PolicyError::CryptoUnavailable => "ANOM crypto unavailable on this build",
        }
    }
}

pub fn boot_session_ready(root: &StateHash) -> bool {
    *root != [0u8; 32]
}

pub fn can_enter(
    boot_completed: bool,
    authority: &AuthorityState,
    equation_root: &StateHash,
    statechain_ok: bool,
    crypto_ok: bool,
) -> Result<(), PolicyError> {
    if !boot_completed {
        return Err(PolicyError::BootIncomplete);
    }
    if authority.authority == AuthorityMode::Anom {
        return Err(PolicyError::AlreadyAnom);
    }
    if *equation_root == [0u8; 32] {
        return Err(PolicyError::EquationRootUnavailable);
    }
    if !boot_session_ready(&authority.boot_session_root) {
        return Err(PolicyError::BootSessionUnavailable);
    }
    if !statechain_ok {
        return Err(PolicyError::StatechainUnavailable);
    }
    if !crypto_ok {
        return Err(PolicyError::CryptoUnavailable);
    }
    Ok(())
}

pub fn can_exit(authority: &AuthorityState) -> Result<(), PolicyError> {
    if authority.authority != AuthorityMode::Anom
        || authority.visibility != VisibilityMode::AnomProtected
    {
        return Err(PolicyError::NotAnom);
    }
    Ok(())
}

pub fn authority_label(mode: AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::Root => "root",
        AuthorityMode::Anom => "anom",
    }
}

pub fn visibility_label(mode: VisibilityMode) -> &'static str {
    match mode {
        VisibilityMode::Transparent => "transparent",
        VisibilityMode::Sealed => "sealed",
        VisibilityMode::AnomProtected => "protected",
        VisibilityMode::Recovery => "recovery",
    }
}

pub fn format_policy_failure(err: &PolicyError) -> String {
    format!("reason: {}", err.reason())
}
