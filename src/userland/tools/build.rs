//! Build tool group — hosted Rust toolchain wrappers.

use crate::platform::String;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::limits::MAX_BUILD_OUTPUT;
use crate::userland::tools::types::{AuthorityRequirement, TargetSupport};

pub fn build(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    #[cfg(not(feature = "std"))]
    {
        return assemble_phase_program(ctx, args);
    }
    #[cfg(feature = "std")]
    {
        ctx.check_target(TargetSupport::Hosted)?;
        let mut cargo_args = vec!["build"];
        if args.iter().any(|a| a == "--release") {
            cargo_args.push("--release");
        }
        run_cargo(ctx, &cargo_args)
    }
}

pub fn test(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    #[cfg(not(feature = "std"))]
    {
        return assemble_phase_program(ctx, args);
    }
    #[cfg(feature = "std")]
    {
        ctx.check_target(TargetSupport::Hosted)?;
        let mut cargo_args = vec!["test"];
        for arg in args {
            cargo_args.push(arg.as_str());
        }
        run_cargo(ctx, &cargo_args)
    }
}

pub fn check(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    #[cfg(not(feature = "std"))]
    {
        return assemble_phase_program(ctx, args);
    }
    #[cfg(feature = "std")]
    {
        let _ = args;
        ctx.check_target(TargetSupport::Hosted)?;
        run_cargo(ctx, &["check"])
    }
}

pub fn format_cmd(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    #[cfg(not(feature = "std"))]
    {
        return assemble_phase_program(ctx, args);
    }
    #[cfg(feature = "std")]
    {
        ctx.check_target(TargetSupport::Hosted)?;
        if args.iter().any(|a| a == "--check") {
            run_cargo(ctx, &["fmt", "--", "--check"])
        } else {
            run_cargo(ctx, &["fmt"])
        }
    }
}

pub fn lint(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    #[cfg(not(feature = "std"))]
    {
        return assemble_phase_program(ctx, args);
    }
    #[cfg(feature = "std")]
    {
        let _ = args;
        ctx.check_target(TargetSupport::Hosted)?;
        run_cargo(ctx, &["clippy", "--", "-D", "warnings"])
    }
}

pub fn program_build(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    assemble_phase_program(ctx, args)
}

fn assemble_phase_program(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    let path = args.first().ok_or(ToolError::InvalidArguments)?;
    let name = args.get(1).cloned().unwrap_or_else(|| path.clone());
    let bytes = ctx
        .kernel
        .fs
        .read_file(path)
        .map_err(|_| ToolError::ReadFailed)?;
    let source = core::str::from_utf8(&bytes).map_err(|_| ToolError::InvalidArguments)?;
    match crate::isa::assemble_instructions(source) {
        Ok(program) => {
            ctx.writeln_line(&format!(
                "assembled {path} -> {name} ({} instructions); run with: run {name}",
                program.len()
            ))?;
            // Named run still uses ProgramLoader defaults; execute immediately.
            match crate::userland::run::run_assembled(
                ctx.kernel,
                ctx.caller,
                ctx.io,
                &name,
                &program,
                ctx.vms,
            ) {
                Ok(_) => Ok(0),
                Err(e) => {
                    ctx.writeln_line(&format!("{e}"))?;
                    Ok(1)
                }
            }
        }
        Err(e) => {
            ctx.writeln_line(&format!("asm: {e}"))?;
            Ok(1)
        }
    }
}

#[cfg(feature = "std")]
fn run_cargo(ctx: &mut CommandContext<'_>, args: &[&str]) -> Result<i32, ToolError> {
    use std::process::Command;
    let output = Command::new("cargo")
        .args(args)
        .output()
        .map_err(|e| ToolError::InternalError(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let truncated = if combined.len() > MAX_BUILD_OUTPUT {
        format!("{}...[truncated]", &combined[..MAX_BUILD_OUTPUT])
    } else {
        combined
    };
    ctx.writeln(&truncated)?;
    Ok(output.status.code().unwrap_or(1))
}

#[cfg(not(feature = "std"))]
fn run_cargo(_ctx: &mut CommandContext<'_>, _args: &[&str]) -> Result<i32, ToolError> {
    Err(ToolError::UnsupportedTarget)
}

pub fn hosted_unavailable(ctx: &mut CommandContext<'_>, cmd: &str) -> Result<i32, ToolError> {
    ctx.writeln_line(&format!("{cmd}: hosted toolchain unavailable on baremetal"))?;
    Ok(1)
}
