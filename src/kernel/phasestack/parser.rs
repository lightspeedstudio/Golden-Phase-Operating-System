//! Bounded GPA1 archive parser. Not zip/JSON.

use crate::kernel::crypto::phase_hash::{ph_code, ph_content, ph_hook_manifest, ph_manifest};
use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::canonical::{
    put_u16, put_u32, put_u64, take, take_fixed, take_u16, take_u32, take_u64,
};
use crate::kernel::phasestack::types::{
    AppExecutionKind, AppVersion, GPA_VERSION_V1, MANIFEST_VERSION_V2, MAX_GPA_BYTES, MAX_GPA_ENTRIES,
    MAX_GPA_NAME_LEN, PhaseStackError, StandardHook, HookMode, ARCH_NATIVE, ARCH_PHASE_VM,
    PHASEVM_MAX_MEM,
};
use crate::kernel::state_root::merkle_root;
use crate::platform::{String, Vec};

pub const GPA_MAGIC: &[u8; 4] = b"GPA1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpaEntry {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPackage {
    pub entries: Vec<GpaEntry>,
}

pub fn parse_gpa(bytes: &[u8]) -> Result<ParsedPackage, PhaseStackError> {
    if bytes.len() > MAX_GPA_BYTES {
        return Err(PhaseStackError::Oversize);
    }
    if bytes.len() < 10 {
        return Err(PhaseStackError::BadMagic);
    }
    if &bytes[0..4] != GPA_MAGIC {
        return Err(PhaseStackError::BadMagic);
    }
    let mut off = 4usize;
    let version = take_u16(bytes, &mut off)?;
    if version != GPA_VERSION_V1 {
        return Err(PhaseStackError::UnknownVersion);
    }
    let count = take_u32(bytes, &mut off)? as usize;
    if count == 0 || count > MAX_GPA_ENTRIES {
        return Err(PhaseStackError::Oversize);
    }
    let mut metas: Vec<(String, u32, u32)> = Vec::new();
    let mut names_seen: Vec<String> = Vec::new();
    for _ in 0..count {
        let name_len = take_u16(bytes, &mut off)? as usize;
        if name_len == 0 || name_len > MAX_GPA_NAME_LEN {
            return Err(PhaseStackError::Oversize);
        }
        let name_bytes = take(bytes, &mut off, name_len)?;
        let name = core::str::from_utf8(name_bytes).map_err(|_| PhaseStackError::CanonicalMismatch)?;
        validate_entry_name(name)?;
        if names_seen.iter().any(|n| n == name) {
            return Err(PhaseStackError::DuplicateEntry);
        }
        names_seen.push(name.into());
        let offset = take_u32(bytes, &mut off)?;
        let size = take_u32(bytes, &mut off)?;
        let end = offset
            .checked_add(size)
            .ok_or(PhaseStackError::Overflow)?;
        if end as usize > bytes.len() {
            return Err(PhaseStackError::Overflow);
        }
        metas.push((name.into(), offset, size));
    }
    let mut entries = Vec::new();
    for (name, offset, size) in metas {
        let start = offset as usize;
        let end = start + size as usize;
        entries.push(GpaEntry {
            name,
            data: bytes[start..end].to_vec(),
        });
    }
    Ok(ParsedPackage { entries })
}

fn validate_entry_name(name: &str) -> Result<(), PhaseStackError> {
    if name.starts_with('/') || name.starts_with('\\') || name.contains(':') {
        return Err(PhaseStackError::AbsolutePath);
    }
    for part in name.split(|c| c == '/' || c == '\\') {
        if part == ".." || part == "." {
            return Err(PhaseStackError::PathTraversal);
        }
        if part.is_empty() {
            return Err(PhaseStackError::PathTraversal);
        }
    }
    Ok(())
}

pub fn encode_gpa(entries: &[GpaEntry]) -> Result<Vec<u8>, PhaseStackError> {
    if entries.len() > MAX_GPA_ENTRIES || entries.is_empty() {
        return Err(PhaseStackError::Oversize);
    }
    let header_fixed: usize = 4 + 2 + 4;
    let mut index_len = 0usize;
    for e in entries {
        validate_entry_name(&e.name)?;
        if e.name.len() > MAX_GPA_NAME_LEN {
            return Err(PhaseStackError::Oversize);
        }
        index_len = index_len
            .checked_add(2 + e.name.len() + 4 + 4)
            .ok_or(PhaseStackError::Overflow)?;
    }
    let mut cursor = header_fixed
        .checked_add(index_len)
        .ok_or(PhaseStackError::Overflow)?;
    let mut out = Vec::new();
    out.extend_from_slice(GPA_MAGIC);
    put_u16(&mut out, GPA_VERSION_V1);
    put_u32(&mut out, entries.len() as u32);
    let mut payloads: Vec<&[u8]> = Vec::new();
    for e in entries {
        put_u16(&mut out, e.name.len() as u16);
        out.extend_from_slice(e.name.as_bytes());
        put_u32(&mut out, cursor as u32);
        put_u32(&mut out, e.data.len() as u32);
        cursor = cursor
            .checked_add(e.data.len())
            .ok_or(PhaseStackError::Overflow)?;
        payloads.push(&e.data);
    }
    for p in payloads {
        out.extend_from_slice(p);
    }
    if out.len() > MAX_GPA_BYTES {
        return Err(PhaseStackError::Oversize);
    }
    Ok(out)
}

pub fn entry<'a>(pkg: &'a ParsedPackage, name: &str) -> Result<&'a [u8], PhaseStackError> {
    pkg.entries
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.data.as_slice())
        .ok_or(PhaseStackError::MissingEntry)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalManifest {
    pub name: String,
    pub version: AppVersion,
    pub exec_kind: AppExecutionKind,
    pub requested_caps: u32,
    pub requested_hooks: u32,
    pub abi_version: u16,
    pub package_type: u16,
    pub arch: u16,
    pub entry_point: u32,
    pub mem_bytes: u32,
    pub declared_syscalls: u32,
    pub mmio_caps: u32,
    pub dependency_root: StateHash,
    pub min_version_counter: u64,
    pub publisher_key_id: [u8; 32],
}

impl CanonicalManifest {
    pub fn phase_vm(name: String, version: AppVersion) -> Self {
        Self {
            name,
            version,
            exec_kind: AppExecutionKind::PhaseVm,
            requested_caps: 0,
            requested_hooks: 0,
            abi_version: crate::kernel::phasestack::types::ABI_VERSION_V1,
            package_type: crate::kernel::phasestack::types::PKG_TYPE_APP,
            arch: ARCH_PHASE_VM,
            entry_point: crate::kernel::phasestack::types::PHASEVM_ENTRY,
            mem_bytes: PHASEVM_MAX_MEM,
            declared_syscalls: 0,
            mmio_caps: 0,
            dependency_root: [0u8; 32],
            min_version_counter: 0,
            publisher_key_id: [0u8; 32],
        }
    }

    pub fn native_a32(name: String, version: AppVersion) -> Self {
        let mut m = Self::phase_vm(name, version);
        m.exec_kind = crate::kernel::phasestack::types::AppExecutionKind::Native;
        m.arch = ARCH_NATIVE;
        m
    }
}

pub fn encode_manifest(m: &CanonicalManifest) -> Vec<u8> {
    let mut out = Vec::new();
    put_u16(&mut out, MANIFEST_VERSION_V2);
    put_u16(&mut out, m.name.len() as u16);
    out.extend_from_slice(m.name.as_bytes());
    out.extend_from_slice(&m.version.encode());
    out.push(m.exec_kind.as_u8());
    put_u32(&mut out, m.requested_caps);
    put_u32(&mut out, m.requested_hooks);
    put_u16(&mut out, m.abi_version);
    put_u16(&mut out, m.package_type);
    put_u16(&mut out, m.arch);
    put_u32(&mut out, m.entry_point);
    put_u32(&mut out, m.mem_bytes);
    put_u32(&mut out, m.declared_syscalls);
    put_u32(&mut out, m.mmio_caps);
    out.extend_from_slice(&m.dependency_root);
    put_u64(&mut out, m.min_version_counter);
    out.extend_from_slice(&m.publisher_key_id);
    out
}

pub fn decode_manifest(bytes: &[u8]) -> Result<CanonicalManifest, PhaseStackError> {
    let mut off = 0usize;
    let ver = take_u16(bytes, &mut off)?;
    if ver != MANIFEST_VERSION_V2 {
        return Err(PhaseStackError::UnknownVersion);
    }
    let nlen = take_u16(bytes, &mut off)? as usize;
    if nlen == 0 || nlen > crate::kernel::phasestack::types::MAX_NAME_LEN {
        return Err(PhaseStackError::Oversize);
    }
    let name_bytes = take(bytes, &mut off, nlen)?;
    let name = core::str::from_utf8(name_bytes)
        .map_err(|_| PhaseStackError::CanonicalMismatch)?
        .into();
    let major = take_u16(bytes, &mut off)?;
    let minor = take_u16(bytes, &mut off)?;
    let patch = take_u16(bytes, &mut off)?;
    let exec = AppExecutionKind::from_u8(crate::kernel::phasestack::canonical::take_u8(bytes, &mut off)?)
        .ok_or(PhaseStackError::UnknownVersion)?;
    let requested_caps = take_u32(bytes, &mut off)?;
    let requested_hooks = take_u32(bytes, &mut off)?;
    let abi_version = take_u16(bytes, &mut off)?;
    let package_type = take_u16(bytes, &mut off)?;
    let arch = take_u16(bytes, &mut off)?;
    let entry_point = take_u32(bytes, &mut off)?;
    let mem_bytes = take_u32(bytes, &mut off)?;
    let declared_syscalls = take_u32(bytes, &mut off)?;
    let mmio_caps = take_u32(bytes, &mut off)?;
    let dependency_root = take_fixed::<32>(bytes, &mut off)?;
    let min_version_counter = take_u64(bytes, &mut off)?;
    let publisher_key_id = take_fixed::<32>(bytes, &mut off)?;
    if off != bytes.len() {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    match exec {
        AppExecutionKind::PhaseVm => {
            if arch != ARCH_PHASE_VM {
                return Err(PhaseStackError::WrongArch);
            }
        }
        AppExecutionKind::Native => {
            if arch != ARCH_NATIVE {
                return Err(PhaseStackError::WrongArch);
            }
        }
    }
    if mem_bytes == 0 || mem_bytes > PHASEVM_MAX_MEM {
        return Err(PhaseStackError::ExcessiveMemory);
    }
    Ok(CanonicalManifest {
        name,
        version: AppVersion {
            major,
            minor,
            patch,
        },
        exec_kind: exec,
        requested_caps,
        requested_hooks,
        abi_version,
        package_type,
        arch,
        entry_point,
        mem_bytes,
        declared_syscalls,
        mmio_caps,
        dependency_root,
        min_version_counter,
        publisher_key_id,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookDecl {
    pub hook: StandardHook,
    pub mode: HookMode,
    pub requested_priority: u8,
}

pub fn encode_hooks(hooks: &[HookDecl]) -> Vec<u8> {
    let mut out = Vec::new();
    put_u16(&mut out, hooks.len() as u16);
    for h in hooks {
        put_u32(&mut out, h.hook.as_u32());
        out.push(h.mode.as_u8());
        out.push(h.requested_priority);
    }
    out
}

pub fn decode_hooks(bytes: &[u8]) -> Result<Vec<HookDecl>, PhaseStackError> {
    let mut off = 0usize;
    let count = take_u16(bytes, &mut off)? as usize;
    let mut out = Vec::new();
    for _ in 0..count {
        let id = take_u32(bytes, &mut off)?;
        let hook = StandardHook::from_u32(id).ok_or(PhaseStackError::UnknownVersion)?;
        let mode = HookMode::from_u8(crate::kernel::phasestack::canonical::take_u8(bytes, &mut off)?)
            .ok_or(PhaseStackError::UnknownVersion)?;
        let requested_priority = crate::kernel::phasestack::canonical::take_u8(bytes, &mut off)?;
        out.push(HookDecl {
            hook,
            mode,
            requested_priority,
        });
    }
    if off != bytes.len() {
        return Err(PhaseStackError::CanonicalMismatch);
    }
    Ok(out)
}

pub fn resource_root(pkg: &ParsedPackage) -> StateHash {
    let mut leaves: Vec<(String, StateHash)> = Vec::new();
    for e in &pkg.entries {
        if e.name == "manifest.phase"
            || e.name == "hooks.phase"
            || e.name == "code.phase"
            || e.name == "package.sig"
            || e.name.starts_with("certs/")
        {
            continue;
        }
        leaves.push((e.name.clone(), ph_content(&e.data)));
    }
    leaves.sort_by(|a, b| a.0.cmp(&b.0));
    let hashes: Vec<StateHash> = leaves.into_iter().map(|(_, h)| h).collect();
    merkle_root(&hashes)
}

pub fn manifest_hash(bytes: &[u8]) -> StateHash {
    ph_manifest(bytes)
}

pub fn hook_manifest_hash(bytes: &[u8]) -> StateHash {
    ph_hook_manifest(bytes)
}

pub fn code_hash(bytes: &[u8]) -> StateHash {
    ph_code(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<GpaEntry> {
        vec![
            GpaEntry {
                name: "manifest.phase".into(),
                data: b"m".to_vec(),
            },
            GpaEntry {
                name: "code.phase".into(),
                data: b"c".to_vec(),
            },
        ]
    }

    #[test]
    fn roundtrip() {
        let bytes = encode_gpa(&sample_entries()).unwrap();
        let parsed = parse_gpa(&bytes).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(entry(&parsed, "code.phase").unwrap(), b"c");
    }

    #[test]
    fn rejects_dotdot() {
        let entries = vec![GpaEntry {
            name: "../etc/passwd".into(),
            data: b"x".to_vec(),
        }];
        assert_eq!(encode_gpa(&entries), Err(PhaseStackError::PathTraversal));
        let mut crafted = Vec::new();
        crafted.extend_from_slice(GPA_MAGIC);
        put_u16(&mut crafted, 1);
        put_u32(&mut crafted, 1);
        let name = b"a/../../x";
        put_u16(&mut crafted, name.len() as u16);
        crafted.extend_from_slice(name);
        put_u32(&mut crafted, 0);
        put_u32(&mut crafted, 0);
        assert_eq!(parse_gpa(&crafted), Err(PhaseStackError::PathTraversal));
    }

    #[test]
    fn rejects_absolute() {
        let entries = vec![GpaEntry {
            name: "/abs".into(),
            data: b"x".to_vec(),
        }];
        assert_eq!(encode_gpa(&entries), Err(PhaseStackError::AbsolutePath));
    }

    #[test]
    fn rejects_duplicate() {
        let entries = vec![
            GpaEntry {
                name: "a".into(),
                data: b"1".to_vec(),
            },
            GpaEntry {
                name: "a".into(),
                data: b"2".to_vec(),
            },
        ];
        let bytes = encode_gpa(&[GpaEntry {
            name: "a".into(),
            data: b"1".to_vec(),
        }])
        .unwrap();
        // Force a duplicate name in the index by rebuilding.
        let _ = entries;
        let mut crafted = Vec::new();
        crafted.extend_from_slice(GPA_MAGIC);
        put_u16(&mut crafted, 1);
        put_u32(&mut crafted, 2);
        for _ in 0..2 {
            put_u16(&mut crafted, 1);
            crafted.push(b'a');
            put_u32(&mut crafted, bytes.len() as u32);
            put_u32(&mut crafted, 0);
        }
        let _ = bytes;
        assert_eq!(parse_gpa(&crafted), Err(PhaseStackError::DuplicateEntry));
    }

    #[test]
    fn rejects_unknown_version() {
        let mut bytes = encode_gpa(&sample_entries()).unwrap();
        bytes[4] = 9;
        bytes[5] = 0;
        assert_eq!(parse_gpa(&bytes), Err(PhaseStackError::UnknownVersion));
    }

    #[test]
    fn rejects_bad_magic() {
        assert_eq!(parse_gpa(b"PK\x03\x04"), Err(PhaseStackError::BadMagic));
    }

    #[test]
    fn rejects_overflow_offset() {
        let mut crafted = Vec::new();
        crafted.extend_from_slice(GPA_MAGIC);
        put_u16(&mut crafted, 1);
        put_u32(&mut crafted, 1);
        put_u16(&mut crafted, 1);
        crafted.push(b'a');
        put_u32(&mut crafted, u32::MAX);
        put_u32(&mut crafted, 2);
        assert_eq!(parse_gpa(&crafted), Err(PhaseStackError::Overflow));
    }

    #[test]
    fn resource_merkle_changes_with_leaf() {
        let mut entries = sample_entries();
        entries.push(GpaEntry {
            name: "res/a".into(),
            data: b"one".to_vec(),
        });
        let a = resource_root(&ParsedPackage {
            entries: entries.clone(),
        });
        entries[2].data = b"two".to_vec();
        let b = resource_root(&ParsedPackage { entries });
        assert_ne!(a, b);
    }

    #[test]
    fn manifest_v2_roundtrip_and_rejects_v1() {
        let m = CanonicalManifest::phase_vm(
            "hello".into(),
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
        );
        let bytes = encode_manifest(&m);
        assert_eq!(decode_manifest(&bytes).unwrap(), m);
        let mut v1 = bytes.clone();
        v1[0] = 1;
        v1[1] = 0;
        assert_eq!(decode_manifest(&v1), Err(PhaseStackError::UnknownVersion));
        let mut extra = bytes.clone();
        extra.push(0);
        assert_eq!(decode_manifest(&extra), Err(PhaseStackError::CanonicalMismatch));
    }
}
