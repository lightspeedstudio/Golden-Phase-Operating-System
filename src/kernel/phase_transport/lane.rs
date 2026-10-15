//! Transport lane: torus lattice + packet membership.

use crate::kernel::phase_transport::{find_carrier, vacuum_phi1};
use crate::platform::Vec;

#[derive(Debug, Clone)]
pub struct TransportPort {
    pub lane_id: u32,
    pub coord: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct TransportLane {
    pub id: u32,
    pub m: usize,
    pub grid: Vec<u8>,
    pub generation: u64,
    pub packet_ids: Vec<u64>,
    pub quarantined: bool,
    /// ACK plane: logical coords = −physical (mod M).
    pub ack_plane: bool,
}

impl TransportLane {
    pub fn new(id: u32, m: usize, ack_plane: bool) -> Self {
        Self {
            id,
            m,
            grid: vacuum_phi1(m),
            generation: 0,
            packet_ids: Vec::new(),
            quarantined: false,
            ack_plane,
        }
    }

    pub fn carrier_pos(&self) -> Option<(usize, usize)> {
        find_carrier(&self.grid, self.m).ok()
    }

    pub fn occupied_at(&self, r: usize, c: usize) -> bool {
        self.grid[r * self.m + c] == 2
    }
}
