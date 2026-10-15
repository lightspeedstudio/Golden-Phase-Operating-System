//! Glider injection into a transport lane.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_transport::error::TransportError;
use crate::kernel::phase_transport::packet::{PhasePacket, TransportPhase};
use crate::kernel::phase_transport::trace::TransportTrace;
use crate::kernel::phase_transport::{place_glider, TransportController};

pub struct Injector;

impl Injector {
    pub fn inject(
        ctl: &mut TransportController,
        lane_id: u32,
        mut pkt: PhasePacket,
    ) -> Result<(), TransportError> {
        if pkt.ttl == 0 {
            return Err(TransportError::ZeroTtl);
        }
        pkt.verify_checksum()?;
        if ctl.packets.iter().any(|p| p.packet_id == pkt.packet_id) {
            return Err(TransportError::DuplicatePacketId);
        }
        let (sr, sc) = pkt.source;
        let (dr, dc) = pkt.destination;
        let m = ctl.m;
        if sr >= m || sc >= m || dr >= m || dc >= m {
            return Err(TransportError::InvalidDestination);
        }

        let lane_idx = ctl
            .lanes
            .iter()
            .position(|l| l.id == lane_id)
            .ok_or(TransportError::NotFound)?;
        let ack_plane = ctl.lanes[lane_idx].ack_plane;
        let (pr, pc) = physical_inject(sr, sc, m, ack_plane);
        {
            let lane = &mut ctl.lanes[lane_idx];
            if lane.quarantined {
                return Err(TransportError::LaneQuarantined);
            }
            if !lane.packet_ids.is_empty() || lane.occupied_at(pr, pc) {
                return Err(TransportError::BusyPort);
            }
            place_glider(&mut lane.grid, m, pr, pc);
            lane.packet_ids.push(pkt.packet_id);
            lane.generation = 0;
        }

        let expected = if ack_plane {
            TransportController::expected_arrival_dir(pkt.source, pkt.destination, m, 1)
        } else {
            TransportController::expected_arrival(pkt.source, pkt.destination, m)
        };
        let header = pkt.header_bytes();
        let mut trace = TransportTrace::start(pkt.packet_id, &header, pkt.source, expected);
        trace.advance(0, pkt.source, 2, 1)?;

        ctl.record(
            pkt.packet_id as u32,
            TransportPhase::Queued.as_phase(),
            TransportPhase::InFlight.as_phase(),
            "glider_inject",
            PhaseState::Phi1,
            PhaseState::Phi2,
            &header,
        )?;
        pkt.status.phase = TransportPhase::InFlight;
        pkt.lane_id = lane_id;
        pkt.injection_hash = Some(trace.digest);
        ctl.traces.push(trace);
        ctl.packets.push(pkt);
        Ok(())
    }
}

fn physical_inject(sr: usize, sc: usize, m: usize, ack_plane: bool) -> (usize, usize) {
    if !ack_plane {
        return (sr, sc);
    }
    let pr = if sr == 0 { 0 } else { m - sr };
    let pc = if sc == 0 { 0 } else { m - sc };
    (pr, pc)
}
