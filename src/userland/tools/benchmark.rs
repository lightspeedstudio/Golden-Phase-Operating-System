//! Benchmark and verification tools.

use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::types::{AuthorityRequirement, TargetSupport};

pub fn bench(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" => ctx.writeln_line("bench: use phase-benchmark or phaseperf")?,
        "run" | "repeat" | "report" => {
            ctx.writeln_line("bench: use phase-benchmark run")?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn phi_stability(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.writeln_line("phi-stability: use phi-stability status")?;
    Ok(0)
}

pub fn phaseperf(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.check_target(TargetSupport::Hosted)?;
    ctx.writeln_line("phaseperf: use phaseperf status")?;
    Ok(0)
}

pub fn test_system(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let mode = args.first().map(|s| s.as_str()).unwrap_or("quick");
    ctx.writeln_line(&format!(
        "test-system {mode}: run cargo test for full verification"
    ))?;
    #[cfg(feature = "std")]
    if mode == "quick" {
        return crate::userland::tools::build::test(
            ctx,
            &["--test".into(), "engineering_tools_tests".into()],
        );
    }
    Ok(0)
}
