//! Hard `authorize_launch`. Fail-closed. Hard failures bypass the soft score.

use crate::kernel::crypto::phase_hash::ph_content;
use crate::kernel::hash::StateHash;

use super::manifest::ProgramManifest;
use super::types::rights_subset;
use super::{CachedLaunch, PhaseSentinel, SentinelError, TaskDisposition};

pub fn authorize_launch(
    sentinel: &mut PhaseSentinel,
    pid: u32,
    program: &[u8],
    manifest: &ProgramManifest,
    now_tick: u64,
) -> Result<StateHash, SentinelError> {
    let content = ph_content(program);
    if content != manifest.content_hash {
        return Err(SentinelError::Unsigned);
    }
    match sentinel.content_disposition(&content) {
        TaskDisposition::Isolated => return Err(SentinelError::Isolated),
        TaskDisposition::Quarantined => return Err(SentinelError::Quarantined),
        TaskDisposition::None => {}
    }
    match sentinel.disposition(pid) {
        TaskDisposition::Isolated => return Err(SentinelError::Isolated),
        TaskDisposition::Quarantined => return Err(SentinelError::Quarantined),
        TaskDisposition::None => {}
    }
    if now_tick >= manifest.expiry_tick {
        return Err(SentinelError::Expired);
    }
    if manifest.generation < sentinel.policy.min_generation {
        return Err(SentinelError::Generation);
    }
    if !rights_subset(manifest.requested_rights, sentinel.policy.max_rights) {
        return Err(SentinelError::RightsEscalation);
    }
    if !sentinel.policy.allows_signer(&manifest.signer_pubkey) {
        return Err(SentinelError::Unauthorized);
    }
    manifest.verify_signature()?;
    sentinel.signers.verify_manifest(
        &manifest.signer_pubkey,
        &manifest.manifest_hash(),
        &manifest.signature,
        now_tick,
        sentinel.policy.min_generation,
    )?;
    sentinel.cache_launch(
        pid,
        CachedLaunch {
            content_hash: content,
            signer: manifest.signer_pubkey,
            generation: manifest.generation,
        },
    );
    Ok(content)
}

/// Unsigned bytes: refuse. Used by paste/file stubs and missing-manifest launches.
pub fn refuse_unsigned(_sentinel: &mut PhaseSentinel) -> Result<(), SentinelError> {
    Err(SentinelError::Unsigned)
}
