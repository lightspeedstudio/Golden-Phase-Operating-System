//! Audited craft episode replay.

use crate::core::gphase_core::{compose, PhaseState};
use crate::kernel::craft::error::CraftError;
use crate::kernel::craft::fdir::FdirController;
use crate::kernel::craft::state::{CraftKind, FDIR_OBJECT_ID};
use crate::kernel::craft::CraftController;
use crate::kernel::state_proof::{compute_event_hash, ObjectKind};
use crate::kernel::state_root::SubsystemKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftSnapshot {
    pub nav: PhaseState,
    pub prop: PhaseState,
    pub power: PhaseState,
    pub fdir: PhaseState,
    pub prop_quarantined: bool,
}

impl CraftController {
    pub fn snapshot_states(&self) -> CraftSnapshot {
        CraftSnapshot {
            nav: self
                .obj(CraftKind::Navigation.object_id())
                .map(|o| o.state)
                .unwrap_or(PhaseState::Phi0),
            prop: self
                .obj(CraftKind::Propulsion.object_id())
                .map(|o| o.state)
                .unwrap_or(PhaseState::Phi0),
            power: self
                .obj(CraftKind::Power.object_id())
                .map(|o| o.state)
                .unwrap_or(PhaseState::Phi0),
            fdir: self.fdir_state,
            prop_quarantined: self
                .obj(CraftKind::Propulsion.object_id())
                .map(|o| o.quarantined)
                .unwrap_or(false),
        }
    }

    /// Replay craft chains; recompute compose and associator parity from proof payload.
    pub fn replay_episode(&self, _episode_id: u64) -> Result<CraftSnapshot, CraftError> {
        let mut nav = PhaseState::Phi0;
        let mut prop = PhaseState::Phi0;
        let mut power = PhaseState::Phi0;
        let mut fdir = PhaseState::Phi0;
        let mut prop_q = false;

        // Seed from craft_init events if present
        for kind in [
            CraftKind::Navigation,
            CraftKind::Propulsion,
            CraftKind::Power,
        ] {
            let id = kind.object_id();
            if let Some(chain) = self.ledger.chain(ObjectKind::Craft, id) {
                let mut state = PhaseState::Phi0;
                let mut prev_hash = [0u8; 32];
                for (i, event) in chain.events.iter().enumerate() {
                    if event.proof.sequence_number != i as u64 {
                        return Err(CraftError::Replay("non-contiguous sequence".into()));
                    }
                    if event.proof.object_kind != ObjectKind::Craft || event.proof.object_id != id {
                        return Err(CraftError::Replay("object identity changed".into()));
                    }
                    if event.subsystem != SubsystemKind::Craft {
                        return Err(CraftError::Replay("subsystem kind substitution".into()));
                    }
                    if event.proof.previous_event_hash != prev_hash {
                        return Err(CraftError::Replay("previous hash mismatch".into()));
                    }
                    let expected = compute_event_hash(&event.proof, event.assoc_parity);
                    if expected != event.event_hash {
                        return Err(CraftError::Replay("event hash recompute failed".into()));
                    }

                    if let Some(proof) = self
                        .proof_ops
                        .iter()
                        .find(|p| p.object_id == id && p.sequence == event.proof.sequence_number)
                    {
                        let composed = compose(proof.left, proof.right);
                        if let Some(stored) = event.proof.compose_result {
                            if stored != composed {
                                return Err(CraftError::Replay("compose mismatch".into()));
                            }
                        }
                        let parity = FdirController::recompute_assoc_parity(
                            proof.assoc_a,
                            proof.assoc_b,
                            proof.assoc_c,
                        )?;
                        if parity != proof.craft_parity {
                            return Err(CraftError::Replay("assoc_parity mismatch".into()));
                        }
                        if event.proof.previous_state != state
                            && event.proof.operation_name != "craft_init"
                        {
                            return Err(CraftError::Replay("old state != reconstructed".into()));
                        }
                        if event.proof.operation_name != "craft_init"
                            && event.proof.operation_name != "rejected_quarantined_operation"
                            && event.proof.operation_name != "fault_inject"
                            && event.proof.operation_name != "fdir_detect"
                            && event.proof.operation_name != "fdir_isolate"
                            && event.proof.operation_name != "statechain_verify"
                            && event.proof.operation_name != "checkpoint_create"
                            && event.proof.operation_name != "replay_verify"
                            && event.proof.operation_name != "fdir_prove"
                        {
                            let next = compose(state, proof.right);
                            if next != event.proof.requested_state {
                                return Err(CraftError::Replay(
                                    "compose result != recorded new".into(),
                                ));
                            }
                            state = next;
                        } else if event.proof.operation_name == "craft_init"
                            || event.proof.operation_name == "fault_inject"
                        {
                            state = event.proof.requested_state;
                        }
                        if event.proof.operation_name == "fdir_isolate" {
                            prop_q = true;
                        }
                        if event.proof.operation_name == "fdir_prove" {
                            prop_q = false;
                        }
                    } else if event.proof.operation_name == "craft_init" {
                        state = event.proof.requested_state;
                    } else {
                        // Non-transition audit ops keep state
                        match event.proof.operation_name.as_str() {
                            "rejected_quarantined_operation"
                            | "fdir_detect"
                            | "fdir_isolate"
                            | "statechain_verify"
                            | "checkpoint_create"
                            | "replay_verify"
                            | "fdir_prove" => {
                                if event.proof.operation_name == "fdir_isolate" {
                                    prop_q = true;
                                }
                                if event.proof.operation_name == "fdir_prove" {
                                    prop_q = false;
                                }
                            }
                            _ => state = event.proof.requested_state,
                        }
                    }
                    prev_hash = event.event_hash;
                }
                match kind {
                    CraftKind::Navigation => nav = state,
                    CraftKind::Propulsion => prop = state,
                    CraftKind::Power => power = state,
                }
            }
        }

        if let Some(chain) = self.ledger.chain(ObjectKind::Craft, FDIR_OBJECT_ID) {
            let mut state = PhaseState::Phi0;
            let mut prev_hash = [0u8; 32];
            for (i, event) in chain.events.iter().enumerate() {
                if event.proof.sequence_number != i as u64 {
                    return Err(CraftError::Replay("fdir non-contiguous".into()));
                }
                if event.subsystem != SubsystemKind::Craft {
                    return Err(CraftError::Replay(
                        "fdir subsystem kind substitution".into(),
                    ));
                }
                if event.proof.previous_event_hash != prev_hash {
                    return Err(CraftError::Replay("fdir previous hash mismatch".into()));
                }
                let expected = compute_event_hash(&event.proof, event.assoc_parity);
                if expected != event.event_hash {
                    return Err(CraftError::Replay("fdir event hash failed".into()));
                }
                if let Some(proof) = self.proof_ops.iter().find(|p| {
                    p.object_id == FDIR_OBJECT_ID && p.sequence == event.proof.sequence_number
                }) {
                    let parity = FdirController::recompute_assoc_parity(
                        proof.assoc_a,
                        proof.assoc_b,
                        proof.assoc_c,
                    )?;
                    if parity != proof.craft_parity {
                        return Err(CraftError::Replay("fdir assoc_parity mismatch".into()));
                    }
                    match event.proof.operation_name.as_str() {
                        "fdir_detect" | "fdir_isolate" | "fdir_recover" | "fdir_prove"
                            if event.proof.compose_result.is_some()
                                || proof.right != proof.left =>
                        {
                            let next = compose(state, proof.right);
                            if next != event.proof.requested_state
                                && matches!(
                                    event.proof.operation_name.as_str(),
                                    "fdir_detect" | "fdir_isolate" | "fdir_recover" | "fdir_prove"
                                )
                                && event.proof.previous_state != event.proof.requested_state
                            {
                                // state-changing FDIR ops
                            }
                            if event.proof.previous_state != event.proof.requested_state {
                                state = event.proof.requested_state;
                            }
                        }
                        _ => {}
                    }
                    if matches!(
                        event.proof.operation_name.as_str(),
                        "fdir_detect" | "fdir_isolate" | "fdir_recover" | "fdir_prove"
                    ) && event.proof.previous_state != event.proof.requested_state
                    {
                        let next = compose(event.proof.previous_state, proof.right);
                        if next != event.proof.requested_state {
                            return Err(CraftError::Replay("fdir compose mismatch".into()));
                        }
                        state = next;
                    }
                }
                prev_hash = event.event_hash;
            }
            fdir = state;
        }

        Ok(CraftSnapshot {
            nav,
            prop,
            power,
            fdir,
            prop_quarantined: prop_q,
        })
    }

    pub fn replay_object(&self, id: u32) -> Result<PhaseState, CraftError> {
        let snap = self.replay_episode(0)?;
        Ok(match id {
            1 => snap.nav,
            2 => snap.prop,
            3 => snap.power,
            FDIR_OBJECT_ID => snap.fdir,
            _ => return Err(CraftError::NotFound),
        })
    }
}
