//! `sched2` — observe `kernel.sched2`'s live run-state/timer-wheel counters.
//! `sleep` (the actual `PROC_SLEEP` syscall this scheduler backs) is a
//! separate top-level command, matching where `open`/`close` live relative
//! to `captable`.

use crate::kernel::Kernel;
use crate::platform::String;

pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "sched2", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.sched2.stats();
        let counts = kernel.sched2.state_counts();
        format!(
            "sched2\n\
             elapsed_tick: {}\n\
             ready: {}\n\
             running: {}\n\
             blocked: {}\n\
             exited: {}\n\
             registered: {}\n\
             unregistered: {}\n\
             dispatches: {}\n\
             preemptions: {}\n\
             timers_armed: {}\n\
             timers_fired: {}\n\
             timers_cancelled: {}\n\
             starvation_boosts: {}\n\
             generation_retired: {}",
            kernel.sched2.elapsed(),
            counts.ready,
            counts.running,
            counts.blocked,
            counts.exited,
            stats.registered,
            stats.unregistered,
            stats.dispatches,
            stats.preemptions,
            stats.timers_armed,
            stats.timers_fired,
            stats.timers_cancelled,
            stats.starvation_boosts,
            stats.generation_retired,
        )
    }
    #[cfg(not(all(feature = "sched2", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "sched2 unavailable (sched2 feature not enabled)".into()
    }
}
