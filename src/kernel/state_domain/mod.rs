//! StateDomain VM — virtual phase states over the four-root alphabet.

pub mod domain;
pub mod encoding;
pub mod state_change;
pub mod visualize;

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::platform::{HashMap, String, Vec};
pub use domain::{
    build_chemistry_domain, build_craft_domain, build_domain_mapping, build_network_domain,
    build_security_domain, build_usb_domain, chemistry_labels, compute_domain_root, usb_labels,
    DomainId, DomainState, DomainStateId, StateDomain,
};
pub use encoding::{
    capacity_for_word_length, decode_phase_word, encode_state_index, render_phase_word,
    render_phase_word_ascii, word_length_for_state_count, PhaseWord,
};
#[cfg(any(feature = "std", feature = "baremetal"))]
pub use state_change::{
    build_state_change_proof, compute_state_change_proof_hash, verify_state_change_proof,
};
pub use state_change::{
    infer_domain_from_object, object_id_from_key, StateChangeMode, StateChangeProof,
    VirtualStateInstance, STATEDOMAIN_STATECHAIN_OBJECT_ID,
};
pub use visualize::{
    render_domain_map, render_domain_map_table, render_domains_summary, render_object_view,
    render_root_alphabet, render_transition,
};

#[derive(Debug)]
pub struct StateDomainVm {
    pub mode: state_change::StateChangeMode,
    pub domains: HashMap<domain::DomainId, domain::StateDomain>,
    pub instances: HashMap<String, state_change::VirtualStateInstance>,
    driver_slots: HashMap<String, u32>,
    next_driver_slot: u32,
    equation_root: StateHash,
}

impl Default for StateDomainVm {
    fn default() -> Self {
        Self::new()
    }
}

impl StateDomainVm {
    pub fn new() -> Self {
        Self {
            mode: state_change::StateChangeMode::Root4,
            domains: HashMap::new(),
            instances: HashMap::new(),
            driver_slots: HashMap::new(),
            next_driver_slot: state_change::STATEDOMAIN_FIRST_OBJECT_SLOT,
            equation_root: [0u8; 32],
        }
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn init_with_equation(&mut self, equation_root: StateHash) {
        self.equation_root = equation_root;
        self.domains.clear();
        self.domains.insert(
            domain::DomainId::Chemistry,
            build_chemistry_domain(&equation_root),
        );
        self.domains
            .insert(domain::DomainId::Craft, build_craft_domain(&equation_root));
        self.domains.insert(
            domain::DomainId::Network,
            build_network_domain(&equation_root),
        );
        self.domains.insert(
            domain::DomainId::Security,
            build_security_domain(&equation_root),
        );
        self.domains
            .insert(domain::DomainId::Usb, build_usb_domain(&equation_root));
    }

    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    pub fn init_with_equation(&mut self, _equation_root: StateHash) {
        self.equation_root = [0u8; 32];
    }

    pub fn set_mode(&mut self, mode: state_change::StateChangeMode) {
        self.mode = mode;
    }

    pub fn domain(&self, id: &domain::DomainId) -> Option<&domain::StateDomain> {
        self.domains.get(id)
    }

    pub fn domains_list(&self) -> Vec<domain::StateDomain> {
        let mut out: Vec<_> = self.domains.values().cloned().collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn instance(&self, object_key: &str) -> Option<&state_change::VirtualStateInstance> {
        self.instances.get(object_key)
    }

    pub fn driver_chain_id_for(&self, object_key: &str) -> Option<u32> {
        self.driver_slots.get(object_key).copied()
    }

    pub fn driver_chain_id(&mut self, object_key: &str) -> u32 {
        if let Some(&id) = self.driver_slots.get(object_key) {
            return id;
        }
        let id = self.next_driver_slot;
        self.next_driver_slot = self.next_driver_slot.saturating_add(1);
        self.driver_slots.insert(object_key.into(), id);
        id
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn change_virtual_state(
        &mut self,
        statechain: &mut crate::kernel::statechain::StatechainLedger,
        clock: &crate::stateclock::StateClock,
        object_key: &str,
        domain_id: &domain::DomainId,
        new_state_label: &str,
        operation: &str,
    ) -> Result<state_change::StateChangeProof, StateChangeError> {
        use crate::kernel::state_proof::ObjectKind;
        use crate::kernel::state_root::SubsystemKind;

        let domain = self
            .domains
            .get(domain_id)
            .ok_or(StateChangeError::UnknownDomain)?
            .clone();
        let new_state = domain
            .state_by_label(new_state_label)
            .ok_or(StateChangeError::UnknownState)?
            .clone();

        let object_id = self.driver_chain_id(object_key);
        let prev_hash = statechain
            .chain(ObjectKind::Driver, object_id)
            .and_then(|c| c.head())
            .unwrap_or([0u8; 32]);

        let snapshot = clock.snapshot();
        let (old_label, old_word) = if let Some(inst) = self.instances.get(object_key) {
            (inst.virtual_state_label.clone(), inst.phase_word.clone())
        } else {
            let initial = domain
                .states
                .first()
                .ok_or(StateChangeError::UnknownState)?;
            (initial.label.clone(), initial.phase_word.clone())
        };

        let proof = state_change::build_state_change_proof(
            &statechain.equation_root(),
            &domain,
            object_id,
            &old_label,
            &old_word,
            &new_state.label,
            &new_state.phase_word,
            operation,
            &snapshot,
            &prev_hash,
        );

        let root_phase = new_state.root_hint.unwrap_or(PhaseState::Phi1);

        let payload = format!(
            "domain_state_change|{}|{}->{}|proof={:02x}{:02x}",
            domain.name, old_label, new_state.label, proof.proof_hash[0], proof.proof_hash[1],
        );

        let event = statechain
            .record_transition(
                ObjectKind::Driver,
                object_id,
                SubsystemKind::Driver,
                root_phase,
                root_phase,
                "domain_state_change",
                None,
                payload.as_bytes(),
            )
            .map_err(|_| StateChangeError::Statechain)?;

        let instance = state_change::VirtualStateInstance {
            object_key: object_key.into(),
            driver_chain_id: object_id,
            domain_id: domain_id.clone(),
            virtual_state_id: new_state.id,
            virtual_state_label: new_state.label.clone(),
            phase_word: new_state.phase_word.clone(),
            current_root_phase: root_phase,
            last_proof_hash: proof.proof_hash,
            last_event_hash: event.event_hash,
        };
        self.instances.insert(object_key.into(), instance);

        Ok(proof)
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn ensure_instance(
        &mut self,
        object_key: &str,
        domain_id: &domain::DomainId,
    ) -> Result<(), StateChangeError> {
        if self.instances.contains_key(object_key) {
            return Ok(());
        }
        let chain_id = self.driver_chain_id(object_key);
        let domain = self
            .domains
            .get(domain_id)
            .ok_or(StateChangeError::UnknownDomain)?;
        let initial = domain
            .states
            .first()
            .ok_or(StateChangeError::UnknownState)?;
        let root_phase = initial.root_hint.unwrap_or(PhaseState::Phi0);
        self.instances.insert(
            object_key.into(),
            state_change::VirtualStateInstance {
                object_key: object_key.into(),
                driver_chain_id: chain_id,
                domain_id: domain_id.clone(),
                virtual_state_id: initial.id,
                virtual_state_label: initial.label.clone(),
                phase_word: initial.phase_word.clone(),
                current_root_phase: root_phase,
                last_proof_hash: [0u8; 32],
                last_event_hash: [0u8; 32],
            },
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateChangeError {
    UnknownDomain,
    UnknownState,
    Statechain,
    HostedOnly,
}

impl StateChangeError {
    pub fn reason(&self) -> &'static str {
        match self {
            StateChangeError::UnknownDomain => "unknown domain",
            StateChangeError::UnknownState => "unknown domain state",
            StateChangeError::Statechain => "statechain unavailable",
            StateChangeError::HostedOnly => "hosted-only",
        }
    }
}
