//! Minimal KairOS script runner (.ks).

use crate::userland::parser::parse;
use crate::userland::shell::ShellAction;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::limits::{
    MAX_SCRIPT_BYTES, MAX_SCRIPT_DEPTH, MAX_SCRIPT_LINES, MAX_SCRIPT_REPEAT,
};
use crate::userland::tools::prelude::*;
use crate::userland::tools::registry::lookup_descriptor;
use crate::userland::tools::types::AuthorityRequirement;

pub fn run(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let path = crate::userland::tools::path_util::resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    if data.len() > MAX_SCRIPT_BYTES {
        return Err(ToolError::InputTooLarge);
    }
    let text = core::str::from_utf8(&data).map_err(|_| ToolError::ReadFailed)?;
    ctx.record_statechain("script_started", path.as_bytes());
    let status = execute_script(ctx, text)?;
    if status == 0 {
        ctx.record_statechain("script_completed", path.as_bytes());
    } else {
        ctx.record_statechain("script_failed", path.as_bytes());
    }
    Ok(status)
}

pub fn check(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let path = crate::userland::tools::path_util::resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let text = core::str::from_utf8(&data).map_err(|_| ToolError::ReadFailed)?;
    validate_script(text)?;
    ctx.writeln_line("script check ok")?;
    Ok(0)
}

fn validate_script(text: &str) -> Result<(), ToolError> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() > MAX_SCRIPT_LINES {
        return Err(ToolError::InputTooLarge);
    }
    let mut depth = 0i32;
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.first().copied() {
            Some("if") | Some("repeat") => {
                depth += 1;
                if depth > MAX_SCRIPT_DEPTH as i32 {
                    return Err(ToolError::InputTooLarge);
                }
            }
            Some("end") => depth -= 1,
            Some("set") | Some("print") | Some("run") | Some("assert") | Some("sleep")
            | Some("exit") => {}
            _ => return Err(ToolError::InvalidArguments),
        }
    }
    Ok(())
}

fn execute_script(ctx: &mut CommandContext<'_>, text: &str) -> Result<i32, ToolError> {
    validate_script(text)?;
    let lines: Vec<String> = text.lines().map(|l| l.into()).collect();
    let mut pc = 0usize;
    let mut status = 0i32;
    while pc < lines.len() {
        let line = lines[pc].trim();
        pc += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<String> = line.split_whitespace().map(|s| s.into()).collect();
        match parts[0].as_str() {
            "set" => {
                if parts.len() < 3 {
                    return Err(ToolError::InvalidArguments);
                }
                let name = parts[1].clone();
                let value = expand_script_value(&parts[2..], ctx);
                ctx.session.env.insert(name, value);
            }
            "print" => {
                let text = expand_script_value(&parts[1..], ctx);
                ctx.writeln_line(&text)?;
            }
            "run" => {
                let cmd = expand_script_value(&parts[1..], ctx);
                status = super::execute_line_inner(ctx, &cmd, false)?;
            }
            "sleep" => {
                let secs: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                crate::userland::tools::process::sleep(ctx, &vec![secs.to_string()])?;
            }
            "assert" => {
                if !eval_assert(&parts[1..]) {
                    return Err(ToolError::VerificationFailed);
                }
            }
            "repeat" => {
                let count: usize = parts
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
                    .min(MAX_SCRIPT_REPEAT);
                let mut end_pc = pc;
                let mut depth = 1i32;
                while end_pc < lines.len() {
                    let l = lines[end_pc].trim();
                    if l.starts_with("repeat") {
                        depth += 1;
                    }
                    if l == "end" {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    end_pc += 1;
                }
                let block: Vec<String> = lines[pc..end_pc].to_vec();
                for _ in 0..count {
                    let block_text = block.join("\n");
                    status = execute_script(ctx, &block_text)?;
                }
                pc = end_pc + 1;
            }
            "exit" => {
                status = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                return Ok(status);
            }
            "if" => {
                if !eval_assert(&parts[1..]) {
                    while pc < lines.len() && lines[pc].trim() != "end" {
                        pc += 1;
                    }
                }
            }
            "end" => {}
            _ => return Err(ToolError::InvalidArguments),
        }
    }
    Ok(status)
}

fn expand_script_value(parts: &[String], ctx: &CommandContext<'_>) -> String {
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if part.starts_with('$') {
            if let Some(val) = ctx.session.env.get(&part[1..]) {
                out.push_str(val);
                continue;
            }
        }
        out.push_str(part);
    }
    out
}

fn eval_assert(parts: &[String]) -> bool {
    if parts.len() < 3 {
        return false;
    }
    let left = parts[0].as_str();
    let op = parts[1].as_str();
    let right = parts[2].as_str();
    match op {
        "==" => left == right,
        "!=" => left != right,
        _ => false,
    }
}

pub fn script_line_command(parts: &[String]) -> bool {
    matches!(
        parts.first().map(|s| s.as_str()),
        Some("set") | Some("print") | Some("assert") | Some("repeat") | Some("end") | Some("if")
    )
}
