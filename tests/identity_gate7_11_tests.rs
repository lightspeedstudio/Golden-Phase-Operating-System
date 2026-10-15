//! Gates 7-11 evidence: sessions bound to BootSessionRoot, `mod` structured
//! verbs (no `mod shell`), `identity.require` AND-equation, PhaseGuard
//! gating, and negative cases.
#![cfg(feature = "identity")]

use kairos::kernel::identity::{
    AuthorityError, ModOperation, PhaseGuardState, TicketError,
};
use kairos::runtime::Runtime;
use kairos::userland::parser::{parse, Command};

fn provision_and_login(rt: &mut Runtime) -> u32 {
    rt.hosted.io.input.extend(
        [
            "correct horse battery staple one",
            "correct horse battery staple one",
        ]
        .map(|s| s.to_string()),
    );
    rt.run_command("passwd").unwrap();
    rt.hosted
        .io
        .input
        .push("correct horse battery staple one".to_string());
    rt.run_command("login").unwrap();
    1
}

#[test]
fn login_opens_a_session_bound_to_boot_session_root() {
    let mut rt = Runtime::new();
    let pid = rt.boot().unwrap();
    provision_and_login(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(out.contains("login ok"), "{out}");
    let session = rt
        .hosted
        .kernel
        .identity
        .sessions()
        .session_for_process(pid)
        .expect("session attached to caller");
    assert_eq!(
        session.boot_session_root,
        rt.hosted.kernel.authority.boot_session_root
    );
    assert_ne!(session.boot_session_root, [0u8; 32]);
}

#[test]
fn logout_closes_the_session() {
    let mut rt = Runtime::new();
    let pid = rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("logout").unwrap();
    assert!(rt
        .hosted
        .kernel
        .identity
        .sessions()
        .session_for_process(pid)
        .is_none());
}

#[test]
fn mod_shell_is_rejected_by_the_parser() {
    match parse("mod shell") {
        Command::Usage(msg) => assert!(msg.contains("not permitted"), "{msg}"),
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn mod_issues_a_ticket_and_require_accepts_it() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("mod fs.rm --lease 100").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("mod ticket="), "{out}");
    let tickets = rt.hosted.kernel.identity.tickets().active();
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0].op, ModOperation::FsRemove);
    assert!(rt
        .hosted
        .kernel
        .identity_require(tickets[0].ticket_id, ModOperation::FsRemove)
        .is_ok());
}

#[test]
fn require_rejects_the_wrong_operation() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("mod fs.rm").unwrap();
    let id = rt.hosted.kernel.identity.tickets().active()[0].ticket_id;
    assert_eq!(
        rt.hosted
            .kernel
            .identity_require(id, ModOperation::PrincipalDisable),
        Err(AuthorityError::Ticket(TicketError::OperationMismatch))
    );
}

#[test]
fn phaseguard_isolate_blocks_mod_elevation() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.hosted
        .kernel
        .identity
        .set_phase_guard(PhaseGuardState::Isolate);
    rt.run_command("mod fs.rm").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("PhaseGuard") || out.contains("mod failed"), "{out}");
    assert!(rt.hosted.kernel.identity.tickets().active().is_empty());
}

#[test]
fn phaseguard_recover_allows_unlock_but_not_fs_rm() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.hosted
        .kernel
        .identity
        .set_phase_guard(PhaseGuardState::Recover);
    rt.run_command("mod fs.rm").unwrap();
    assert!(rt.hosted.kernel.identity.tickets().active().is_empty());
    rt.run_command("mod credential.unlock").unwrap();
    let tickets = rt.hosted.kernel.identity.tickets().active();
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0].op, ModOperation::CredentialUnlock);
}

#[test]
fn disable_principal_revokes_its_sessions() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    let id = rt
        .hosted
        .kernel
        .identity_create_principal(kairos::kernel::identity::PrincipalKind::Mod)
        .unwrap();
    let boot = rt.hosted.kernel.authority.boot_session_root;
    let tick = rt.hosted.kernel.statechain.tick();
    rt.hosted
        .kernel
        .identity
        .open_session(
            id,
            kairos::kernel::identity::SessionOrigin::Local,
            kairos::kernel::identity::AuthenticationMethod::Password,
            boot,
            tick,
            99,
        )
        .unwrap();
    assert!(rt
        .hosted
        .kernel
        .identity
        .sessions()
        .session_for_process(99)
        .is_some());
    rt.hosted.kernel.identity_disable_principal(id).unwrap();
    assert!(rt
        .hosted
        .kernel
        .identity
        .sessions()
        .session_for_process(99)
        .is_none());
}

#[test]
fn unprovisioned_login_is_denied() {
    let mut rt = Runtime::with_input(["anything at all xx"]);
    rt.boot().unwrap();
    rt.run_command("login").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("login failed"), "{out}");
}

#[test]
fn mod_without_session_is_denied() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.run_command("mod fs.rm").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("no authenticated session"), "{out}");
}

#[test]
fn wrong_password_login_is_denied() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.hosted.io.input.extend(
        [
            "correct horse battery staple one",
            "correct horse battery staple one",
        ]
        .map(|s| s.to_string()),
    );
    rt.run_command("passwd").unwrap();
    rt.hosted
        .io
        .input
        .push("this is a long but wrong password xx".to_string());
    rt.run_command("login").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("login failed"), "{out}");
}

#[test]
fn phaseguard_suspect_blocks_mod_elevation() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.hosted
        .kernel
        .identity
        .set_phase_guard(PhaseGuardState::Suspect);
    rt.run_command("mod fs.rm").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("PhaseGuard") || out.contains("mod failed"), "{out}");
    assert!(rt.hosted.kernel.identity.tickets().active().is_empty());
}

#[test]
fn logout_then_mod_is_denied() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("logout").unwrap();
    rt.run_command("mod fs.rm").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("no authenticated session"), "{out}");
}

#[test]
fn unknown_mod_verb_is_rejected() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("mod not.a.verb").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("unknown operation"), "{out}");
}

#[test]
fn mod_kill_revokes_the_ticket() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("mod fs.rm").unwrap();
    let id = rt.hosted.kernel.identity.tickets().active()[0].ticket_id;
    rt.run_command(&format!("mod -k {id}")).unwrap();
    assert!(rt.hosted.kernel.identity.tickets().active().is_empty());
    let out = rt.output_lines().join("\n");
    assert!(out.contains("revoked"), "{out}");
}

#[test]
fn zero_lease_ticket_expires_after_checkpoint() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    provision_and_login(&mut rt);
    rt.run_command("mod fs.rm --lease 0").unwrap();
    let tickets = rt.hosted.kernel.identity.tickets().active();
    assert_eq!(tickets.len(), 1);
    assert_eq!(
        rt.hosted
            .kernel
            .identity_require(tickets[0].ticket_id, ModOperation::FsRemove),
        Err(AuthorityError::Ticket(TicketError::Expired))
    );
}

#[test]
fn logged_in_rm_without_fs_rm_ticket_is_denied() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.run_command("write /tmp/x hello").unwrap();
    provision_and_login(&mut rt);
    rt.run_command("rm /tmp/x").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("identity"), "{out}");
}

#[test]
fn open_shell_rm_without_a_session_still_works() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.run_command("write /tmp/y hello").unwrap();
    rt.run_command("rm /tmp/y").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(!out.contains("identity"), "{out}");
}

#[test]
fn parse_mod_list_and_verbs_flags() {
    assert_eq!(parse("mod -l"), Command::ModList);
    assert_eq!(parse("mod -v"), Command::ModList);
    assert_eq!(parse("mod --verbs"), Command::ModList);
    match parse("mod fs.rm --lease 12") {
        Command::Mod { verb, lease } => {
            assert_eq!(verb, "fs.rm");
            assert_eq!(lease, 12);
        }
        other => panic!("expected Mod, got {other:?}"),
    }
}

#[test]
fn identity_on_fills_boot_session_root_and_does_not_extend_object_kind() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    assert_ne!(rt.hosted.kernel.authority.boot_session_root, [0u8; 32]);
    assert_eq!(kairos::kernel::state_proof::ObjectKind::Craft.as_u8(), 8);
    assert!(kairos::kernel::state_proof::ObjectKind::from_u8(9).is_none());
    assert_eq!(kairos::kernel::state_root::SubsystemKind::Craft.as_u8(), 6);
    assert!(kairos::kernel::state_root::SubsystemKind::from_u8(7).is_none());
}
