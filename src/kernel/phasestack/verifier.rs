//! Sole constructor of `VerifiedPackage` / `VerifiedInstallation`.

use crate::kernel::crypto::phase_hash::{ph_app_id, ph_package_root};
use crate::kernel::crypto::sign;
use crate::kernel::phasestack::capability::GrantSet;
use crate::kernel::phasestack::cert::{
    decode_certificate, DeveloperCertificateStore, DeveloperCertificateV1,
};
use crate::kernel::phasestack::parser::{
    code_hash, decode_hooks, decode_manifest, entry, hook_manifest_hash, manifest_hash, parse_gpa,
    resource_root, CanonicalManifest, HookDecl,
};
use crate::kernel::phasestack::revocation::PackageRevocationStore;
use crate::kernel::phasestack::types::{
    hex_lower, ABI_VERSION_V1, AppExecutionKind, ExecEnvelope, InstallScope, PhaseStackError,
    PKG_TYPE_PLUGIN, PRIVILEGED_HOOK_PRIORITY, StagedPackage, VerifiedInstallation, VerifiedPackage,
    PHASEVM_ENTRY, SYSCALL_EMIT, SYSCALL_PRINT,
};
use crate::platform::Vec;
use crate::vm::instruction::decode_program;

pub struct VerifyInputs<'a> {
    pub staged: &'a StagedPackage,
    pub certs: &'a DeveloperCertificateStore,
    pub revocation: &'a PackageRevocationStore,
    pub now_generation: u64,
    pub grants: GrantSet,
    pub scope: InstallScope,
    pub installed_version: Option<(crate::kernel::phasestack::types::AppVersion, u64)>,
}

pub fn verify_package(input: VerifyInputs<'_>) -> Result<VerifiedPackage, PhaseStackError> {
    let parsed = parse_gpa(&input.staged.bytes)?;
    let manifest_bytes = entry(&parsed, "manifest.phase")?;
    let hooks_bytes = entry(&parsed, "hooks.phase")?;
    let code_bytes = entry(&parsed, "code.phase")?;
    let sig_bytes = entry(&parsed, "package.sig")?;
    if sig_bytes.len() != 64 {
        return Err(PhaseStackError::Unsigned);
    }
    let mut signature = [0u8; 64];
    signature.copy_from_slice(sig_bytes);

    let manifest = decode_manifest(manifest_bytes)?;
    if encode_check(&manifest, manifest_bytes)? {}
    let _hooks = decode_hooks(hooks_bytes)?;
    if manifest.abi_version != ABI_VERSION_V1 {
        return Err(PhaseStackError::AbiRejected);
    }
    if manifest.exec_kind == AppExecutionKind::PhaseVm && manifest.entry_point != PHASEVM_ENTRY {
        return Err(PhaseStackError::BadEntry);
    }
    if manifest.mmio_caps != 0 {
        return Err(PhaseStackError::UndeclaredMmio);
    }

    match manifest.exec_kind {
        AppExecutionKind::PhaseVm => {
            let program = decode_program(code_bytes).map_err(|_| PhaseStackError::AbiRejected)?;
            let used = program_syscall_bits(&program)?;
            if used & !manifest.declared_syscalls != 0 {
                return Err(PhaseStackError::UndeclaredSyscall);
            }
        }
        AppExecutionKind::Native => {
            crate::kernel::phasestack::native::parse_elf(code_bytes)?;
        }
    }

    let mut chain: Vec<DeveloperCertificateV1> = Vec::new();
    for e in &parsed.entries {
        if e.name.starts_with("certs/") {
            chain.push(decode_certificate(&e.data)?);
        }
    }
    if chain.is_empty() {
        return Err(PhaseStackError::UnknownCert);
    }
    let leaf = chain.remove(0);
    let leaf_hash = input.certs.verify_chain(
        &leaf,
        &chain,
        input.now_generation,
        &|h| input.revocation.cert_revoked(h),
    )?;
    if manifest.publisher_key_id != leaf.subject_public_key {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    if (leaf.package_type_mask & manifest.package_type) == 0 {
        return Err(PhaseStackError::AbiRejected);
    }
    let issuer_hash = if leaf.issuer_certificate_hash == crate::kernel::phasestack::types::NO_ISSUER
    {
        leaf_hash
    } else {
        leaf.issuer_certificate_hash
    };
    let m_hash = manifest_hash(manifest_bytes);
    let h_hash = hook_manifest_hash(hooks_bytes);
    let c_hash = code_hash(code_bytes);
    let r_root = resource_root(&parsed);
    let app_id = ph_app_id(&leaf_hash, manifest.name.as_bytes());
    let package_root = ph_package_root(
        &app_id,
        &manifest.version.encode(),
        &m_hash,
        &c_hash,
        &h_hash,
        &r_root,
        &leaf_hash,
    );
    input.revocation.check_package(
        package_root,
        leaf_hash,
        issuer_hash,
        input.now_generation,
    )?;
    if !sign::verify(&leaf.subject_public_key, &package_root, &signature) {
        return Err(PhaseStackError::WrongSignature);
    }
    if let Some((installed_ver, installed_counter)) = input.installed_version {
        let older_ver = manifest.version.cmp_tuple() < installed_ver.cmp_tuple();
        let older_counter = manifest.min_version_counter < installed_counter;
        if older_ver || older_counter {
            return Err(PhaseStackError::RollbackVersion);
        }
    }

    let envelope = ExecEnvelope {
        arch: manifest.arch,
        entry_point: manifest.entry_point,
        mem_bytes: manifest.mem_bytes,
        declared_syscalls: manifest.declared_syscalls,
        mmio_caps: manifest.mmio_caps,
        dependency_root: manifest.dependency_root,
        min_version_counter: manifest.min_version_counter,
        publisher_key_id: manifest.publisher_key_id,
    };
    let pkg = VerifiedPackage::from_parts(
        app_id,
        manifest.version,
        package_root,
        c_hash,
        m_hash,
        h_hash,
        r_root,
        leaf_hash,
        manifest.exec_kind,
        manifest.requested_caps,
        manifest.requested_hooks,
        manifest.abi_version,
        manifest.name,
        code_bytes.to_vec(),
        manifest.package_type,
        envelope,
    );
    check_grants(&pkg, &_hooks, input.grants)?;
    let _ = hex_lower(&package_root);
    Ok(pkg)
}

fn encode_check(m: &CanonicalManifest, bytes: &[u8]) -> Result<bool, PhaseStackError> {
    let recon = crate::kernel::phasestack::parser::encode_manifest(m);
    if recon.as_slice() != bytes {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    Ok(true)
}

fn program_syscall_bits(
    program: &[crate::vm::instruction::Instruction],
) -> Result<u32, PhaseStackError> {
    use crate::vm::instruction::Instruction;
    let mut bits = 0u32;
    for insn in program {
        match insn {
            Instruction::Print { .. } => bits |= SYSCALL_PRINT,
            Instruction::Syscall { nr } => {
                if *nr == 0 || *nr >= 32 {
                    return Err(PhaseStackError::UndeclaredSyscall);
                }
                bits |= 1u32 << nr;
            }
            crate::vm::instruction::Instruction::EmitMem { .. } => bits |= SYSCALL_EMIT,
            _ => {}
        }
    }
    Ok(bits)
}

fn check_grants(
    pkg: &VerifiedPackage,
    hooks: &[HookDecl],
    grants: GrantSet,
) -> Result<(), PhaseStackError> {
    if grants.caps & !pkg.requested_caps() != 0 {
        return Err(PhaseStackError::GrantNotSubset);
    }
    if grants.hooks & !pkg.requested_hooks() != 0 {
        return Err(PhaseStackError::GrantNotSubset);
    }
    let wants_privileged = hooks
        .iter()
        .any(|h| h.requested_priority == PRIVILEGED_HOOK_PRIORITY);
    if wants_privileged && !grants.allow_privileged_priority {
        if grants.max_priority == PRIVILEGED_HOOK_PRIORITY {
            return Err(PhaseStackError::PrivilegedPriorityDenied);
        }
    }
    if !grants.allow_privileged_priority && grants.max_priority == PRIVILEGED_HOOK_PRIORITY {
        return Err(PhaseStackError::PrivilegedPriorityDenied);
    }
    Ok(())
}

pub fn commit_installation(
    package: VerifiedPackage,
    grants: GrantSet,
    scope: InstallScope,
    store_path: crate::platform::String,
) -> Result<VerifiedInstallation, PhaseStackError> {
    if grants.caps & !package.requested_caps() != 0 {
        return Err(PhaseStackError::GrantNotSubset);
    }
    if package.package_type() == PKG_TYPE_PLUGIN {
        // plugins still require a verified type; intersection is applied by caller
    }
    Ok(VerifiedInstallation::from_verified(
        package,
        grants.caps,
        grants.hooks,
        grants.max_priority,
        scope,
        store_path,
    ))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::kernel::phasestack::cert::{
        certificate_hash, encode_certificate, issue_leaf_cert, issue_root_cert_with,
        sign_certificate, DeveloperCertificateStore,
    };
    use crate::kernel::phasestack::parser::{encode_gpa, encode_hooks, encode_manifest, GpaEntry};
    use crate::kernel::crypto::sign::SignKeypair;
    use crate::vm::instruction::{encode_program, Instruction};

    pub fn signed_gpa(grants_ok: bool) -> (StagedPackage, DeveloperCertificateStore, GrantSet) {
        signed_gpa_with_generation(grants_ok, None)
    }

    pub fn signed_gpa_with_generation(
        grants_ok: bool,
        generation_end: Option<u64>,
    ) -> (StagedPackage, DeveloperCertificateStore, GrantSet) {
        let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
        let mut store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
        let root = issue_root_cert_with(1, &root_kp);
        store.insert(root.clone()).unwrap();
        let leaf_kp = SignKeypair::from_seed(&[0x44u8; 32]);
        let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, generation_end);
        let code = encode_program(&[Instruction::Halt]);
        let mut manifest = CanonicalManifest::phase_vm(
            "hello".into(),
            crate::kernel::phasestack::types::AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
        );
        manifest.requested_caps = crate::kernel::captable::RIGHT_READ;
        manifest.requested_hooks = crate::kernel::phasestack::types::StandardHook::OnLoad.bit();
        manifest.publisher_key_id = leaf.subject_public_key;
        let hooks = [HookDecl {
            hook: crate::kernel::phasestack::types::StandardHook::OnLoad,
            mode: crate::kernel::phasestack::types::HookMode::Notify,
            requested_priority: 8,
        }];
        let m_bytes = encode_manifest(&manifest);
        let h_bytes = encode_hooks(&hooks);
        let parsed_for_root = crate::kernel::phasestack::parser::ParsedPackage {
            entries: vec![
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
                    name: "certs/leaf".into(),
                    data: encode_certificate(&leaf),
                },
                GpaEntry {
                    name: "certs/root".into(),
                    data: encode_certificate(&root),
                },
            ],
        };
        let leaf_hash = certificate_hash(&leaf);
        let app_id = ph_app_id(&leaf_hash, b"hello");
        let package_root = ph_package_root(
            &app_id,
            &manifest.version.encode(),
            &manifest_hash(&m_bytes),
            &code_hash(&code),
            &hook_manifest_hash(&h_bytes),
            &resource_root(&parsed_for_root),
            &leaf_hash,
        );
        let sig = leaf_kp.sign(&package_root);
        let mut entries = parsed_for_root.entries;
        entries.insert(
            3,
            GpaEntry {
                name: "package.sig".into(),
                data: sig.to_vec(),
            },
        );
        let bytes = encode_gpa(&entries).unwrap();
        let grants = if grants_ok {
            GrantSet {
                caps: crate::kernel::captable::RIGHT_READ,
                hooks: crate::kernel::phasestack::types::StandardHook::OnLoad.bit(),
                max_priority: 8,
                allow_privileged_priority: false,
            }
        } else {
            GrantSet {
                caps: crate::kernel::captable::RIGHT_WRITE,
                hooks: 0,
                max_priority: 8,
                allow_privileged_priority: false,
            }
        };
        (StagedPackage { bytes }, store, grants)
    }

    pub fn signed_gpa_custom<F>(mut tweak: F) -> (StagedPackage, DeveloperCertificateStore, GrantSet)
    where
        F: FnMut(&mut CanonicalManifest, &mut crate::platform::Vec<u8>),
    {
        let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
        let mut store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
        let root = issue_root_cert_with(1, &root_kp);
        store.insert(root.clone()).unwrap();
        let leaf_kp = SignKeypair::from_seed(&[0x44u8; 32]);
        let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
        let mut code = encode_program(&[Instruction::Halt]);
        let mut manifest = CanonicalManifest::phase_vm(
            "hello".into(),
            crate::kernel::phasestack::types::AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
        );
        manifest.publisher_key_id = leaf.subject_public_key;
        tweak(&mut manifest, &mut code);
        let m_bytes = encode_manifest(&manifest);
        let h_bytes = encode_hooks(&[]);
        let parsed_for_root = crate::kernel::phasestack::parser::ParsedPackage {
            entries: vec![
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
                    name: "certs/leaf".into(),
                    data: encode_certificate(&leaf),
                },
                GpaEntry {
                    name: "certs/root".into(),
                    data: encode_certificate(&root),
                },
            ],
        };
        let leaf_hash = certificate_hash(&leaf);
        let app_id = ph_app_id(&leaf_hash, manifest.name.as_bytes());
        let package_root = ph_package_root(
            &app_id,
            &manifest.version.encode(),
            &manifest_hash(&m_bytes),
            &code_hash(&code),
            &hook_manifest_hash(&h_bytes),
            &resource_root(&parsed_for_root),
            &leaf_hash,
        );
        let sig = leaf_kp.sign(&package_root);
        let mut entries = parsed_for_root.entries;
        entries.insert(
            3,
            GpaEntry {
                name: "package.sig".into(),
                data: sig.to_vec(),
            },
        );
        (
            StagedPackage {
                bytes: encode_gpa(&entries).unwrap(),
            },
            store,
            GrantSet::empty(),
        )
    }

    #[test]
    fn signed_package_verifies() {
        let (staged, store, grants) = signed_gpa(true);
        let rev = PackageRevocationStore::new();
        let pkg = verify_package(VerifyInputs {
            staged: &staged,
            certs: &store,
            revocation: &rev,
            now_generation: 0,
            grants,
            scope: InstallScope::User,
            installed_version: None,
        })
        .unwrap();
        assert_eq!(pkg.name(), "hello");
    }

    #[test]
    fn wrong_signature_rejected() {
        let (mut staged, store, grants) = signed_gpa(true);
        let len = staged.bytes.len();
        staged.bytes[len - 1] ^= 0xff;
        let rev = PackageRevocationStore::new();
        let err = verify_package(VerifyInputs {
            staged: &staged,
            certs: &store,
            revocation: &rev,
            now_generation: 0,
            grants,
            scope: InstallScope::User,
            installed_version: None,
        })
        .unwrap_err();
        assert!(
            err == PhaseStackError::WrongSignature
                || err == PhaseStackError::SignatureInvalid
                || err == PhaseStackError::UnknownVersion
                || err == PhaseStackError::CanonicalMismatch
                || err == PhaseStackError::Overflow
        );
    }

    #[test]
    fn grant_not_subset_rejected() {
        let (staged, store, grants) = signed_gpa(false);
        let rev = PackageRevocationStore::new();
        assert_eq!(
            verify_package(VerifyInputs {
                staged: &staged,
                certs: &store,
                revocation: &rev,
                now_generation: 0,
                grants,
                scope: InstallScope::User,
                installed_version: None,
            }),
            Err(PhaseStackError::GrantNotSubset)
        );
    }

    #[test]
    fn unsigned_rejected() {
        let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
        let root = issue_root_cert_with(1, &root_kp);
        let mut store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
        store.insert(root.clone()).unwrap();
        let mut manifest = CanonicalManifest::phase_vm(
            "x".into(),
            crate::kernel::phasestack::types::AppVersion {
                major: 0,
                minor: 0,
                patch: 1,
            },
        );
        manifest.publisher_key_id = root.subject_public_key;
        let code = encode_program(&[Instruction::Halt]);
        let entries = vec![
            GpaEntry {
                name: "manifest.phase".into(),
                data: encode_manifest(&manifest),
            },
            GpaEntry {
                name: "hooks.phase".into(),
                data: encode_hooks(&[]),
            },
            GpaEntry {
                name: "code.phase".into(),
                data: code,
            },
            GpaEntry {
                name: "package.sig".into(),
                data: vec![0u8; 64],
            },
            GpaEntry {
                name: "certs/root".into(),
                data: encode_certificate(&root),
            },
        ];
        let staged = StagedPackage {
            bytes: encode_gpa(&entries).unwrap(),
        };
        let rev = PackageRevocationStore::new();
        let err = verify_package(VerifyInputs {
            staged: &staged,
            certs: &store,
            revocation: &rev,
            now_generation: 0,
            grants: GrantSet::empty(),
            scope: InstallScope::User,
            installed_version: None,
        })
        .unwrap_err();
        assert!(
            err == PhaseStackError::WrongSignature
                || err == PhaseStackError::SignatureInvalid
                || err == PhaseStackError::UnknownIssuer
        );
        let _ = sign_certificate;
    }
}
