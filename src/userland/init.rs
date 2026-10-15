//! init — first process; starts the hosted shell.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::kernel::Kernel;
use crate::userland::hosted::Hosted;
use crate::userland::shell::ShellError;

pub fn boot(hosted: &mut Hosted) -> ProcessId {
    let pid = hosted.kernel.spawn_process("init");
    hosted
        .kernel
        .trace
        .record(crate::kernel::trace::TraceRecord::Boot { init_pid: pid });
    hosted.kernel.wake(pid).expect("init wake");
    hosted.kernel.schedule(pid).expect("init schedule");
    crate::stateclock::record_kairos_on_kernel(
        &mut hosted.kernel,
        crate::stateclock::KairosEventKind::Boot,
        "init",
    );
    hosted.init_pid = Some(pid);
    crate::kernel::anom::finalize_boot(&mut hosted.kernel, pid);
    #[cfg(feature = "std")]
    crate::kernel::network::configure_hosted_identity(&mut hosted.kernel);
    pid
}

pub fn start_shell(hosted: &mut Hosted) -> Result<(), ShellError> {
    let pid = hosted.init_pid.expect("init not booted");
    hosted.shell.run_session(
        &mut hosted.kernel,
        pid,
        &mut hosted.io,
        &hosted.programs,
        &mut hosted.vms,
        &mut hosted.session_state,
    )
}

pub fn init_state(kernel: &Kernel, pid: ProcessId) -> Option<PhaseState> {
    kernel.get_process(pid).ok().map(|p| p.state)
}
