//! Operator terminal command tests.

use kairos::platform::HashMap;
use kairos::userland::audit_cmd;
use kairos::userland::checkpoint_cmd;
use kairos::userland::dashboard;
use kairos::userland::init::boot;
use kairos::userland::parser::{parse, Command};
use kairos::userland::phase_view;
use kairos::userland::root_view;
use kairos::userland::shell::Shell;
use kairos::userland::top::{self, TopOptions};
use kairos::userland::topo_view;
use kairos::userland::watchdog_view;
use kairos::userland::Hosted;

fn booted() -> Hosted {
    let mut hosted = Hosted::new();
    boot(&mut hosted);
    hosted
}

#[test]
fn parser_recognizes_terminal_commands() {
    assert_eq!(parse("dashboard"), Command::Dashboard);
    assert_eq!(parse("help"), Command::Help);
    assert_eq!(parse("version"), Command::Version);
    assert_eq!(parse("state"), Command::State);
    assert_eq!(parse("trace"), Command::Trace);
    assert_eq!(parse("isa"), Command::Isa);
    assert_eq!(parse("dump"), Command::Dump);
    assert_eq!(
        parse("spawn test"),
        Command::Spawn {
            name: "test".into()
        }
    );
    assert_eq!(parse("watchdog"), Command::Watchdog);
    assert_eq!(parse("phase"), Command::Phase);
    assert_eq!(parse("topo"), Command::Topo);
    assert_eq!(parse("root"), Command::Root);
    assert_eq!(parse("audit"), Command::Audit);
    assert_eq!(parse("checkpoint"), Command::Checkpoint);
    assert_eq!(parse("top -n 3"), Command::Top { frames: 3 });
}

#[test]
fn dashboard_output_contains_roots() {
    let hosted = booted();
    let text = dashboard::render(&hosted.kernel);
    assert!(text.contains("Golden Phase"));
    assert!(text.contains("EquationRoot"));
    assert!(text.contains("KernelStateRoot"));
}

#[test]
fn top_lists_pid_and_phi_state() {
    let hosted = booted();
    let text = top::render(&hosted.kernel, &HashMap::new(), TopOptions { frames: 1 });
    assert!(text.contains("PID"));
    assert!(text.contains("Kairos2"));
}

#[test]
fn top_includes_phase_totals() {
    let hosted = booted();
    let text = top::render(&hosted.kernel, &HashMap::new(), TopOptions { frames: 1 });
    assert!(text.contains("Kairos0 sleep/free"));
    assert!(text.contains("Kairos1 ready/user"));
    assert!(text.contains("Kairos2 running/active"));
    assert!(text.contains("Kairos3 kernel/locked"));
}

#[test]
fn watchdog_output_contains_audit_fields() {
    let hosted = booted();
    let text = watchdog_view::render(&hosted.kernel);
    assert!(text.contains("EquationRoot"));
    assert!(text.contains("KernelStateRoot"));
    assert!(text.contains("Statechain audit"));
}

#[test]
fn watchdog_reports_alert_on_tamper() {
    let mut hosted = booted();
    hosted
        .kernel
        .statechain
        .set_kernel_root_for_audit_test([9u8; 32]);
    let view = watchdog_view::analyze(&hosted.kernel);
    assert!(view.alert);
    let text = watchdog_view::render(&hosted.kernel);
    assert!(text.contains("WATCHDOG ALERT"));
}

#[test]
fn phase_output_includes_subsystem_counts() {
    let mut hosted = booted();
    hosted.kernel.alloc_page();
    let text = phase_view::render(&hosted.kernel);
    assert!(text.contains("processes:"));
    assert!(text.contains("memory pages:"));
    assert!(text.contains("fs inodes:"));
    assert!(text.contains("fs blocks:"));
}

#[test]
fn topo_empty_and_nonempty() {
    let hosted = booted();
    assert_eq!(topo_view::render(&hosted.kernel), "no topology events");

    let mut with_reflection = booted();
    let pid = with_reflection.init_pid.unwrap();
    with_reflection.kernel.syscall(pid).unwrap();
    with_reflection.kernel.kreturn(pid).unwrap();
    with_reflection.kernel.schedule(pid).unwrap();
    let text = topo_view::render(&with_reflection.kernel);
    assert!(text.contains("reflection"));
}

#[test]
fn root_audit_checkpoint_commands() {
    let mut hosted = booted();
    let pid = hosted.init_pid.unwrap();
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    let root = root_view::render(&hosted.kernel);
    assert!(root.contains("EquationRoot"));
    assert!(root.contains("KernelStateRoot"));

    let audit = audit_cmd::render(&hosted.kernel);
    assert!(audit.contains("audit PASS") || audit.contains("audit FAIL"));

    let cp = checkpoint_cmd::render(&mut hosted.kernel).expect("seal");
    assert!(cp.contains("checkpoint sealed"));
    assert!(cp.contains("class: INTEGRITY_ONLY"));
    assert!(cp.contains("mac: HMAC-SHA256"));
    assert!(cp.contains("hmac:"));
    assert!(!cp.contains("signature:"));
    assert!(cp.contains("sequence:"));
}

#[test]
fn shell_terminal_commands_integrate() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    for cmd in ["dashboard", "top", "watchdog", "phase", "ps", "mem"] {
        hosted.execute_line(pid, cmd).unwrap();
    }
    let joined = hosted.output_lines().join("\n");
    assert!(joined.contains("Golden Phase"));
    assert!(joined.contains("Kairos2"));
    assert!(joined.contains("running"));
}

#[test]
fn splash_is_readable() {
    use kairos::userland::splash::{self, BootSplashInfo};
    assert_eq!(
        splash::render_boot(),
        "Golden Phase Operating System boot splash"
    );
    let text = splash::render_with_equation();
    assert!(text.contains("Golden Phase"));
    assert!(text.contains("y^4"));
    let mut kernel = kairos::Kernel::new();
    let pid = kernel.spawn_process("init");
    let screen = splash::render_boot_screen(&BootSplashInfo::from_kernel(&kernel, pid, true));
    assert!(screen.contains("gphase_core"));
    assert!(screen.contains("phasefs"));
    assert!(screen.contains("statechain"));
    assert!(screen.contains("hosted loopback"));
    assert!(!screen.contains("loopback/UART"));
}
