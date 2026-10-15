//! Per-object append-only statechains.

use crate::core::gphase_core::PhaseState;
use crate::kernel::hash::StateHash;
use crate::kernel::state_proof::{
    compute_event_hash, verify_event_hash, verify_proof, ObjectKind, ProofError,
    StateTransitionProof,
};
use crate::kernel::state_root::{compute_equation_root, SubsystemKind};
use crate::kernel::statechain_checkpoint::{
    derive_signing_key, prime_log_for_equation, sign_checkpoint, verify_checkpoint,
    CheckpointError, SignedCheckpoint,
};
use crate::platform::{HashMap, String, Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEvent {
    pub proof: StateTransitionProof,
    pub event_hash: StateHash,
    pub tick: u64,
    pub subsystem: SubsystemKind,
    pub assoc_parity: u8,
    pub accepted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    Proof(ProofError),
    EventHash,
    Reorder,
    Fork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStatechain {
    pub object_kind: ObjectKind,
    pub object_id: u32,
    pub events: Vec<StateEvent>,
}

impl ObjectStatechain {
    pub fn new(object_kind: ObjectKind, object_id: u32) -> Self {
        Self {
            object_kind,
            object_id,
            events: Vec::new(),
        }
    }

    pub fn head(&self) -> Option<StateHash> {
        self.events.last().map(|e| e.event_hash)
    }

    pub fn append(
        &mut self,
        mut proof: StateTransitionProof,
        tick: u64,
        subsystem: SubsystemKind,
        assoc_parity: u8,
    ) -> Result<StateEvent, ChainError> {
        let expected_prev = self.head().unwrap_or([0u8; 32]);
        proof.previous_event_hash = expected_prev;
        proof.sequence_number = self.events.len() as u64;
        verify_proof(&proof, &expected_prev).map_err(ChainError::Proof)?;
        let event_hash = compute_event_hash(&proof, assoc_parity);
        if !verify_event_hash(&proof, assoc_parity, &event_hash) {
            return Err(ChainError::EventHash);
        }
        let event = StateEvent {
            proof,
            event_hash,
            tick,
            subsystem,
            assoc_parity,
            accepted: true,
            reason: String::new(),
        };
        self.events.push(event.clone());
        Ok(event)
    }

    pub fn verify(&self) -> Result<StateHash, ChainError> {
        let mut prev = [0u8; 32];
        for (seq, event) in self.events.iter().enumerate() {
            if event.proof.sequence_number != seq as u64 {
                return Err(ChainError::Reorder);
            }
            if event.proof.previous_event_hash != prev {
                return Err(ChainError::Fork);
            }
            verify_proof(&event.proof, &prev).map_err(ChainError::Proof)?;
            if !verify_event_hash(&event.proof, event.assoc_parity, &event.event_hash) {
                return Err(ChainError::EventHash);
            }
            prev = event.event_hash;
        }
        Ok(prev)
    }
}

#[derive(Debug, Default)]
pub struct StatechainLedger {
    equation_root: StateHash,
    /// Public-derivable HMAC key (`derive_signing_key(equation_root)`). Not a secret.
    signing_key: StateHash,
    prime_log: StateHash,
    tick: u64,
    reflection_defects: u64,
    checkpoint_sequence: u64,
    checkpoints: Vec<SignedCheckpoint>,
    chains: HashMap<(ObjectKind, u32), ObjectStatechain>,
    process_heads: Vec<StateHash>,
    memory_heads: Vec<StateHash>,
    filesystem_heads: Vec<StateHash>,
    vm_heads: Vec<StateHash>,
    syscall_heads: Vec<StateHash>,
    driver_heads: Vec<StateHash>,
    craft_heads: Vec<StateHash>,
    kernel_root: StateHash,
}

impl StatechainLedger {
    pub fn new() -> Self {
        let equation = compute_equation_root();
        let equation_root = equation.hash;
        let signing_key = derive_signing_key(&equation_root);
        let prime_log = prime_log_for_equation(&equation_root);
        Self {
            equation_root,
            signing_key,
            prime_log,
            tick: 0,
            reflection_defects: 0,
            checkpoint_sequence: 0,
            checkpoints: Vec::new(),
            chains: HashMap::new(),
            process_heads: Vec::new(),
            memory_heads: Vec::new(),
            filesystem_heads: Vec::new(),
            vm_heads: Vec::new(),
            syscall_heads: Vec::new(),
            driver_heads: Vec::new(),
            craft_heads: Vec::new(),
            kernel_root: [0u8; 32],
        }
    }

    pub fn equation_root(&self) -> StateHash {
        self.equation_root
    }

    pub fn kernel_root(&self) -> StateHash {
        self.kernel_root
    }

    /// HMAC key used to tag checkpoints. Not a secret: public-derivable
    /// from [`Self::equation_root`]. See [`crate::kernel::statechain_checkpoint`].
    pub fn signing_key(&self) -> &StateHash {
        &self.signing_key
    }

    pub fn checkpoints(&self) -> &[SignedCheckpoint] {
        &self.checkpoints
    }

    pub fn latest_checkpoint(&self) -> Option<&SignedCheckpoint> {
        self.checkpoints.last()
    }

    #[doc(hidden)]
    pub fn replace_checkpoint_for_audit_test(
        &mut self,
        index: usize,
        checkpoint: SignedCheckpoint,
    ) {
        self.checkpoints[index] = checkpoint;
    }

    pub fn seal_checkpoint(&mut self) -> Result<SignedCheckpoint, CheckpointError> {
        if self.kernel_root == [0u8; 32] {
            return Err(CheckpointError::Empty);
        }
        let checkpoint = sign_checkpoint(
            &self.signing_key,
            self.checkpoint_sequence,
            self.tick,
            &self.kernel_root,
            &self.equation_root,
            &self.prime_log,
        );
        self.checkpoint_sequence += 1;
        self.checkpoints.push(checkpoint.clone());
        Ok(checkpoint)
    }

    #[cfg(feature = "phasestack")]
    pub fn seal_checkpoint_v2(
        &mut self,
        app_registry_root: StateHash,
    ) -> Result<SignedCheckpoint, CheckpointError> {
        if self.kernel_root == [0u8; 32] {
            return Err(CheckpointError::Empty);
        }
        let checkpoint = crate::kernel::statechain_checkpoint::sign_checkpoint_v2(
            &self.signing_key,
            self.checkpoint_sequence,
            self.tick,
            &self.kernel_root,
            &self.equation_root,
            &self.prime_log,
            &app_registry_root,
        );
        self.checkpoint_sequence += 1;
        self.checkpoints.push(checkpoint.clone());
        Ok(checkpoint)
    }

    pub fn verify_checkpoints(&self) -> Result<(), CheckpointError> {
        if self.checkpoints.is_empty() {
            return Err(CheckpointError::Empty);
        }
        for checkpoint in &self.checkpoints {
            if !verify_checkpoint(&self.signing_key, checkpoint) {
                #[cfg(feature = "phasestack")]
                {
                    if crate::kernel::statechain_checkpoint::verify_checkpoint_v2(
                        &self.signing_key,
                        checkpoint,
                    ) {
                        continue;
                    }
                }
                return Err(CheckpointError::BadSignature);
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn set_kernel_root_for_audit_test(&mut self, hash: StateHash) {
        self.kernel_root = hash;
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn reflection_defects(&self) -> u64 {
        self.reflection_defects
    }

    pub fn assoc_parity(&self) -> u8 {
        (self.reflection_defects % 2) as u8
    }

    pub fn sync_reflection_defects(&mut self, count: u64) {
        self.reflection_defects = count;
    }

    #[doc(hidden)]
    pub fn flip_assoc_parity_for_audit_test(&mut self, kind: ObjectKind, id: u32, index: usize) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].assoc_parity ^= 1;
    }

    #[doc(hidden)]
    pub fn xor_event_hash_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        mask: u8,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].event_hash[0] ^= mask;
    }

    #[doc(hidden)]
    pub fn set_event_object_id_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        new_id: u32,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].proof.object_id = new_id;
    }

    #[doc(hidden)]
    pub fn swap_events_for_audit_test(&mut self, kind: ObjectKind, id: u32, i: usize, j: usize) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events.swap(i, j);
    }

    #[doc(hidden)]
    pub fn delete_event_for_audit_test(&mut self, kind: ObjectKind, id: u32, index: usize) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events.remove(index);
    }

    #[doc(hidden)]
    pub fn set_event_previous_state_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        state: PhaseState,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].proof.previous_state = state;
    }

    #[doc(hidden)]
    pub fn set_event_operation_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        name: &str,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].proof.operation_name = name.into();
    }

    #[doc(hidden)]
    pub fn set_event_sequence_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        sequence: u64,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].proof.sequence_number = sequence;
    }

    #[doc(hidden)]
    pub fn set_event_subsystem_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        subsystem: SubsystemKind,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].subsystem = subsystem;
    }

    #[doc(hidden)]
    pub fn set_event_compose_result_for_audit_test(
        &mut self,
        kind: ObjectKind,
        id: u32,
        index: usize,
        result: Option<PhaseState>,
    ) {
        let chain = self.chains.get_mut(&(kind, id)).expect("chain exists");
        chain.events[index].proof.compose_result = result;
    }

    pub fn chain(&self, kind: ObjectKind, id: u32) -> Option<&ObjectStatechain> {
        self.chains.get(&(kind, id))
    }

    fn subsystem_heads_mut(&mut self, kind: SubsystemKind) -> &mut Vec<StateHash> {
        match kind {
            SubsystemKind::Process => &mut self.process_heads,
            SubsystemKind::Memory => &mut self.memory_heads,
            SubsystemKind::Filesystem => &mut self.filesystem_heads,
            SubsystemKind::Vm => &mut self.vm_heads,
            SubsystemKind::Syscall => &mut self.syscall_heads,
            SubsystemKind::Driver => &mut self.driver_heads,
            SubsystemKind::Craft => &mut self.craft_heads,
        }
    }

    fn refresh_subsystem_head(&mut self, kind: SubsystemKind, object_id: u32, head: StateHash) {
        let heads = self.subsystem_heads_mut(kind);
        if (object_id as usize) >= heads.len() {
            heads.resize(object_id as usize + 1, [0u8; 32]);
        }
        heads[object_id as usize] = head;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_transition(
        &mut self,
        object_kind: ObjectKind,
        object_id: u32,
        subsystem: SubsystemKind,
        previous_state: PhaseState,
        requested_state: PhaseState,
        operation: &str,
        compose_result: Option<PhaseState>,
        payload: &[u8],
    ) -> Result<StateEvent, ChainError> {
        self.tick += 1;
        let key = (object_kind, object_id);
        self.chains
            .entry(key)
            .or_insert_with(|| ObjectStatechain::new(object_kind, object_id));
        let proof = StateTransitionProof {
            object_kind,
            object_id,
            sequence_number: 0,
            previous_state,
            requested_state,
            operation_name: operation.into(),
            compose_result,
            topology_status: crate::kernel::state_proof::TopologyProofStatus::None,
            payload_hash: crate::kernel::state_proof::hash_payload(payload),
            previous_event_hash: [0u8; 32],
            equation_root: self.equation_root,
        };
        let assoc_parity = self.assoc_parity();
        let chain = self.chains.get_mut(&key).expect("just inserted");
        let event = chain.append(proof, self.tick, subsystem, assoc_parity)?;
        if let Some(head) = chain.head() {
            self.refresh_subsystem_head(subsystem, object_id, head);
        }
        self.recompute_kernel_root();
        Ok(event)
    }

    pub fn recompute_kernel_root(&mut self) {
        let root = crate::kernel::state_root::compute_kernel_state_root(
            &self.equation_root,
            &self.heads_for_audit(SubsystemKind::Process),
            &self.heads_for_audit(SubsystemKind::Memory),
            &self.heads_for_audit(SubsystemKind::Filesystem),
            &self.heads_for_audit(SubsystemKind::Vm),
            &self.heads_for_audit(SubsystemKind::Syscall),
            &self.heads_for_audit(SubsystemKind::Driver),
            &self.heads_for_audit(SubsystemKind::Craft),
        );
        self.kernel_root = root.hash;
    }

    pub fn verify_all(&self) -> Result<(), ChainError> {
        for chain in self.chains.values() {
            chain.verify()?;
        }
        Ok(())
    }

    pub(crate) fn heads_for_audit(&self, kind: SubsystemKind) -> Vec<StateHash> {
        let raw = match kind {
            SubsystemKind::Process => &self.process_heads,
            SubsystemKind::Memory => &self.memory_heads,
            SubsystemKind::Filesystem => &self.filesystem_heads,
            SubsystemKind::Vm => &self.vm_heads,
            SubsystemKind::Syscall => &self.syscall_heads,
            SubsystemKind::Driver => &self.driver_heads,
            SubsystemKind::Craft => &self.craft_heads,
        };
        raw.iter().copied().filter(|h| *h != [0u8; 32]).collect()
    }

    pub fn subsystem_merkle_root(&self, kind: SubsystemKind) -> StateHash {
        crate::kernel::state_root::merkle_root(&self.heads_for_audit(kind))
    }
}
