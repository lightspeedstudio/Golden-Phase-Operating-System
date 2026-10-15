//! AI tool permission registry.

use crate::kernel::ai::error::AiError;
use crate::kernel::ai::types::{AiBridgeController, ToolPermission};
use crate::platform::{String, Vec};

pub fn tool_permission(controller: &AiBridgeController, name: &str) -> Option<ToolPermission> {
    controller
        .tool_permissions
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, p)| *p)
}

pub fn set_tool_permission(
    controller: &mut AiBridgeController,
    name: &str,
    permission: ToolPermission,
) -> Result<(), AiError> {
    if name == "ai.permissions" || name == "log.delete" {
        return Err(AiError::ToolDenied);
    }
    if let Some(entry) = controller
        .tool_permissions
        .iter_mut()
        .find(|(n, _)| n == name)
    {
        entry.1 = permission;
        return Ok(());
    }
    controller.tool_permissions.push((name.into(), permission));
    Ok(())
}

pub fn format_tools_table(controller: &AiBridgeController) -> String {
    let mut lines = Vec::new();
    lines.push(String::from("TOOL              PERMISSION"));
    for (name, perm) in &controller.tool_permissions {
        let label = match perm {
            ToolPermission::Allowed => "allowed",
            ToolPermission::Denied => "denied",
        };
        lines.push(format!("{name:<18}{label}"));
    }
    lines.join("\n")
}

pub fn allow_tool(controller: &mut AiBridgeController, name: &str) -> Result<String, AiError> {
    set_tool_permission(controller, name, ToolPermission::Allowed)?;
    Ok(format!("ai tool allowed: {name}"))
}

pub fn deny_tool(controller: &mut AiBridgeController, name: &str) -> Result<String, AiError> {
    set_tool_permission(controller, name, ToolPermission::Denied)?;
    Ok(format!("ai tool denied: {name}"))
}
