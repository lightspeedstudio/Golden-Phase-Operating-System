//! Per-object ANOM protection protocol — protect, open, close, rekey, unprotect.

use crate::core::gphase_core::PhaseState;
use crate::kernel::anom::object_access::ObjectAccessController;
use crate::kernel::anom::object_key::{
    generate_object_data_key, generate_object_salt, secure_random_bytes, unwrap_object_key,
    wrap_object_key, zeroize_key, WrappedObjectKey, WRAP_NONCE_SIZE,
};
use crate::kernel::anom::object_policy::{
    ObjectProtectionError, ObjectProtectionRecord, ObjectProtectionStatus, ProtectedObjectId,
    ProtectedObjectKind, OBJECT_PROTECTION_VERSION,
};
use crate::kernel::anom::object_record::{
    open_protected_phase, seal_protected_phase, ObjectStateAadContext, ProtectedPhasePlaintext,
};
use crate::kernel::anom::object_registry::ObjectProtectionRegistry;
use crate::kernel::authority::PublicPhaseView;
use crate::kernel::hash::StateHash;
use crate::kernel::state_proof::{hash_payload, ObjectKind};
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;
use crate::platform::{String, ToString, Vec};

#[derive(Debug, Default)]
pub struct ObjectProtectionController {
    pub registry: ObjectProtectionRegistry,
    pub access: ObjectAccessController,
    pub kernel_keys: crate::kernel::anom::object_access::KernelTrustedKeys,
}

impl ObjectProtectionController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_persisted(
        &mut self,
        boot_session_root: StateHash,
        equation_root: StateHash,
        statechain: &crate::kernel::statechain::StatechainLedger,
    ) {
        #[cfg(feature = "std")]
        self.registry
            .load_all(&boot_session_root, &equation_root, statechain);
        let _ = (boot_session_root, equation_root, statechain);
    }

    pub fn load_persisted_fs(
        &mut self,
        fs: &mut crate::fs::PhaseFs,
        boot_session_root: StateHash,
        equation_root: StateHash,
        statechain: &crate::kernel::statechain::StatechainLedger,
    ) {
        self.registry
            .load_from_fs(fs, &boot_session_root, &equation_root, statechain);
    }

    pub fn public_view_for_process(
        &self,
        pid: u32,
        true_phase: PhaseState,
        global_anom: bool,
    ) -> PublicPhaseView {
        let id = ProtectedObjectId::new(ProtectedObjectKind::Process, pid.to_string());
        if self.registry.is_protected(&id) {
            if let Some(commitment) = self.registry.public_commitment(&id) {
                return PublicPhaseView::AnomCommitment(commitment);
            }
            return PublicPhaseView::Locked;
        }
        if global_anom {
            PublicPhaseView::Unknown
        } else {
            PublicPhaseView::Visible(true_phase)
        }
    }

    pub fn protection_label_for_process(&self, pid: u32, global_anom: bool) -> &'static str {
        let id = ProtectedObjectId::new(ProtectedObjectKind::Process, pid.to_string());
        if self.registry.is_protected(&id) {
            if self.registry.is_open(&id) {
                "object/open"
            } else {
                "object"
            }
        } else if global_anom {
            "global"
        } else {
            "none"
        }
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn verify_object_exists(
    kernel: &Kernel,
    object_id: &ProtectedObjectId,
) -> Result<(), ObjectProtectionError> {
    match object_id.kind {
        ProtectedObjectKind::Process => {
            let pid: u32 = object_id
                .value
                .parse()
                .map_err(|_| ObjectProtectionError::InvalidObjectId)?;
            kernel
                .get_process(pid)
                .map_err(|_| ObjectProtectionError::ObjectNotFound)?;
        }
        ProtectedObjectKind::File => {
            kernel
                .fs
                .stat(&object_id.value)
                .map_err(|_| ObjectProtectionError::ObjectNotFound)?;
        }
        ProtectedObjectKind::Craft => {
            use crate::kernel::state_domain::DomainId;
            if DomainId::parse(&object_id.value).is_none() {
                return Err(ObjectProtectionError::ObjectNotFound);
            }
        }
        ProtectedObjectKind::Memory
        | ProtectedObjectKind::Vm
        | ProtectedObjectKind::StateClock
        | ProtectedObjectKind::Network => {
            return Err(ObjectProtectionError::ObjectNotFound);
        }
    }
    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn resolve_statechain_target(
    kernel: &Kernel,
    object_id: &ProtectedObjectId,
) -> Result<(ObjectKind, u32, SubsystemKind), ObjectProtectionError> {
    match object_id.kind {
        ProtectedObjectKind::Process => {
            let pid: u32 = object_id
                .value
                .parse()
                .map_err(|_| ObjectProtectionError::InvalidObjectId)?;
            Ok((ObjectKind::Process, pid, SubsystemKind::Process))
        }
        ProtectedObjectKind::File => {
            let digest = hash_payload(object_id.value.as_bytes());
            let id = u16::from_le_bytes([digest[0], digest[1]]) as u32;
            Ok((ObjectKind::File, id, SubsystemKind::Filesystem))
        }
        ProtectedObjectKind::Craft => {
            let slot = kernel
                .object_protection
                .registry
                .craft_slot_lookup(&object_id.value)
                .ok_or(ObjectProtectionError::NotProtected)?;
            Ok((ObjectKind::Craft, slot, SubsystemKind::Craft))
        }
        _ => Err(ObjectProtectionError::ObjectNotFound),
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn true_phase_for_object(
    kernel: &Kernel,
    object_id: &ProtectedObjectId,
) -> Result<PhaseState, ObjectProtectionError> {
    match object_id.kind {
        ProtectedObjectKind::Process => {
            let pid: u32 = object_id
                .value
                .parse()
                .map_err(|_| ObjectProtectionError::InvalidObjectId)?;
            Ok(kernel
                .get_process(pid)
                .map_err(|_| ObjectProtectionError::ObjectNotFound)?
                .state)
        }
        ProtectedObjectKind::File => Ok(PhaseState::Phi1),
        ProtectedObjectKind::Craft => Ok(PhaseState::Phi2),
        _ => Err(ObjectProtectionError::ObjectNotFound),
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn make_nonce() -> [u8; 12] {
    let bytes = secure_random_bytes(WRAP_NONCE_SIZE);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes[..WRAP_NONCE_SIZE]);
    nonce
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn protect_object(
    kernel: &mut Kernel,
    object_spec: &str,
    passphrase: &[u8],
) -> Result<String, ObjectProtectionError> {
    let object_id = ProtectedObjectId::parse(object_spec)?;
    if kernel.object_protection.registry.is_protected(&object_id) {
        return Err(ObjectProtectionError::AlreadyProtected);
    }
    verify_object_exists(kernel, &object_id)?;

    let equation_root = kernel.statechain.equation_root();
    let boot_session_root = kernel.authority.boot_session_root;
    let object_salt = generate_object_salt();
    let mut object_data_key = generate_object_data_key();
    let k_wrap = crate::kernel::anom::object_key::derive_wrap_key(
        passphrase,
        &equation_root,
        &boot_session_root,
        &object_id,
        &object_salt,
        OBJECT_PROTECTION_VERSION,
    );
    let wrapped = wrap_object_key(
        &k_wrap,
        &object_data_key,
        &equation_root,
        &boot_session_root,
        &object_id,
        &object_salt,
        OBJECT_PROTECTION_VERSION,
    )?;

    if object_id.kind == ProtectedObjectKind::Craft {
        let _ = kernel
            .object_protection
            .registry
            .craft_slot(&object_id.value);
    }
    let (obj_kind, obj_id, subsystem) = resolve_statechain_target(kernel, &object_id)?;
    let prev_hash = kernel
        .statechain
        .chain(obj_kind, obj_id)
        .and_then(|c| c.head())
        .unwrap_or([0u8; 32]);
    let seq = kernel
        .statechain
        .chain(obj_kind, obj_id)
        .map(|c| c.events.len() as u64)
        .unwrap_or(0);

    let true_phase = true_phase_for_object(kernel, &object_id)?;
    let clock = kernel.clock.snapshot();
    let nonce = make_nonce();
    let aad_ctx = ObjectStateAadContext {
        equation_root,
        boot_session_root,
        object_id: object_id.clone(),
        protection_version: OBJECT_PROTECTION_VERSION,
        sequence_number: seq,
        previous_event_hash: prev_hash,
        operation: "protect_object".into(),
        public_commitment: [0u8; 32],
        chronos_tick: clock.proper_tick,
        kairos_tick: clock.kairos_tick,
        sector_id: clock.sector_id,
    };
    let plaintext = ProtectedPhasePlaintext::from_clock(
        true_phase,
        true_phase,
        "protect_object",
        &clock,
        Vec::new(),
        secure_random_bytes(8),
    );
    let phase_record = seal_protected_phase(&object_data_key, &nonce, &plaintext, &aad_ctx)
        .map_err(|_| ObjectProtectionError::CryptoUnavailable)?;

    let payload = format!(
        "protect_object|commit={}|obj={}",
        short4(&phase_record.public_commitment),
        object_id.canonical()
    );
    let event = kernel
        .statechain
        .record_transition(
            obj_kind,
            obj_id,
            subsystem,
            true_phase,
            true_phase,
            "protect_object",
            None,
            payload.as_bytes(),
        )
        .map_err(|_| ObjectProtectionError::StatechainUnavailable)?;

    let record = ObjectProtectionRecord {
        object_id: object_id.clone(),
        version: OBJECT_PROTECTION_VERSION,
        equation_root,
        boot_session_root,
        object_salt: wrapped.object_salt,
        wrap_nonce: wrapped.wrap_nonce.to_vec(),
        wrapped_object_key: wrapped.wrapped_object_key,
        wrap_tag: wrapped.wrap_tag,
        created_sequence: event.proof.sequence_number,
        last_rekey_sequence: None,
        commitment_root: phase_record.public_commitment,
        current_statechain_head: event.event_hash,
        status: ObjectProtectionStatus::ProtectedLocked,
    };

    kernel
        .object_protection
        .registry
        .append_phase_record(phase_record);
    kernel
        .object_protection
        .registry
        .insert_record(record.clone());
    kernel
        .object_protection
        .kernel_keys
        .store(&object_id, object_data_key);
    ObjectProtectionRegistry::persist_record(&mut kernel.fs, &record)
        .map_err(|_| ObjectProtectionError::PersistenceFailed)?;

    zeroize_key(&mut object_data_key);
    Ok(object_id.canonical())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn open_object(
    kernel: &mut Kernel,
    object_spec: &str,
    passphrase: &[u8],
) -> Result<(PhaseState, PublicPhaseView), ObjectProtectionError> {
    let object_id = ProtectedObjectId::parse(object_spec)?;
    if !kernel.object_protection.registry.is_protected(&object_id) {
        return Err(ObjectProtectionError::NotProtected);
    }

    let prot_record = kernel
        .object_protection
        .registry
        .record(&object_id)
        .ok_or(ObjectProtectionError::NotProtected)?
        .clone();

    let wrapped = WrappedObjectKey {
        object_salt: prot_record.object_salt.clone(),
        wrap_nonce: prot_record
            .wrap_nonce
            .clone()
            .try_into()
            .map_err(|_| ObjectProtectionError::AuthenticationFailed)?,
        wrapped_object_key: prot_record.wrapped_object_key.clone(),
        wrap_tag: prot_record.wrap_tag.clone(),
    };

    let object_data_key = unwrap_object_key(
        passphrase,
        &prot_record.equation_root,
        &prot_record.boot_session_root,
        &object_id,
        &wrapped,
        prot_record.version,
    )
    .map_err(|_| {
        record_failed_open(kernel, object_spec);
        ObjectProtectionError::AuthenticationFailed
    })?;

    let phase_record = kernel
        .object_protection
        .registry
        .latest_phase_record(&object_id)
        .ok_or(ObjectProtectionError::InvalidState)?
        .clone();

    let aad_ctx = ObjectStateAadContext {
        equation_root: prot_record.equation_root,
        boot_session_root: prot_record.boot_session_root,
        object_id: object_id.clone(),
        protection_version: prot_record.version,
        sequence_number: phase_record.sequence_number,
        previous_event_hash: phase_record.previous_event_hash,
        operation: phase_record.operation.clone(),
        public_commitment: phase_record.public_commitment,
        chronos_tick: phase_record.chronos_tick,
        kairos_tick: phase_record.kairos_tick,
        sector_id: phase_record.sector_id,
    };

    let opened = open_protected_phase(&object_data_key, &phase_record, &aad_ctx).map_err(|_| {
        record_failed_open(kernel, object_spec);
        ObjectProtectionError::AuthenticationFailed
    })?;

    let public = PublicPhaseView::AnomCommitment(phase_record.public_commitment);

    let (obj_kind, obj_id, subsystem) = resolve_statechain_target(kernel, &object_id)?;
    let payload = format!("privileged_object_open|{}", object_id.canonical());
    let _ = kernel.statechain.record_transition(
        obj_kind,
        obj_id,
        subsystem,
        PhaseState::Phi3,
        PhaseState::Phi3,
        "privileged_object_open",
        None,
        payload.as_bytes(),
    );

    kernel
        .object_protection
        .access
        .open_session(&object_id, object_data_key);
    kernel
        .object_protection
        .registry
        .set_status(&object_id, ObjectProtectionStatus::ProtectedOpen);

    Ok((opened.true_phase, public))
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn record_failed_open(kernel: &mut Kernel, object_spec: &str) {
    if let Ok(object_id) = ProtectedObjectId::parse(object_spec) {
        if let Ok((obj_kind, obj_id, subsystem)) = resolve_statechain_target(kernel, &object_id) {
            let payload = format!("failed_object_open|{}", object_id.canonical());
            let _ = kernel.statechain.record_transition(
                obj_kind,
                obj_id,
                subsystem,
                PhaseState::Phi1,
                PhaseState::Phi1,
                "failed_object_open",
                None,
                payload.as_bytes(),
            );
        }
    }
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn close_object(kernel: &mut Kernel, object_spec: &str) -> Result<(), ObjectProtectionError> {
    let object_id = ProtectedObjectId::parse(object_spec)?;
    if !kernel.object_protection.registry.is_protected(&object_id) {
        return Err(ObjectProtectionError::NotProtected);
    }
    kernel.object_protection.access.close_session(&object_id);
    kernel
        .object_protection
        .registry
        .set_status(&object_id, ObjectProtectionStatus::ProtectedLocked);

    if let Ok((obj_kind, obj_id, subsystem)) = resolve_statechain_target(kernel, &object_id) {
        let payload = format!("close_object|{}", object_id.canonical());
        let _ = kernel.statechain.record_transition(
            obj_kind,
            obj_id,
            subsystem,
            PhaseState::Phi2,
            PhaseState::Phi1,
            "close_object",
            None,
            payload.as_bytes(),
        );
    }
    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn rekey_object(
    kernel: &mut Kernel,
    object_spec: &str,
    current_passphrase: &[u8],
    new_passphrase: &[u8],
) -> Result<(), ObjectProtectionError> {
    let object_id = ProtectedObjectId::parse(object_spec)?;
    let prot_record = kernel
        .object_protection
        .registry
        .record(&object_id)
        .ok_or(ObjectProtectionError::NotProtected)?
        .clone();

    let wrapped = WrappedObjectKey {
        object_salt: prot_record.object_salt.clone(),
        wrap_nonce: prot_record
            .wrap_nonce
            .clone()
            .try_into()
            .map_err(|_| ObjectProtectionError::AuthenticationFailed)?,
        wrapped_object_key: prot_record.wrapped_object_key.clone(),
        wrap_tag: prot_record.wrap_tag.clone(),
    };

    let mut object_data_key = unwrap_object_key(
        current_passphrase,
        &prot_record.equation_root,
        &prot_record.boot_session_root,
        &object_id,
        &wrapped,
        prot_record.version,
    )?;

    let new_salt = generate_object_salt();
    let new_k_wrap = crate::kernel::anom::object_key::derive_wrap_key(
        new_passphrase,
        &prot_record.equation_root,
        &prot_record.boot_session_root,
        &object_id,
        &new_salt,
        prot_record.version,
    );
    let new_wrapped = wrap_object_key(
        &new_k_wrap,
        &object_data_key,
        &prot_record.equation_root,
        &prot_record.boot_session_root,
        &object_id,
        &new_salt,
        prot_record.version,
    )?;

    if let Ok((obj_kind, obj_id, subsystem)) = resolve_statechain_target(kernel, &object_id) {
        let payload = format!("rekey_object|{}", object_id.canonical());
        let event = kernel
            .statechain
            .record_transition(
                obj_kind,
                obj_id,
                subsystem,
                PhaseState::Phi2,
                PhaseState::Phi2,
                "rekey_object",
                None,
                payload.as_bytes(),
            )
            .map_err(|_| ObjectProtectionError::StatechainUnavailable)?;
        if let Some(r) = kernel.object_protection.registry.record_mut(&object_id) {
            r.last_rekey_sequence = Some(event.proof.sequence_number);
            r.current_statechain_head = event.event_hash;
            r.object_salt = new_wrapped.object_salt;
            r.wrap_nonce = new_wrapped.wrap_nonce.to_vec();
            r.wrapped_object_key = new_wrapped.wrapped_object_key;
            r.wrap_tag = new_wrapped.wrap_tag;
            let updated = r.clone();
            ObjectProtectionRegistry::persist_record(&mut kernel.fs, &updated)
                .map_err(|_| ObjectProtectionError::PersistenceFailed)?;
        }
    }

    kernel.object_protection.access.close_session(&object_id);
    zeroize_key(&mut object_data_key);
    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn unprotect_object(
    kernel: &mut Kernel,
    object_spec: &str,
    passphrase: &[u8],
) -> Result<(), ObjectProtectionError> {
    let object_id = ProtectedObjectId::parse(object_spec)?;
    let prot_record = kernel
        .object_protection
        .registry
        .record(&object_id)
        .ok_or(ObjectProtectionError::NotProtected)?
        .clone();

    let wrapped = WrappedObjectKey {
        object_salt: prot_record.object_salt.clone(),
        wrap_nonce: prot_record
            .wrap_nonce
            .clone()
            .try_into()
            .map_err(|_| ObjectProtectionError::AuthenticationFailed)?,
        wrapped_object_key: prot_record.wrapped_object_key.clone(),
        wrap_tag: prot_record.wrap_tag.clone(),
    };

    let mut object_data_key = unwrap_object_key(
        passphrase,
        &prot_record.equation_root,
        &prot_record.boot_session_root,
        &object_id,
        &wrapped,
        prot_record.version,
    )?;

    if let Ok((obj_kind, obj_id, subsystem)) = resolve_statechain_target(kernel, &object_id) {
        let payload = format!("unprotect_object|{}", object_id.canonical());
        let _ = kernel.statechain.record_transition(
            obj_kind,
            obj_id,
            subsystem,
            PhaseState::Phi2,
            PhaseState::Phi1,
            "unprotect_object",
            None,
            payload.as_bytes(),
        );
    }

    kernel.object_protection.access.close_session(&object_id);
    kernel.object_protection.kernel_keys.remove(&object_id);
    kernel.object_protection.registry.mark_inactive(&object_id);
    if let Some(r) = kernel.object_protection.registry.record_mut(&object_id) {
        let updated = r.clone();
        let _ = ObjectProtectionRegistry::persist_record(&mut kernel.fs, &updated);
    }
    zeroize_key(&mut object_data_key);
    Ok(())
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn record_protected_transition(
    kernel: &mut Kernel,
    pid: u32,
    old: PhaseState,
    new: PhaseState,
    op: &str,
) -> bool {
    let object_id = ProtectedObjectId::new(ProtectedObjectKind::Process, pid.to_string());
    if !kernel.object_protection.registry.is_protected(&object_id) {
        return false;
    }

    let prot = match kernel.object_protection.registry.record(&object_id) {
        Some(r) => r.clone(),
        None => return false,
    };

    let (obj_kind, obj_id, subsystem) = match resolve_statechain_target(kernel, &object_id) {
        Ok(t) => t,
        Err(_) => return false,
    };

    let prev_hash = kernel
        .statechain
        .chain(obj_kind, obj_id)
        .and_then(|c| c.head())
        .unwrap_or([0u8; 32]);
    let seq = kernel
        .statechain
        .chain(obj_kind, obj_id)
        .map(|c| c.events.len() as u64)
        .unwrap_or(0);

    let clock = kernel.clock.snapshot();
    let nonce = make_nonce();
    let aad_ctx = ObjectStateAadContext {
        equation_root: prot.equation_root,
        boot_session_root: prot.boot_session_root,
        object_id: object_id.clone(),
        protection_version: prot.version,
        sequence_number: seq,
        previous_event_hash: prev_hash,
        operation: op.into(),
        public_commitment: [0u8; 32],
        chronos_tick: clock.proper_tick,
        kairos_tick: clock.kairos_tick,
        sector_id: clock.sector_id,
    };

    let odk = kernel
        .object_protection
        .access
        .object_data_key(&object_id)
        .or_else(|| kernel.object_protection.kernel_keys.get(&object_id));

    let odk = match odk {
        Some(k) => k,
        None => return record_protected_transition_masked(kernel, pid, old, new, op),
    };

    let plaintext = ProtectedPhasePlaintext::from_clock(
        old,
        new,
        op,
        &clock,
        Vec::new(),
        secure_random_bytes(8),
    );

    let phase_record = match seal_protected_phase(&odk, &nonce, &plaintext, &aad_ctx) {
        Ok(r) => r,
        Err(_) => return false,
    };

    let payload = format!(
        "protected_state_transition|commit={}",
        short4(&phase_record.public_commitment)
    );
    if kernel
        .statechain
        .record_transition(
            obj_kind,
            obj_id,
            subsystem,
            PhaseState::Phi1,
            PhaseState::Phi1,
            "protected_state_transition",
            None,
            payload.as_bytes(),
        )
        .is_err()
    {
        return false;
    }

    kernel
        .object_protection
        .registry
        .append_phase_record(phase_record);
    if let Some(commitment) = kernel
        .object_protection
        .registry
        .public_commitment(&object_id)
    {
        if let Some(r) = kernel.object_protection.registry.record_mut(&object_id) {
            r.commitment_root = commitment;
        }
    }
    true
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn record_protected_transition_masked(
    kernel: &mut Kernel,
    pid: u32,
    _old: PhaseState,
    _new: PhaseState,
    _op: &str,
) -> bool {
    let object_id = ProtectedObjectId::new(ProtectedObjectKind::Process, pid.to_string());
    let commitment = kernel
        .object_protection
        .registry
        .public_commitment(&object_id)
        .unwrap_or([0u8; 32]);

    let (obj_kind, obj_id, subsystem) = match resolve_statechain_target(kernel, &object_id) {
        Ok(t) => t,
        Err(_) => return false,
    };

    let payload = format!("protected_state_transition|commit={}", short4(&commitment));
    kernel
        .statechain
        .record_transition(
            obj_kind,
            obj_id,
            subsystem,
            PhaseState::Phi1,
            PhaseState::Phi1,
            "protected_state_transition",
            None,
            payload.as_bytes(),
        )
        .is_ok()
}

fn short4(hash: &StateHash) -> String {
    hash.iter().take(4).map(|b| format!("{b:02x}")).collect()
}
