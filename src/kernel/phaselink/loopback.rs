//! In-memory loopback transport.

use crate::kernel::phaselink::types::PHASELINK_MAX_QUEUE;
use crate::platform::Vec;

#[derive(Debug, Default, Clone)]
pub struct LoopbackPhaseLink {
    pub outbound: Vec<Vec<u8>>,
    pub inbound: Vec<Vec<u8>>,
}

impl LoopbackPhaseLink {
    pub fn send(&mut self, bytes: Vec<u8>) {
        if self.outbound.len() >= PHASELINK_MAX_QUEUE {
            self.outbound.remove(0);
        }
        self.outbound.push(bytes.clone());
        if self.inbound.len() >= PHASELINK_MAX_QUEUE {
            self.inbound.remove(0);
        }
        self.inbound.push(bytes);
    }

    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if self.inbound.is_empty() {
            None
        } else {
            Some(self.inbound.remove(0))
        }
    }
}
