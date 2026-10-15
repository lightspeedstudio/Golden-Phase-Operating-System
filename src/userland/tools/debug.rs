//! Debug and binary inspection tools.

use crate::kernel::state_proof::hash_payload;
use crate::platform::String;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::limits::{MAX_HEX_DUMP, MAX_STRINGS, MIN_STRING_LEN};
use crate::userland::tools::path_util::resolve_path;
use crate::userland::tools::types::{AuthorityRequirement, TargetSupport};

pub fn hexdump(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.first().map(|s| s.as_str()) == Some("memory") {
        ctx.check_authority(AuthorityRequirement::Root)?;
        return hexdump_memory(ctx, &args[1..]);
    }
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let mut offset = 0usize;
    let mut length = MAX_HEX_DUMP;
    let mut i = 1;
    while i + 1 < args.len() {
        if args[i] == "--offset" {
            offset = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
            i += 2;
            continue;
        }
        if args[i] == "--length" {
            length = args
                .get(i + 1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(MAX_HEX_DUMP)
                .min(MAX_HEX_DUMP);
            i += 2;
            continue;
        }
        i += 1;
    }
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let end = offset.saturating_add(length).min(data.len());
    let slice = &data[offset..end];
    for (idx, chunk) in slice.chunks(16).enumerate() {
        let addr = offset + idx * 16;
        let hex: String = chunk.iter().map(|b| format!("{b:02x} ")).collect();
        let ascii: String = chunk
            .iter()
            .map(|b| {
                if b.is_ascii_graphic() {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        ctx.writeln_line(&format!("{addr:08x}  {hex:<48} {ascii}"))?;
    }
    Ok(0)
}

pub fn strings(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let min_len = args
        .windows(2)
        .find(|w| w[0] == "--min-length")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(MIN_STRING_LEN)
        .max(MIN_STRING_LEN);
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let mut current = String::new();
    let mut count = 0usize;
    for byte in data {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte as char);
        } else if current.len() >= min_len {
            ctx.writeln_line(&current)?;
            count += 1;
            current.clear();
            if count >= MAX_STRINGS {
                break;
            }
        } else {
            current.clear();
        }
    }
    if current.len() >= min_len && count < MAX_STRINGS {
        ctx.writeln_line(&current)?;
    }
    Ok(0)
}

pub fn checksum(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.first().map(|s| s.as_str()) == Some("verify") {
        let path = resolve_path(&ctx.session.cwd, &args[1])?;
        let expected = args.get(2).ok_or(ToolError::InvalidArguments)?;
        let data = ctx
            .kernel
            .fs
            .read_file(&path)
            .map_err(|_| ToolError::ReadFailed)?;
        let hash = hash_payload(&data);
        let ok = format!("{:02x?}", hash).contains(expected);
        if ok {
            ctx.writeln_line("checksum ok")?;
            return Ok(0);
        }
        return Err(ToolError::VerificationFailed);
    }
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    let hash = hash_payload(&data);
    ctx.writeln_line(&format!("{:02x?}", hash))?;
    Ok(0)
}

pub fn size(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
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
    ctx.writeln_line(&format!("file: {} bytes", data.len()))?;
    Ok(0)
}

pub fn symbols(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.check_target(TargetSupport::Hosted)?;
    let path = resolve_path(
        &ctx.session.cwd,
        args.first().ok_or(ToolError::InvalidArguments)?,
    )?;
    let pattern = args.get(1).map(|s| s.as_str()).unwrap_or("");
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    if data.starts_with(b"\x7fELF") {
        ctx.writeln_line("ELF binary detected; symbol table not embedded in base tool")?;
    } else {
        ctx.writeln_line("no symbol table available")?;
    }
    let _ = pattern;
    Ok(0)
}

pub fn objdump(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let sub = args[0].as_str();
    let path = resolve_path(&ctx.session.cwd, &args[1])?;
    let data = ctx
        .kernel
        .fs
        .read_file(&path)
        .map_err(|_| ToolError::ReadFailed)?;
    match sub {
        "headers" => {
            ctx.writeln_line(&format!("file: {path}"))?;
            ctx.writeln_line(&format!("size: {} bytes", data.len()))?;
            if data.starts_with(b"\x7fELF") {
                ctx.writeln_line("format: ELF")?;
            }
        }
        "sections" => ctx.writeln_line("sections: unavailable in minimal objdump")?,
        "symbols" => {
            symbols(ctx, args[1..].to_vec().as_slice())?;
        }
        "disasm" => {
            for (i, chunk) in data.chunks(4).enumerate() {
                if chunk.len() < 4 {
                    break;
                }
                let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                match crate::isa::disassemble_word(word) {
                    Ok(text) => ctx.writeln_line(&format!("{i:04x}: {text}"))?,
                    Err(_) => ctx.writeln_line(&format!("{i:04x}: .word 0x{word:08x}"))?,
                }
            }
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

fn parse_usize(s: &str) -> Option<usize> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        usize::from_str_radix(h, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn hexdump_memory(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    let addr = parse_usize(args.first().ok_or(ToolError::InvalidArguments)?)
        .ok_or(ToolError::InvalidArguments)?;
    let mut length = 256usize.min(MAX_HEX_DUMP);
    let mut i = 1;
    while i + 1 < args.len() {
        if args[i] == "--length" {
            length = args
                .get(i + 1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(length)
                .min(MAX_HEX_DUMP);
            i += 2;
            continue;
        }
        i += 1;
    }
    #[cfg(all(
        feature = "pmm",
        target_arch = "arm",
        feature = "zynq-a9",
        any(feature = "std", feature = "baremetal")
    ))]
    {
        let (lo, hi) = ctx.kernel.physical_memory.region();
        if addr < lo || addr.saturating_add(length) > hi {
            ctx.writeln_line(&format!(
                "hexdump memory: refuse 0x{addr:x} len {length} — PMM window is 0x{lo:x}..0x{hi:x} (kernel/DDR below window closed; use mmio read for device windows)"
            ))?;
            return Ok(1);
        }
        let mut data = crate::platform::Vec::new();
        data.resize(length, 0u8);
        for (off, b) in data.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile((addr + off) as *const u8) };
        }
        for (idx, chunk) in data.chunks(16).enumerate() {
            let a = addr + idx * 16;
            let hex: String = chunk.iter().map(|b| format!("{b:02x} ")).collect();
            let ascii: String = chunk
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() {
                        *b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            ctx.writeln_line(&format!("{a:08x}  {hex:<48} {ascii}"))?;
        }
        return Ok(0);
    }
    #[cfg(not(all(
        feature = "pmm",
        target_arch = "arm",
        feature = "zynq-a9",
        any(feature = "std", feature = "baremetal")
    )))]
    {
        let _ = (addr, length);
        ctx.writeln_line(
            "hexdump memory: physical page dump is PZ7020 PMM (MMU off); this build cannot bus-read DDR",
        )?;
        Ok(1)
    }
}
