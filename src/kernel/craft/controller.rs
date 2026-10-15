//! Craft controller — semantic guards + canonical compose + statechain.

use crate::core::gphase_core::{compose, PhaseState};
use crate::kernel::craft::command::CraftCommand;
use crate::kernel::craft::error::CraftError;
use crate::kernel::craft::fdir::FdirController;
use crate::kernel::craft::state::{
    fdir_label, CraftKind, CraftObject, FdirEpisode, FDIR_OBJECT_ID, PROP_FIRING_MAX_TICKS,
};
use crate::kernel::craft::transport::{CraftTransport, LocalCraftTransport};
use crate::kernel::curvature::CurvatureLog;
use crate::kernel::hash::StateHash;
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::statechain::{StateEvent, StatechainLedger};
use crate::kernel::statechain_checkpoint::SignedCheckpoint;
use crate::platform::{String, Vec};

#[derive(Debug, Clone)]
pub struct TransitionProofOp {
    pub object_id: u32,
    pub sequence: u64,
    pub left: PhaseState,
    pub right: PhaseState,
    pub assoc_a: PhaseState,
    pub assoc_b: PhaseState,
    pub assoc_c: PhaseState,
    pub craft_parity: u8,
    pub episode_id: u64,
    pub packet_id: Option<u64>,
}

#[derive(Debug)]
pub struct CraftController {
    pub objects: Vec<CraftObject>,
    pub fdir_state: PhaseState,
    pub fdir_last_hash: Option<StateHash>,
    pub ledger: StatechainLedger,
    pub curvature: CurvatureLog,
    pub episodes: Vec<FdirEpisode>,
    pub next_episode: u64,
    pub tick: u64,
    pub proof_ops: Vec<TransitionProofOp>,
    pub transport: Box<dyn CraftTransport>,
    pub last_checkpoint: Option<SignedCheckpoint>,
    pub stuck_injected: bool,
    pub transcript: Vec<String>,
}

impl Default for CraftController {
    fn default() -> Self {
        Self::new()
    }
}

impl CraftController {
    pub fn new() -> Self {
        Self::with_transport(Box::new(LocalCraftTransport::new()))
    }

    /// Construct a craft controller wired to a caller-supplied transport
    /// (e.g. `PhaseTransportCraftAdapter` over the real glider carrier).
    pub fn with_transport(transport: Box<dyn CraftTransport>) -> Self {
        Self {
            objects: Vec::new(),
            fdir_state: PhaseState::Phi0,
            fdir_last_hash: None,
            ledger: StatechainLedger::new(),
            curvature: CurvatureLog::new(),
            episodes: Vec::new(),
            next_episode: 1,
            tick: 0,
            proof_ops: Vec::new(),
            transport,
            last_checkpoint: None,
            stuck_injected: false,
            transcript: Vec::new(),
        }
    }

    /// Convenience constructor: craft fixture over the real silicon-verified
    /// glider carrier instead of the in-process `LocalCraftTransport`.
    pub fn new_over_glider() -> Result<Self, String> {
        let adapter = crate::kernel::craft::transport_phase::PhaseTransportCraftAdapter::new()?;
        Ok(Self::with_transport(Box::new(adapter)))
    }

    pub fn obj(&self, id: u32) -> Result<&CraftObject, CraftError> {
        self.objects
            .iter()
            .find(|o| o.id == id)
            .ok_or(CraftError::NotFound)
    }

    pub fn obj_mut(&mut self, id: u32) -> Result<&mut CraftObject, CraftError> {
        self.objects
            .iter_mut()
            .find(|o| o.id == id)
            .ok_or(CraftError::NotFound)
    }

    pub fn init_craft(&mut self) -> Result<(), CraftError> {
        self.objects.clear();
        self.fdir_state = PhaseState::Phi0;
        self.fdir_last_hash = None;
        self.proof_ops.clear();
        self.episodes.clear();
        self.transcript.clear();
        self.stuck_injected = false;
        self.ledger = StatechainLedger::new();
        // transport is intentionally not reconstructed here: it may be a
        // real hardware-backed adapter (see `with_transport`/`new_over_glider`)
        // and mission re-init should not swap it out from under the caller.
        self.tick = 0;
        self.next_episode = 1;
        self.last_checkpoint = None;

        // Mission init: nav stale, prop idle, power off, FDIR normal
        let specs = [
            (CraftKind::Navigation, PhaseState::Phi1),
            (CraftKind::Propulsion, PhaseState::Phi0),
            (CraftKind::Power, PhaseState::Phi0),
        ];
        for (kind, initial) in specs {
            let id = kind.object_id();
            let obj = CraftObject {
                id,
                kind,
                state: initial,
                ticks_in_state: 0,
                quarantined: false,
                active_command: None,
                deadline_tick: None,
                current_episode: None,
                last_transition_hash: None,
                measured_load: 0,
            };
            let event = self.record_raw(
                id,
                PhaseState::Phi0,
                initial,
                "craft_init",
                Some(compose(PhaseState::Phi0, initial)),
                b"craft_init",
                PhaseState::Phi0,
                initial,
                0,
                None,
            )?;
            let mut obj = obj;
            obj.last_transition_hash = Some(event.event_hash);
            self.objects.push(obj);
        }
        let fe = self.record_raw(
            FDIR_OBJECT_ID,
            PhaseState::Phi0,
            PhaseState::Phi0,
            "craft_init",
            Some(compose(PhaseState::Phi0, PhaseState::Phi0)),
            b"fdir_init",
            PhaseState::Phi0,
            PhaseState::Phi0,
            0,
            None,
        )?;
        self.fdir_last_hash = Some(fe.event_hash);
        self.note("craft init: nav=stale prop=idle power=off fdir=normal");
        Ok(())
    }

    pub fn note(&mut self, s: impl Into<String>) {
        self.transcript.push(s.into());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_raw(
        &mut self,
        object_id: u32,
        previous: PhaseState,
        requested: PhaseState,
        op: &str,
        compose_result: Option<PhaseState>,
        payload: &[u8],
        left: PhaseState,
        right: PhaseState,
        episode_id: u64,
        packet_id: Option<u64>,
    ) -> Result<StateEvent, CraftError> {
        let event = self.ledger.record_transition(
            ObjectKind::Craft,
            object_id,
            SubsystemKind::Craft,
            previous,
            requested,
            op,
            compose_result,
            payload,
        )?;
        let assoc_a = previous;
        let assoc_b = right;
        let assoc_c = requested;
        let craft_parity =
            FdirController::recompute_assoc_parity(assoc_a, assoc_b, assoc_c).unwrap_or(0);
        self.proof_ops.push(TransitionProofOp {
            object_id,
            sequence: event.proof.sequence_number,
            left,
            right,
            assoc_a,
            assoc_b,
            assoc_c,
            craft_parity,
            episode_id,
            packet_id,
        });
        Ok(event)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn audit_payload(
        &mut self,
        object_id: u32,
        previous: PhaseState,
        requested: PhaseState,
        op: &str,
        payload: &[u8],
        left: PhaseState,
        right: PhaseState,
        episode_id: u64,
        packet_id: Option<u64>,
    ) -> Result<StateEvent, CraftError> {
        let composed = if previous == requested {
            None
        } else {
            Some(compose(left, right))
        };
        self.record_raw(
            object_id, previous, requested, op, composed, payload, left, right, episode_id,
            packet_id,
        )
    }

    pub fn execute(
        &mut self,
        cmd: CraftCommand,
        episode_id: Option<u64>,
    ) -> Result<PhaseState, CraftError> {
        if matches!(
            cmd,
            CraftCommand::FdirDetect
                | CraftCommand::FdirIsolate
                | CraftCommand::FdirRecover
                | CraftCommand::FdirProve
        ) {
            return self.apply_fdir(cmd, episode_id.unwrap_or(0));
        }

        let kind = cmd.kind().ok_or(CraftError::UnknownCommand)?;
        let id = kind.object_id();
        let obj = self.obj(id)?.clone();

        if obj.quarantined && !cmd.allowed_while_quarantined() {
            let pkt = self
                .transport
                .send("kernel", kind.as_str(), cmd.as_str(), b"reject")
                .ok();
            self.audit_payload(
                id,
                obj.state,
                obj.state,
                "rejected_quarantined_operation",
                cmd.as_str().as_bytes(),
                obj.state,
                obj.state,
                episode_id.unwrap_or(0),
                pkt,
            )?;
            self.note(format!(
                "REJECT {} while quarantined (audited)",
                cmd.as_str()
            ));
            return Err(CraftError::Quarantined);
        }

        cmd.guard(obj.state)?;
        let operand = cmd.operand();
        let next = compose(obj.state, operand);
        if next != cmd.expected_next() {
            return Err(CraftError::IllegalTransition(
                "compose result != expected mission state",
            ));
        }

        let pkt = self
            .transport
            .send(
                "kernel",
                kind.as_str(),
                cmd.as_str(),
                cmd.as_str().as_bytes(),
            )
            .map_err(|e| CraftError::Chain(e))?;
        let _ = self.transport.acknowledge(pkt);

        let event = self.record_raw(
            id,
            obj.state,
            next,
            cmd.as_str(),
            Some(next),
            cmd.as_str().as_bytes(),
            obj.state,
            operand,
            episode_id.unwrap_or(0),
            Some(pkt),
        )?;

        let deadline = if cmd == CraftCommand::PropFire {
            Some(self.tick.saturating_add(PROP_FIRING_MAX_TICKS))
        } else {
            None
        };
        let o = self.obj_mut(id)?;
        o.state = next;
        o.ticks_in_state = 0;
        o.active_command = Some(cmd.as_str().into());
        o.last_transition_hash = Some(event.event_hash);
        if let Some(d) = deadline {
            o.deadline_tick = Some(d);
        }
        if cmd == CraftCommand::PowerLoad {
            o.measured_load = 10;
        }
        if cmd == CraftCommand::PowerProtect {
            o.measured_load = 0;
        }

        self.note(format!(
            "{}: {} --operand Φ{}--> {} (compose OK) pkt={}",
            kind.as_str(),
            kind.phase_label(obj.state),
            operand.as_u8(),
            kind.phase_label(next),
            pkt
        ));
        Ok(next)
    }

    pub fn apply_fdir(
        &mut self,
        cmd: CraftCommand,
        episode_id: u64,
    ) -> Result<PhaseState, CraftError> {
        if cmd == CraftCommand::FdirProve && self.fdir_state == PhaseState::Phi3 {
            // prove path already verified in FdirController::prove before calling
        }
        cmd.guard(self.fdir_state)?;
        let operand = cmd.operand();
        let next = compose(self.fdir_state, operand);
        if next != cmd.expected_next() {
            return Err(CraftError::IllegalTransition("fdir compose mismatch"));
        }
        let prev = self.fdir_state;
        let event = self.record_raw(
            FDIR_OBJECT_ID,
            prev,
            next,
            cmd.as_str(),
            Some(next),
            &episode_id.to_le_bytes(),
            prev,
            operand,
            episode_id,
            None,
        )?;
        self.fdir_state = next;
        self.fdir_last_hash = Some(event.event_hash);
        self.note(format!(
            "FDIR: {} --operand Φ{}--> {}",
            fdir_label(prev),
            operand.as_u8(),
            fdir_label(next)
        ));
        Ok(next)
    }

    pub fn command_str(&mut self, subsystem: &str, op: &str) -> Result<PhaseState, CraftError> {
        let cmd = CraftCommand::parse(subsystem, op)?;
        self.execute(cmd, None)
    }

    /// Explicit fixture inject — no silent phase mutation for stuck firing.
    pub fn fault_inject_prop_stuck(&mut self, reason: &str) -> Result<(), CraftError> {
        if reason.is_empty() {
            return Err(CraftError::InjectRequiresReason);
        }
        let prop = CraftKind::Propulsion.object_id();
        let obj = self.obj(prop)?.clone();
        if obj.state != PhaseState::Phi2 {
            return Err(CraftError::IllegalTransition(
                "prop-stuck inject requires firing",
            ));
        }
        let episode = self.next_episode;
        let payload = format!("fault_inject:{reason}:episode={episode}");
        self.audit_payload(
            prop,
            obj.state,
            obj.state,
            "fault_inject",
            payload.as_bytes(),
            obj.state,
            obj.state,
            episode,
            None,
        )?;
        self.stuck_injected = true;
        self.note(format!(
            "fault inject prop-stuck reason={reason} (state unchanged)"
        ));
        Ok(())
    }

    pub fn tick(&mut self, n: u64) -> Result<Vec<String>, CraftError> {
        let mut notes = Vec::new();
        for _ in 0..n {
            self.tick += 1;
            let defects = self
                .objects
                .iter()
                .filter(|o| o.state == PhaseState::Phi2)
                .count();
            let total = self.curvature.last_total + defects;
            self.curvature.record_tick(total);
            self.ledger
                .sync_reflection_defects(self.curvature.lifetime_sum);
            for o in self.objects.iter_mut() {
                o.ticks_in_state = o.ticks_in_state.saturating_add(1);
            }
            notes.push(format!(
                "tick={} prop_ticks={}",
                self.tick,
                self.obj(2)?.ticks_in_state
            ));
        }
        Ok(notes)
    }

    pub fn status_report(&self) -> String {
        let mut out = String::from("CRAFT STATUS\n");
        for o in &self.objects {
            out.push_str(&format!(
                "  {} id={} state={} ({}) ticks={} quarantined={} episode={:?}\n",
                o.kind.as_str(),
                o.id,
                o.state.as_u8(),
                o.kind.phase_label(o.state),
                o.ticks_in_state,
                o.quarantined,
                o.current_episode
            ));
        }
        out.push_str(&format!(
            "  FDIR state={} ({}) tick={} episodes={} stuck_injected={}\n",
            self.fdir_state.as_u8(),
            fdir_label(self.fdir_state),
            self.tick,
            self.episodes.len(),
            self.stuck_injected
        ));
        out
    }

    pub fn quarantine_status(&self) -> String {
        let mut out = String::from("QUARANTINE\n");
        for o in &self.objects {
            out.push_str(&format!(
                "  {} quarantined={} episode={:?}\n",
                o.kind.as_str(),
                o.quarantined,
                o.current_episode
            ));
        }
        out
    }
}
