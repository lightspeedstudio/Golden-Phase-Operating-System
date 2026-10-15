use kairos::core::gphase_core::{PhaseState, TopologyKind};
use kairos::runtime::trace::TraceEvent;
use kairos::Runtime;

#[test]
fn boots_init() {
    let mut rt = Runtime::new();
    let pid = rt.boot().unwrap();
    assert_eq!(rt.hosted.init_pid, Some(pid));
    assert_eq!(
        rt.hosted.kernel.get_process(pid).unwrap().state,
        PhaseState::Phi2
    );
}

#[test]
fn run_shell_ps_mem_log() {
    let mut rt = Runtime::with_input(["ps", "mem", "log", "exit"]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();

    let output = rt.output_lines().join("\n");
    assert!(output.contains("running"));
    assert!(output.contains("user"));
    assert!(output.contains("awakening"));
    assert!(output.contains("stateclock"));
}

#[test]
fn spawn_multiple_processes() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let init = rt.hosted.init_pid.unwrap();
    let a = rt.spawn_background().unwrap();
    let b = rt.spawn_background().unwrap();
    assert_ne!(a, b);
    assert!(rt.hosted.kernel.process_ids().len() >= 3);
    assert_eq!(
        rt.hosted.kernel.get_process(init).unwrap().state,
        PhaseState::Phi2
    );
    assert_eq!(
        rt.hosted.kernel.get_process(a).unwrap().state,
        PhaseState::Phi1
    );
}

#[test]
fn timer_preemption_changes_states_legally() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let init = rt.hosted.init_pid.unwrap();
    let child = rt.spawn_background().unwrap();
    rt.hosted.kernel.schedule(child).unwrap();

    let running_before = rt
        .hosted
        .kernel
        .process_ids()
        .iter()
        .filter(|pid| rt.hosted.kernel.get_process(**pid).unwrap().state == PhaseState::Phi2)
        .count();
    assert!(running_before >= 1);

    rt.force_timer_tick().unwrap();

    assert!(rt
        .trace
        .events
        .iter()
        .any(|e| matches!(e, TraceEvent::Preempt { .. })));
    assert!(rt
        .trace
        .events
        .iter()
        .any(|e| matches!(e, TraceEvent::Schedule { .. })));

    let init_state = rt.hosted.kernel.get_process(init).unwrap().state;
    assert!(
        init_state == PhaseState::Phi1 || init_state == PhaseState::Phi2,
        "init state after preempt: {init_state:?}"
    );
}

#[test]
fn reflection_survives_log_command() {
    let mut rt = Runtime::with_input(["log", "exit"]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();

    assert!(rt
        .hosted
        .kernel
        .topology_report()
        .iter()
        .any(|e| e.kind == TopologyKind::Reflection));
    assert!(rt.trace.reflections().len() >= 1);
    let output = rt.output_lines().join("\n");
    assert!(output.contains("reflection"));
}

#[test]
fn run_command_ps_and_fs() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.run_command("mkdir /tmp/demo").unwrap();
    rt.run_command("write /tmp/demo/note.txt ok").unwrap();
    rt.run_command("cat /tmp/demo/note.txt").unwrap();
    let output = rt.output_lines().join("\n");
    assert!(output.contains("ok"));
}

#[test]
fn memory_initialized_on_boot() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let tags = rt.hosted.kernel.tag_report();
    assert!(tags.len() >= 4);
}

/// Regression test for a bug where any command returning an error (e.g.
/// `cat` of a path that does not exist) caused `Runtime::run`'s shell loop to
/// propagate the error out of the loop via `?`, exiting the whole runtime
/// instead of printing the error and returning to the prompt. Commands after
/// the failing one must still execute.
#[test]
fn failing_command_does_not_halt_the_shell_loop() {
    let mut rt = Runtime::with_input(["cat /does/not/exist", "mkdir /proof-of-life", "exit"]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();

    let output = rt.output_lines().join("\n");
    assert!(output.contains("error:"));
    assert!(rt
        .hosted
        .kernel
        .fs
        .list_dir("/")
        .unwrap()
        .contains(&"proof-of-life".to_string()));
}

#[test]
fn phasefs_mounted_on_boot() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let listing = rt.hosted.kernel.fs.list_dir("/tmp").unwrap();
    assert!(listing.is_empty());
}
