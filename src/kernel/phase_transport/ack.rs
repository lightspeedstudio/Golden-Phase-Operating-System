//! Acknowledgment plane: physical v=(-1,-1), logical v=(+1,+1) via coord flip.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_transport::error::TransportError;
use crate::kernel::phase_transport::packet::{PacketOpcode, PhasePacket, TransportPhase};
use crate::kernel::phase_transport::TransportController;
use crate::platform::Vec;

pub struct AckPlane;

impl AckPlane {
    pub fn logical_from_physical(r: usize, c: usize, m: usize) -> (usize, usize) {
        let lr = if r == 0 { 0 } else { m - r };
        let lc = if c == 0 { 0 } else { m - c };
        (lr, lc)
    }

    pub fn physical_from_logical(lr: usize, lc: usize, m: usize) -> (usize, usize) {
        Self::logical_from_physical(lr, lc, m)
    }

    /// Send ACK on lane 1 (ack plane) from original destination → source.
    pub fn send_ack(
        ctl: &mut TransportController,
        original_id: u64,
    ) -> Result<u64, TransportError> {
        let orig = ctl
            .packets
            .iter()
            .find(|p| p.packet_id == original_id)
            .ok_or(TransportError::NotFound)?
            .clone();
        if orig.status.phase != TransportPhase::Received {
            return Err(TransportError::InvalidOpcode);
        }
        let ack_id = ctl.next_packet_id;
        ctl.next_packet_id += 1;
        let mut payload = Vec::new();
        payload.extend_from_slice(&original_id.to_le_bytes());
        payload.extend_from_slice(&orig.episode_id.to_le_bytes());
        payload.extend_from_slice(&orig.sequence.to_le_bytes());
        if let Some(tr) = ctl.traces.iter().find(|t| t.packet_id == original_id) {
            payload.extend_from_slice(&tr.digest);
        }
        let mut ack = PhasePacket::new(
            ack_id,
            orig.destination, // ACK source = original destination (logical)
            orig.source,      // ACK dest = original source
            PacketOpcode::Ack,
            &payload,
            ctl.tick,
        );
        ack.parent_packet_id = Some(original_id);
        ack.episode_id = orig.episode_id;
        ack.checksum = ack.compute_checksum();

        ctl.record(
            ack_id as u32,
            TransportPhase::Empty.as_phase(),
            TransportPhase::Queued.as_phase(),
            "packet_create",
            PhaseState::Phi0,
            PhaseState::Phi1,
            &ack.header_bytes(),
        )?;
        ack.status.phase = TransportPhase::Queued;

        // Inject onto ACK plane (lane 1)
        let ack_lane = ctl
            .lanes
            .iter()
            .find(|l| l.ack_plane)
            .map(|l| l.id)
            .ok_or(TransportError::NotFound)?;
        crate::kernel::phase_transport::injector::Injector::inject(ctl, ack_lane, ack)?;
        ctl.record(
            ack_id as u32,
            TransportPhase::InFlight.as_phase(),
            TransportPhase::InFlight.as_phase(),
            "glider_ack",
            PhaseState::Phi2,
            PhaseState::Phi2,
            &original_id.to_le_bytes(),
        )?;
        Ok(ack_id)
    }
}
