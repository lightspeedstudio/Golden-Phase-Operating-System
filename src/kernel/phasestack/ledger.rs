//! App ledger events (install/launch/hook/fault/remove).

use crate::kernel::crypto::phase_hash::ph_content;
use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::types::{AppId, PackageRoot};
use crate::platform::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerKind {
    Install,
    Launch,
    Hook,
    Fault,
    Remove,
    Quarantine,
    Revoke,
}

impl LedgerKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Install => 0,
            Self::Launch => 1,
            Self::Hook => 2,
            Self::Fault => 3,
            Self::Remove => 4,
            Self::Quarantine => 5,
            Self::Revoke => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEvent {
    pub kind: LedgerKind,
    pub app_id: AppId,
    pub package_root: PackageRoot,
    pub tick: u64,
    pub payload_hash: StateHash,
}

#[derive(Debug, Clone, Default)]
pub struct AppLedger {
    events: Vec<LedgerEvent>,
    head: StateHash,
}

impl AppLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, event: LedgerEvent) -> StateHash {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.head);
        buf.push(event.kind.as_u8());
        buf.extend_from_slice(&event.app_id);
        buf.extend_from_slice(&event.package_root);
        buf.extend_from_slice(&event.tick.to_le_bytes());
        buf.extend_from_slice(&event.payload_hash);
        self.head = ph_content(&buf);
        self.events.push(event);
        self.head
    }

    pub fn head(&self) -> StateHash {
        self.head
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_moves_on_record() {
        let mut l = AppLedger::new();
        let a = l.head();
        l.record(LedgerEvent {
            kind: LedgerKind::Install,
            app_id: [1u8; 32],
            package_root: [2u8; 32],
            tick: 1,
            payload_hash: [3u8; 32],
        });
        assert_ne!(a, l.head());
    }
}
