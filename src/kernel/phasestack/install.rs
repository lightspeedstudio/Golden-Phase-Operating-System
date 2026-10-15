//! Stage → verify → content-addressed store → registry commit.

use crate::fs::FsError;
use crate::kernel::crypto::phase_hash::{ph_app_registry, ph_code};
use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::capability::GrantSet;
use crate::kernel::phasestack::hooks::HookGrant;
use crate::kernel::phasestack::ledger::{LedgerEvent, LedgerKind};
use crate::kernel::phasestack::canonical::{put_u32, put_u8, take_u32, take_u8};
use crate::kernel::phasestack::parser::{entry, parse_gpa};
use crate::kernel::phasestack::registry::AppRegistry;
use crate::kernel::phasestack::revocation::PackageRevocationStore;
use crate::kernel::phasestack::types::{
    hex_lower, parse_hex32, InstallScope, PhaseStackError, StagedPackage, StandardHook,
    VerifiedInstallation,
};
use crate::platform::Vec;
#[cfg(test)]
use crate::kernel::phasestack::types::AppState;
use crate::kernel::phasestack::verifier::{commit_installation, verify_package, VerifyInputs};
use crate::kernel::phasestack::PhaseStack;
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;

pub fn app_registry_root(stack: &PhaseStack) -> StateHash {
    ph_app_registry(
        &stack.registry.installed_package_root(),
        &stack.registry.granted_capability_root(),
        &stack.hooks.registry.merkle_root(),
        &stack.revocation.merkle_root(),
        &stack.ledger.head(),
    )
}

fn mkdir_p(kernel: &mut Kernel, path: &str) -> Result<(), PhaseStackError> {
    match crate::kernel::syscall::persist_mkdir(kernel, path) {
        Ok(()) | Err(crate::kernel::syscall::SyscallError::Fs(FsError::AlreadyExists(_))) => {
            Ok(())
        }
        Err(_) => Err(PhaseStackError::Fs),
    }
}

fn write_store(kernel: &mut Kernel, path: &str, data: &[u8]) -> Result<(), PhaseStackError> {
    crate::kernel::syscall::persist_write_file(kernel, path, data)
        .map_err(|_| PhaseStackError::InstallFailed)
}

const PACKAGES_DIR: &str = "/phase/packages";
const PACKAGE_GPA: &str = "package.gpa";
const INSTALL_META: &str = "install.meta";
const REVOCATION_PATH: &str = "/phase/revocation";
const META_MAGIC: &[u8; 4] = b"PSIM";
const META_VERSION: u8 = 1;

pub struct RebuildReport {
    pub recovered: u32,
    pub skipped: u32,
}

fn encode_install_meta(grants: GrantSet, scope: InstallScope) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(META_MAGIC);
    put_u8(&mut out, META_VERSION);
    put_u32(&mut out, grants.caps);
    put_u32(&mut out, grants.hooks);
    put_u8(&mut out, grants.max_priority);
    put_u8(
        &mut out,
        if grants.allow_privileged_priority { 1 } else { 0 },
    );
    put_u8(
        &mut out,
        match scope {
            InstallScope::User => 0,
            InstallScope::System => 1,
        },
    );
    out
}

fn decode_install_meta(bytes: &[u8]) -> Result<(GrantSet, InstallScope), PhaseStackError> {
    if bytes.len() < 5 || &bytes[0..4] != META_MAGIC {
        return Err(PhaseStackError::BadMagic);
    }
    let mut off = 4usize;
    if take_u8(bytes, &mut off)? != META_VERSION {
        return Err(PhaseStackError::UnknownVersion);
    }
    let caps = take_u32(bytes, &mut off)?;
    let hooks = take_u32(bytes, &mut off)?;
    let max_priority = take_u8(bytes, &mut off)?;
    let allow = take_u8(bytes, &mut off)?;
    let scope_b = take_u8(bytes, &mut off)?;
    if off != bytes.len() {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    let scope = match scope_b {
        0 => InstallScope::User,
        1 => InstallScope::System,
        _ => return Err(PhaseStackError::CanonicalMismatch),
    };
    Ok((
        GrantSet {
            caps,
            hooks,
            max_priority,
            allow_privileged_priority: allow == 1,
        },
        scope,
    ))
}

pub fn persist_revocation(kernel: &mut Kernel) -> Result<(), PhaseStackError> {
    mkdir_p(kernel, "/phase")?;
    write_store(
        kernel,
        REVOCATION_PATH,
        &kernel.phasestack.revocation.encode(),
    )
}

fn load_persisted_revocation(
    kernel: &mut Kernel,
) -> Result<PackageRevocationStore, PhaseStackError> {
    match kernel.fs.read_file(REVOCATION_PATH) {
        Ok(bytes) => PackageRevocationStore::decode(&bytes),
        Err(crate::fs::FsError::NotFound(_)) => Ok(PackageRevocationStore::new()),
        Err(_) => Err(PhaseStackError::Fs),
    }
}

fn stored_matches_gpa(
    kernel: &mut Kernel,
    dir: &str,
    staged: &StagedPackage,
) -> Result<(), PhaseStackError> {
    let parsed = parse_gpa(&staged.bytes)?;
    for name in ["manifest.phase", "hooks.phase", "code.phase"] {
        let want = entry(&parsed, name)?;
        let got = kernel
            .fs
            .read_file(&format!("{dir}/{name}"))
            .map_err(|_| PhaseStackError::Fs)?;
        if got.as_slice() != want {
            return Err(PhaseStackError::HashMismatch);
        }
    }
    if let Ok(want) = entry(&parsed, "sentinel.psm1") {
        let got = kernel
            .fs
            .read_file(&format!("{dir}/sentinel.psm1"))
            .map_err(|_| PhaseStackError::Fs)?;
        if got.as_slice() != want {
            return Err(PhaseStackError::HashMismatch);
        }
    }
    Ok(())
}

fn recover_one_package(
    kernel: &mut Kernel,
    dir_name: &str,
) -> Result<VerifiedInstallation, PhaseStackError> {
    if dir_name.contains('/') || dir_name.contains('\\') || dir_name.contains('.') {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    let named_root = parse_hex32(dir_name)?;
    let dir = format!("{PACKAGES_DIR}/{dir_name}");
    let gpa = kernel
        .fs
        .read_file(&format!("{dir}/{PACKAGE_GPA}"))
        .map_err(|_| PhaseStackError::NotFound)?;
    let meta = kernel
        .fs
        .read_file(&format!("{dir}/{INSTALL_META}"))
        .map_err(|_| PhaseStackError::NotFound)?;
    for name in ["manifest.phase", "hooks.phase", "code.phase"] {
        kernel
            .fs
            .read_file(&format!("{dir}/{name}"))
            .map_err(|_| PhaseStackError::NotFound)?;
    }
    let (grants, scope) = decode_install_meta(&meta)?;
    let staged = StagedPackage { bytes: gpa };
    stored_matches_gpa(kernel, &dir, &staged)?;
    let verified = {
        let stack = &kernel.phasestack;
        verify_package(VerifyInputs {
            staged: &staged,
            certs: &stack.certs,
            revocation: &stack.revocation,
            now_generation: 0,
            grants,
            scope,
            installed_version: None,
        })?
    };
    if verified.package_root() != named_root {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    if ph_code(
        &kernel
            .fs
            .read_file(&format!("{dir}/code.phase"))
            .map_err(|_| PhaseStackError::Fs)?,
    ) != verified.code_hash()
    {
        return Err(PhaseStackError::HashMismatch);
    }
    let parsed = parse_gpa(&staged.bytes)?;
    for e in &parsed.entries {
        if e.name.starts_with("certs/") {
            if let Ok(cert) = crate::kernel::phasestack::cert::decode_certificate(&e.data) {
                let _ = kernel.phasestack.certs.insert(cert);
            }
        }
    }
    let inst = commit_installation(verified, grants, scope, dir)?;
    kernel.phasestack.registry.insert(inst.clone())?;
    let app_id = inst.package().app_id();
    for hook in [
        StandardHook::OnLoad,
        StandardHook::OnStart,
        StandardHook::OnStop,
    ] {
        if inst.granted_hooks() & hook.bit() != 0 {
            let _ = kernel.phasestack.hooks.registry.grant(
                app_id,
                HookGrant {
                    hook,
                    max_priority: inst.granted_max_priority(),
                },
            );
        }
    }
    kernel
        .phasestack
        .registry
        .get(app_id)
        .cloned()
        .ok_or(PhaseStackError::InstallFailed)
}

/// Rebuild RAM AppRegistry from recovered `/phase/packages/<package-id>/`
/// trees. Must run after WAL recover and PhaseFS/extmap rebuild. Every
/// recovered package goes through `verify_package`. Directories without a
/// verifiable `package.gpa` are skipped. A corrupt revocation file refuses
/// all packages (fail-closed: unknown revocation must not reactivate).
pub fn rebuild_from_store(kernel: &mut Kernel) -> RebuildReport {
    match load_persisted_revocation(kernel) {
        Ok(store) => kernel.phasestack.revocation = store,
        Err(_) => {
            kernel.phasestack.registry = AppRegistry::new();
            kernel.phasestack.hooks = crate::kernel::phasestack::hooks::PhaseHookBus::new();
            return RebuildReport {
                recovered: 0,
                skipped: 0,
            };
        }
    }
    kernel.phasestack.registry = AppRegistry::new();
    kernel.phasestack.hooks = crate::kernel::phasestack::hooks::PhaseHookBus::new();
    let mut names = match kernel.fs.list_dir(PACKAGES_DIR) {
        Ok(n) => n,
        Err(_) => {
            return RebuildReport {
                recovered: 0,
                skipped: 0,
            };
        }
    };
    names.sort();
    let mut recovered = 0u32;
    let mut skipped = 0u32;
    for name in names {
        match recover_one_package(kernel, &name) {
            Ok(_) => recovered = recovered.saturating_add(1),
            Err(_) => skipped = skipped.saturating_add(1),
        }
    }
    RebuildReport { recovered, skipped }
}

pub fn install(
    kernel: &mut Kernel,
    staged: &StagedPackage,
    grants: GrantSet,
    scope: InstallScope,
) -> Result<VerifiedInstallation, PhaseStackError> {
    let now_generation = 0u64;
    let verified = {
        let stack = &kernel.phasestack;
        verify_package(VerifyInputs {
            staged,
            certs: &stack.certs,
            revocation: &stack.revocation,
            now_generation,
            grants,
            scope,
            installed_version: None,
        })?
    };
    if let Some(existing) = kernel.phasestack.registry.get(verified.app_id()) {
        let older_ver =
            verified.version().cmp_tuple() < existing.package().version().cmp_tuple();
        let older_counter = verified.envelope().min_version_counter
            < existing.package().envelope().min_version_counter;
        if older_ver || older_counter {
            return Err(PhaseStackError::RollbackVersion);
        }
        return Err(PhaseStackError::AlreadyInstalled);
    }
    let root_hex = hex_lower(&verified.package_root());
    let dir = format!("/phase/packages/{root_hex}");
    mkdir_p(kernel, "/phase")?;
    mkdir_p(kernel, "/phase/packages")?;
    mkdir_p(kernel, &dir)?;

    let parsed = crate::kernel::phasestack::parser::parse_gpa(&staged.bytes)?;
    let writes: [(&str, &[u8]); 3] = [
        ("manifest.phase", entry(&parsed, "manifest.phase")?),
        ("hooks.phase", entry(&parsed, "hooks.phase")?),
        ("code.phase", entry(&parsed, "code.phase")?),
    ];
    let psm = entry(&parsed, "sentinel.psm1").ok();

    for (name, data) in writes {
        write_store(kernel, &format!("{dir}/{name}"), data)?;
    }
    if let Some(psm) = psm {
        write_store(kernel, &format!("{dir}/sentinel.psm1"), psm)?;
    }
    write_store(kernel, &format!("{dir}/{PACKAGE_GPA}"), &staged.bytes)?;
    write_store(
        kernel,
        &format!("{dir}/{INSTALL_META}"),
        &encode_install_meta(grants, scope),
    )?;

    let stored = kernel
        .fs
        .read_file(&format!("{dir}/code.phase"))
        .map_err(|_| PhaseStackError::Fs)?;
    if ph_code(&stored) != verified.code_hash() {
        let _ = kernel.fs.rm(&dir);
        return Err(PhaseStackError::HashMismatch);
    }
    let _ = crate::kernel::syscall::persist_lock(kernel, &format!("{dir}/code.phase"));
    let _ = crate::kernel::syscall::persist_lock(kernel, &format!("{dir}/manifest.phase"));
    let _ = crate::kernel::syscall::persist_lock(kernel, &format!("{dir}/hooks.phase"));
    if psm.is_some() {
        let _ = crate::kernel::syscall::persist_lock(kernel, &format!("{dir}/sentinel.psm1"));
    }
    let _ = crate::kernel::syscall::persist_lock(kernel, &format!("{dir}/{PACKAGE_GPA}"));
    let _ = crate::kernel::syscall::persist_lock(kernel, &format!("{dir}/{INSTALL_META}"));

    let inst = commit_installation(verified, grants, scope, dir)?;
    let app_id = inst.package().app_id();
    let package_root = inst.package().package_root();
    for e in &parsed.entries {
        if e.name.starts_with("certs/") {
            if let Ok(cert) = crate::kernel::phasestack::cert::decode_certificate(&e.data) {
                let _ = kernel.phasestack.certs.insert(cert);
            }
        }
    }
    if let Err(e) = kernel.phasestack.registry.insert(inst.clone()) {
        let _ = kernel.fs.rm(inst.store_path());
        return Err(e);
    }
    let inst = kernel
        .phasestack
        .registry
        .get(app_id)
        .cloned()
        .ok_or(PhaseStackError::InstallFailed)?;
    for hook in [
        StandardHook::OnLoad,
        StandardHook::OnStart,
        StandardHook::OnStop,
    ] {
        if inst.granted_hooks() & hook.bit() != 0 {
            let _ = kernel.phasestack.hooks.registry.grant(
                app_id,
                HookGrant {
                    hook,
                    max_priority: inst.granted_max_priority(),
                },
            );
        }
    }
    let tick = kernel.clock.snapshot().chronos_tick;
    kernel.phasestack.ledger.record(LedgerEvent {
        kind: LedgerKind::Install,
        app_id,
        package_root,
        tick,
        payload_hash: package_root,
    });
    let _ = kernel.statechain.record_transition(
        ObjectKind::App,
        u32::from_le_bytes(app_id[0..4].try_into().unwrap()) & 0x0FFF,
        SubsystemKind::Vm,
        crate::core::gphase_core::PhaseState::Phi0,
        crate::core::gphase_core::PhaseState::Phi1,
        "phasestack_install",
        None,
        &package_root,
    );
    Ok(inst)
}

pub use crate::kernel::phasestack::run::{launch, revoke, stop};

pub fn authority_allows_system_scope(kernel: &Kernel) -> Result<(), PhaseStackError> {
    use crate::kernel::authority::AuthorityMode;
    if kernel.authority.authority != AuthorityMode::Root {
        return Err(PhaseStackError::AuthorityDenied);
    }
    #[cfg(feature = "identity")]
    {
        if kernel.identity.tickets().active().is_empty() {
            return Err(PhaseStackError::AuthorityDenied);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::phasestack::verifier::tests::signed_gpa;

    #[test]
    fn failed_verify_leaves_no_registry_row() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(false);
        kernel.phasestack.certs = store;
        let before = kernel.phasestack.registry.is_empty();
        assert!(install(&mut kernel, &staged, grants, InstallScope::User).is_err());
        assert!(before);
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[test]
    fn install_and_run_phasevm() {
        let mut kernel = Kernel::new();
        #[cfg(feature = "phasesentinel")]
        {
            kernel.sentinel.policy.max_rights = u32::MAX;
        }
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        launch(&mut kernel, inst.package().app_id(), 32).unwrap();
        assert_eq!(
            kernel
                .phasestack
                .registry
                .get(inst.package().app_id())
                .unwrap()
                .state(),
            AppState::Running
        );
    }

    #[test]
    fn root_cannot_skip_verify() {
        let mut kernel = Kernel::new();
        assert_eq!(
            kernel.authority.authority,
            crate::kernel::authority::AuthorityMode::Root
        );
        let staged = StagedPackage {
            bytes: b"not-a-gpa".to_vec(),
        };
        assert_eq!(
            install(&mut kernel, &staged, GrantSet::empty(), InstallScope::System),
            Err(PhaseStackError::BadMagic)
        );
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[test]
    fn launch_rehash_mismatch_quarantines() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let app_id = inst.package().app_id();
        let path = format!("{}/code.phase", inst.store_path());
        kernel.fs.unlock_path(&path).unwrap();
        kernel.fs.write_file(&path, b"tampered").unwrap();
        assert_eq!(launch(&mut kernel, app_id, 8), Err(PhaseStackError::PayloadRootMismatch));
        assert_eq!(
            kernel.phasestack.registry.get(app_id).unwrap().state(),
            AppState::Quarantined
        );
    }

    #[test]
    fn installed_store_is_immutable() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let path = format!("{}/code.phase", inst.store_path());
        assert!(matches!(
            kernel.fs.write_file(&path, b"x"),
            Err(crate::fs::FsError::Locked(_))
        ));
    }

    #[test]
    fn empty_app_registry_root_matches_gate_k_uart_prefix() {
        let kernel = Kernel::new();
        let root = app_registry_root(&kernel.phasestack);
        assert_eq!(&root[0..4], &[0x97, 0x5c, 0xfc, 0xd8]);
        let hex = hex_lower(&root);
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn app_registry_root_changes_on_install() {
        let mut kernel = Kernel::new();
        let before = app_registry_root(&kernel.phasestack);
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        assert_ne!(before, app_registry_root(&kernel.phasestack));
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal"))]
    #[test]
    fn install_writes_through_journal_not_txlog() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let before = kernel.txlog.stats();
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let path = format!("{}/code.phase", inst.store_path());
        let code = kernel.fs.read_file(&path).unwrap();
        assert!(!code.is_empty());
        assert_eq!(
            kernel.txlog.stats(),
            before,
            "GPA install must not record txlog when WAL succeeds"
        );
        let mut recovered = crate::fs::PhaseFs::new();
        kernel.fs_journal.recover(&mut recovered).unwrap();
        assert_eq!(recovered.read_file(&path).unwrap(), code);
        let gpa_path = format!("{}/package.gpa", inst.store_path());
        assert_eq!(
            recovered.read_file(&gpa_path).unwrap(),
            staged.bytes
        );
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    fn simulate_reboot(kernel: &mut Kernel) {
        let mut recovered = crate::fs::PhaseFs::new();
        kernel.fs_journal.recover(&mut recovered).unwrap();
        kernel.fs = recovered;
        kernel.extmap = crate::kernel::extmap::Extmap::default();
        kernel.phasestack.registry = AppRegistry::new();
        kernel.phasestack.hooks = crate::kernel::phasestack::hooks::PhaseHookBus::new();
        kernel.phasestack.revocation = PackageRevocationStore::new();
        kernel.phasestack.ledger = crate::kernel::phasestack::ledger::AppLedger::new();
        kernel.journal_post_recover();
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    fn registry_count(kernel: &Kernel) -> usize {
        kernel.phasestack.registry.iter().count()
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn wal_recover_rebuilds_app_registry_from_verified_gpa() {
        let mut kernel = Kernel::new();
        #[cfg(feature = "phasesentinel")]
        {
            kernel.sentinel.policy.max_rights = u32::MAX;
        }
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let app_id = inst.package().app_id();
        let publisher = inst.package().publisher_key_id();
        let store_path = inst.store_path().to_string();
        let gpa_path = format!("{store_path}/package.gpa");
        assert_eq!(kernel.fs.read_file(&gpa_path).unwrap(), staged.bytes);
        assert_eq!(registry_count(&kernel), 1);
        assert_ne!(kernel.phasestack.registry.generation(), 0);

        let mut recovered = crate::fs::PhaseFs::new();
        kernel.fs_journal.recover(&mut recovered).unwrap();
        kernel.fs = recovered;
        kernel.extmap = crate::kernel::extmap::Extmap::default();
        kernel.phasestack.registry = AppRegistry::new();
        kernel.phasestack.hooks = crate::kernel::phasestack::hooks::PhaseHookBus::new();
        kernel.phasestack.revocation = PackageRevocationStore::new();
        kernel.phasestack.ledger = crate::kernel::phasestack::ledger::AppLedger::new();
        assert!(kernel.phasestack.registry.is_empty());
        assert_eq!(kernel.extmap.stats().blocks_live, 0);
        assert_eq!(kernel.fs.read_file(&gpa_path).unwrap(), staged.bytes);

        kernel.journal_post_recover();
        assert!(kernel.extmap.stats().blocks_live >= 1);
        assert_eq!(registry_count(&kernel), 1);
        let got = kernel.phasestack.registry.get(app_id).unwrap();
        assert_eq!(got.package().app_id(), app_id);
        assert_eq!(got.package().publisher_key_id(), publisher);
        assert_ne!(
            &app_registry_root(&kernel.phasestack)[0..4],
            &[0x97, 0x5c, 0xfc, 0xd8]
        );
        assert_ne!(kernel.phasestack.registry.generation(), 0);
        launch(&mut kernel, app_id, 32).unwrap();

        let root = app_registry_root(&kernel.phasestack);
        let gen = kernel.phasestack.registry.generation();
        let report = rebuild_from_store(&mut kernel);
        assert_eq!(report.recovered, 1);
        assert_eq!(registry_count(&kernel), 1);
        assert_eq!(app_registry_root(&kernel.phasestack), root);
        assert_eq!(kernel.phasestack.registry.generation(), gen);
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_modified_gpa() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        assert_eq!(registry_count(&kernel), 1);
        let path = format!("{}/package.gpa", inst.store_path());
        kernel.fs.unlock_path(&path).unwrap();
        let mut bytes = kernel.fs.read_file(&path).unwrap();
        bytes[0] ^= 0xff;
        kernel.fs.write_file(&path, &bytes).unwrap();
        rebuild_from_store(&mut kernel);
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_missing_package_file() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        let path = format!("{}/code.phase", inst.store_path());
        kernel.fs.unlock_path(&path).unwrap();
        kernel.fs.rm(&path).unwrap();
        rebuild_from_store(&mut kernel);
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_unknown_signer() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        assert_eq!(registry_count(&kernel), 1);
        kernel.phasestack.certs = crate::kernel::phasestack::cert::DeveloperCertificateStore::new();
        rebuild_from_store(&mut kernel);
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_retired_signer() {
        use crate::kernel::phasestack::cert::{
            encode_certificate, issue_root_cert_with, RETIRED_RESEARCH_CA_PIN,
        };
        use crate::kernel::phasestack::parser::{
            encode_gpa, encode_hooks, encode_manifest, GpaEntry, CanonicalManifest,
        };
        use crate::kernel::crypto::sign::SignKeypair;
        use crate::kernel::phasestack::types::AppVersion;
        use crate::vm::instruction::{encode_program, Instruction};

        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        assert_eq!(registry_count(&kernel), 1);

        let kp = SignKeypair::from_seed(&[0x51u8; 32]);
        let mut root = issue_root_cert_with(1, &kp);
        root.subject_public_key = RETIRED_RESEARCH_CA_PIN;
        root.subject_id = RETIRED_RESEARCH_CA_PIN;
        root.issuer_id = RETIRED_RESEARCH_CA_PIN;
        let code = encode_program(&[Instruction::Halt]);
        let mut manifest = CanonicalManifest::phase_vm(
            "retired".into(),
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
        );
        manifest.publisher_key_id = RETIRED_RESEARCH_CA_PIN;
        let m_bytes = encode_manifest(&manifest);
        let h_bytes = encode_hooks(&[]);
        let retired = encode_gpa(&[
            GpaEntry {
                name: "manifest.phase".into(),
                data: m_bytes.clone(),
            },
            GpaEntry {
                name: "hooks.phase".into(),
                data: h_bytes.clone(),
            },
            GpaEntry {
                name: "code.phase".into(),
                data: code.clone(),
            },
            GpaEntry {
                name: "package.sig".into(),
                data: vec![0u8; 64],
            },
            GpaEntry {
                name: "certs/root".into(),
                data: encode_certificate(&root),
            },
        ])
        .unwrap();
        let dir = format!("{}/{}", PACKAGES_DIR, hex_lower(&[0x11u8; 32]));
        kernel.fs.mkdir(&dir).unwrap();
        kernel
            .fs
            .write_file(&format!("{dir}/package.gpa"), &retired)
            .unwrap();
        kernel
            .fs
            .write_file(
                &format!("{dir}/install.meta"),
                &encode_install_meta(GrantSet::empty(), InstallScope::User),
            )
            .unwrap();
        kernel
            .fs
            .write_file(&format!("{dir}/manifest.phase"), &m_bytes)
            .unwrap();
        kernel
            .fs
            .write_file(&format!("{dir}/hooks.phase"), &h_bytes)
            .unwrap();
        kernel
            .fs
            .write_file(&format!("{dir}/code.phase"), &code)
            .unwrap();
        rebuild_from_store(&mut kernel);
        assert_eq!(registry_count(&kernel), 1);
        assert!(kernel
            .phasestack
            .registry
            .iter()
            .all(|(_, inst)| inst.package().name() != "retired"));
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_canonical_package_id_mismatch() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        let src = inst.store_path().to_string();
        let dst = format!("{}/{}", PACKAGES_DIR, hex_lower(&[0x22u8; 32]));
        kernel.fs.mkdir(&dst).unwrap();
        for name in [
            PACKAGE_GPA,
            INSTALL_META,
            "manifest.phase",
            "hooks.phase",
            "code.phase",
        ] {
            let data = kernel.fs.read_file(&format!("{src}/{name}")).unwrap();
            kernel.fs.write_file(&format!("{dst}/{name}"), &data).unwrap();
        }
        let report = rebuild_from_store(&mut kernel);
        assert_eq!(report.recovered, 1);
        assert!(report.skipped >= 1);
        assert_eq!(registry_count(&kernel), 1);
        assert_eq!(
            hex_lower(
                &kernel
                    .phasestack
                    .registry
                    .get(inst.package().app_id())
                    .unwrap()
                    .package()
                    .package_root()
            ),
            hex_lower(&inst.package().package_root())
        );
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_over_capability_meta() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        let path = format!("{}/install.meta", inst.store_path());
        kernel.fs.unlock_path(&path).unwrap();
        let mut bad = grants;
        bad.caps |= crate::kernel::captable::RIGHT_WRITE;
        kernel
            .fs
            .write_file(&path, &encode_install_meta(bad, InstallScope::User))
            .unwrap();
        rebuild_from_store(&mut kernel);
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_malformed_package_directory() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        kernel.fs.mkdir(&format!("{PACKAGES_DIR}/not-hex")).unwrap();
        kernel
            .fs
            .write_file(&format!("{PACKAGES_DIR}/not-hex/{PACKAGE_GPA}"), b"nope")
            .unwrap();
        let report = rebuild_from_store(&mut kernel);
        assert_eq!(report.recovered, 1);
        assert_eq!(registry_count(&kernel), 1);
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_duplicate_package_id_stays_one_entry() {
        use crate::kernel::phasestack::verifier::tests::signed_gpa_custom;
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let first = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let app_id = first.package().app_id();
        simulate_reboot(&mut kernel);
        let (staged2, _, grants2) = signed_gpa_custom(|m, _| {
            m.version.patch = 1;
            m.requested_caps = crate::kernel::captable::RIGHT_READ;
        });
        let verified = crate::kernel::phasestack::verifier::verify_package(
            crate::kernel::phasestack::verifier::VerifyInputs {
                staged: &staged2,
                certs: &kernel.phasestack.certs,
                revocation: &kernel.phasestack.revocation,
                now_generation: 0,
                grants: grants2,
                scope: InstallScope::User,
                installed_version: None,
            },
        )
        .unwrap();
        assert_eq!(verified.app_id(), app_id);
        let dir = format!("{}/{}", PACKAGES_DIR, hex_lower(&verified.package_root()));
        kernel.fs.mkdir(&dir).unwrap();
        let parsed = parse_gpa(&staged2.bytes).unwrap();
        kernel
            .fs
            .write_file(&format!("{dir}/{PACKAGE_GPA}"), &staged2.bytes)
            .unwrap();
        kernel
            .fs
            .write_file(
                &format!("{dir}/{INSTALL_META}"),
                &encode_install_meta(grants2, InstallScope::User),
            )
            .unwrap();
        for name in ["manifest.phase", "hooks.phase", "code.phase"] {
            kernel
                .fs
                .write_file(&format!("{dir}/{name}"), entry(&parsed, name).unwrap())
                .unwrap();
        }
        rebuild_from_store(&mut kernel);
        assert_eq!(registry_count(&kernel), 1);
        assert!(kernel.phasestack.registry.get(app_id).is_some());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_refuses_corrupted_recovered_file() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        let path = format!("{}/code.phase", inst.store_path());
        kernel.fs.unlock_path(&path).unwrap();
        kernel.fs.write_file(&path, b"tampered-code").unwrap();
        rebuild_from_store(&mut kernel);
        assert!(kernel.phasestack.registry.is_empty());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn invalid_package_does_not_block_valid_rebuild() {
        use crate::kernel::phasestack::verifier::tests::signed_gpa_custom;
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let hello = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let (staged2, _, grants2) = signed_gpa_custom(|m, _| {
            m.name = "other".into();
            m.requested_caps = crate::kernel::captable::RIGHT_READ;
        });
        let other = install(&mut kernel, &staged2, grants2, InstallScope::User).unwrap();
        simulate_reboot(&mut kernel);
        assert_eq!(registry_count(&kernel), 2);
        let path = format!("{}/package.gpa", hello.store_path());
        kernel.fs.unlock_path(&path).unwrap();
        let mut bytes = kernel.fs.read_file(&path).unwrap();
        bytes[0] ^= 0xff;
        kernel.fs.write_file(&path, &bytes).unwrap();
        rebuild_from_store(&mut kernel);
        assert_eq!(registry_count(&kernel), 1);
        assert!(kernel
            .phasestack
            .registry
            .get(other.package().app_id())
            .is_some());
        assert!(kernel
            .phasestack
            .registry
            .get(hello.package().app_id())
            .is_none());
    }

    #[cfg(all(feature = "txlog", feature = "phasefs-journal", feature = "extmap"))]
    #[test]
    fn rebuild_does_not_reactivate_revoked_package() {
        let mut kernel = Kernel::new();
        let (staged, store, grants) = signed_gpa(true);
        kernel.phasestack.certs = store;
        let inst = install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
        let app_id = inst.package().app_id();
        revoke(&mut kernel, app_id).unwrap();
        assert_eq!(
            kernel.phasestack.registry.get(app_id).unwrap().state(),
            AppState::Revoked
        );
        simulate_reboot(&mut kernel);
        assert!(kernel.phasestack.registry.get(app_id).is_none());
        assert!(kernel
            .phasestack
            .revocation
            .package_revoked(inst.package().package_root()));
    }
}
