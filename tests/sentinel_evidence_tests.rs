use kairos::kernel::identity::{
    AuthenticationMethod, ModOperation, SessionOrigin, ROOT_PRINCIPAL_ID,
};
use kairos::kernel::sentinel::event::{EventKind, SentinelEvent};
use kairos::kernel::sentinel::{evidence::MAX_EVIDENCE, SentinelDecision};
use kairos::userland::init::boot;
use kairos::userland::Hosted;

#[test]
fn evidence_ring_counts_drops() {
    let mut hosted = Hosted::new();
    for i in 0..(MAX_EVIDENCE as u32 + 3) {
        hosted.kernel.sentinel.record_decision(
            SentinelEvent {
                kind: EventKind::Syscall,
                pid: i,
                nr: 0,
                from: None,
                to: None,
                content: None,
            },
            SentinelDecision::Allow,
        );
    }
    assert_eq!(hosted.kernel.sentinel.evidence.len(), MAX_EVIDENCE);
    assert_eq!(hosted.kernel.sentinel.evidence.drop_count(), 3);
}

#[test]
fn statechain_records_sentinel_seal() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    hosted.kernel.sentinel_quarantine_task(caller).unwrap();
    let chain = hosted.kernel.statechain.chain(
        kairos::kernel::state_proof::ObjectKind::Syscall,
        kairos::kernel::sentinel::SENTINEL_EVIDENCE_OBJECT_ID,
    );
    assert!(chain.is_some());
    let ops: Vec<_> = chain
        .unwrap()
        .events
        .iter()
        .map(|e| e.proof.operation_name.as_str())
        .collect();
    assert!(ops.contains(&"sentinel_evidence_checkpoint"));
}

#[test]
fn sentinel_status_requires_root_or_mod() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.execute_line(pid, "sentinel status").unwrap();
    let out = hosted.output_lines().join("\n");
    assert!(out.contains("unauthorized"));
}

#[test]
fn sentinel_status_with_root_session() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    let boot_root = hosted.kernel.authority.boot_session_root;
    let tick = hosted.kernel.statechain.tick();
    hosted
        .kernel
        .identity
        .open_session(
            ROOT_PRINCIPAL_ID,
            SessionOrigin::Local,
            AuthenticationMethod::Password,
            boot_root,
            tick,
            pid,
        )
        .unwrap();
    hosted.execute_line(pid, "sentinel status").unwrap();
    let out = hosted.output_lines().join("\n");
    assert!(out.contains("sentinel observe="));
}

#[test]
fn sentinel_cert_pin_requires_ticket() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    let boot_root = hosted.kernel.authority.boot_session_root;
    let tick = hosted.kernel.statechain.tick();
    let session = hosted
        .kernel
        .identity
        .open_session(
            ROOT_PRINCIPAL_ID,
            SessionOrigin::Local,
            AuthenticationMethod::Password,
            boot_root,
            tick,
            pid,
        )
        .unwrap();
    hosted.execute_line(pid, "sentinel cert pin 00").unwrap();
    let out = hosted.output_lines().join("\n");
    assert!(out.contains("unauthorized"));
    hosted
        .kernel
        .identity_issue_mod_ticket(session.session_id, ModOperation::SentinelCertInstall, 64)
        .unwrap();
    let pk = "11".repeat(32);
    hosted
        .execute_line(pid, &format!("sentinel cert pin {pk}"))
        .unwrap();
    let out = hosted.output_lines().join("\n");
    assert!(out.contains("trust anchor pinned"));
}
