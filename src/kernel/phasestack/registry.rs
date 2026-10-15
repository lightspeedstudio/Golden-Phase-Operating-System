//! Installed verified-app registry.

use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::types::{
    AppId, AppState, PackageRoot, PhaseStackError, VerifiedInstallation, MAX_APPS,
};
use crate::kernel::state_root::merkle_root;
use crate::platform::{BTreeMap, Vec};

#[derive(Debug, Clone, Default)]
pub struct AppRegistry {
    by_app: BTreeMap<AppId, VerifiedInstallation>,
    journal: Vec<PackageRoot>,
    generation: u64,
    next_install_sequence: u64,
}

impl AppRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn next_install_sequence(&self) -> u64 {
        self.next_install_sequence
    }

    pub fn insert(
        &mut self,
        mut inst: VerifiedInstallation,
    ) -> Result<(), PhaseStackError> {
        if self.by_app.len() >= MAX_APPS {
            return Err(PhaseStackError::BoundExceeded);
        }
        let id = inst.package().app_id();
        if self.by_app.contains_key(&id) {
            return Err(PhaseStackError::AlreadyInstalled);
        }
        self.generation = self.generation.saturating_add(1);
        self.next_install_sequence = self.next_install_sequence.saturating_add(1);
        inst.set_registry_generation(self.generation);
        inst.set_install_sequence(self.next_install_sequence);
        self.journal.push(inst.package().package_root());
        self.by_app.insert(id, inst);
        Ok(())
    }

    pub fn bump_generation(&mut self) -> Result<u64, PhaseStackError> {
        if self.generation == u64::MAX {
            return Err(PhaseStackError::BoundExceeded);
        }
        self.generation = self.generation.saturating_add(1);
        for inst in self.by_app.values_mut() {
            inst.set_registry_generation(self.generation);
        }
        Ok(self.generation)
    }

    #[cfg(any(test, feature = "phasesentinel-research"))]
    pub fn test_set_install_sequence(
        &mut self,
        id: AppId,
        sequence: u64,
    ) -> Result<(), PhaseStackError> {
        let inst = self.by_app.get_mut(&id).ok_or(PhaseStackError::NotFound)?;
        inst.set_install_sequence(sequence);
        Ok(())
    }

    pub fn get(&self, id: AppId) -> Option<&VerifiedInstallation> {
        self.by_app.get(&id)
    }

    pub fn get_mut(&mut self, id: AppId) -> Option<&mut VerifiedInstallation> {
        self.by_app.get_mut(&id)
    }

    pub fn remove(&mut self, id: AppId) -> Option<VerifiedInstallation> {
        self.by_app.remove(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AppId, &VerifiedInstallation)> {
        self.by_app.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.by_app.is_empty()
    }

    pub fn installed_package_root(&self) -> StateHash {
        let mut leaves: Vec<StateHash> = self
            .by_app
            .values()
            .map(|i| i.package().package_root())
            .collect();
        leaves.sort_unstable();
        merkle_root(&leaves)
    }

    pub fn granted_capability_root(&self) -> StateHash {
        let mut leaves: Vec<StateHash> = Vec::new();
        for inst in self.by_app.values() {
            let mut leaf = [0u8; 32];
            leaf[..4].copy_from_slice(&inst.granted_caps().to_le_bytes());
            leaf[4..8].copy_from_slice(&inst.granted_hooks().to_le_bytes());
            leaves.push(leaf);
        }
        leaves.sort_unstable();
        merkle_root(&leaves)
    }

    pub fn rollback_last(&mut self) {
        if let Some(root) = self.journal.pop() {
            let key = self
                .by_app
                .iter()
                .find(|(_, v)| v.package().package_root() == root)
                .map(|(k, _)| *k);
            if let Some(k) = key {
                self.by_app.remove(&k);
            }
        }
    }

    pub fn set_state(&mut self, id: AppId, state: AppState) -> Result<(), PhaseStackError> {
        let inst = self.by_app.get_mut(&id).ok_or(PhaseStackError::NotFound)?;
        inst.set_state(state);
        Ok(())
    }

    pub fn set_process_id(&mut self, id: AppId, pid: Option<u32>) -> Result<(), PhaseStackError> {
        let inst = self.by_app.get_mut(&id).ok_or(PhaseStackError::NotFound)?;
        inst.set_process_id(pid);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::phasestack::capability::GrantSet;
    use crate::kernel::phasestack::types::{
        AppExecutionKind, AppVersion, ABI_VERSION_V1, InstallScope, PKG_TYPE_APP, VerifiedPackage,
    };

    fn inst(name: &str, root_byte: u8) -> VerifiedInstallation {
        let mut root = [0u8; 32];
        root[0] = root_byte;
        let pkg = VerifiedPackage::from_parts(
            root,
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            root,
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            AppExecutionKind::PhaseVm,
            0,
            0,
            ABI_VERSION_V1,
            name.into(),
            Vec::new(),
            PKG_TYPE_APP,
            crate::kernel::phasestack::types::ExecEnvelope::default(),
        );
        crate::kernel::phasestack::verifier::commit_installation(
            pkg,
            GrantSet::empty(),
            InstallScope::User,
            "/phase/packages/x".into(),
        )
        .unwrap()
    }

    #[test]
    fn leaf_change_changes_installed_root() {
        let mut r = AppRegistry::new();
        r.insert(inst("a", 1)).unwrap();
        let a = r.installed_package_root();
        r.insert(inst("b", 2)).unwrap();
        assert_ne!(a, r.installed_package_root());
    }
}
