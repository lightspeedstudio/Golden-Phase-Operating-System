//! Operator access sessions for per-object ANOM protection.

use crate::kernel::anom::object_policy::ProtectedObjectId;
use crate::kernel::hash::StateHash;
use crate::platform::{HashMap, String};

/// Kernel-trusted ODK storage — never exposed to userland or terminal.
#[derive(Debug, Default)]
pub struct KernelTrustedKeys {
    keys: HashMap<String, StateHash>,
}

impl KernelTrustedKeys {
    pub fn store(&mut self, object_id: &ProtectedObjectId, key: StateHash) {
        self.keys.insert(object_id.canonical(), key);
    }

    pub fn get(&self, object_id: &ProtectedObjectId) -> Option<StateHash> {
        self.keys.get(&object_id.canonical()).copied()
    }

    pub fn remove(&mut self, object_id: &ProtectedObjectId) {
        if let Some(mut key) = self.keys.remove(&object_id.canonical()) {
            key.fill(0);
        }
    }
}

#[derive(Debug)]
struct ObjectAccessSession {
    object_data_key: StateHash,
}

#[derive(Debug, Default)]
pub struct ObjectAccessController {
    sessions: HashMap<String, ObjectAccessSession>,
}

impl ObjectAccessController {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn is_open(&self, object_id: &ProtectedObjectId) -> bool {
        self.sessions.contains_key(&object_id.canonical())
    }

    pub fn open_session(&mut self, object_id: &ProtectedObjectId, object_data_key: StateHash) {
        self.sessions.insert(
            object_id.canonical(),
            ObjectAccessSession { object_data_key },
        );
    }

    pub fn object_data_key(&self, object_id: &ProtectedObjectId) -> Option<StateHash> {
        self.sessions
            .get(&object_id.canonical())
            .map(|s| s.object_data_key)
    }

    pub fn close_session(&mut self, object_id: &ProtectedObjectId) {
        if let Some(mut session) = self.sessions.remove(&object_id.canonical()) {
            session.object_data_key.fill(0);
        }
    }

    pub fn close_all(&mut self) {
        for (_, mut session) in self.sessions.drain() {
            session.object_data_key.fill(0);
        }
    }
}
