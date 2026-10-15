use kairos::core::gphase_core::{PhaseState, TopologyKind};
use kairos::userland::init::{boot, start_shell};
use kairos::userland::parser::{parse, Command};
use kairos::userland::Hosted;

#[test]
fn parser_recognizes_commands() {
    assert_eq!(parse("ps"), Command::Ps);
    assert_eq!(parse("mem"), Command::Mem);
    assert_eq!(parse("log"), Command::Log);
    assert_eq!(
        parse("run hello"),
        Command::RunProgram {
            name: "hello".into()
        }
    );
    assert_eq!(parse("run"), Command::RunPaste);
    assert_eq!(
        parse("run file foo.k"),
        Command::RunFile {
            filename: "foo.k".into()
        }
    );
    assert_eq!(parse("help"), Command::Help);
    assert_eq!(
        parse("spawn worker"),
        Command::Spawn {
            name: "worker".into()
        }
    );
    assert_eq!(parse("trace"), Command::Trace);
    assert_eq!(parse("isa"), Command::Isa);
    assert_eq!(parse("dump"), Command::Dump);
    assert_eq!(parse("state"), Command::State);
    assert_eq!(parse("version"), Command::Version);
    assert_eq!(parse("kill 3"), Command::Kill { pid: 3 });
    assert_eq!(parse("exit"), Command::Exit);
}

#[test]
fn ps_lists_scheduler_state() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "ps").unwrap();

    let text = hosted.output_lines().last().unwrap();
    assert!(text.contains("running"));
    assert!(text.contains(&format!("{pid}")));
}

#[test]
fn mem_lists_page_tags() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    let page = hosted.kernel.alloc_page();
    hosted.kernel.map_user(page).unwrap();

    hosted.execute_line(pid, "mem").unwrap();

    let text = hosted.output_lines().last().unwrap();
    assert!(text.contains(&format!("{page}\tuser")));
}

#[test]
fn log_shows_reflection_events() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "log").unwrap();

    let text = hosted.output_lines().last().unwrap();
    assert!(text.contains("reflection"));
    assert!(text.contains("triple=(1,2,3)"));
}

#[test]
fn kill_exits_target_legally() {
    let mut hosted = Hosted::new();
    let shell_pid = boot(&mut hosted);
    let child = hosted.kernel.spawn();
    hosted.kernel.wake(child).unwrap();

    hosted
        .execute_line(shell_pid, &format!("kill {child}"))
        .unwrap();

    assert_eq!(
        hosted.kernel.get_process(child).unwrap().state,
        PhaseState::Phi0
    );
}

#[cfg(not(feature = "phasesentinel"))]
#[test]
fn run_records_vm_statechain() {
    use kairos::kernel::state_proof::ObjectKind;
    use kairos::userland::init::boot;
    use kairos::userland::run::run_program;
    use kairos::userland::Hosted;

    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let child = run_program(
        &mut hosted.kernel,
        caller,
        &mut hosted.io,
        &hosted.programs,
        "hello",
        &mut hosted.vms,
    )
    .unwrap();
    let chain = hosted
        .kernel
        .statechain
        .chain(ObjectKind::Vm, child)
        .expect("vm chain");
    let ops: Vec<_> = chain
        .events
        .iter()
        .map(|e| e.proof.operation_name.as_str())
        .collect();
    assert!(ops.contains(&"vm_start"));
    assert!(ops.contains(&"vm_trap_exit"));
    assert!(ops.contains(&"vm_halt"));
}

#[cfg(not(feature = "phasesentinel"))]
#[test]
fn run_loads_asm_program() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);

    hosted.execute_line(pid, "run hello_asm").unwrap();

    let child_pid = hosted.kernel.process_ids().into_iter().max().unwrap();
    assert!(hosted.vms.contains_key(&child_pid));
    assert_eq!(
        hosted.kernel.get_process(child_pid).unwrap().state,
        PhaseState::Phi0
    );
}

#[cfg(not(feature = "phasesentinel"))]
#[test]
fn run_loads_program() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);

    hosted.execute_line(pid, "run hello").unwrap();

    let child_pid = hosted.kernel.process_ids().into_iter().max().unwrap();
    assert!(hosted.vms.contains_key(&child_pid));
    assert_eq!(
        hosted.kernel.get_process(child_pid).unwrap().state,
        PhaseState::Phi0
    );
}

#[test]
fn whoami_reports_root_authority() {
    let mut hosted = Hosted::with_input(["whoami", "ps", "exit"]);
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    start_shell(&mut hosted).unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("authority: root"));
    assert!(out.contains("anom: standby"));
    assert!(out.contains("running"));
}

#[test]
fn boots_init_and_runs_shell_session() {
    let mut hosted = Hosted::with_input(["ps", "mem", "log", "exit"]);
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();
    hosted.kernel.alloc_page();

    start_shell(&mut hosted).unwrap();

    assert_eq!(
        hosted.kernel.get_process(pid).unwrap().state,
        PhaseState::Phi0
    );
    assert!(hosted.output_lines().iter().any(|l| l.contains("gpos> ps")));
    assert!(hosted.output_lines().iter().any(|l| l.contains("running")));
    assert!(hosted.output_lines().iter().any(|l| l.contains("free")));
    let events = hosted.kernel.topology_report();
    assert!(events.iter().any(|e| e.kind == TopologyKind::Reflection));
    assert!(hosted
        .output_lines()
        .iter()
        .any(|l| l.contains("reflection")));
}
