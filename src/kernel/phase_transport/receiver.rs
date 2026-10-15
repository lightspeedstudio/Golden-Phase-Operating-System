//! Receiver: verify packet + path at destination.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_transport::ack::AckPlane;
use crate::kernel::phase_transport::error::TransportError;
use crate::kernel::phase_transport::packet::TransportPhase;
use crate::kernel::phase_transport::{find_carrier, vacuum_phi1, TransportController};

pub struct TransportReceiver;

impl TransportReceiver {
    pub fn receive(
        ctl: &mut TransportController,
        lane_idx: usize,
        packet_id: u64,
    ) -> Result<(), TransportError> {
        let m = ctl.m;
        let ack_plane = ctl.lanes[lane_idx].ack_plane;
        let (pr, pc) = find_carrier(&ctl.lanes[lane_idx].grid, m)?;
        let (lr, lc) = if ack_plane {
            AckPlane::logical_from_physical(pr, pc, m)
        } else {
            (pr, pc)
        };

        let pkt_idx = ctl
            .packets
            .iter()
            .position(|p| p.packet_id == packet_id)
            .ok_or(TransportError::NotFound)?;
        {
            let pkt = &ctl.packets[pkt_idx];
            if pkt.status.quarantined {
                return Err(TransportError::Quarantined);
            }
            if pkt.destination != (lr, lc) {
                return Err(TransportError::InvalidDestination);
            }
            if pkt.ttl == 0 {
                return Err(TransportError::ZeroTtl);
            }
            pkt.verify_checksum()?;
        }

        // Expected physical (or logical) position vs generation
        if let Some(tr) = ctl.traces.iter_mut().find(|t| t.packet_id == packet_id) {
            tr.verify()?;
            tr.actual_arrival = Some(tr.steps.len().saturating_sub(1) as u64);
        }

        ctl.record(
            packet_id as u32,
            TransportPhase::InFlight.as_phase(),
            TransportPhase::Received.as_phase(),
            "glider_receive",
            PhaseState::Phi2,
            PhaseState::Phi3,
            &packet_id.to_le_bytes(),
        )?;
        ctl.packets[pkt_idx].status.phase = TransportPhase::Received;

        // Clear carrier from lane
        ctl.lanes[lane_idx].packet_ids.retain(|&id| id != packet_id);
        if ctl.lanes[lane_idx].packet_ids.is_empty() {
            ctl.lanes[lane_idx].grid = vacuum_phi1(m);
        }
        Ok(())
    }
}
