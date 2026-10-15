//! Hosted PhaseStack packager. Emits `hooks.phase` bytes; never writes the kernel hook table.

use crate::kernel::crypto::phase_hash::{ph_app_id, ph_package_root};
use crate::kernel::crypto::sign::SignKeypair;
use crate::kernel::phasestack::cert::{
    certificate_hash, encode_certificate, DeveloperCertificateV1,
};
use crate::kernel::phasestack::parser::{
    code_hash, encode_gpa, encode_hooks, encode_manifest, hook_manifest_hash, manifest_hash,
    resource_root, CanonicalManifest, GpaEntry, HookDecl, ParsedPackage,
};
use crate::kernel::phasestack::types::{
    AppExecutionKind, AppVersion, ABI_VERSION_V1, PKG_TYPE_APP, PhaseStackError,
};
use crate::platform::Vec;
use crate::vm::instruction::{encode_program, Instruction};

pub struct PhaseApplication {
    pub name: crate::platform::String,
    pub version: AppVersion,
    pub code: Vec<Instruction>,
    pub hooks: Vec<HookDecl>,
    pub requested_caps: u32,
    pub requested_hooks: u32,
    pub package_type: u16,
    pub declared_syscalls: u32,
}

impl PhaseApplication {
    pub fn pack(
        &self,
        signing_key: &SignKeypair,
        leaf: &DeveloperCertificateV1,
        rest_chain: &[DeveloperCertificateV1],
    ) -> Result<Vec<u8>, PhaseStackError> {
        let code = encode_program(&self.code);
        let mut manifest = CanonicalManifest::phase_vm(self.name.clone(), self.version);
        manifest.exec_kind = AppExecutionKind::PhaseVm;
        manifest.requested_caps = self.requested_caps;
        manifest.requested_hooks = self.requested_hooks;
        manifest.abi_version = ABI_VERSION_V1;
        manifest.package_type = if self.package_type == 0 {
            PKG_TYPE_APP
        } else {
            self.package_type
        };
        manifest.declared_syscalls = self.declared_syscalls;
        manifest.publisher_key_id = leaf.subject_public_key;
        let m_bytes = encode_manifest(&manifest);
        let h_bytes = encode_hooks(&self.hooks);
        let mut entries = vec![
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
        ];
        #[cfg(feature = "phasesentinel")]
        {
            use crate::kernel::crypto::phase_hash::ph_content;
            use crate::kernel::sentinel::manifest::ProgramManifest;
            let mut psm = ProgramManifest {
                generation: 0,
                expiry_tick: u64::MAX,
                content_hash: ph_content(&code),
                requested_rights: 0,
                signer_pubkey: [0u8; 32],
                signature: [0u8; 64],
            };
            psm.sign_with(signing_key);
            entries.push(GpaEntry {
                name: "sentinel.psm1".into(),
                data: psm.encode(),
            });
        }
        let mut cert_entries = Vec::new();
        cert_entries.push(GpaEntry {
            name: "certs/leaf".into(),
            data: encode_certificate(leaf),
        });
        for (i, c) in rest_chain.iter().enumerate() {
            cert_entries.push(GpaEntry {
                name: format!("certs/c{i}"),
                data: encode_certificate(c),
            });
        }
        let mut for_root = entries.clone();
        for_root.extend(cert_entries.iter().cloned());
        let leaf_hash = certificate_hash(leaf);
        let app_id = ph_app_id(&leaf_hash, self.name.as_bytes());
        let package_root = ph_package_root(
            &app_id,
            &self.version.encode(),
            &manifest_hash(&m_bytes),
            &code_hash(&code),
            &hook_manifest_hash(&h_bytes),
            &resource_root(&ParsedPackage {
                entries: for_root,
            }),
            &leaf_hash,
        );
        let sig = signing_key.sign(&package_root);
        entries.push(GpaEntry {
            name: "package.sig".into(),
            data: sig.to_vec(),
        });
        entries.extend(cert_entries);
        encode_gpa(&entries)
    }
}

pub fn pack_native_a32(
    name: &str,
    version: AppVersion,
    code: &[u8],
    signing_key: &SignKeypair,
    leaf: &DeveloperCertificateV1,
    rest_chain: &[DeveloperCertificateV1],
) -> Result<Vec<u8>, PhaseStackError> {
    if crate::kernel::phasestack::native::is_elf(code) {
        return Err(PhaseStackError::AbiRejected);
    }
    let mut manifest = CanonicalManifest::native_a32(name.into(), version);
    manifest.publisher_key_id = leaf.subject_public_key;
    let m_bytes = encode_manifest(&manifest);
    let h_bytes = encode_hooks(&[]);
    let mut entries = vec![
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
            data: code.to_vec(),
        },
    ];
    #[cfg(feature = "phasesentinel")]
    {
        use crate::kernel::crypto::phase_hash::ph_content;
        use crate::kernel::sentinel::manifest::ProgramManifest;
        let mut psm = ProgramManifest {
            generation: 0,
            expiry_tick: u64::MAX,
            content_hash: ph_content(code),
            requested_rights: 0,
            signer_pubkey: [0u8; 32],
            signature: [0u8; 64],
        };
        psm.sign_with(signing_key);
        entries.push(GpaEntry {
            name: "sentinel.psm1".into(),
            data: psm.encode(),
        });
    }
    let mut cert_entries = Vec::new();
    cert_entries.push(GpaEntry {
        name: "certs/leaf".into(),
        data: encode_certificate(leaf),
    });
    for (i, c) in rest_chain.iter().enumerate() {
        cert_entries.push(GpaEntry {
            name: format!("certs/c{i}"),
            data: encode_certificate(c),
        });
    }
    let mut for_root = entries.clone();
    for_root.extend(cert_entries.iter().cloned());
    let leaf_hash = certificate_hash(leaf);
    let app_id = ph_app_id(&leaf_hash, name.as_bytes());
    let package_root = ph_package_root(
        &app_id,
        &version.encode(),
        &manifest_hash(&m_bytes),
        &code_hash(code),
        &hook_manifest_hash(&h_bytes),
        &resource_root(&ParsedPackage {
            entries: for_root,
        }),
        &leaf_hash,
    );
    let sig = signing_key.sign(&package_root);
    entries.push(GpaEntry {
        name: "package.sig".into(),
        data: sig.to_vec(),
    });
    entries.extend(cert_entries);
    encode_gpa(&entries)
}

pub fn pack_native_elf(
    name: &str,
    version: AppVersion,
    elf: &[u8],
    signing_key: &SignKeypair,
    leaf: &DeveloperCertificateV1,
    rest_chain: &[DeveloperCertificateV1],
) -> Result<Vec<u8>, PhaseStackError> {
    crate::kernel::phasestack::native::parse_elf(elf)?;
    let mut manifest = CanonicalManifest::native_a32(name.into(), version);
    manifest.publisher_key_id = leaf.subject_public_key;
    let m_bytes = encode_manifest(&manifest);
    let h_bytes = encode_hooks(&[]);
    let mut entries = vec![
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
            data: elf.to_vec(),
        },
    ];
    #[cfg(feature = "phasesentinel")]
    {
        use crate::kernel::crypto::phase_hash::ph_content;
        use crate::kernel::sentinel::manifest::ProgramManifest;
        let mut psm = ProgramManifest {
            generation: 0,
            expiry_tick: u64::MAX,
            content_hash: ph_content(elf),
            requested_rights: 0,
            signer_pubkey: [0u8; 32],
            signature: [0u8; 64],
        };
        psm.sign_with(signing_key);
        entries.push(GpaEntry {
            name: "sentinel.psm1".into(),
            data: psm.encode(),
        });
    }
    let mut cert_entries = Vec::new();
    cert_entries.push(GpaEntry {
        name: "certs/leaf".into(),
        data: encode_certificate(leaf),
    });
    for (i, c) in rest_chain.iter().enumerate() {
        cert_entries.push(GpaEntry {
            name: format!("certs/c{i}"),
            data: encode_certificate(c),
        });
    }
    let mut for_root = entries.clone();
    for_root.extend(cert_entries.iter().cloned());
    let leaf_hash = certificate_hash(leaf);
    let app_id = ph_app_id(&leaf_hash, name.as_bytes());
    let package_root = ph_package_root(
        &app_id,
        &version.encode(),
        &manifest_hash(&m_bytes),
        &code_hash(elf),
        &hook_manifest_hash(&h_bytes),
        &resource_root(&ParsedPackage {
            entries: for_root,
        }),
        &leaf_hash,
    );
    let sig = signing_key.sign(&package_root);
    entries.push(GpaEntry {
        name: "package.sig".into(),
        data: sig.to_vec(),
    });
    entries.extend(cert_entries);
    encode_gpa(&entries)
}

pub fn hello_program() -> Vec<Instruction> {
    (0..16)
        .map(|i| Instruction::EmitMem { addr: i })
        .chain(core::iter::once(Instruction::Halt))
        .collect()
}

pub const HELLO_NONCE_NAME: &str = "hello-a7f3c91e";

pub fn hello_app() -> PhaseApplication {
    PhaseApplication {
        name: HELLO_NONCE_NAME.into(),
        version: AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        code: hello_program(),
        hooks: Vec::new(),
        requested_caps: 0,
        requested_hooks: 0,
        package_type: PKG_TYPE_APP,
        declared_syscalls: crate::kernel::phasestack::types::SYSCALL_EMIT,
    }
}
