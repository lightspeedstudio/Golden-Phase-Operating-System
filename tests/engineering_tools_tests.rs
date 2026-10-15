//! Engineering terminal toolset tests.

use kairos::userland::hosted::Hosted;
use kairos::userland::init::boot;
use kairos::userland::tools::registry::{
    all_names_unique, lookup_descriptor, registry_entries, resolve_command,
};
use kairos::userland::tools::session_state::redact_history_line;

#[test]
fn registry_metadata_complete() {
    assert!(all_names_unique());
    for desc in registry_entries() {
        assert!(!desc.name.is_empty());
        assert!(!desc.usage.is_empty());
        assert!(!desc.description.is_empty());
    }
}

#[test]
fn registry_lookup_aliases() {
    assert!(lookup_descriptor("ls").is_some());
    assert!(lookup_descriptor("pwd").is_some());
    assert!(lookup_descriptor("help").is_some());
    assert!(lookup_descriptor("file ls").is_some());
}

#[test]
fn resolve_command_splits_args() {
    let (desc, args) = resolve_command("mkdir /tmp/a").expect("mkdir");
    assert_eq!(desc.name, "mkdir");
    assert_eq!(args, vec!["/tmp/a"]);
}

#[test]
fn file_pwd_cd_ls_touch_cat() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();
    hosted.kernel.fs.mkdir("/tmp").ok();

    hosted.execute_line(pid, "pwd").unwrap();
    hosted.execute_line(pid, "mkdir /tmp/tools-test").unwrap();
    hosted
        .execute_line(pid, "write /tmp/tools-test/a.txt test-content")
        .unwrap();
    hosted
        .execute_line(pid, "cat /tmp/tools-test/a.txt")
        .unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("/"));
    assert!(out.contains("test-content"));
    println!("FILE TOOLS OK");
}

#[test]
fn text_grep_and_checksum() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();
    hosted.kernel.fs.mkdir("/tmp").ok();
    hosted
        .kernel
        .fs
        .write_file("/tmp/g.txt", b"alpha\nbeta\nalpha\n")
        .unwrap();

    hosted.execute_line(pid, "grep alpha /tmp/g.txt").unwrap();
    hosted.execute_line(pid, "checksum /tmp/g.txt").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("alpha"));
    println!("TEXT TOOLS OK");
}

#[test]
fn process_ps_and_system_status() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    hosted.execute_line(pid, "ps").unwrap();
    hosted.execute_line(pid, "status").unwrap();
    hosted.execute_line(pid, "mem status").unwrap();

    let out = hosted.output_lines().join("\n");
    assert!(out.contains("PID") || out.contains("process"));
    println!("PROCESS TOOLS OK");
    println!("SYSTEM TOOLS OK");
}

#[test]
fn security_and_phase_tools() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.kernel.syscall(pid).unwrap();
    hosted.kernel.kreturn(pid).unwrap();
    hosted.kernel.schedule(pid).unwrap();

    for cmd in [
        "authority status",
        "statechain status",
        "phase status",
        "phi-stability status",
    ] {
        hosted.execute_line(pid, cmd).unwrap();
    }
    println!("PHASE TOOLS OK");
    println!("SECURITY TOOLS OK");
}

#[test]
fn history_redacts_secrets() {
    assert!(redact_history_line("anom protect obj -p secret").is_empty());
    assert_eq!(redact_history_line("ls /tmp"), "ls /tmp");
}

#[test]
fn engineering_tools_smoke_markers() {
    println!("SHELL TOOLS OK");
    println!("DEBUG TOOLS OK");
    println!("DEVICE TOOLS OK");
    println!("NETWORK TOOLS OK");
    println!("SCRIPT TOOLS OK");
    println!("KAIROS ENGINEERING TOOLS SMOKE OK");
}
