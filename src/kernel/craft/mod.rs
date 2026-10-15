//! Avenue 10 — Layer 11 Craft Fixture.
//!
//! Universal GPL mission cycle (operands Φ1, Φ3, Φ1, Φ3) shared by
//! navigation, propulsion, power, and FDIR. Feature-gated: `craft`.

pub mod command;
pub mod controller;
pub mod error;
pub mod fdir;
pub mod replay;
pub mod state;
pub mod transport;
#[cfg(feature = "craft")]
pub mod transport_phase;

pub use command::CraftCommand;
pub use controller::{CraftController, TransitionProofOp};
pub use error::CraftError;
pub use fdir::FdirController;
pub use replay::CraftSnapshot;
pub use state::{
    fdir_label, CraftKind, CraftObject, FdirEpisode, FDIR_OBJECT_ID, NAV_SOLVING_MAX_TICKS,
    POWER_LOAD_MAX_TICKS, PROP_FIRING_MAX_TICKS,
};
pub use transport::{CraftPacketRecord, CraftTransport, LocalCraftTransport};
pub use transport_phase::PhaseTransportCraftAdapter;

use crate::core::gphase_core::PhaseState;
use crate::kernel::state_proof::ObjectKind;
use crate::platform::String;

/// Scripted propulsion-stuck mission (Avenue 10 acceptance scenario).
pub fn prop_stuck_scenario(craft: &mut CraftController) -> Result<String, CraftError> {
    craft.init_craft()?;
    craft.execute(CraftCommand::PowerReady, None)?;
    craft.execute(CraftCommand::NavSolve, None)?;
    craft.execute(CraftCommand::NavLock, None)?;
    craft.execute(CraftCommand::PropArm, None)?;
    craft.execute(CraftCommand::PowerLoad, None)?;
    craft.execute(CraftCommand::PropFire, None)?;
    craft.fault_inject_prop_stuck("stuck_firing_behavior")?;
    craft.tick(PROP_FIRING_MAX_TICKS + 1)?;
    let prop = CraftKind::Propulsion.object_id();
    if craft.obj(prop)?.ticks_in_state <= PROP_FIRING_MAX_TICKS {
        return Err(CraftError::Replay("dwell did not exceed threshold".into()));
    }
    let episode = FdirController::detect_propulsion_stuck(craft)?;
    FdirController::isolate(craft, episode)?;
    if !craft.obj(prop)?.quarantined {
        return Err(CraftError::Replay("expected quarantine".into()));
    }
    // Mark fault + protect, then rejected fire, then safe reset + FDIR recover
    craft.execute(CraftCommand::PropMarkFault, Some(episode))?;
    craft.execute(CraftCommand::PowerProtect, Some(episode))?;
    match craft.execute(CraftCommand::PropFire, Some(episode)) {
        Err(CraftError::Quarantined) => craft.note("attempt PROP_FIRE rejected and audited"),
        other => {
            return Err(CraftError::Replay(format!(
                "expected quarantine reject, got {other:?}"
            )))
        }
    }
    craft.execute(CraftCommand::PropSafeReset, Some(episode))?;
    craft.apply_fdir(CraftCommand::FdirRecover, episode)?;
    FdirController::prove(craft, episode)?;

    let snap = craft.snapshot_states();
    if snap.nav != PhaseState::Phi3
        || snap.prop != PhaseState::Phi0
        || snap.power != PhaseState::Phi3
        || snap.fdir != PhaseState::Phi0
        || snap.prop_quarantined
    {
        return Err(CraftError::Replay(format!(
            "final state mismatch: {snap:?}"
        )));
    }

    craft.note("CRAFT FDIR PASS");
    let mut out = String::new();
    for line in &craft.transcript {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&craft.status_report());
    out.push_str("CRAFT FDIR PASS\n");
    Ok(out)
}

/// Cross-check the transported packet log against the audited statechain.
///
/// For every recorded object transition that crossed the transport (i.e.
/// carries `Some(packet_id)` in `proof_ops`), resolve the matching
/// `CraftPacketRecord` from `craft.transport.records()` and assert:
///   - the transported opcode equals the statechain's recorded operation
///     name for that same (object, sequence) pair;
///   - the packet was acknowledged (every `execute()` send is auto-acked);
///   - the associator parity recomputed from the proof triple matches the
///     parity recorded alongside the transition.
///
/// This does not re-derive compose or hash chains — `replay_episode` (also
/// invoked here) already owns that. This function only proves the *wire*
/// (what physically crossed the carrier) agrees with the *ledger* (what the
/// kernel says happened).
pub fn verify_packet_replay(craft: &CraftController) -> Result<String, CraftError> {
    let records = craft.transport.records();
    let mut out = String::new();
    out.push_str("PACKET REPLAY CROSS-CHECK\n");
    let mut checked = 0usize;

    for kind in [
        CraftKind::Navigation,
        CraftKind::Propulsion,
        CraftKind::Power,
    ] {
        let id = kind.object_id();
        let chain = craft
            .ledger
            .chain(ObjectKind::Craft, id)
            .ok_or_else(|| CraftError::Replay(format!("missing statechain for object {id}")))?;
        for event in &chain.events {
            let seq = event.proof.sequence_number;
            let Some(proof_op) = craft
                .proof_ops
                .iter()
                .find(|p| p.object_id == id && p.sequence == seq)
            else {
                continue;
            };
            let Some(pkt_id) = proof_op.packet_id else {
                continue;
            };
            let record = records
                .iter()
                .find(|r| r.packet_id == pkt_id)
                .ok_or_else(|| {
                    CraftError::Replay(format!("packet {pkt_id} missing from transport log"))
                })?;
            if event.proof.operation_name == "rejected_quarantined_operation" {
                // Reject path: the packet on the wire still carries the
                // *attempted* opcode (e.g. "prop_fire"), while the ledger
                // audits it under the wrapper op name "rejected_quarantined_
                // operation" with the attempted opcode bytes as payload. Tie
                // the two together via the payload hash rather than a direct
                // op-name string match.
                let expected_hash =
                    crate::kernel::state_proof::hash_payload(record.opcode.as_bytes());
                if expected_hash != event.proof.payload_hash {
                    return Err(CraftError::Replay(format!(
                        "packet {pkt_id} attempted opcode '{}' does not match audited reject payload hash",
                        record.opcode
                    )));
                }
            } else if record.opcode != event.proof.operation_name {
                return Err(CraftError::Replay(format!(
                    "packet {pkt_id} transported opcode '{}' != audited op '{}'",
                    record.opcode, event.proof.operation_name
                )));
            }
            // Rejected-while-quarantined sends are deliberately never
            // acknowledged (the command was refused, not delivered) — every
            // other transported op is auto-acked inside `execute()`.
            if event.proof.operation_name != "rejected_quarantined_operation" && !record.ack {
                return Err(CraftError::Replay(format!(
                    "packet {pkt_id} (op={}) was never acknowledged",
                    record.opcode
                )));
            }
            let parity = FdirController::recompute_assoc_parity(
                proof_op.assoc_a,
                proof_op.assoc_b,
                proof_op.assoc_c,
            )?;
            if parity != proof_op.craft_parity {
                return Err(CraftError::Replay(format!(
                    "packet {pkt_id} assoc_parity mismatch: recomputed {parity} != recorded {}",
                    proof_op.craft_parity
                )));
            }
            checked += 1;
            out.push_str(&format!(
                "  pkt={pkt_id} object={} op={} ack={} assoc_parity={}\n",
                kind.as_str(),
                event.proof.operation_name,
                record.ack,
                parity
            ));
        }
    }

    if checked == 0 {
        return Err(CraftError::Replay(
            "no transported packets cross-checked against statechain".into(),
        ));
    }

    // Statechain-level replay: rebuild state from the ledger alone (hashes,
    // compose, assoc_parity) and confirm it matches the live controller —
    // independent confirmation on top of the per-packet opcode check above.
    let live = craft.snapshot_states();
    let replayed = craft.replay_episode(0)?;
    if live != replayed {
        return Err(CraftError::Replay(format!(
            "packet-replay: live {live:?} != statechain-replayed {replayed:?}"
        )));
    }

    out.push_str(&format!(
        "packets cross-checked: {checked} (opcode+ack+assoc_parity match audited statechain)\n"
    ));
    out.push_str("statechain replay: live == replayed (OK)\n");

    match &craft.last_checkpoint {
        Some(cp) => out.push_str(&format!(
            "sealed checkpoint: sequence={} tick={}\n",
            cp.sequence, cp.tick
        )),
        None => {
            return Err(CraftError::Replay(
                "no sealed checkpoint found after mission proof".into(),
            ))
        }
    }
    Ok(out)
}

/// Avenue 10B WP-2 — the same propulsion-stuck FDIR mission as
/// `prop_stuck_scenario`, but every `execute()` send crosses the real
/// (hosted-sim) glider carrier. Caller supplies a `craft` controller built
/// with `CraftController::new_over_glider()` so every command physically
/// transits the M=4 diagonal port lattice. After the mission proves and
/// seals its checkpoint, the transported packet log is cross-checked
/// against the audited statechain.
///
/// Hosted-sim scenario: this runs the compose kernel and the phase-transport
/// glider carrier entirely in software over the sealed Avenue 2 glider law —
/// it is not a claim of live-on-silicon execution.
pub fn prop_stuck_over_glider_scenario(craft: &mut CraftController) -> Result<String, CraftError> {
    let mission_report = prop_stuck_scenario(craft)?;
    let replay_report = verify_packet_replay(craft)?;

    let mut out = String::new();
    out.push_str("CRAFT GLIDER SCENARIO (hosted-sim over Avenue 2 glider carrier)\n");
    out.push_str(&mission_report);
    out.push_str(&replay_report);
    out.push_str("CRAFT GLIDER SCENARIO PASS\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::compose;
    use crate::kernel::state_proof::ObjectKind;
    use crate::kernel::state_root::SubsystemKind;

    #[test]
    fn object_kind_numeric_compatibility() {
        assert_eq!(ObjectKind::Process.as_u8(), 0);
        assert_eq!(ObjectKind::Driver.as_u8(), 7);
        assert_eq!(ObjectKind::Craft.as_u8(), 8);
        assert_eq!(ObjectKind::from_u8(8), Some(ObjectKind::Craft));
        #[cfg(not(feature = "phasestack"))]
        assert_eq!(ObjectKind::from_u8(9), None);
        #[cfg(feature = "phasestack")]
        assert_eq!(ObjectKind::from_u8(9), Some(ObjectKind::App));
        for v in 0u8..=8 {
            let k = ObjectKind::from_u8(v).unwrap();
            assert_eq!(k.as_u8(), v);
        }
    }

    #[test]
    fn subsystem_kind_numeric_compatibility() {
        assert_eq!(SubsystemKind::Process.as_u8(), 0);
        assert_eq!(SubsystemKind::Driver.as_u8(), 5);
        assert_eq!(SubsystemKind::Craft.as_u8(), 6);
        assert_eq!(SubsystemKind::from_u8(6), Some(SubsystemKind::Craft));
        assert_eq!(SubsystemKind::from_u8(7), None);
    }

    #[test]
    fn mission_cycle_compose() {
        assert_eq!(
            compose(PhaseState::Phi0, PhaseState::Phi1),
            PhaseState::Phi1
        );
        assert_eq!(
            compose(PhaseState::Phi1, PhaseState::Phi3),
            PhaseState::Phi2
        );
        assert_eq!(
            compose(PhaseState::Phi2, PhaseState::Phi1),
            PhaseState::Phi3
        );
        assert_eq!(
            compose(PhaseState::Phi3, PhaseState::Phi3),
            PhaseState::Phi0
        );
    }

    #[test]
    fn prop_stuck_passes() {
        let mut c = CraftController::new();
        let report = prop_stuck_scenario(&mut c).unwrap();
        assert!(report.contains("CRAFT FDIR PASS"));
    }

    #[test]
    fn prop_stuck_over_glider_passes() {
        let mut c = CraftController::new_over_glider().expect("controller over glider carrier");
        let report = prop_stuck_over_glider_scenario(&mut c).unwrap();
        assert!(report.contains("CRAFT FDIR PASS"));
        assert!(report.contains("CRAFT GLIDER SCENARIO PASS"));
        assert!(report.contains("packets cross-checked"));
    }
}
