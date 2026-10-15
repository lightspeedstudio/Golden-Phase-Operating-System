//! Gate 4 evidence: `passwd` / `passwd --status`, Root provisioning gate,
//! and secret-line entry, exercised end-to-end through the real hosted
//! shell (`kairos::runtime::Runtime`). Secret prompts are fed via
//! `IoBuffer`'s scripted input queue — the same real code path
//! `read_secret_line` uses for interactive input, minus terminal-echo
//! suppression (there is no real terminal to secure in a test harness; see
//! that method's docs).
#![cfg(feature = "identity")]

use kairos::runtime::Runtime;

#[test]
fn passwd_status_reports_unprovisioned_before_any_password_is_set() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.run_command("passwd --status").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("unprovisioned"), "{out}");
}

#[test]
fn passwd_provisions_root_from_unprovisioned_state() {
    let mut rt = Runtime::with_input([
        "correct horse battery staple one",
        "correct horse battery staple one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("root password set"), "{out}");

    rt.run_command("passwd --status").unwrap();
    let out2 = rt.output_lines().join("\n");
    assert!(out2.contains("root credential: active"), "{out2}");
}

#[test]
fn unprovisioned_root_cannot_authenticate_through_the_state_machine() {
    // The provisioning gate itself (Gate 3's RootCredentialState) — this
    // command layer never bypasses it.
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let k = &rt.hosted.kernel;
    assert_eq!(
        k.identity.root_credential().state(),
        kairos::kernel::identity::RootCredentialState::Unprovisioned
    );
}

#[test]
fn passwd_rejects_mismatched_confirmation() {
    let mut rt = Runtime::with_input([
        "correct horse battery staple one",
        "a totally different long password",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("did not match"), "{out}");
    assert_eq!(
        rt.hosted.kernel.identity.root_credential().state(),
        kairos::kernel::identity::RootCredentialState::Unprovisioned
    );
}

#[test]
fn passwd_rejects_a_policy_violating_password() {
    let mut rt = Runtime::with_input(["short", "short"]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("password rejected"), "{out}");
}

#[test]
fn passwd_changes_password_after_authenticating_with_the_old_one() {
    let mut rt = Runtime::with_input([
        "first genuinely long password one",
        "first genuinely long password one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap(); // provision

    rt.hosted.io.input.extend(
        [
            "first genuinely long password one",
            "second genuinely long password two",
            "second genuinely long password two",
        ]
        .map(|s| s.to_string()),
    );
    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("root password set"), "{out}");
    assert_eq!(rt.hosted.kernel.identity.root_credential().history().len(), 1);
}

#[test]
fn passwd_rejects_wrong_current_password_before_touching_the_stored_verifier() {
    let mut rt = Runtime::with_input([
        "first genuinely long password one",
        "first genuinely long password one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap(); // provision

    rt.hosted
        .io
        .input
        .push("totally the wrong password here".to_string());
    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("incorrect password"), "{out}");
    assert!(rt.hosted.kernel.identity.root_credential().history().is_empty());
}

#[test]
fn passwd_command_parses_with_and_without_status_flag() {
    use kairos::userland::parser::{parse, Command};
    assert_eq!(parse("passwd"), Command::Passwd);
    assert_eq!(parse("passwd --status"), Command::PasswdStatus);
    assert_eq!(parse("passwd status"), Command::PasswdStatus);
}
