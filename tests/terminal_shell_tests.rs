//! New terminal command integration tests.

use kairos::userland::init::boot;
use kairos::userland::parser::{parse, Command};
use kairos::userland::Hosted;

#[test]
fn parser_run_paste_and_file() {
    assert_eq!(parse("run"), Command::RunPaste);
    assert_eq!(parse("net"), Command::NetStatus);
    assert_eq!(parse("net init"), Command::NetInit);
    assert_eq!(parse("net tx-test"), Command::NetTxTest);
    assert_eq!(parse("net rx-test"), Command::NetRxTest);
    assert_eq!(
        parse("run file prog.k"),
        Command::RunFile {
            filename: "prog.k".into()
        }
    );
    assert_eq!(parse("spawn"), Command::Usage("usage: spawn <name>".into()));
    assert_eq!(parse("kill"), Command::Usage("usage: kill <pid>".into()));
}

#[test]
fn help_lists_commands() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "help").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("spawn"));
    assert!(out.contains("trace"));
    assert!(out.contains("isa"));
    assert!(out.contains("net"));
}

#[test]
fn net_status_reports_hosted_loopback_identity() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "net status").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("node: gpos-host"));
    assert!(out.contains("driver: hosted-phaselink-loopback"));
    assert!(out.contains("state: initialized"));
}

#[test]
fn spawn_and_trace() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "spawn worker").unwrap();
    hosted.execute_line(pid, "trace").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("spawned worker"));
    assert!(out.contains("spawn pid="));
}

#[test]
fn run_paste_mode() {
    let mut hosted = Hosted::with_input([
        "LOAD A PHI1",
        "LOAD B PHI1",
        "COMPOSE C A B",
        "PRINT C",
        "HALT",
        ".end",
    ]);
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "run").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("[parser] 5 lines loaded"));
    assert!(out.contains("[2] COMPOSE C A B"));
    assert!(out.contains("[runner] execution not implemented yet"));
}

#[test]
fn regs_without_vm() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "regs").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("register system not initialized"));
}

#[test]
fn dump_includes_sections() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "dump").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("=== dump ==="));
    assert!(out.contains("--- processes ---"));
    assert!(out.contains("--- trace ---"));
}

#[test]
fn about_prints_zen() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "about").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("The Zen of Golden Phase Operating System"));
    assert!(out.contains("State is better than status"));
    assert!(out.contains("Math is the formulation of truth"));
    assert!(out.contains("y^4 - phi*y^2 + alpha = 0"));
}

#[test]
fn help_lists_about_command() {
    assert_eq!(parse("about"), Command::About);
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "help").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("about"));
}

#[test]
fn run_file_stub() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "run file hello.k").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("file runner not implemented yet"));
}
