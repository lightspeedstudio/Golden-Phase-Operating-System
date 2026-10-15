//! trace — recent kernel trace events.

use crate::kernel::trace::TraceRecord;
use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn render(events: &[TraceRecord]) -> String {
    if events.is_empty() {
        return "no trace events".into();
    }
    let mut out = String::from("kernel trace:\n");
    for event in events {
        out.push_str("  ");
        out.push_str(&format_event(event));
        out.push('\n');
    }
    out
}

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let text = render(ctx.kernel.get_trace_log());
    ctx.writeln(&text)?;
    Ok(())
}

fn format_event(event: &TraceRecord) -> String {
    match event {
        TraceRecord::Boot { init_pid } => format!("boot init_pid={init_pid}"),
        TraceRecord::Command { line } => format!("command: {line}"),
        TraceRecord::Spawn { pid, name } => format!("spawn pid={pid} name={name}"),
        TraceRecord::Kill { pid, ok } => format!("kill pid={pid} ok={ok}"),
        TraceRecord::Error { message } => format!("error: {message}"),
        TraceRecord::PhaseChange { pid, from, to } => {
            format!("phase pid={pid} {from:?} -> {to:?}")
        }
    }
}
