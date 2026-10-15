//! PhaseHook registry and bus. Single `require_delivery` path.

use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::types::{
    AppId, BoundedVec, HookMode, MAX_HOOK_FUEL, MAX_HOOK_PAYLOAD, MAX_HOOK_QUEUE,
    PRIVILEGED_HOOK_PRIORITY, PhaseStackError, StandardHook,
};
use crate::kernel::state_root::merkle_root;
use crate::platform::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMessage {
    pub hook: StandardHook,
    pub source: AppId,
    pub payload: BoundedVec<u8>,
    pub fuel: u32,
    pub deadline_tick: u64,
    pub mode: HookMode,
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookGrant {
    pub hook: StandardHook,
    pub max_priority: u8,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseHookRegistry {
    grants: Vec<(AppId, HookGrant)>,
}

impl PhaseHookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&mut self, app: AppId, grant: HookGrant) -> Result<(), PhaseStackError> {
        if grant.max_priority == PRIVILEGED_HOOK_PRIORITY {
            return Err(PhaseStackError::PrivilegedPriorityDenied);
        }
        self.grants.push((app, grant));
        Ok(())
    }

    pub fn grant_privileged(
        &mut self,
        app: AppId,
        grant: HookGrant,
        allow: bool,
    ) -> Result<(), PhaseStackError> {
        if grant.max_priority == PRIVILEGED_HOOK_PRIORITY && !allow {
            return Err(PhaseStackError::PrivilegedPriorityDenied);
        }
        self.grants.push((app, grant));
        Ok(())
    }

    pub fn allowed(&self, app: AppId, hook: StandardHook, priority: u8) -> bool {
        self.grants.iter().any(|(id, g)| {
            *id == app && g.hook == hook && priority >= g.max_priority
        })
    }

    pub fn merkle_root(&self) -> StateHash {
        let mut leaves: Vec<StateHash> = Vec::new();
        for (id, g) in &self.grants {
            let mut leaf = [0u8; 32];
            leaf.copy_from_slice(id);
            leaf[31] ^= g.hook.as_u32() as u8;
            leaves.push(leaf);
        }
        merkle_root(&leaves)
    }
}

#[derive(Debug)]
pub struct PhaseHookBus {
    pub registry: PhaseHookRegistry,
    queue: BoundedVec<HookMessage>,
}

impl Default for PhaseHookBus {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseHookBus {
    pub fn new() -> Self {
        Self {
            registry: PhaseHookRegistry::new(),
            queue: BoundedVec::new(MAX_HOOK_QUEUE),
        }
    }

    /// The only delivery entry point.
    pub fn require_delivery(
        &mut self,
        msg: HookMessage,
        now_tick: u64,
    ) -> Result<(), PhaseStackError> {
        if msg.payload.len() > MAX_HOOK_PAYLOAD {
            return Err(PhaseStackError::HookPayloadOversize);
        }
        if msg.fuel > MAX_HOOK_FUEL {
            return Err(PhaseStackError::HookFuelExceeded);
        }
        if now_tick > msg.deadline_tick {
            return Err(PhaseStackError::HookDeadline);
        }
        if !self.registry.allowed(msg.source, msg.hook, msg.priority) {
            return Err(PhaseStackError::AuthorityDenied);
        }
        if self.queue.len() >= MAX_HOOK_QUEUE {
            return Err(PhaseStackError::HookQueueFull);
        }
        let payload_len = msg.payload.len();
        if payload_len > MAX_HOOK_PAYLOAD {
            return Err(PhaseStackError::HookPayloadOversize);
        }
        self.queue.push(msg)
    }

    pub fn pop(&mut self) -> Option<HookMessage> {
        let slice = self.queue.as_slice();
        if slice.is_empty() {
            return None;
        }
        // BoundedVec has no pop; drain via reconstruct.
        let first = slice[0].clone();
        let rest: Vec<HookMessage> = slice[1..].to_vec();
        self.queue.clear();
        for m in rest {
            let _ = self.queue.push(m);
        }
        Some(first)
    }
}

pub fn payload_from_bytes(bytes: &[u8]) -> Result<BoundedVec<u8>, PhaseStackError> {
    if bytes.len() > MAX_HOOK_PAYLOAD {
        return Err(PhaseStackError::HookPayloadOversize);
    }
    let mut v = BoundedVec::new(MAX_HOOK_PAYLOAD);
    for b in bytes {
        v.push(*b)?;
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(src: AppId) -> HookMessage {
        HookMessage {
            hook: StandardHook::OnLoad,
            source: src,
            payload: BoundedVec::new(MAX_HOOK_PAYLOAD),
            fuel: 10,
            deadline_tick: 100,
            mode: HookMode::Notify,
            priority: 8,
        }
    }

    #[test]
    fn require_delivery_is_single_path() {
        let mut bus = PhaseHookBus::new();
        let src = [1u8; 32];
        bus.registry
            .grant(
                src,
                HookGrant {
                    hook: StandardHook::OnLoad,
                    max_priority: 8,
                },
            )
            .unwrap();
        bus.require_delivery(msg(src), 1).unwrap();
        assert_eq!(bus.pop().unwrap().source, src);
    }

    #[test]
    fn privileged_priority_not_auto_granted() {
        let mut reg = PhaseHookRegistry::new();
        assert_eq!(
            reg.grant(
                [1u8; 32],
                HookGrant {
                    hook: StandardHook::OnStart,
                    max_priority: PRIVILEGED_HOOK_PRIORITY,
                },
            ),
            Err(PhaseStackError::PrivilegedPriorityDenied)
        );
    }

    #[test]
    fn oversize_payload_rejected() {
        let mut bus = PhaseHookBus::new();
        let src = [2u8; 32];
        bus.registry
            .grant(
                src,
                HookGrant {
                    hook: StandardHook::OnLoad,
                    max_priority: 8,
                },
            )
            .unwrap();
        let mut m = msg(src);
        for _ in 0..MAX_HOOK_PAYLOAD {
            m.payload.push(1).unwrap();
        }
        assert_eq!(
            m.payload.push(1),
            Err(PhaseStackError::BoundExceeded)
        );
    }

    #[test]
    fn deadline_rejected() {
        let mut bus = PhaseHookBus::new();
        let src = [3u8; 32];
        bus.registry
            .grant(
                src,
                HookGrant {
                    hook: StandardHook::OnLoad,
                    max_priority: 8,
                },
            )
            .unwrap();
        assert_eq!(
            bus.require_delivery(msg(src), 101),
            Err(PhaseStackError::HookDeadline)
        );
    }
}
