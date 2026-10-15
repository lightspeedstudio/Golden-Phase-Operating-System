//! ANOM security protocol — enter, exit, and privileged open.

use crate::core::gphase_core::PhaseState;
use crate::kernel::anom::decryptor::{open_anom_phase_state, session_seal_from_passphrase};
use crate::kernel::anom::mode::AnomSession;
use crate::kernel::anom::policy::{self, PolicyError};
use crate::kernel::anom::record::{AnomAadContext, AnomProtectedRecord, SealError};
use crate::kernel::authority::{AuthorityMode, PublicPhaseView, VisibilityMode};
use crate::kernel::crypto::keyring::AnomKeyring;
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::state_proof::{hash_payload, ObjectKind};
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::Kernel;
use crate::platform::String;

pub const ANOM_STATECHAIN_OBJECT_ID: u32 = 2;
pub const ROOT_MANIFEST_PATH: &str = "/boot/root.manifest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Policy(PolicyError),
    Seal(SealError),
    NoSession,
    BadObject,
    Decrypt(crate::kernel::anom::decryptor::AnomError),
}

impl From<PolicyError> for ProtocolError {
    fn from(value: PolicyError) -> Self {
        ProtocolError::Policy(value)
    }
}

impl From<SealError> for ProtocolError {
    fn from(value: SealError) -> Self {
        ProtocolError::Seal(value)
    }
}

pub struct EnterReport {
    pub enter_event_hash: StateHash,
    pub anom_root: StateHash,
}

pub(crate) fn load_root_history(kernel: &mut Kernel) {
    if let Ok(data) = kernel.fs.read_file(ROOT_MANIFEST_PATH) {
        parse_root_manifest(&data, &mut kernel.authority.history);
    }
}

pub fn parse_root_manifest(data: &[u8], history: &mut crate::kernel::authority::RootFileHistory) {
    let text = core::str::from_utf8(data).unwrap_or("");
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "last_authority_mode" if v == "anom" => {
                history.last_authority_mode = AuthorityMode::Anom;
            }
            "last_authority_mode" => history.last_authority_mode = AuthorityMode::Root,
            "last_visibility_mode" if v == "protected" => {
                history.last_visibility_mode = VisibilityMode::AnomProtected;
            }
            "last_visibility_mode" if v == "transparent" => {
                history.last_visibility_mode = VisibilityMode::Transparent;
            }
            "anom_available" => history.anom_available = v == "true",
            "last_anom_session_root" => history.last_anom_session_root = hex32(v),
            "last_anom_enter_event" => history.last_anom_enter_event = hex32(v),
            "last_anom_exit_event" => history.last_anom_exit_event = hex32(v),
            _ => {}
        }
    }
}

fn hex32(s: &str) -> StateHash {
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    let n = bytes.len().min(64) / 2;
    for (i, byte) in out.iter_mut().enumerate().take(n) {
        let hi = hex_nibble(bytes.get(i * 2).copied().unwrap_or(b'0'));
        let lo = hex_nibble(bytes.get(i * 2 + 1).copied().unwrap_or(b'0'));
        *byte = (hi << 4) | lo;
    }
    out
}

fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

pub fn write_root_manifest(kernel: &mut Kernel) {
    let h = &kernel.authority.history;
    let content = format!(
        "last_authority_mode={}\n\
         last_visibility_mode={}\n\
         anom_available={}\n\
         last_anom_session_root={}\n\
         last_anom_enter_event={}\n\
         last_anom_exit_event={}\n",
        policy::authority_label(kernel.authority.authority),
        policy::visibility_label(kernel.authority.visibility),
        kernel.anom.is_crypto_available(),
        hex_encode(&h.last_anom_session_root),
        hex_encode(&h.last_anom_enter_event),
        hex_encode(&h.last_anom_exit_event),
    );
    let _ = kernel.fs.mkdir("/boot");
    let _ = kernel.fs.write_file(ROOT_MANIFEST_PATH, content.as_bytes());
}

fn hex_encode(bytes: &StateHash) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn enter_anom_mode(
    kernel: &mut Kernel,
    passphrase: &[u8],
) -> Result<EnterReport, ProtocolError> {
    enter_anom_mode_impl(kernel, passphrase)
}

fn enter_anom_mode_impl(
    kernel: &mut Kernel,
    passphrase: &[u8],
) -> Result<EnterReport, ProtocolError> {
    let equation_root = kernel.statechain.equation_root();
    policy::can_enter(
        kernel.anom.boot_completed(),
        &kernel.authority,
        &equation_root,
        true,
        true,
    )?;

    let boot_session_root = kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(passphrase, &equation_root, &boot_session_root);
    let session_seal = keyring.k_anom_session_seal;

    let clock = kernel.clock.snapshot();
    let payload = format!(
        "enter_anom_mode|root->anom|operator_command|anom security -p|bsr={}|eq={}|sec={}",
        short4(&boot_session_root),
        short4(&equation_root),
        clock.sector_id,
    );
    let event = record_anom_authority_event(
        kernel,
        "enter_anom_mode",
        PhaseState::Phi1,
        PhaseState::Phi2,
        &payload,
    )?;

    let anom_root = tagged_hash(
        "KAIROS_ANOM_SESSION_ROOT_V1",
        &[
            &equation_root,
            &boot_session_root,
            &event.event_hash,
            &session_seal,
        ],
    );

    let session = AnomSession {
        session_id: kernel.anom.next_session_id(),
        boot_session_root,
        equation_root,
        started_sequence: event.proof.sequence_number,
        authority: AuthorityMode::Anom,
        visibility: VisibilityMode::AnomProtected,
        key_available: true,
        current_anom_root: anom_root,
        enter_event_hash: event.event_hash,
        last_anom_event_hash: event.event_hash,
    };

    kernel.authority.authority = AuthorityMode::Anom;
    kernel.authority.visibility = VisibilityMode::AnomProtected;
    kernel.anom.arm_session(keyring, session, session_seal);
    kernel.authority.history.last_authority_mode = AuthorityMode::Anom;
    kernel.authority.history.last_visibility_mode = VisibilityMode::AnomProtected;
    kernel.authority.history.anom_available = true;
    kernel.authority.history.last_anom_session_root = anom_root;
    kernel.authority.history.last_anom_enter_event = event.event_hash;
    write_root_manifest(kernel);

    Ok(EnterReport {
        enter_event_hash: event.event_hash,
        anom_root,
    })
}

pub fn exit_anom_mode(kernel: &mut Kernel, passphrase: &[u8]) -> Result<StateHash, ProtocolError> {
    policy::can_exit(&kernel.authority)?;
        let session = kernel.anom.session().ok_or(ProtocolError::NoSession)?;
        let expected = session_seal_from_passphrase(
            passphrase,
            &session.equation_root,
            &session.boot_session_root,
        );
        if expected != kernel.anom.session_seal() {
            return Err(ProtocolError::Policy(PolicyError::PassphraseRejected));
        }

        let payload = "exit_anom_mode|anom->root|operator_command|anom off -p";
        let event = record_anom_authority_event(
            kernel,
            "exit_anom_mode",
            PhaseState::Phi2,
            PhaseState::Phi1,
            payload,
        )?;

        kernel.authority.authority = AuthorityMode::Root;
        kernel.authority.visibility = VisibilityMode::Transparent;
        kernel.anom.disarm();
        kernel.authority.history.last_authority_mode = AuthorityMode::Root;
        kernel.authority.history.last_visibility_mode = VisibilityMode::Transparent;
        kernel.authority.history.last_anom_exit_event = event.event_hash;
        write_root_manifest(kernel);
        Ok(event.event_hash)
}

pub fn privileged_open(
    kernel: &mut Kernel,
    object_spec: &str,
    passphrase: &[u8],
) -> Result<(PhaseState, PublicPhaseView), ProtocolError> {
        if !kernel.anom.is_active() {
            return Err(ProtocolError::Policy(PolicyError::NotAnom));
        }
        let (kind, id) = parse_object_spec(object_spec)?;
        let keyring = kernel.anom.keyring().ok_or(ProtocolError::NoSession)?;
        let session = kernel.anom.session().ok_or(ProtocolError::NoSession)?;
        if session_seal_from_passphrase(
            passphrase,
            &session.equation_root,
            &session.boot_session_root,
        ) != kernel.anom.session_seal()
        {
            return Err(ProtocolError::Policy(PolicyError::PassphraseRejected));
        }

        let record = kernel
            .anom
            .record(kind, id)
            .ok_or(ProtocolError::BadObject)?
            .clone();
        let ctx = aad_context_from_record(kernel, &record);
        let phase =
            open_anom_phase_state(keyring, &record, &ctx).map_err(ProtocolError::Decrypt)?;
        let public = record.public_view;

        let payload = format!("privileged_anom_open|{object_spec}");
        let _ = record_anom_authority_event(
            kernel,
            "privileged_anom_open",
            PhaseState::Phi3,
            PhaseState::Phi3,
            &payload,
        );

        Ok((phase, public))
}

pub fn seal_test(
    kernel: &mut Kernel,
    phase: PhaseState,
    passphrase: &[u8],
) -> Result<AnomProtectedRecord, ProtocolError> {
        if !kernel.anom.is_active() {
            return Err(ProtocolError::Policy(PolicyError::NotAnom));
        }
        let keyring = AnomKeyring::derive(
            passphrase,
            &kernel.statechain.equation_root(),
            &kernel.authority.boot_session_root,
        );
        if keyring.k_anom_session_seal != kernel.anom.session_seal() {
            return Err(ProtocolError::Policy(PolicyError::PassphraseRejected));
        }
        let ctx = aad_context_for_kernel(kernel);
        let prev = [0u8; 32];
        kernel
            .anom
            .seal_with_keyring(
                &keyring,
                phase,
                PhaseState::Phi0,
                ObjectKind::Driver,
                ANOM_STATECHAIN_OBJECT_ID,
                0,
                "seal-test",
                prev,
                &ctx,
            )
            .map_err(ProtocolError::Seal)
}

fn parse_object_spec(spec: &str) -> Result<(ObjectKind, u32), ProtocolError> {
    let (kind, id_str) = spec.split_once(':').ok_or(ProtocolError::BadObject)?;
    let id = id_str.parse().map_err(|_| ProtocolError::BadObject)?;
    let object_kind = match kind {
        "process" => ObjectKind::Process,
        "page" | "memory" => ObjectKind::MemoryPage,
        _ => return Err(ProtocolError::BadObject),
    };
    Ok((object_kind, id))
}

pub fn record_anom_authority_event(
    kernel: &mut Kernel,
    operation: &str,
    from: PhaseState,
    to: PhaseState,
    payload: &str,
) -> Result<crate::kernel::statechain::StateEvent, ProtocolError> {
    kernel
        .statechain
        .record_transition(
            ObjectKind::Driver,
            ANOM_STATECHAIN_OBJECT_ID,
            SubsystemKind::Driver,
            from,
            to,
            operation,
            None,
            payload.as_bytes(),
        )
        .map_err(|_| ProtocolError::Policy(PolicyError::StatechainUnavailable))
}

pub fn record_failed_entry(kernel: &mut Kernel, reason: &str) {
    let payload = format!("failed_anom_entry|{reason}");
    let _ = record_anom_authority_event(
        kernel,
        "failed_anom_entry",
        PhaseState::Phi1,
        PhaseState::Phi1,
        &payload,
    );
}

pub fn aad_context_for_kernel(kernel: &Kernel) -> AnomAadContext {
    let clock = kernel.clock.snapshot();
    AnomAadContext {
        equation_root: kernel.statechain.equation_root(),
        boot_session_root: kernel.authority.boot_session_root,
        kernel_state_root: Some(kernel.statechain.kernel_root()),
        previous_event_hash: [0u8; 32],
        object_kind: ObjectKind::Process,
        object_id: 0,
        sequence_number: 0,
        commitment_hash: [0u8; 32],
        operation: String::new(),
        chronos_tick: clock.proper_tick,
        kairos_tick: clock.kairos_tick,
        sector_id: clock.sector_id,
        x: clock.x,
    }
}

fn aad_context_from_record(kernel: &Kernel, record: &AnomProtectedRecord) -> AnomAadContext {
    AnomAadContext {
        equation_root: kernel.statechain.equation_root(),
        boot_session_root: kernel.authority.boot_session_root,
        kernel_state_root: record.kernel_state_root,
        previous_event_hash: record.previous_event_hash,
        object_kind: record.object_kind,
        object_id: record.object_id,
        sequence_number: record.sequence_number,
        commitment_hash: record.commitment_hash,
        operation: record.operation.clone(),
        chronos_tick: record.chronos_tick,
        kairos_tick: record.kairos_tick,
        sector_id: record.sector_id,
        x: record.x,
    }
}

fn short4(hash: &StateHash) -> String {
    hash.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

pub fn mask_and_record_process(
    kernel: &mut Kernel,
    pid: u32,
    old: PhaseState,
    new: PhaseState,
    op: &str,
) {
    if !kernel.anom.is_active() {
        let _ = kernel.statechain.record_transition(
            ObjectKind::Process,
            pid,
            SubsystemKind::Process,
            old,
            new,
            op,
            None,
            &[],
        );
        return;
    }

    let prev_hash = kernel
        .statechain
        .chain(ObjectKind::Process, pid)
        .and_then(|c| c.head())
        .unwrap_or([0u8; 32]);
    let seq = kernel
        .statechain
        .chain(ObjectKind::Process, pid)
        .map(|c| c.events.len() as u64)
        .unwrap_or(0);

    let ctx = aad_context_for_kernel(kernel);
    if let Some(keyring) = kernel.anom.keyring().cloned() {
        if let Ok(record) = kernel.anom.seal_with_keyring(
            &keyring,
            new,
            old,
            ObjectKind::Process,
            pid,
            seq,
            op,
            prev_hash,
            &ctx,
        ) {
            kernel
                .anom
                .store_record(ObjectKind::Process, pid, record.clone());
            let payload = format!(
                "anom_phase|commit={}|ct={}",
                short4(&record.commitment_hash),
                short4(&hash_payload(&record.ciphertext))
            );
            let _ = kernel.statechain.record_transition(
                ObjectKind::Process,
                pid,
                SubsystemKind::Process,
                PhaseState::Phi1,
                PhaseState::Phi1,
                "anom_phase_seal",
                None,
                payload.as_bytes(),
            );
        }
    }
}
