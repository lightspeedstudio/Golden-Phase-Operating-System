//! Local craft transport adapter — does not own glider mathematics.

use crate::kernel::hash::{tagged_hash, StateHash};
use crate::platform::{String, ToString, Vec};

#[derive(Debug, Clone)]
pub struct CraftPacketRecord {
    pub packet_id: u64,
    pub source: String,
    pub destination: String,
    pub opcode: String,
    pub ack: bool,
}

/// Deterministic in-process transport for the craft fixture.
/// Pluggable: `LocalCraftTransport` (default) or `PhaseTransportCraftAdapter`
/// (real glider carrier). `Debug` supertrait so `CraftController` can derive it.
pub trait CraftTransport: core::fmt::Debug {
    fn send(
        &mut self,
        source: &str,
        destination: &str,
        opcode: &str,
        payload: &[u8],
    ) -> Result<u64, String>;
    fn acknowledge(&mut self, packet_id: u64) -> Result<u64, String>;
    fn records(&self) -> &[CraftPacketRecord];
}

#[derive(Debug, Default)]
pub struct LocalCraftTransport {
    next_id: u64,
    pub log: Vec<CraftPacketRecord>,
}

impl LocalCraftTransport {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            log: Vec::new(),
        }
    }

    pub fn receipt_digest(packet_id: u64, opcode: &str) -> StateHash {
        tagged_hash(
            "CRAFT_TRANSPORT_PKT_V1",
            &[&packet_id.to_le_bytes(), opcode.as_bytes()],
        )
    }
}

impl CraftTransport for LocalCraftTransport {
    fn send(
        &mut self,
        source: &str,
        destination: &str,
        opcode: &str,
        _payload: &[u8],
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
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
        let (src, dst) = {
            let found = self
                .log
                .iter_mut()
                .find(|r| r.packet_id == packet_id)
                .ok_or_else(|| "packet not found".to_string())?;
            found.ack = true;
            (found.source.clone(), found.destination.clone())
        };
        let ack_id = self.next_id;
        self.next_id += 1;
        self.log.push(CraftPacketRecord {
            packet_id: ack_id,
            source: dst,
            destination: src,
            opcode: "ACK".into(),
            ack: true,
        });
        Ok(ack_id)
    }

    fn records(&self) -> &[CraftPacketRecord] {
        &self.log
    }
}
