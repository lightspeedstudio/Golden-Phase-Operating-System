//! FDIR detect → isolate → recover → prove.

use crate::core::gphase_core::{compose, left_group, reflect, right_group, PhaseState};
use crate::kernel::craft::command::CraftCommand;
use crate::kernel::craft::error::CraftError;
use crate::kernel::craft::state::{
    fdir_label, CraftKind, FdirEpisode, CURVATURE_Z_SUPPORT, FDIR_OBJECT_ID, NAV_SOLVING_MAX_TICKS,
    POWER_LOAD_MAX_MEASURED, POWER_LOAD_MAX_TICKS, PROP_FIRING_MAX_TICKS,
};
use crate::kernel::craft::CraftController;
use crate::platform::{String, Vec};

pub struct FdirController;

impl FdirController {
    pub fn anomaly(
        kind: CraftKind,
        state: PhaseState,
        ticks: u64,
        measured_load: u64,
    ) -> Option<(u64, u64)> {
        match kind {
            CraftKind::Navigation => {
                if state == PhaseState::Phi2 && ticks > NAV_SOLVING_MAX_TICKS {
                    Some((ticks, NAV_SOLVING_MAX_TICKS))
                } else {
                    None
                }
            }
            CraftKind::Propulsion => {
                if state == PhaseState::Phi3 {
                    Some((ticks, 0))
                } else if state == PhaseState::Phi2 && ticks > PROP_FIRING_MAX_TICKS {
                    Some((ticks, PROP_FIRING_MAX_TICKS))
                } else {
                    None
                }
            }
            CraftKind::Power => {
                if state == PhaseState::Phi2
                    && (ticks > POWER_LOAD_MAX_TICKS || measured_load > POWER_LOAD_MAX_MEASURED)
                {
                    Some((ticks, POWER_LOAD_MAX_TICKS))
                } else {
                    None
                }
            }
        }
    }

    pub fn detect_propulsion_stuck(ctl: &mut CraftController) -> Result<u64, CraftError> {
        let prop = CraftKind::Propulsion.object_id();
        let obj = ctl.obj(prop)?.clone();
        let Some((dwell, threshold)) = Self::anomaly(
            CraftKind::Propulsion,
            obj.state,
            obj.ticks_in_state,
            obj.measured_load,
        ) else {
            return Err(CraftError::IllegalTransition("no propulsion anomaly"));
        };
        let z = ctl.curvature.rolling_z().unwrap_or(0.0);
        let _support = z.abs() >= CURVATURE_Z_SUPPORT;
        let episode_id = ctl.next_episode;
        ctl.next_episode += 1;
        let prev_hash = obj.last_transition_hash.unwrap_or([0u8; 32]);
        ctl.episodes.push(FdirEpisode {
            id: episode_id,
            object_id: prop,
            reason: "prop_firing_dwell".into(),
            detected_tick: ctl.tick,
            dwell,
            threshold,
            curvature_z: z,
            previous_event_hash: prev_hash,
        });
        ctl.apply_fdir(CraftCommand::FdirDetect, episode_id)?;
        ctl.note(format!(
            "detect: dwell={dwell} threshold={threshold} z={z:.3} episode={episode_id}"
        ));
        Ok(episode_id)
    }

    pub fn isolate(ctl: &mut CraftController, episode_id: u64) -> Result<(), CraftError> {
        let ep = ctl
            .episodes
            .iter()
            .find(|e| e.id == episode_id)
            .ok_or(CraftError::NotFound)?
            .clone();
        ctl.apply_fdir(CraftCommand::FdirIsolate, episode_id)?;
        let o = ctl.obj_mut(ep.object_id)?;
        o.quarantined = true;
        o.current_episode = Some(episode_id);
        ctl.note(format!(
            "isolate: quarantine object={} episode={}",
            ep.object_id, episode_id
        ));
        Ok(())
    }

    pub fn recover_propulsion(
        ctl: &mut CraftController,
        episode_id: u64,
    ) -> Result<(), CraftError> {
        ctl.execute(CraftCommand::PropMarkFault, Some(episode_id))?;
        ctl.execute(CraftCommand::PowerProtect, Some(episode_id))?;
        ctl.execute(CraftCommand::PropSafeReset, Some(episode_id))?;
        ctl.apply_fdir(CraftCommand::FdirRecover, episode_id)?;
        Ok(())
    }

    pub fn prove(ctl: &mut CraftController, episode_id: u64) -> Result<(), CraftError> {
        if ctl.fdir_state != PhaseState::Phi3 {
            return Err(CraftError::ProofIncomplete(
                "FDIR must be recovered_pending_proof before prove",
            ));
        }
        // Episode link must still point at a real prior event hash.
        let ep = ctl
            .episodes
            .iter()
            .find(|e| e.id == episode_id)
            .ok_or(CraftError::NotFound)?;
        if ep.object_id == 0 {
            return Err(CraftError::Replay("episode link missing object".into()));
        }
        ctl.ledger.verify_all()?;
        let live = ctl.snapshot_states();
        let replayed = ctl.replay_episode(episode_id)?;
        if live.nav != replayed.nav
            || live.prop != replayed.prop
            || live.power != replayed.power
            || live.fdir != replayed.fdir
        {
            return Err(CraftError::Replay(format!(
                "live {:?} != replayed {:?}",
                live, replayed
            )));
        }
        let cp = ctl.ledger.seal_checkpoint()?;
        ctl.last_checkpoint = Some(cp.clone());
        ctl.ledger.verify_checkpoints()?;
        ctl.audit_payload(
            FDIR_OBJECT_ID,
            ctl.fdir_state,
            ctl.fdir_state,
            "statechain_verify",
            b"PASS",
            ctl.fdir_state,
            ctl.fdir_state,
            episode_id,
            None,
        )?;
        ctl.audit_payload(
            FDIR_OBJECT_ID,
            ctl.fdir_state,
            ctl.fdir_state,
            "checkpoint_create",
            &cp.sequence.to_le_bytes(),
            ctl.fdir_state,
            ctl.fdir_state,
            episode_id,
            None,
        )?;
        ctl.audit_payload(
            FDIR_OBJECT_ID,
            ctl.fdir_state,
            ctl.fdir_state,
            "replay_verify",
            b"PASS",
            ctl.fdir_state,
            ctl.fdir_state,
            episode_id,
            None,
        )?;
        ctl.apply_fdir(CraftCommand::FdirProve, episode_id)?;
        for o in ctl.objects.iter_mut() {
            if o.current_episode == Some(episode_id) {
                o.quarantined = false;
                o.current_episode = None;
            }
        }
        ctl.note("quarantine cleared after proof");
        Ok(())
    }

    pub fn recompute_assoc_parity(
        a: PhaseState,
        b: PhaseState,
        c: PhaseState,
    ) -> Result<u8, CraftError> {
        let left = left_group(a, b, c);
        let right = right_group(a, b, c);
        if left == right {
            Ok(0)
        } else if left == reflect(right) {
            Ok(1)
        } else {
            Err(CraftError::Replay("invalid associator parity".into()))
        }
    }

    pub fn status_line(ctl: &CraftController) -> String {
        format!(
            "FDIR={} ({})",
            ctl.fdir_state.as_u8(),
            fdir_label(ctl.fdir_state)
        )
    }

    pub fn verify_compose_step(prev: PhaseState, operand: PhaseState, next: PhaseState) -> bool {
        compose(prev, operand) == next
    }
}

pub fn scan_anomalies(ctl: &CraftController) -> Vec<(u32, CraftKind, u64, u64)> {
    let mut out = Vec::new();
    for o in &ctl.objects {
        if o.quarantined {
            continue;
        }
        if let Some((dwell, thr)) =
            FdirController::anomaly(o.kind, o.state, o.ticks_in_state, o.measured_load)
        {
            out.push((o.id, o.kind, dwell, thr));
        }
    }
    out
}
