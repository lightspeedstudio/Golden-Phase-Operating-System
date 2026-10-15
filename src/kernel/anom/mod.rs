//! Audited Non-Observable Mode (ANOM) — kernel controller.

pub mod boot;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod decryptor;
pub mod mode;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod object_access;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod object_key;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod object_policy;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod object_protection;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod object_record;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod object_registry;
pub mod policy;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod protocol;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod record;

pub use mode::{AnomSession, AnomStatus};
pub use policy::{authority_label, visibility_label, PolicyError};

pub use boot::finalize_boot;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use object_policy::{
    ObjectProtectionError, ObjectProtectionStatus, ProtectedObjectId, ProtectedObjectKind,
};
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use object_protection::ObjectProtectionController;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use protocol::{
    enter_anom_mode, exit_anom_mode, mask_and_record_process, privileged_open, record_failed_entry,
    seal_test, write_root_manifest, ANOM_STATECHAIN_OBJECT_ID, ROOT_MANIFEST_PATH,
};
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use record::{AnomAadContext, AnomProtectedRecord};

use crate::kernel::authority::StateHash;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_proof::ObjectKind;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::platform::HashMap;

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::anom::record::{seal_anom_phase_state, SealError};
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::crypto::keyring::AnomKeyring;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::crypto::nonce::NonceGenerator;

#[derive(Debug)]
pub struct AnomController {
    status: AnomStatus,
    boot_completed: bool,
    session_counter: u64,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    session: Option<AnomSession>,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    keyring: Option<AnomKeyring>,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    session_seal: StateHash,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    nonce_gen: Option<NonceGenerator>,
    #[cfg(any(feature = "std", feature = "baremetal"))]
    records: HashMap<(ObjectKind, u32), AnomProtectedRecord>,
}

impl Default for AnomController {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomController {
    pub fn new() -> Self {
        Self {
            status: AnomStatus::Disabled,
            boot_completed: false,
            session_counter: 0,
            #[cfg(any(feature = "std", feature = "baremetal"))]
            session: None,
            #[cfg(any(feature = "std", feature = "baremetal"))]
            keyring: None,
            #[cfg(any(feature = "std", feature = "baremetal"))]
            session_seal: [0u8; 32],
            #[cfg(any(feature = "std", feature = "baremetal"))]
            nonce_gen: None,
            #[cfg(any(feature = "std", feature = "baremetal"))]
            records: HashMap::new(),
        }
    }

    pub fn status(&self) -> AnomStatus {
        self.status
    }

    pub fn set_status(&mut self, status: AnomStatus) {
        self.status = status;
    }

    pub fn mark_boot_completed(&mut self) {
        self.boot_completed = true;
        if self.status == AnomStatus::Disabled {
            #[cfg(any(feature = "std", feature = "baremetal"))]
            {
                self.status = AnomStatus::Available;
            }
            #[cfg(not(any(feature = "std", feature = "baremetal")))]
            {
                self.status = AnomStatus::BaremetalStub;
            }
        }
    }

    pub fn boot_completed(&self) -> bool {
        self.boot_completed
    }

    pub fn is_active(&self) -> bool {
        self.status == AnomStatus::Active
    }

    pub fn is_crypto_available(&self) -> bool {
        #[cfg(any(feature = "std", feature = "baremetal"))]
        {
            matches!(
                self.status,
                AnomStatus::Available | AnomStatus::Armed | AnomStatus::Active
            )
        }
        #[cfg(not(any(feature = "std", feature = "baremetal")))]
        {
            false
        }
    }

    pub fn next_session_id(&mut self) -> u64 {
        self.session_counter += 1;
        self.session_counter
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn arm_session(
        &mut self,
        keyring: AnomKeyring,
        session: AnomSession,
        session_seal: StateHash,
    ) {
        let salt = session.current_anom_root;
        self.nonce_gen = Some(NonceGenerator::new(salt));
        self.keyring = Some(keyring);
        self.session = Some(session);
        self.session_seal = session_seal;
        self.status = AnomStatus::Active;
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn disarm(&mut self) {
        self.keyring = None;
        self.session = None;
        self.session_seal = [0u8; 32];
        self.nonce_gen = None;
        self.status = AnomStatus::Available;
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn session(&self) -> Option<&AnomSession> {
        self.session.as_ref()
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn keyring(&self) -> Option<&AnomKeyring> {
        self.keyring.as_ref()
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn session_seal(&self) -> StateHash {
        self.session_seal
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn store_record(&mut self, kind: ObjectKind, id: u32, record: AnomProtectedRecord) {
        self.records.insert((kind, id), record);
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn record(&self, kind: ObjectKind, id: u32) -> Option<&AnomProtectedRecord> {
        self.records.get(&(kind, id))
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn public_view(
        &self,
        kind: ObjectKind,
        id: u32,
        true_phase: crate::core::gphase_core::PhaseState,
        authority: &crate::kernel::authority::AuthorityState,
    ) -> crate::kernel::authority::PublicPhaseView {
        use crate::kernel::authority::PublicPhaseView;
        if !authority.is_anom_protected() {
            return PublicPhaseView::Visible(true_phase);
        }
        self.records
            .get(&(kind, id))
            .map(|r| r.public_view)
            .unwrap_or(PublicPhaseView::Locked)
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    #[allow(clippy::too_many_arguments)]
    pub fn seal_with_keyring(
        &mut self,
        keyring: &AnomKeyring,
        true_phase: crate::core::gphase_core::PhaseState,
        previous_phase: crate::core::gphase_core::PhaseState,
        object_kind: ObjectKind,
        object_id: u32,
        sequence_number: u64,
        operation: &str,
        previous_event_hash: StateHash,
        ctx: &crate::kernel::anom::record::AnomAadContext,
    ) -> Result<AnomProtectedRecord, SealError> {
        let nonce_gen = self
            .nonce_gen
            .as_mut()
            .expect("ANOM session nonce generator");
        seal_anom_phase_state(
            keyring,
            nonce_gen,
            true_phase,
            previous_phase,
            object_kind,
            object_id,
            sequence_number,
            operation,
            previous_event_hash,
            ctx,
        )
    }

    pub fn status_lines(
        &self,
        authority: &crate::kernel::authority::AuthorityState,
    ) -> crate::platform::String {
        use crate::platform::String;
        let mut out = String::new();
        out.push_str(&format!(
            "authority: {}\n",
            policy::authority_label(authority.authority)
        ));
        out.push_str(&format!(
            "visibility: {}\n",
            policy::visibility_label(authority.visibility)
        ));
        let anom_line = match self.status {
            AnomStatus::BaremetalStub => "baremetal stub",
            AnomStatus::HostedOnly | AnomStatus::Available | AnomStatus::Armed => "available",
            AnomStatus::Active => "active",
            AnomStatus::Disabled => "disabled",
            AnomStatus::VerificationFailed => "verification failed",
        };
        out.push_str(&format!("anom: {anom_line}\n"));
        if authority.is_anom_protected() {
            out.push_str("phase view: commitments\n");
            out.push_str("decryptor: kernel/root only\n");
            out.push_str("statechain: recording\n");
            out.push_str("merkle: sealed\n");
        } else {
            out.push_str("phase view: visible\n");
        }
        out
    }

    pub fn anom_root_short(&self) -> Option<StateHash> {
        #[cfg(any(feature = "std", feature = "baremetal"))]
        {
            self.session.as_ref().map(|s| s.current_anom_root)
        }
        #[cfg(not(any(feature = "std", feature = "baremetal")))]
        {
            None
        }
    }
}
