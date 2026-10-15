//! Text tool group.

use crate::kernel::state_proof::hash_payload;
use crate::userland::fs_cmd;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::limits::{
    MAX_EDIT_FILE_BYTES, MAX_EDIT_LINES, MAX_EDIT_LINE_LEN, MAX_GREP_DEPTH, MAX_GREP_FILES,
};
use crate::userland::tools::path_util::resolve_path;
use crate::userland::tools::prelude::*;
use crate::userland::tools::session_state::EditSession;
use crate::userland::tools::types::AuthorityRequirement;

pub fn edit(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    if args.is_empty() {
        return Err(ToolError::InvalidArguments);
    }
    let sub = args[0].as_str();
    if sub == "save"
        || sub == "quit"
        || sub == "show"
        || sub.starts_with("insert")
        || sub.starts_with("append")
        || sub.starts_with("replace")
        || sub.starts_with("delete")
    {
        return edit_command(ctx, args);
    }
    let path = resolve_path(&ctx.session.cwd, sub)?;
    let data = ctx.kernel.fs.read_file(&path).unwrap_or_default();
    if data.len() > MAX_EDIT_FILE_BYTES {
        return Err(ToolError::InputTooLarge);
    }
    let text = core::str::from_utf8(&data).map_err(|_| ToolError::ReadFailed)?;
    let lines: Vec<String> = text.lines().map(|l| l.into()).collect();
    if lines.len() > MAX_EDIT_LINES {
        return Err(ToolError::InputTooLarge);
    }
    ctx.session.edit_buffer = Some(EditSession {
        path,
        lines,
        dirty: false,
    });
    ctx.writeln_line("edit session opened; use show/save/quit")?;
    Ok(0)
}

fn edit_command(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    let cmd = args[0].as_str();
    match cmd {
        "show" => {
            let lines: Vec<String> = ctx
                .session
                .edit_buffer
                .as_ref()
                .ok_or(ToolError::InvalidState)?
                .lines
                .clone();
            for (i, line) in lines.iter().enumerate() {
                ctx.writeln_line(&format!("{:>4} {}", i + 1, line))?;
            }
        }
        "save" => {
            let buf = ctx
                .session
                .edit_buffer
                .take()
                .ok_or(ToolError::InvalidState)?;
            let text = buf.lines.join("\n");
            let backup = format!("{}.bak", buf.path);
            if let Ok(old) = ctx.kernel.fs.read_file(&buf.path) {
                fs_cmd::write_path(
                    ctx.kernel,
                    ctx.caller,
                    ctx.io,
                    &backup,
                    core::str::from_utf8(&old).unwrap_or(""),
                )
                .ok();
            }
            fs_cmd::write_path(ctx.kernel, ctx.caller, ctx.io, &buf.path, &text)
                .map_err(|_| ToolError::WriteFailed)?;
            ctx.record_statechain("file_modified", buf.path.as_bytes());
            ctx.writeln_line("saved")?;
        }
        "quit" => {
            ctx.session.edit_buffer = None;
            ctx.writeln_line("edit closed")?;
        }
        "insert" => {
            let line_no: usize = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .ok_or(ToolError::InvalidArguments)?;
            let text = args.get(2..).map(|p| p.join(" ")).unwrap_or_default();
            if text.len() > MAX_EDIT_LINE_LEN {
                return Err(ToolError::InputTooLarge);
            }
            let buf = ctx
                .session
                .edit_buffer
                .as_mut()
                .ok_or(ToolError::InvalidState)?;
            let idx = line_no.saturating_sub(1).min(buf.lines.len());
            buf.lines.insert(idx, text);
            buf.dirty = true;
        }
        "append" => {
            let text = args.get(1..).map(|p| p.join(" ")).unwrap_or_default();
            let buf = ctx
                .session
                .edit_buffer
                .as_mut()
                .ok_or(ToolError::InvalidState)?;
            buf.lines.push(text);
            buf.dirty = true;
        }
        "replace" => {
            let line_no: usize = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .ok_or(ToolError::InvalidArguments)?;
            let text = args.get(2..).map(|p| p.join(" ")).unwrap_or_default();
            let buf = ctx
                .session
                .edit_buffer
                .as_mut()
                .ok_or(ToolError::InvalidState)?;
            let idx = line_no.saturating_sub(1);
            if idx < buf.lines.len() {
                buf.lines[idx] = text;
                buf.dirty = true;
            }
        }
        "delete" => {
            let line_no: usize = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .ok_or(ToolError::InvalidArguments)?;
            let buf = ctx
                .session
                .edit_buffer
                .as_mut()
                .ok_or(ToolError::InvalidState)?;
            let idx = line_no.saturating_sub(1);
            if idx < buf.lines.len() {
                buf.lines.remove(idx);
                buf.dirty = true;
            }
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn grep(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let pattern = &args[0];
    let target = resolve_path(&ctx.session.cwd, &args[1])?;
    let mut ignore_case = false;
    let mut line_number = false;
    let mut recursive = false;
    for arg in &args[2..] {
        match arg.as_str() {
            "--ignore-case" => ignore_case = true,
            "--line-number" => line_number = true,
            "--recursive" => recursive = true,
            _ => {}
        }
    }
    if recursive {
        grep_recursive(ctx, pattern, &target, 0, ignore_case, line_number, &mut 0)?;
    } else {
        grep_file(ctx, pattern, &target, ignore_case, line_number)?;
    }
    Ok(0)
}

fn grep_recursive(
    ctx: &mut CommandContext<'_>,
    pattern: &str,
    path: &str,
    depth: usize,
    ignore_case: bool,
    line_number: bool,
    files: &mut usize,
) -> Result<(), ToolError> {
    if *files >= MAX_GREP_FILES || depth > MAX_GREP_DEPTH {
        return Ok(());
    }
    if ctx.kernel.fs.read_file(path).is_ok() {
        grep_file(ctx, pattern, path, ignore_case, line_number)?;
        *files += 1;
        return Ok(());
    }
    if let Ok(entries) = ctx.kernel.fs.list_dir(path) {
        for entry in entries {
            let child = if path == "/" {
                format!("/{entry}")
            } else {
                format!("{path}/{entry}")
            };
            grep_recursive(
                ctx,
                pattern,
                &child,
                depth + 1,
                ignore_case,
                line_number,
                files,
            )?;
        }
    }
    Ok(())
}

fn grep_file(
    ctx: &mut CommandContext<'_>,
    pattern: &str,
    path: &str,
    ignore_case: bool,
    line_number: bool,
) -> Result<(), ToolError> {
    let data = ctx
        .kernel
        .fs
        .read_file(path)
        .map_err(|_| ToolError::ReadFailed)?;
    let text = core::str::from_utf8(&data).map_err(|_| ToolError::ReadFailed)?;
    let pat = if ignore_case {
        pattern.to_ascii_lowercase()
    } else {
        pattern.to_string()
    };
    for (i, line) in text.lines().enumerate() {
        let hay = if ignore_case {
            line.to_ascii_lowercase()
        } else {
            line.to_string()
        };
        if hay.contains(&pat) {
            if line_number {
                ctx.writeln_line(&format!("{path}:{}:{}", i + 1, line))?;
            } else {
                ctx.writeln_line(&format!("{path}:{line}"))?;
            }
        }
    }
    Ok(())
}

pub fn sort(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let mut reverse = args.iter().any(|a| a == "--reverse");
    let numeric = args.iter().any(|a| a == "--numeric");
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let text = core::str::from_utf8(&data).map_err(|_| ToolError::ReadFailed)?;
    let mut lines: Vec<String> = text.lines().map(|l| l.into()).collect();
    if numeric {
        lines.sort_by_key(|l| l.parse::<i64>().unwrap_or(0));
    } else {
        lines.sort();
    }
    if reverse {
        lines.reverse();
    }
    for line in lines {
        ctx.writeln_line(&line)?;
    }
    Ok(0)
}

pub fn uniq(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let count = args.iter().any(|a| a == "--count");
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let text = core::str::from_utf8(&data).map_err(|_| ToolError::ReadFailed)?;
    let mut last: Option<&str> = None;
    let mut run = 0usize;
    for line in text.lines() {
        if last == Some(line) {
            run += 1;
            continue;
        }
        if let Some(prev) = last {
            if count {
                ctx.writeln_line(&format!("{run:>4} {prev}"))?;
            } else {
                ctx.writeln_line(prev)?;
            }
        }
        last = Some(line);
        run = 1;
    }
    if let Some(prev) = last {
        if count {
            ctx.writeln_line(&format!("{run:>4} {prev}"))?;
        } else {
            ctx.writeln_line(prev)?;
        }
    }
    Ok(0)
}

pub fn wc(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let (path, mode) = parse_wc_args(ctx, args)?;
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let text = core::str::from_utf8(&data).unwrap_or("");
    let lines = text.lines().count();
    let words = text.split_whitespace().count();
    let bytes = data.len();
    let out = match mode {
        WcMode::Lines => format!("{lines}"),
        WcMode::Words => format!("{words}"),
        WcMode::Bytes => format!("{bytes}"),
        WcMode::All => format!("{lines} {words} {bytes} {path}"),
    };
    ctx.writeln_line(&out)?;
    Ok(0)
}

enum WcMode {
    All,
    Lines,
    Words,
    Bytes,
}

fn parse_wc_args(ctx: &CommandContext<'_>, args: &[String]) -> Result<(String, WcMode), ToolError> {
    if args.is_empty() {
        return Err(ToolError::InvalidArguments);
    }
    if args[0] == "--lines" {
        return Ok((resolve_path(&ctx.session.cwd, &args[1])?, WcMode::Lines));
    }
    if args[0] == "--words" {
        return Ok((resolve_path(&ctx.session.cwd, &args[1])?, WcMode::Words));
    }
    if args[0] == "--bytes" {
        return Ok((resolve_path(&ctx.session.cwd, &args[1])?, WcMode::Bytes));
    }
    Ok((resolve_path(&ctx.session.cwd, &args[0])?, WcMode::All))
}

pub fn diff(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let old = resolve_path(&ctx.session.cwd, &args[0])?;
    let new = resolve_path(&ctx.session.cwd, &args[1])?;
    let old_data = ctx
        .kernel
        .fs
        .read_file(&old)
        .map_err(|_| ToolError::ReadFailed)?;
    let new_data = ctx
        .kernel
        .fs
        .read_file(&new)
        .map_err(|_| ToolError::ReadFailed)?;
    let old_text = core::str::from_utf8(&old_data).map_err(|_| ToolError::ReadFailed)?;
    let new_text = core::str::from_utf8(&new_data).map_err(|_| ToolError::ReadFailed)?;
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let max = old_lines.len().max(new_lines.len());
    for i in 0..max {
        let o = old_lines.get(i);
        let n = new_lines.get(i);
        match (o, n) {
            (Some(a), Some(b)) if a == b => {}
            (Some(a), Some(b)) => ctx.writeln_line(&format!("~ {} -> {}", a, b))?,
            (None, Some(b)) => ctx.writeln_line(&format!("+ {b}"))?,
            (Some(a), None) => ctx.writeln_line(&format!("- {a}"))?,
            _ => {}
        }
    }
    Ok(0)
}

pub fn patch(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let patch_path = resolve_path(
        &ctx.session.cwd,
        args.get(1).ok_or(ToolError::InvalidArguments)?,
    )?;
    let original = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let expected = hash_payload(&original);
    let patch_data = ctx
        .kernel
        .fs
        .read_file(&patch_path)
        .map_err(|_| ToolError::ReadFailed)?;
    let patch_text = core::str::from_utf8(&patch_data).map_err(|_| ToolError::ReadFailed)?;
    let mut lines: Vec<String> = core::str::from_utf8(&original)
        .map_err(|_| ToolError::ReadFailed)?
        .lines()
        .map(|l| l.into())
        .collect();
    for line in patch_text.lines() {
        if let Some(rest) = line.strip_prefix("+ ") {
            lines.push(rest.into());
        } else if let Some(rest) = line.strip_prefix("- ") {
            if let Some(pos) = lines.iter().position(|l| l == rest) {
                lines.remove(pos);
            }
        }
    }
    let result = lines.join("\n");
    let tmp = format!("{path}.tmp");
    fs_cmd::write_path(ctx.kernel, ctx.caller, ctx.io, &tmp, &result)
        .map_err(|_| ToolError::WriteFailed)?;
    let new_hash = hash_payload(result.as_bytes());
    if patch_text.contains("hash:") {
        let expected_line = patch_text
            .lines()
            .find(|l| l.starts_with("hash:"))
            .unwrap_or("");
        if !expected_line.is_empty() && !expected_line.contains(&format!("{:02x?}", &expected[..4]))
        {
            return Err(ToolError::VerificationFailed);
        }
    }
    fs_cmd::write_path(ctx.kernel, ctx.caller, ctx.io, &path, &result)
        .map_err(|_| ToolError::WriteFailed)?;
    let _ = ctx.kernel.fs.rm(&tmp);
    ctx.record_statechain("patch_applied", path.as_bytes());
    let _ = new_hash;
    Ok(0)
}
