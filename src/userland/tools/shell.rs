//! Shell tool group.

use crate::userland::commands::{clear, version};
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellAction;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::registry::{lookup_descriptor, registry_entries};
use crate::userland::tools::types::{full_command_name, AuthorityRequirement, CommandGroup};

pub fn help(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Public)?;
    if args.is_empty() {
        let mut out = String::from("KairOS engineering tools:\n\n");
        let mut last_group = None;
        for desc in registry_entries() {
            if last_group != Some(desc.group) {
                out.push_str(&format!("\n[{}]\n", desc.group.label()));
                last_group = Some(desc.group);
            }
            out.push_str(&format!(
                "  {:<24} {}\n",
                full_command_name(desc),
                desc.description
            ));
        }
        ctx.writeln(&out)?;
        return Ok(0);
    }
    if args.len() == 1 {
        if let Ok(group) = parse_group(&args[0]) {
            return help_group(ctx, group);
        }
        return help_command(ctx, &args[0]);
    }
    let key = format!("{} {}", args[0], args[1]);
    help_command(ctx, &key)
}

fn help_group(ctx: &mut CommandContext<'_>, group: CommandGroup) -> Result<i32, ToolError> {
    let mut out = format!("{} commands:\n", group.label());
    for desc in registry_entries() {
        if desc.group == group {
            out.push_str(&format!(
                "  {:<20} {}\n  usage: {}\n  authority: {}\n  target: {}\n\n",
                desc.name,
                desc.description,
                desc.usage,
                desc.authority.label(),
                desc.target_support.label()
            ));
        }
    }
    ctx.writeln(&out)?;
    Ok(0)
}

fn help_command(ctx: &mut CommandContext<'_>, key: &str) -> Result<i32, ToolError> {
    let Some(desc) = lookup_descriptor(key) else {
        return Err(ToolError::UnknownCommand);
    };
    let out = format!(
        "command: {}\nusage: {}\nauthority: {}\ntarget: {}\n{}\n",
        full_command_name(desc),
        desc.usage,
        desc.authority.label(),
        desc.target_support.label(),
        desc.description
    );
    ctx.writeln(&out)?;
    Ok(0)
}

fn parse_group(s: &str) -> Result<CommandGroup, ToolError> {
    match s {
        "shell" => Ok(CommandGroup::Shell),
        "file" => Ok(CommandGroup::File),
        "text" => Ok(CommandGroup::Text),
        "build" => Ok(CommandGroup::Build),
        "process" => Ok(CommandGroup::Process),
        "system" => Ok(CommandGroup::System),
        "debug" => Ok(CommandGroup::Debug),
        "device" => Ok(CommandGroup::Device),
        "network" => Ok(CommandGroup::Network),
        "phase" => Ok(CommandGroup::Phase),
        "security" => Ok(CommandGroup::Security),
        "benchmark" => Ok(CommandGroup::Benchmark),
        "script" => Ok(CommandGroup::Script),
        _ => Err(ToolError::UnknownCommand),
    }
}

pub fn clear(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Public)?;
    let mut shell_ctx = ShellContext::new(ctx.kernel, ctx.caller, ctx.io, ctx.loader, ctx.vms);
    clear::run(&mut shell_ctx).map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    Ok(0)
}

pub fn history(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Public)?;
    if args.first().map(|s| s.as_str()) == Some("clear") {
        ctx.session.clear_history();
        ctx.writeln_line("history cleared")?;
        return Ok(0);
    }
    let count = args
        .first()
        .and_then(|s| s.parse().ok())
        .unwrap_or(ctx.session.history.len());
    let start = ctx.session.history.len().saturating_sub(count);
    let lines: Vec<String> = ctx.session.history[start..].to_vec();
    for (i, line) in lines.iter().enumerate() {
        ctx.writeln_line(&format!("{:>4} {}", start + i + 1, line))?;
    }
    Ok(0)
}

pub fn echo(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Public)?;
    let text = expand_vars(args, ctx.session);
    ctx.writeln(&text)?;
    ctx.writeln("\n")?;
    Ok(0)
}

fn expand_vars(
    args: &[String],
    session: &crate::userland::tools::session_state::ShellSessionState,
) -> String {
    let mut out = String::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if arg.starts_with('$') {
            let name = &arg[1..];
            if let Some(val) = session.env.get(name) {
                out.push_str(val);
                continue;
            }
        }
        out.push_str(arg);
    }
    out
}

pub fn which(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Public)?;
    let name = args.first().ok_or(ToolError::InvalidArguments)?;
    if let Some(desc) = lookup_descriptor(name) {
        ctx.writeln_line(&format!(
            "{}: built-in {} ({})",
            name,
            full_command_name(desc),
            desc.target_support.label()
        ))?;
        return Ok(0);
    }
    ctx.writeln_line(&format!("{name}: unavailable"))?;
    Ok(1)
}

pub fn version_cmd(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Public)?;
    let mut shell_ctx = ShellContext::new(ctx.kernel, ctx.caller, ctx.io, ctx.loader, ctx.vms);
    version::run(&mut shell_ctx).map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    #[cfg(feature = "std")]
    ctx.writeln_line(&format!("profile: hosted std"))?;
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    ctx.writeln_line("profile: baremetal")?;
    Ok(0)
}

pub fn exit_cmd(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    Ok(0)
}

pub fn shell_context<'a, 'b>(ctx: &'b mut CommandContext<'a>) -> ShellContext<'b> {
    ShellContext::new(ctx.kernel, ctx.caller, ctx.io, ctx.loader, ctx.vms)
}

pub fn is_exit_command(args: &[String]) -> bool {
    matches!(
        args.first().map(|s| s.as_str()),
        Some("exit") | Some("quit")
    )
}
