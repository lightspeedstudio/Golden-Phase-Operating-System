//! Phase addresses — `phase://` URIs and PhaseAddr hashing.

use crate::kernel::authority::StateHash;
use crate::platform::{String, Vec};

#[cfg(feature = "std")]
use crate::kernel::hash::tagged_hash;

pub const PHASE_ADDR_TAG: &str = "KAIROS_PHASE_ADDR_V1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseUri {
    pub raw: String,
    pub domain: String,
    pub object_path: String,
}

pub fn parse_phase_uri(input: &str) -> Option<PhaseUri> {
    let trimmed = input.trim();
    let path = trimmed
        .strip_prefix("phase://")
        .or_else(|| trimmed.strip_prefix("PHASE://"))?;
    if path.is_empty() {
        return None;
    }
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let domain = if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        parts[0].into()
    };
    let object_path = if parts.len() > 2 {
        parts[2..].join("/")
    } else if parts.len() == 2 {
        parts[1].into()
    } else {
        String::new()
    };
    Some(PhaseUri {
        raw: trimmed.into(),
        domain,
        object_path,
    })
}

#[cfg(feature = "std")]
pub fn compute_phase_addr(
    equation_root: &StateHash,
    node_root: &StateHash,
    domain: &str,
    object_kind: u8,
    object_id: u32,
    capability: &str,
) -> StateHash {
    let mut buf = Vec::new();
    buf.extend_from_slice(equation_root);
    buf.extend_from_slice(node_root);
    buf.extend_from_slice(domain.as_bytes());
    buf.push(object_kind);
    buf.extend_from_slice(&object_id.to_le_bytes());
    buf.extend_from_slice(capability.as_bytes());
    tagged_hash(PHASE_ADDR_TAG, &[&buf])
}

#[cfg(not(feature = "std"))]
pub fn compute_phase_addr(
    _equation_root: &StateHash,
    _node_root: &StateHash,
    _domain: &str,
    _object_kind: u8,
    _object_id: u32,
    _capability: &str,
) -> StateHash {
    [0u8; 32]
}

pub fn addr_for_uri(
    equation_root: &StateHash,
    node_root: &StateHash,
    uri: &PhaseUri,
    object_kind: u8,
    object_id: u32,
) -> StateHash {
    let capability = if uri.object_path.is_empty() {
        "read"
    } else {
        uri.object_path.as_str()
    };
    compute_phase_addr(
        equation_root,
        node_root,
        &uri.domain,
        object_kind,
        object_id,
        capability,
    )
}

pub fn render_addr_short(hash: &StateHash) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    )
}
