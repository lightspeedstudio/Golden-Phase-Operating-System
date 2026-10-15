//! File tool group — canonical PhaseFS operations.

use crate::core::gphase_core::PhaseState;
use crate::fs::inode::InodeKind;
use crate::userland::fs_cmd::{self, phase_label};
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::limits::{
    MAX_DEFAULT_LINES, MAX_FIND_DEPTH, MAX_FIND_RESULTS, MAX_HEAD_TAIL_LINES, MAX_TREE_DEPTH,
};
use crate::userland::tools::path_util::{parent_path, resolve_path};
use crate::userland::tools::prelude::*;
use crate::userland::tools::types::AuthorityRequirement;

pub fn pwd(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let cwd = ctx.session.cwd.clone();
    ctx.writeln_line(&cwd)?;
    Ok(0)
}

pub fn cd(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let raw = args.first().map(|s| s.as_str()).unwrap_or("/");
    let target = if raw == ".." {
        parent_path(&ctx.session.cwd).unwrap_or_else(|| "/".into())
    } else {
        resolve_path(&ctx.session.cwd, raw)?
    };
    ctx.kernel
        .fs
        .list_dir(&target)
        .map_err(|_| ToolError::ObjectNotFound)?;
    ctx.session.cwd = target;
    Ok(0)
}

pub fn ls(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let mut long = false;
    let mut all = false;
    let mut path = ctx.session.cwd.clone();
    for arg in args {
        match arg.as_str() {
            "-l" => long = true,
            "-a" => all = true,
            _ if !arg.starts_with('-') => path = resolve_path(&ctx.session.cwd, arg)?,
            _ => {}
        }
    }
    let entries = ctx
        .kernel
        .fs
        .list_dir(&path)
        .map_err(|_| ToolError::ObjectNotFound)?;
    let mut names: Vec<String> = entries
        .into_iter()
        .filter(|n| all || !n.starts_with('.'))
        .collect();
    names.sort();
    if long {
        ctx.writeln_line(&format!("listing: {path}"))?;
        for name in &names {
            if !all && name.starts_with('.') {
                continue;
            }
            let full = if path == "/" {
                format!("/{name}")
            } else {
                format!("{path}/{name}")
            };
            let phase = ctx.kernel.fs.stat(&full).unwrap_or(PhaseState::Phi0);
            let kind = if ctx.kernel.fs.list_dir(&full).is_ok() {
                "dir"
            } else {
                "file"
            };
            let size = ctx.kernel.fs.read_file(&full).map(|d| d.len()).unwrap_or(0);
            ctx.writeln_line(&format!(
                "{:<16} {:<5} {:>8} {:<4} viewer chronos:{}",
                name,
                kind,
                size,
                phase_label(phase),
                ctx.kernel.get_clock().snapshot().chronos_tick
            ))?;
        }
    } else {
        ctx.writeln_line(&names.join("  "))?;
    }
    Ok(0)
}

pub fn tree(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let mut path = ctx.session.cwd.clone();
    let mut depth = MAX_TREE_DEPTH;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--depth" {
            if let Some(v) = args.get(i + 1).and_then(|s| s.parse::<usize>().ok()) {
                depth = v.min(MAX_TREE_DEPTH);
            }
            i += 2;
            continue;
        }
        if !args[i].starts_with('-') {
            path = resolve_path(&ctx.session.cwd, &args[i])?;
        }
        i += 1;
    }
    tree_walk(ctx, &path, 0, depth, "")?;
    Ok(0)
}

fn tree_walk(
    ctx: &mut CommandContext<'_>,
    path: &str,
    level: usize,
    max_depth: usize,
    prefix: &str,
) -> Result<(), ToolError> {
    if level > max_depth {
        return Ok(());
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let display = if path == "/" {
        "/".to_string()
    } else {
        name.to_string()
    };
    ctx.writeln_line(&format!("{prefix}{display}"))?;
    if level == max_depth {
        return Ok(());
    }
    let entries = ctx
        .kernel
        .fs
        .list_dir(path)
        .map_err(|_| ToolError::ObjectNotFound)?;
    for entry in entries {
        let child = if path == "/" {
            format!("/{entry}")
        } else {
            format!("{path}/{entry}")
        };
        tree_walk(ctx, &child, level + 1, max_depth, &format!("{prefix}  "))?;
    }
    Ok(())
}

pub fn mkdir(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let mut parents = false;
    let mut paths = Vec::new();
    for arg in args {
        if arg == "-p" {
            parents = true;
        } else {
            paths.push(resolve_path(&ctx.session.cwd, arg)?);
        }
    }
    if paths.is_empty() {
        return Err(ToolError::InvalidArguments);
    }
    for path in paths {
        if parents {
            create_parents(ctx, &path)?;
        }
        fs_cmd::mkdir(ctx.kernel, ctx.caller, ctx.io, &path).map_err(|e| ToolError::WriteFailed)?;
        ctx.record_statechain("file_created", path.as_bytes());
    }
    Ok(0)
}

fn create_parents(ctx: &mut CommandContext<'_>, path: &str) -> Result<(), ToolError> {
    let mut parts = path
        .split('/')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(());
    }
    parts.pop();
    let mut current = String::from("/");
    for part in parts {
        current = if current == "/" {
            format!("/{part}")
        } else {
            format!("{current}/{part}")
        };
        let _ = fs_cmd::mkdir(ctx.kernel, ctx.caller, ctx.io, &current);
    }
    Ok(())
}

pub fn touch(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    match ctx.kernel.fs.read_file(&path) {
        Ok(_) => {
            let data = ctx.kernel.fs.read_file(&path).unwrap_or_default();
            fs_cmd::write_path(
                ctx.kernel,
                ctx.caller,
                ctx.io,
                &path,
                core::str::from_utf8(&data).unwrap_or(""),
            )
            .map_err(|_| ToolError::WriteFailed)?;
        }
        Err(_) => {
            fs_cmd::mkdir(
                ctx.kernel,
                ctx.caller,
                ctx.io,
                &parent_path(&path).unwrap_or("/".into()),
            )
            .ok();
            ctx.kernel
                .fs
                .create_file(&path)
                .map_err(|_| ToolError::WriteFailed)?;
            ctx.record_statechain("file_created", path.as_bytes());
        }
    }
    Ok(0)
}

pub fn cp(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let src = resolve_path(&ctx.session.cwd, &args[0])?;
    let dst = resolve_path(&ctx.session.cwd, &args[1])?;
    if ctx.kernel.fs.list_dir(&src).is_ok() {
        return Err(ToolError::NotImplemented);
    }
    let data = ctx
        .kernel
        .fs
        .read_file(&src)
        .map_err(|_| ToolError::ReadFailed)?;
    if ctx.kernel.fs.read_file(&dst).is_err() {
        ctx.kernel
            .fs
            .create_file(&dst)
            .map_err(|_| ToolError::WriteFailed)?;
    }
    fs_cmd::write_path(
        ctx.kernel,
        ctx.caller,
        ctx.io,
        &dst,
        core::str::from_utf8(&data).unwrap_or(""),
    )
    .map_err(|_| ToolError::WriteFailed)?;
    ctx.record_statechain("file_created", dst.as_bytes());
    Ok(0)
}

pub fn mv(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let src = resolve_path(&ctx.session.cwd, &args[0])?;
    let dst = resolve_path(&ctx.session.cwd, &args[1])?;
    if ctx.kernel.fs.list_dir(&src).is_ok() {
        return Err(ToolError::NotImplemented);
    }
    let data = ctx
        .kernel
        .fs
        .read_file(&src)
        .map_err(|_| ToolError::ReadFailed)?;
    if ctx.kernel.fs.read_file(&dst).is_err() {
        ctx.kernel
            .fs
            .create_file(&dst)
            .map_err(|_| ToolError::WriteFailed)?;
    }
    fs_cmd::write_path(
        ctx.kernel,
        ctx.caller,
        ctx.io,
        &dst,
        core::str::from_utf8(&data).unwrap_or(""),
    )
    .map_err(|_| ToolError::WriteFailed)?;
    fs_cmd::rm(ctx.kernel, ctx.caller, ctx.io, &src).map_err(|_| ToolError::WriteFailed)?;
    ctx.record_statechain("file_moved", format!("{src}->{dst}").as_bytes());
    Ok(0)
}

pub fn rm(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    if let Ok(entries) = ctx.kernel.fs.list_dir(&path) {
        if !entries.is_empty() {
            return Err(ToolError::InvalidState);
        }
    }
    fs_cmd::rm(ctx.kernel, ctx.caller, ctx.io, &path).map_err(|_| ToolError::WriteFailed)?;
    ctx.record_statechain("file_removed", path.as_bytes());
    Ok(0)
}

pub fn cat(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    if core::str::from_utf8(&data).is_err() {
        ctx.writeln_line("binary file; use debug hexdump")?;
        return Ok(0);
    }
    let text = core::str::from_utf8(&data).unwrap_or("");
    ctx.writeln(text)?;
    if !text.ends_with('\n') {
        ctx.writeln("\n")?;
    }
    Ok(0)
}

pub fn head(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let (path, count) = parse_path_count(ctx, args, MAX_DEFAULT_LINES)?;
    let data = read_text_file(ctx, &path)?;
    for line in data.lines().take(count.min(MAX_HEAD_TAIL_LINES)) {
        ctx.writeln_line(line)?;
    }
    Ok(0)
}

pub fn tail(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let (path, count) = parse_path_count(ctx, args, MAX_DEFAULT_LINES)?;
    let data = read_text_file(ctx, &path)?;
    let lines: Vec<&str> = data.lines().collect();
    let start = lines.len().saturating_sub(count.min(MAX_HEAD_TAIL_LINES));
    for line in &lines[start..] {
        ctx.writeln_line(line)?;
    }
    Ok(0)
}

fn parse_path_count(
    ctx: &mut CommandContext<'_>,
    args: &[String],
    default: usize,
) -> Result<(String, usize), ToolError> {
    if args.is_empty() {
        return Err(ToolError::InvalidArguments);
    }
    if args.len() == 1 {
        return Ok((resolve_path(&ctx.session.cwd, &args[0])?, default));
    }
    let count = args[1].parse().unwrap_or(default);
    Ok((resolve_path(&ctx.session.cwd, &args[0])?, count))
}

fn read_text_file(ctx: &mut CommandContext<'_>, path: &str) -> Result<String, ToolError> {
    let data = ctx
        .kernel
        .fs
        .read_file(path)
        .map_err(|_| ToolError::ReadFailed)?;
    core::str::from_utf8(&data)
        .map(|s| s.to_string())
        .map_err(|_| ToolError::ReadFailed)
}

pub fn stat(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let phase = ctx
        .kernel
        .fs
        .stat(&path)
        .map_err(|_| ToolError::ObjectNotFound)?;
    let kind = if ctx.kernel.fs.list_dir(&path).is_ok() {
        InodeKind::Directory
    } else {
        InodeKind::File
    };
    let size = ctx.kernel.fs.read_file(&path).map(|d| d.len()).unwrap_or(0);
    let hash = crate::kernel::state_proof::hash_payload(
        &ctx.kernel.fs.read_file(&path).unwrap_or_default(),
    );
    let clock = ctx.kernel.get_clock().snapshot();
    ctx.writeln_line(&format!("path: {path}"))?;
    ctx.writeln_line(&format!("type: {:?}", kind))?;
    ctx.writeln_line(&format!("size: {size}"))?;
    ctx.writeln_line(&format!("phase: {}", phase_label(phase)))?;
    ctx.writeln_line(&format!("chronos: {}", clock.chronos_tick))?;
    ctx.writeln_line(&format!("kairos: {}", clock.kairos_tick))?;
    ctx.writeln_line(&format!("hash: {:02x?}", &hash[..8]))?;
    ctx.writeln_line(&format!(
        "anom: {}",
        if ctx.kernel.authority.is_anom_protected() {
            "protected"
        } else {
            "transparent"
        }
    ))?;
    Ok(0)
}

pub fn find(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let root = resolve_path(&ctx.session.cwd, &args[0])?;
    let pattern = &args[1];
    let mut found = 0usize;
    find_walk(ctx, &root, pattern, 0, MAX_FIND_DEPTH, &mut found)?;
    Ok(0)
}

fn find_walk(
    ctx: &mut CommandContext<'_>,
    path: &str,
    pattern: &str,
    depth: usize,
    max_depth: usize,
    found: &mut usize,
) -> Result<(), ToolError> {
    if *found >= MAX_FIND_RESULTS || depth > max_depth {
        return Ok(());
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.contains(pattern) {
        ctx.writeln_line(path)?;
        *found += 1;
    }
    if let Ok(entries) = ctx.kernel.fs.list_dir(path) {
        for entry in entries {
            let child = if path == "/" {
                format!("/{entry}")
            } else {
                format!("{path}/{entry}")
            };
            find_walk(ctx, &child, pattern, depth + 1, max_depth, found)?;
        }
    }
    Ok(())
}

pub fn fsj_status(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let text = crate::userland::fsj_cmd::status(ctx.kernel);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_check(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let text = crate::userland::fsj_cmd::check(ctx.kernel);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_format(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let confirm = args.first().map(|s| s.as_str()) == Some("CONFIRM");
    let text = crate::userland::fsj_cmd::format_cmd(ctx.kernel, confirm);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_recover(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let text = crate::userland::fsj_cmd::recover(ctx.kernel);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_crash(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let n = match args.first().map(|s| s.as_str()) {
        Some("off") | None => 0,
        Some(s) => s.parse::<u32>().map_err(|_| ToolError::InvalidArguments)?,
    };
    let text = crate::userland::fsj_cmd::crash(ctx.kernel, n);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_p6(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let confirm = args.first().map(|s| s.as_str()) == Some("CONFIRM");
    let text = crate::userland::fsj_cmd::p6_arm(ctx.kernel, confirm);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_p7(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let run_id = args
        .first()
        .ok_or(ToolError::InvalidArguments)?
        .parse::<u64>()
        .map_err(|_| ToolError::InvalidArguments)?;
    let token_ok = args.get(1).map(|s| s.as_str()) == Some("PHASEFS-P7-CORRUPT-TAIL");
    let text = crate::userland::fsj_cmd::p7_run(ctx.kernel, run_id, token_ok);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_p8(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let run_id = args
        .first()
        .ok_or(ToolError::InvalidArguments)?
        .parse::<u64>()
        .map_err(|_| ToolError::InvalidArguments)?;
    let token_ok = args.get(1).map(|s| s.as_str()) == Some("PHASEFS-P8-WRAP");
    let text = crate::userland::fsj_cmd::p8_run(ctx.kernel, run_id, token_ok);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_p10(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let run_id = args
        .first()
        .ok_or(ToolError::InvalidArguments)?
        .parse::<u64>()
        .map_err(|_| ToolError::InvalidArguments)?;
    let token_ok = args.get(1).map(|s| s.as_str()) == Some("PHASEFS-P10-BEFORE-COMMIT");
    let text = crate::userland::fsj_cmd::p10_run(ctx.kernel, run_id, token_ok);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_p11(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let run_id = args
        .first()
        .ok_or(ToolError::InvalidArguments)?
        .parse::<u64>()
        .map_err(|_| ToolError::InvalidArguments)?;
    let token_ok = args.get(1).map(|s| s.as_str()) == Some("PHASEFS-P11-AFTER-COMMIT");
    let text = crate::userland::fsj_cmd::p11_run(ctx.kernel, run_id, token_ok);
    ctx.writeln_line(&text)?;
    Ok(0)
}

pub fn fsj_p1reset(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Root)?;
    let text = crate::userland::fsj_cmd::p1_reset(ctx.kernel);
    ctx.writeln_line(&text)?;
    Ok(0)
}
