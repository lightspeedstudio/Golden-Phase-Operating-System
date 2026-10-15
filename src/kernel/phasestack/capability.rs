//! Requested vs granted capability intersection (over captable rights).

use crate::kernel::phasestack::types::PhaseStackError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantSet {
    pub caps: u32,
    pub hooks: u32,
    pub max_priority: u8,
    pub allow_privileged_priority: bool,
}

impl GrantSet {
    pub fn empty() -> Self {
        Self {
            caps: 0,
            hooks: 0,
            max_priority: 8,
            allow_privileged_priority: false,
        }
    }

    pub fn subset_of(self, requested_caps: u32, requested_hooks: u32) -> Result<Self, PhaseStackError> {
        if self.caps & !requested_caps != 0 {
            return Err(PhaseStackError::GrantNotSubset);
        }
        if self.hooks & !requested_hooks != 0 {
            return Err(PhaseStackError::GrantNotSubset);
        }
        Ok(self)
    }
}

/// `PluginCaps = HostGranted ∩ PluginRequested ∩ PolicyAllowed`
pub fn plugin_caps(
    host_granted: u32,
    plugin_requested: u32,
    policy_allowed: u32,
) -> Result<u32, PhaseStackError> {
    let caps = host_granted & plugin_requested & policy_allowed;
    if plugin_requested & !host_granted & !policy_allowed != 0 && plugin_requested & !(host_granted & policy_allowed) == plugin_requested {
        // requested bits outside intersection are simply not granted
    }
    if plugin_requested & !(host_granted & policy_allowed) != 0
        && (plugin_requested & !(host_granted & policy_allowed)) == plugin_requested
        && plugin_requested != 0
        && caps == 0
        && plugin_requested != 0
    {
        // zero grant is allowed; escalation is requesting more than intersection
        // and treating it as granted — callers must use `caps`, never `plugin_requested`.
    }
    let _ = PhaseStackError::PluginCapEscalation;
    Ok(caps)
}

pub fn deny_escalation(requested: u32, granted: u32) -> Result<(), PhaseStackError> {
    if requested & !granted != 0 {
        Err(PhaseStackError::PluginCapEscalation)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::captable::{RIGHT_READ, RIGHT_WRITE};

    #[test]
    fn intersection_is_and() {
        let caps = plugin_caps(RIGHT_READ | RIGHT_WRITE, RIGHT_READ, RIGHT_READ | RIGHT_WRITE).unwrap();
        assert_eq!(caps, RIGHT_READ);
    }

    #[test]
    fn using_requested_instead_of_granted_is_escalation() {
        assert_eq!(
            deny_escalation(RIGHT_READ | RIGHT_WRITE, RIGHT_READ),
            Err(PhaseStackError::PluginCapEscalation)
        );
    }
}
