//! PhaseAppLoader — accepts only `&VerifiedInstallation`.

use crate::kernel::crypto::phase_hash::ph_code;
use crate::kernel::phasestack::types::{
    AppExecutionKind, AppState, PhaseStackError, VerifiedInstallation,
};
use crate::platform::Vec;

#[derive(Debug, Clone)]
pub struct LoadedApp {
    pub code: Vec<u8>,
    pub exec_kind: AppExecutionKind,
    pub granted_caps: u32,
}

pub struct PhaseAppLoader;

impl PhaseAppLoader {
    pub fn load(inst: &VerifiedInstallation) -> Result<LoadedApp, PhaseStackError> {
        if inst.state() == AppState::Quarantined {
            return Err(PhaseStackError::Quarantined);
        }
        if inst.state() == AppState::Revoked {
            return Err(PhaseStackError::Revoked);
        }
        let code = inst.package().code();
        if ph_code(code) != inst.package().code_hash() {
            return Err(PhaseStackError::LoadDenied);
        }
        Ok(LoadedApp {
            code: code.to_vec(),
            exec_kind: inst.package().exec_kind(),
            granted_caps: inst.granted_caps(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::phasestack::capability::GrantSet;
    use crate::kernel::phasestack::types::{
        AppVersion, ABI_VERSION_V1, InstallScope, PKG_TYPE_APP, VerifiedPackage,
    };
    use crate::kernel::phasestack::verifier::commit_installation;
    use crate::vm::instruction::{encode_program, Instruction};

    fn halt_inst() -> VerifiedInstallation {
        let code = encode_program(&[Instruction::Halt]);
        let hash = ph_code(&code);
        let pkg = VerifiedPackage::from_parts(
            [1u8; 32],
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            [2u8; 32],
            hash,
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            AppExecutionKind::PhaseVm,
            0,
            0,
            ABI_VERSION_V1,
            "t".into(),
            code,
            PKG_TYPE_APP,
            crate::kernel::phasestack::types::ExecEnvelope::default(),
        );
        commit_installation(
            pkg,
            GrantSet::empty(),
            InstallScope::User,
            "/phase/packages/x".into(),
        )
        .unwrap()
    }

    #[test]
    fn load_requires_verified_installation() {
        let inst = halt_inst();
        let loaded = PhaseAppLoader::load(&inst).unwrap();
        assert_eq!(loaded.exec_kind, AppExecutionKind::PhaseVm);
    }

    #[test]
    fn hash_mismatch_is_load_denied() {
        let pkg = VerifiedPackage::from_parts(
            [1u8; 32],
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            [2u8; 32],
            [0x11u8; 32],
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            AppExecutionKind::PhaseVm,
            0,
            0,
            ABI_VERSION_V1,
            "t".into(),
            encode_program(&[Instruction::Halt]),
            PKG_TYPE_APP,
            crate::kernel::phasestack::types::ExecEnvelope::default(),
        );
        let inst = commit_installation(
            pkg,
            GrantSet::empty(),
            InstallScope::User,
            "/x".into(),
        )
        .unwrap();
        assert_eq!(
            PhaseAppLoader::load(&inst).unwrap_err(),
            PhaseStackError::LoadDenied
        );
    }
}
