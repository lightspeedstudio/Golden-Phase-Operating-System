//! Phase-transport craft adapter — bridges the craft fixture to the real
//! silicon-verified glider packet carrier (`phase_transport::TransportController`)
//! in place of the in-process `LocalCraftTransport` stand-in.
//!
//! Port table: all four craft endpoints sit on the main diagonal (r == c) of
//! the M=4 lattice. The Avenue 2 carrier's physical velocity is (-1,-1), so
//! only cells sharing (r - c) mod m stay mutually reachable over a single
//! lane send — a diagonal is the only placement where every endpoint can
//! reach every other endpoint directly.
//!   kernel / ground -> (1,1)
//!   navigation      -> (2,2)
//!   propulsion      -> (0,0)
//!   power           -> (3,3)

use crate::kernel::craft::transport::{CraftPacketRecord, CraftTransport};
use crate::kernel::phase_transport::packet::PacketOpcode;
use crate::kernel::phase_transport::TransportController;
use crate::platform::{String, ToString, Vec};

const M: usize = 4;

fn port(name: &str) -> Result<(usize, usize), String> {
    match name {
        "kernel" | "ground" => Ok((1, 1)),
        "navigation" => Ok((2, 2)),
        "propulsion" => Ok((0, 0)),
        "power" => Ok((3, 3)),
        other => Err(format!(
            "PhaseTransportCraftAdapter: unknown endpoint '{other}'"
        )),
    }
}

#[derive(Debug)]
pub struct PhaseTransportCraftAdapter {
    ctl: TransportController,
    log: Vec<CraftPacketRecord>,
}

impl PhaseTransportCraftAdapter {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            ctl: TransportController::init(M).map_err(|e| e.to_string())?,
            log: Vec::new(),
        })
    }
}

impl CraftTransport for PhaseTransportCraftAdapter {
    fn send(
        &mut self,
        source: &str,
        destination: &str,
        opcode: &str,
        payload: &[u8],
    ) -> Result<u64, String> {
        let src = port(source)?;
        let dst = port(destination)?;
        let op = PacketOpcode::parse(opcode).unwrap_or(PacketOpcode::Data);
        let id = self
            .ctl
            .send(src, dst, op, payload, 0)
            .map_err(|e| e.to_string())?;
        let eta = TransportController::expected_arrival(src, dst, M);
        self.ctl.tick(eta).map_err(|e| e.to_string())?;
        self.ctl.verify_trace(id).map_err(|e| e.to_string())?;
        self.log.push(CraftPacketRecord {
            packet_id: id,
            source: source.into(),
            destination: destination.into(),
            opcode: opcode.into(),
            ack: false,
        });
        Ok(id)
    }

    fn acknowledge(&mut self, packet_id: u64) -> Result<u64, String> {
        let (src_name, dst_name) = {
            let found = self
                .log
                .iter_mut()
                .find(|r| r.packet_id == packet_id)
                .ok_or_else(|| "packet not found".to_string())?;
            found.ack = true;
            (found.source.clone(), found.destination.clone())
        };
        let src = port(&src_name)?;
        let dst = port(&dst_name)?;
        let ack_id = self.ctl.acknowledge(packet_id).map_err(|e| e.to_string())?;
        // ACK plane: logical source=dst, logical dest=src, velocity (+1,+1).
        let ack_eta = TransportController::expected_arrival_dir(dst, src, M, 1);
        self.ctl.tick(ack_eta).map_err(|e| e.to_string())?;
        self.ctl.verify_trace(ack_id).map_err(|e| e.to_string())?;
        self.log.push(CraftPacketRecord {
            packet_id: ack_id,
            source: dst_name,
            destination: src_name,
            opcode: "ACK".into(),
            ack: true,
        });
        Ok(ack_id)
    }

    fn records(&self) -> &[CraftPacketRecord] {
        &self.log
    }
}
