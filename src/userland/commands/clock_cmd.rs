//! clock — StateClock operator commands.

use crate::userland::commands::state_cmd;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&format!(
        "{}\n",
        state_cmd::render_clock(ctx.kernel.get_clock())
    ))?;
    Ok(())
}

pub fn run_skew(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let clock = ctx.kernel.get_clock();
    ctx.writeln_line(&format!(
        "skew: {} (state_tick {} proper_tick {})",
        clock.skew(),
        clock.state_tick,
        clock.proper_tick
    ))?;
    Ok(())
}

pub fn run_set_dilation(ctx: &mut ShellContext<'_>, value: f64) -> Result<(), ShellError> {
    ctx.kernel
        .set_clock_dilation(value)
        .map_err(|_| ShellError::Unknown("invalid dilation factor".into()))?;
    ctx.writeln_line(&format!("dilation_factor set to {value:.3}"))?;
    Ok(())
}

pub fn run_resync(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.kernel.resync_clock("operator resync");
    let clock = ctx.kernel.get_clock();
    ctx.writeln_line(&format!("clock resynced (skew now {})", clock.skew()))?;
    Ok(())
}
