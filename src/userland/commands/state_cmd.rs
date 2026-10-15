//! state — kernel summary and StateClock display.

use crate::core::gphase_core::PhaseState;
use crate::platform::{String, Vec};
use crate::stateclock::StateClock;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn render_clock(clock: &StateClock) -> String {
    format!(
        "GPOS StateClock\n\n\
         chronos:\n\
           tick: {}\n\
           phi: {:.3}\n\
           x: {:.3}\n\n\
         kairos:\n\
           tick: {}\n\
           sector_id: {}\n\
           epsilon: {:.3}\n\n\
         dual clock:\n\
           proper_tick: {}\n\
           state_tick: {}\n\
           dilation_factor: {:.3}\n\
           skew: {}\n",
        clock.chronos_tick,
        clock.phi,
        clock.x,
        clock.kairos_tick,
        clock.sector_id,
        clock.epsilon,
        clock.proper_tick,
        clock.state_tick,
        clock.dilation_factor,
        clock.skew(),
    )
}

pub fn render(kernel: &crate::kernel::Kernel) -> String {
    let summary = kernel.get_state();
    let phase = kernel
        .process_ids()
        .into_iter()
        .filter_map(|pid| {
            kernel
                .get_process(pid)
                .ok()
                .filter(|p| p.state == PhaseState::Phi2)
                .map(|p| format!("pid {} ({}) {}", pid, p.name, phase_label(p.state)))
        })
        .collect::<Vec<_>>()
        .join(", ");

    let phase_line = if phase.is_empty() {
        "Phi2 running".into()
    } else {
        format!("active: {phase}")
    };

    format!(
        "{}\n\
         kernel phase:\n\
           current: {phase_line}\n\
         processes: {}\n\
         running: {}\n\
         memory pages: {}\n",
        render_clock(kernel.get_clock()),
        summary.process_count,
        summary.running_count,
        summary.page_count,
    )
}

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let text = render(ctx.kernel);
    ctx.writeln(&text)?;
    Ok(())
}

fn phase_label(state: PhaseState) -> &'static str {
    state.kairos_label()
}
