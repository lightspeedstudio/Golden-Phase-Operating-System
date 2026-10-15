//! Store-and-forward router (not a collision router).
//!
//! Sealed collision classes live in [`super::collision_policy`]. Production
//! forwarding ignores them while `COLLISION_ROUTING_ENABLED` is false.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_transport::collision_policy::COLLISION_ROUTING_ENABLED;
use crate::kernel::phase_transport::error::TransportError;
use crate::kernel::phase_transport::packet::{PacketOpcode, PhasePacket, TransportPhase};
use crate::kernel::phase_transport::trace::TransportTrace;
use crate::kernel::phase_transport::{place_glider, vacuum_phi1, TransportController};
use crate::platform::Vec;

#[derive(Debug, Clone)]
pub struct TransportRouter {
    pub id: u32,
    pub input_lane: u32,
    pub output_lane: u32,
    pub port: (usize, usize),
}

impl TransportRouter {
    pub fn forward(
        ctl: &mut TransportController,
        router_id: u32,
        packet_id: u64,
    ) -> Result<(), TransportError> {
        // Collision-class routing stays off until an explicit seal flips the gate.
        debug_assert!(
            !COLLISION_ROUTING_ENABLED,
            "collision routing must stay gated in production forward"
        );

        let router = ctl
            .routers
            .iter()
            .find(|r| r.id == router_id)
            .cloned()
            .ok_or(TransportError::NotFound)?;

        // Verify packet received / at router port on input lane
        let in_idx = ctl
            .lanes
            .iter()
            .position(|l| l.id == router.input_lane)
            .ok_or(TransportError::NotFound)?;
        let out_idx = ctl
            .lanes
            .iter()
            .position(|l| l.id == router.output_lane)
            .ok_or(TransportError::NotFound)?;

        let old_digest = ctl
            .traces
            .iter()
            .find(|t| t.packet_id == packet_id)
            .map(|t| t.digest)
            .ok_or(TransportError::NotFound)?;
        ctl.verify_trace(packet_id)?;

        let pkt = ctl
            .packets
            .iter()
            .find(|p| p.packet_id == packet_id)
            .cloned()
            .ok_or(TransportError::NotFound)?;

        // Remove from input
        ctl.lanes[in_idx].packet_ids.retain(|&id| id != packet_id);
        if ctl.lanes[in_idx].packet_ids.is_empty() {
            let m = ctl.m;
            ctl.lanes[in_idx].grid = vacuum_phi1(m);
        }

        // New packet on output lane (store-and-forward reinject)
        let new_id = ctl.next_packet_id;
        ctl.next_packet_id += 1;
        let mut np = PhasePacket::new(
            new_id,
            router.port,
            pkt.destination,
            PacketOpcode::Route,
            &pkt.payload,
            ctl.tick,
        );
        np.parent_packet_id = Some(packet_id);
        np.episode_id = pkt.episode_id;
        np.checksum = np.compute_checksum();

        if !ctl.lanes[out_idx].packet_ids.is_empty() {
            return Err(TransportError::BusyPort);
        }
        let m = ctl.m;
        place_glider(
            &mut ctl.lanes[out_idx].grid,
            m,
            router.port.0,
            router.port.1,
        );
        ctl.lanes[out_idx].packet_ids.push(new_id);
        ctl.lanes[out_idx].generation = 0;

        let expected = TransportController::expected_arrival(np.source, np.destination, m);
        let header = np.header_bytes();
        let mut tr = TransportTrace::start(new_id, &header, np.source, expected);
        tr.linked_from = Some(old_digest);
        tr.advance(0, np.source, 2, 1)?;

        ctl.record(
            new_id as u32,
            TransportPhase::Queued.as_phase(),
            TransportPhase::InFlight.as_phase(),
            "glider_route",
            PhaseState::Phi1,
            PhaseState::Phi2,
            &header,
        )?;
        // Link digests in payload record
        let mut link = Vec::new();
        link.extend_from_slice(&old_digest);
        link.extend_from_slice(&tr.digest);
        ctl.record(
            new_id as u32,
            TransportPhase::InFlight.as_phase(),
            TransportPhase::InFlight.as_phase(),
            "glider_route",
            PhaseState::Phi2,
            PhaseState::Phi2,
            &link,
        )?;

        np.status.phase = TransportPhase::InFlight;
        np.lane_id = router.output_lane;
        np.injection_hash = Some(tr.digest);
        ctl.traces.push(tr);
        ctl.packets.push(np);

        // Mark original received/completed
        if let Some(p) = ctl.packets.iter_mut().find(|p| p.packet_id == packet_id) {
            p.status.phase = TransportPhase::Empty;
        }
        Ok(())
    }
}
