//! GPL Transfer Base — glider-carrier packet transport (Project A).
//!
//! Physical carrier: Avenue 2 verified rule_a = colpass(rowpass), Φ1 vacuum,
//! one Φ2 defect, velocity (−1,−1). Logical packet envelope lives in kernel
//! memory; lattice supplies verified motion only.
//!
//! Brief naming note: some prompts swap “row/column” labels; this module uses
//! the silicon-verified Avenue 2 definitions (horizontal rowpass, then vertical
//! colpass).

pub mod ack;
pub mod collision_policy;
pub mod error;
pub mod fpga_glider_transport;
pub mod injector;
pub mod lane;
pub mod packet;
pub mod receiver;
pub mod router;
pub mod trace;

use crate::core::gphase_core::{compose, PhaseState};
use crate::kernel::hash::StateHash;
use crate::kernel::phase_transport::ack::AckPlane;
use crate::kernel::phase_transport::error::TransportError;
use crate::kernel::phase_transport::injector::Injector;
use crate::kernel::phase_transport::lane::{TransportLane, TransportPort};
use crate::kernel::phase_transport::packet::{PacketOpcode, PhasePacket, TransportPhase};
use crate::kernel::phase_transport::receiver::TransportReceiver;
use crate::kernel::phase_transport::router::TransportRouter;
use crate::kernel::phase_transport::trace::TransportTrace;
use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;
use crate::kernel::statechain::StatechainLedger;
use crate::platform::{String, Vec};

/// Avenue 2 compose table (must match gphase_core).
const COMPOSE: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];

#[inline]
fn o(a: u8, b: u8) -> u8 {
    COMPOSE[a as usize][b as usize]
}

/// Horizontal rowpass: (W ∘ x) ∘ E — Avenue 2 verified.
pub fn rowpass(grid: &[u8], m: usize) -> Vec<u8> {
    let mut out = vec![0u8; m * m];
    for i in 0..m {
        for j in 0..m {
            let w = grid[i * m + ((j + m - 1) % m)];
            let x = grid[i * m + j];
            let e = grid[i * m + ((j + 1) % m)];
            out[i * m + j] = o(o(w, x), e);
        }
    }
    out
}

/// Vertical colpass: (N ∘ x) ∘ S — Avenue 2 verified.
pub fn colpass(grid: &[u8], m: usize) -> Vec<u8> {
    let mut out = vec![0u8; m * m];
    for i in 0..m {
        for j in 0..m {
            let n = grid[((i + m - 1) % m) * m + j];
            let x = grid[i * m + j];
            let s = grid[((i + 1) % m) * m + j];
            out[i * m + j] = o(o(n, x), s);
        }
    }
    out
}

pub fn rule_a(grid: &[u8], m: usize) -> Vec<u8> {
    colpass(&rowpass(grid, m), m)
}

pub fn vacuum_phi1(m: usize) -> Vec<u8> {
    vec![1u8; m * m]
}

pub fn place_glider(grid: &mut [u8], m: usize, r: usize, c: usize) {
    for x in grid.iter_mut() {
        *x = 1;
    }
    grid[r * m + c] = 2;
}

pub fn find_carrier(grid: &[u8], m: usize) -> Result<(usize, usize), TransportError> {
    let mut found = None;
    for i in 0..m {
        for j in 0..m {
            if grid[i * m + j] != 1 {
                if grid[i * m + j] != 2 {
                    return Err(TransportError::CarrierCorrupt);
                }
                if found.is_some() {
                    return Err(TransportError::CarrierDuplicated);
                }
                found = Some((i, j));
            }
        }
    }
    found.ok_or(TransportError::CarrierLost)
}

pub fn footprint(grid: &[u8]) -> usize {
    grid.iter().filter(|&&c| c != 1).count()
}

/// Golden M=4 sequence from silicon evidence (8 gens).
pub fn golden_m4_states() -> [&'static str; 8] {
    [
        "1111111111211111",
        "1111121111111111",
        "2111111111111111",
        "1111111111111112",
        "1111111111211111",
        "1111121111111111",
        "2111111111111111",
        "1111111111111112",
    ]
}

pub fn flat(grid: &[u8]) -> String {
    grid.iter().map(|c| char::from(b'0' + c)).collect()
}

#[derive(Debug)]
pub struct TransportController {
    pub m: usize,
    pub generation: u64,
    pub tick: u64,
    pub ledger: StatechainLedger,
    pub packets: Vec<PhasePacket>,
    pub lanes: Vec<TransportLane>,
    pub routers: Vec<TransportRouter>,
    pub traces: Vec<TransportTrace>,
    pub next_packet_id: u64,
}

impl TransportController {
    pub fn init(m: usize) -> Result<Self, TransportError> {
        if m < 4 || !m.is_power_of_two() && m != 24 {
            // allow 4,8,16,24,32,64 — 24 used in collision research
            if ![4, 8, 16, 24, 32, 64].contains(&m) {
                return Err(TransportError::InvalidDestination);
            }
        }
        let mut ctl = Self {
            m,
            generation: 0,
            tick: 0,
            ledger: StatechainLedger::new(),
            packets: Vec::new(),
            lanes: Vec::new(),
            routers: Vec::new(),
            traces: Vec::new(),
            next_packet_id: 1,
        };
        // Default single outbound lane + ack lane (logical reverse plane).
        ctl.lanes.push(TransportLane::new(0, ctl.m, false));
        ctl.lanes.push(TransportLane::new(1, ctl.m, true)); // ACK plane
        Ok(ctl)
    }

    pub fn status(&self) -> String {
        let mut s = format!(
            "TRANSPORT m={} gen={} tick={} packets={}\n",
            self.m,
            self.generation,
            self.tick,
            self.packets.len()
        );
        for lane in &self.lanes {
            s.push_str(&format!(
                "  lane={} ack_plane={} quarantined={} carrier={:?} packets={:?}\n",
                lane.id,
                lane.ack_plane,
                lane.quarantined,
                lane.carrier_pos(),
                lane.packet_ids
            ));
        }
        s
    }

    pub(crate) fn record(
        &mut self,
        object_id: u32,
        prev: PhaseState,
        next: PhaseState,
        op: &str,
        left: PhaseState,
        right: PhaseState,
        payload: &[u8],
    ) -> Result<(), TransportError> {
        let composed = compose(left, right);
        self.ledger
            .record_transition(
                ObjectKind::Driver,
                object_id,
                SubsystemKind::Driver,
                prev,
                next,
                op,
                Some(composed),
                payload,
            )
            .map_err(|e| TransportError::Chain(format!("{e:?}")))?;
        Ok(())
    }

    pub fn send(
        &mut self,
        src: (usize, usize),
        dst: (usize, usize),
        opcode: PacketOpcode,
        payload: &[u8],
        lane_id: u32,
    ) -> Result<u64, TransportError> {
        let id = self.next_packet_id;
        self.next_packet_id += 1;
        let mut pkt = PhasePacket::new(id, src, dst, opcode, payload, self.tick);
        self.record(
            id as u32,
            TransportPhase::Empty.as_phase(),
            TransportPhase::Queued.as_phase(),
            "packet_create",
            PhaseState::Phi0,
            PhaseState::Phi1,
            &pkt.header_bytes(),
        )?;
        pkt.status.phase = TransportPhase::Queued;
        Injector::inject(self, lane_id, pkt)?;
        Ok(id)
    }

    pub fn tick(&mut self, n: u64) -> Result<Vec<String>, TransportError> {
        let mut notes = Vec::new();
        for _ in 0..n {
            self.tick += 1;
            self.generation = self.generation.wrapping_add(1);
            for lane_idx in 0..self.lanes.len() {
                if self.lanes[lane_idx].quarantined {
                    continue;
                }
                if self.lanes[lane_idx].packet_ids.is_empty() {
                    continue;
                }
                let note = self.step_lane(lane_idx)?;
                if let Some(n) = note {
                    notes.push(n);
                }
            }
            // Timeouts
            let timed_out: Vec<u64> = self
                .packets
                .iter()
                .filter(|p| {
                    p.status.phase == TransportPhase::InFlight
                        && (p.ttl == 0
                            || self.tick.saturating_sub(p.created_tick) > (self.m as u64 * 4))
                })
                .map(|p| p.packet_id)
                .collect();
            for pid in timed_out {
                self.timeout(pid)?;
                notes.push(format!("timeout packet={pid}"));
            }
        }
        Ok(notes)
    }

    fn step_lane(&mut self, lane_idx: usize) -> Result<Option<String>, TransportError> {
        let m = self.m;
        let lane = &mut self.lanes[lane_idx];
        let before = lane.grid.clone();
        let (pr, pc) = find_carrier(&before, m)?;
        if footprint(&before) != 1 {
            return Err(TransportError::CarrierSpreading);
        }
        let next = rule_a(&before, m);
        let (nr, nc) = find_carrier(&next, m)?;
        if footprint(&next) != 1 {
            return Err(TransportError::CarrierSpreading);
        }
        // Physical velocity (−1,−1)
        let er = (pr + m - 1) % m;
        let ec = (pc + m - 1) % m;
        if (nr, nc) != (er, ec) {
            return Err(TransportError::PositionMismatch {
                expected: (er, ec),
                got: (nr, nc),
            });
        }
        lane.grid = next;
        lane.generation += 1;

        let packet_ids = lane.packet_ids.clone();
        let ack_plane = lane.ack_plane;
        let gen = lane.generation;
        let mut delivered = None;
        for pid in packet_ids {
            let pkt_idx = self
                .packets
                .iter()
                .position(|p| p.packet_id == pid)
                .ok_or(TransportError::NotFound)?;
            if self.packets[pkt_idx].ttl > 0 {
                self.packets[pkt_idx].ttl -= 1;
            }
            // Logical position for digest / arrival
            let (lr, lc) = if ack_plane {
                AckPlane::logical_from_physical(nr, nc, m)
            } else {
                (nr, nc)
            };
            // Update trace
            if let Some(tr) = self.traces.iter_mut().find(|t| t.packet_id == pid) {
                tr.advance(gen, (lr, lc), 2, 1)?;
            }
            let dst = self.packets[pkt_idx].destination;
            if (lr, lc) == dst {
                delivered = Some(pid);
            }
        }
        if let Some(pid) = delivered {
            TransportReceiver::receive(self, lane_idx, pid)?;
            return Ok(Some(format!(
                "received packet={pid} at {:?}",
                self.packets
                    .iter()
                    .find(|p| p.packet_id == pid)
                    .map(|p| p.destination)
            )));
        }
        Ok(None)
    }

    pub fn receive(&mut self, port: TransportPort) -> Result<Option<u64>, TransportError> {
        // Poll: if carrier already at destination for a packet on that lane
        let lane_idx = self
            .lanes
            .iter()
            .position(|l| l.id == port.lane_id)
            .ok_or(TransportError::NotFound)?;
        if self.lanes[lane_idx].packet_ids.is_empty() {
            return Ok(None);
        }
        let m = self.m;
        let (r, c) = match find_carrier(&self.lanes[lane_idx].grid, m) {
            Ok(pos) => pos,
            Err(TransportError::CarrierLost) => return Ok(None),
            Err(e) => return Err(e),
        };
        let ack = self.lanes[lane_idx].ack_plane;
        let (lr, lc) = if ack {
            AckPlane::logical_from_physical(r, c, m)
        } else {
            (r, c)
        };
        let hit = self.lanes[lane_idx]
            .packet_ids
            .iter()
            .copied()
            .find(|&pid| {
                self.packets
                    .iter()
                    .any(|p| p.packet_id == pid && p.destination == (lr, lc))
            });
        if let Some(pid) = hit {
            TransportReceiver::receive(self, lane_idx, pid)?;
            Ok(Some(pid))
        } else {
            Ok(None)
        }
    }

    pub fn acknowledge(&mut self, original_id: u64) -> Result<u64, TransportError> {
        AckPlane::send_ack(self, original_id)
    }

    pub fn route(&mut self, router_id: u32, packet_id: u64) -> Result<(), TransportError> {
        TransportRouter::forward(self, router_id, packet_id)
    }

    pub fn timeout(&mut self, packet_id: u64) -> Result<(), TransportError> {
        let pkt = self
            .packets
            .iter_mut()
            .find(|p| p.packet_id == packet_id)
            .ok_or(TransportError::NotFound)?;
        pkt.status.error = Some(TransportError::Timeout);
        let prev = pkt.status.phase.as_phase();
        let id = packet_id as u32;
        self.record(id, prev, prev, "glider_timeout", prev, prev, b"timeout")?;
        // Remove carrier from any lane
        for lane in &mut self.lanes {
            lane.packet_ids.retain(|&x| x != packet_id);
            if lane.packet_ids.is_empty() {
                lane.grid = vacuum_phi1(lane.m);
            }
        }
        Ok(())
    }

    pub fn quarantine(&mut self, packet_id: u64) -> Result<(), TransportError> {
        let pkt = self
            .packets
            .iter_mut()
            .find(|p| p.packet_id == packet_id)
            .ok_or(TransportError::NotFound)?;
        pkt.status.quarantined = true;
        let id = packet_id as u32;
        let prev = pkt.status.phase.as_phase();
        self.record(
            id,
            prev,
            prev,
            "glider_quarantine",
            prev,
            prev,
            b"quarantine",
        )?;
        Ok(())
    }

    pub fn release(&mut self, packet_id: u64) -> Result<(), TransportError> {
        let pkt = self
            .packets
            .iter_mut()
            .find(|p| p.packet_id == packet_id)
            .ok_or(TransportError::NotFound)?;
        pkt.status.quarantined = false;
        Ok(())
    }

    pub fn verify_trace(&self, packet_id: u64) -> Result<(), TransportError> {
        let tr = self
            .traces
            .iter()
            .find(|t| t.packet_id == packet_id)
            .ok_or(TransportError::NotFound)?;
        tr.verify()
    }

    pub fn replay_trace(&self, packet_id: u64) -> Result<StateHash, TransportError> {
        let tr = self
            .traces
            .iter()
            .find(|t| t.packet_id == packet_id)
            .ok_or(TransportError::NotFound)?;
        tr.replay()
    }

    pub fn inject_fault(&mut self, packet_id: u64, fault: &str) -> Result<(), TransportError> {
        match fault {
            "delete_carrier" => {
                for lane in &mut self.lanes {
                    if lane.packet_ids.contains(&packet_id) {
                        lane.grid = vacuum_phi1(lane.m);
                    }
                }
            }
            "duplicate_carrier" => {
                for lane in &mut self.lanes {
                    if lane.packet_ids.contains(&packet_id) {
                        if let Ok((r, c)) = find_carrier(&lane.grid, lane.m) {
                            let r2 = (r + 1) % lane.m;
                            lane.grid[r2 * lane.m + c] = 2;
                        }
                    }
                }
            }
            "spread" => {
                for lane in &mut self.lanes {
                    if lane.packet_ids.contains(&packet_id) {
                        for cell in &mut lane.grid {
                            if *cell == 1 {
                                *cell = 2;
                                break;
                            }
                        }
                    }
                }
            }
            "corrupt_header" => {
                let pkt = self
                    .packets
                    .iter_mut()
                    .find(|p| p.packet_id == packet_id)
                    .ok_or(TransportError::NotFound)?;
                pkt.payload.push(0xFF);
            }
            "tamper_digest" => {
                let tr = self
                    .traces
                    .iter_mut()
                    .find(|t| t.packet_id == packet_id)
                    .ok_or(TransportError::NotFound)?;
                tr.digest[0] ^= 0xA5;
            }
            _ => return Err(TransportError::InvalidOpcode),
        }
        let id = packet_id as u32;
        self.record(
            id,
            PhaseState::Phi2,
            PhaseState::Phi2,
            "glider_corrupt",
            PhaseState::Phi2,
            PhaseState::Phi2,
            fault.as_bytes(),
        )?;
        Ok(())
    }

    pub fn reset(&mut self) {
        let m = self.m;
        *self = Self::init(m).expect("init");
    }

    /// Expected arrival gens for outbound/ack logical motion.
    /// Outbound: p(t)=p0+t*(-1,-1). ACK logical: p(t)=p0+t*(+1,+1).
    pub fn expected_arrival(src: (usize, usize), dst: (usize, usize), m: usize) -> u64 {
        Self::expected_arrival_dir(src, dst, m, -1)
    }

    pub fn expected_arrival_dir(
        src: (usize, usize),
        dst: (usize, usize),
        m: usize,
        dir: isize,
    ) -> u64 {
        let m_i = m as isize;
        for t in 0..=(m as u64 * m as u64) {
            let r = (src.0 as isize + dir * t as isize).rem_euclid(m_i) as usize;
            let c = (src.1 as isize + dir * t as isize).rem_euclid(m_i) as usize;
            if (r, c) == dst {
                return t;
            }
        }
        u64::MAX
    }
}

#[cfg(test)]
mod golden_tests {
    use super::*;

    #[test]
    fn m4_golden_eight_gens() {
        let m = 4;
        let mut g = vacuum_phi1(m);
        place_glider(&mut g, m, 2, 2);
        for (t, exp) in golden_m4_states().iter().enumerate() {
            assert_eq!(flat(&g), *exp, "gen {t}");
            assert_eq!(footprint(&g), 1);
            let (r, c) = find_carrier(&g, m).unwrap();
            let er = ((2 as isize - t as isize).rem_euclid(4)) as usize;
            let ec = ((2 as isize - t as isize).rem_euclid(4)) as usize;
            assert_eq!((r, c), (er, ec));
            g = rule_a(&g, m);
        }
    }

    #[test]
    fn compose_table_matches_core() {
        for a in 0..4u8 {
            for b in 0..4u8 {
                let pa = PhaseState::from_u8(a).unwrap();
                let pb = PhaseState::from_u8(b).unwrap();
                assert_eq!(o(a, b), compose(pa, pb).as_u8());
            }
        }
    }
}
