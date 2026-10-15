//! AI control layer shell commands.

use crate::kernel::ai::bridge;
use crate::kernel::ai::error::AiError;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

fn format_ai_error(err: AiError) -> String {
    err.reason()
}

pub fn run_connect(ctx: &mut ShellContext<'_>, host: &str, port: u16) -> Result<(), ShellError> {
    match bridge::connect(ctx.kernel, host, port) {
        Ok(msg) => ctx.writeln_line(&msg)?,
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

pub fn run_disconnect(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let msg = bridge::disconnect(ctx.kernel);
    ctx.writeln_line(&msg)?;
    Ok(())
}

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    bridge::poll_completions(ctx.kernel);
    ctx.writeln(&bridge::status(ctx.kernel))?;
    Ok(())
}

pub fn run_ask(ctx: &mut ShellContext<'_>, prompt: &str) -> Result<(), ShellError> {
    match bridge::ask(ctx.kernel, prompt) {
        Ok(summary) => {
            ctx.writeln_line(&format!("AI job {} created", summary.job_id))?;
            ctx.writeln_line(&format!("PID: {}", summary.process_id))?;
            ctx.writeln_line(&format!("Phase: {}", phase_label(summary.phase)))?;
            ctx.writeln_line(&format!("Status: {}", summary.status.label()))?;
        }
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

pub fn run_jobs(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let table = bridge::jobs_table(ctx.kernel);
    ctx.writeln(&table)?;
    Ok(())
}

pub fn run_show(ctx: &mut ShellContext<'_>, job_id: u64) -> Result<(), ShellError> {
    match bridge::show_job(ctx.kernel, job_id, false) {
        Ok(text) => ctx.writeln(&text)?,
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

pub fn run_trace(ctx: &mut ShellContext<'_>, job_id: u64) -> Result<(), ShellError> {
    match bridge::trace_job(ctx.kernel, job_id) {
        Ok(text) => ctx.writeln(&text)?,
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

pub fn run_cancel(ctx: &mut ShellContext<'_>, job_id: u64) -> Result<(), ShellError> {
    match bridge::cancel(ctx.kernel, job_id) {
        Ok(msg) => ctx.writeln_line(&msg)?,
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

pub fn run_tools(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&bridge::tools(ctx.kernel))?;
    Ok(())
}

pub fn run_allow(ctx: &mut ShellContext<'_>, tool: &str) -> Result<(), ShellError> {
    match bridge::allow(ctx.kernel, tool) {
        Ok(msg) => ctx.writeln_line(&msg)?,
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

pub fn run_deny(ctx: &mut ShellContext<'_>, tool: &str) -> Result<(), ShellError> {
    match bridge::deny(ctx.kernel, tool) {
        Ok(msg) => ctx.writeln_line(&msg)?,
        Err(err) => ctx.writeln_line(&format_ai_error(err))?,
    }
    Ok(())
}

fn phase_label(phase: crate::core::gphase_core::PhaseState) -> &'static str {
    match phase {
        crate::core::gphase_core::PhaseState::Phi0 => "Phi0",
        crate::core::gphase_core::PhaseState::Phi1 => "Phi1",
        crate::core::gphase_core::PhaseState::Phi2 => "Phi2",
        crate::core::gphase_core::PhaseState::Phi3 => "Phi3",
    }
}
