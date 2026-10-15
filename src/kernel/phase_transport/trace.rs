//! Rolling path digest for compact transport audit.

use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::phase_transport::error::TransportError;
use crate::platform::Vec;

#[derive(Debug, Clone)]
pub struct DigestStep {
    pub generation: u64,
    pub coord: (usize, usize),
    pub carrier: u8,
    pub footprint: usize,
}

#[derive(Debug, Clone)]
pub struct TransportTrace {
    pub packet_id: u64,
    pub header_hash: StateHash,
    pub inject_coord: (usize, usize),
    pub digest: StateHash,
    pub steps: Vec<DigestStep>,
    pub expected_arrival: u64,
    pub actual_arrival: Option<u64>,
    pub linked_from: Option<StateHash>,
}

impl TransportTrace {
    pub fn start(
        packet_id: u64,
        header: &[u8],
        inject_coord: (usize, usize),
        expected_arrival: u64,
    ) -> Self {
        let header_hash = tagged_hash("GPL_TRANSPORT_HDR_V1", &[header]);
        let mut coord = [0u8; 8];
        coord[..4].copy_from_slice(&(inject_coord.0 as u32).to_le_bytes());
        coord[4..].copy_from_slice(&(inject_coord.1 as u32).to_le_bytes());
        let digest = tagged_hash("GPL_TRANSPORT_PATH_V1", &[&header_hash, &coord]);
        Self {
            packet_id,
            header_hash,
            inject_coord,
            digest,
            steps: Vec::new(),
            expected_arrival,
            actual_arrival: None,
            linked_from: None,
        }
    }

    pub fn advance(
        &mut self,
        generation: u64,
        coord: (usize, usize),
        carrier: u8,
        footprint: usize,
    ) -> Result<(), TransportError> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&generation.to_le_bytes());
        buf.extend_from_slice(&(coord.0 as u32).to_le_bytes());
        buf.extend_from_slice(&(coord.1 as u32).to_le_bytes());
        buf.push(carrier);
        buf.push(footprint as u8);
        self.digest = tagged_hash("GPL_TRANSPORT_PATH_V1", &[&self.digest, &buf]);
        self.steps.push(DigestStep {
            generation,
            coord,
            carrier,
            footprint,
        });
        Ok(())
    }

    pub fn verify(&self) -> Result<(), TransportError> {
        let mut coord = [0u8; 8];
        coord[..4].copy_from_slice(&(self.inject_coord.0 as u32).to_le_bytes());
        coord[4..].copy_from_slice(&(self.inject_coord.1 as u32).to_le_bytes());
        let mut dig = tagged_hash("GPL_TRANSPORT_PATH_V1", &[&self.header_hash, &coord]);
        for st in &self.steps {
            let mut buf = Vec::new();
            buf.extend_from_slice(&st.generation.to_le_bytes());
            buf.extend_from_slice(&(st.coord.0 as u32).to_le_bytes());
            buf.extend_from_slice(&(st.coord.1 as u32).to_le_bytes());
            buf.push(st.carrier);
            buf.push(st.footprint as u8);
            dig = tagged_hash("GPL_TRANSPORT_PATH_V1", &[&dig, &buf]);
        }
        if dig != self.digest {
            return Err(TransportError::TraceMismatch);
        }
        Ok(())
    }

    pub fn replay(&self) -> Result<StateHash, TransportError> {
        self.verify()?;
        Ok(self.digest)
    }
}
