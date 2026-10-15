//! Gate 5 evidence: the identity ledger, per-category Merkle roots, and
//! `AuthorityStateRoot` are exercised end-to-end through the real hosted
//! shell (`kairos::runtime::Runtime`) driving the actual `passwd` command —
//! not a synthetic direct call into `kernel::identity::Identity`. Also
//! proves the resulting `AuthorityStateRoot` is really checkpointed into
//! the real `StatechainLedger`.
#![cfg(feature = "identity")]

use kairos::kernel::identity::{
    IdentityEventKind, LedgerError, PasswordPolicy, IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID,
};
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::state_root::SubsystemKind;
use kairos::runtime::Runtime;

#[test]
fn provisioning_root_through_passwd_records_a_real_ledger_event() {
    let mut rt = Runtime::with_input([
        "correct horse battery staple one",
        "correct horse battery staple one",
    ]);
    rt.boot().unwrap();

    let before_head = rt.hosted.kernel.identity.ledger().chain_head();
    let before_state_root = rt.hosted.kernel.identity.authority_state_root();
    assert_eq!(rt.hosted.kernel.identity.ledger().sequence(), 0);

    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("root password set"), "{out}");

    let ledger = rt.hosted.kernel.identity.ledger();
    assert_eq!(ledger.sequence(), 1);
    assert_ne!(ledger.chain_head(), before_head);
    assert_eq!(ledger.events()[0].kind, IdentityEventKind::CredentialProvisioned);
    assert_eq!(
        rt.hosted.kernel.identity.authority_state_root(),
        kairos::kernel::identity::ledger::compute_authority_state_root(
            rt.hosted.kernel.identity.authority_root(),
            rt.hosted.kernel.identity.ledger(),
        )
    );
    assert_ne!(
        rt.hosted.kernel.identity.authority_state_root(),
        before_state_root
    );
}

#[test]
fn changing_root_password_through_passwd_records_a_second_ledger_event() {
    let mut rt = Runtime::with_input([
        "first genuinely long password one",
        "first genuinely long password one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap(); // provision
    assert_eq!(rt.hosted.kernel.identity.ledger().sequence(), 1);

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

    let ledger = rt.hosted.kernel.identity.ledger();
    assert_eq!(ledger.sequence(), 2);
    assert_eq!(ledger.events()[1].kind, IdentityEventKind::CredentialChanged);
    assert_eq!(ledger.verify(), Ok(()));
}

#[test]
fn wrong_current_password_does_not_record_a_ledger_event() {
    let mut rt = Runtime::with_input([
        "first genuinely long password one",
        "first genuinely long password one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap(); // provision
    assert_eq!(rt.hosted.kernel.identity.ledger().sequence(), 1);

    rt.hosted
        .io
        .input
        .push("totally the wrong password here".to_string());
    rt.run_command("passwd").unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("incorrect password"), "{out}");
    // A merely-wrong password must not itself grow the ledger (would let an
    // attacker grow it unbounded purely by guessing) — see
    // `Identity::authenticate_root_credential` docs.
    assert_eq!(rt.hosted.kernel.identity.ledger().sequence(), 1);
}

#[test]
fn repeated_failed_authentication_locks_root_and_records_exactly_one_event() {
    let mut rt = Runtime::with_input([
        "first genuinely long password one",
        "first genuinely long password one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap(); // provision, sequence == 1

    // `passwd` itself rate-limits inside the lockout window and never
    // reaches `maximum_failures` on the default policy (delay > 0, Chronos
    // does not advance between scripted attempts). Drive the lock
    // transition through the real kernel wrapper with a zero-delay policy
    // so every failed attempt is actually counted.
    rt.hosted.kernel.identity.set_password_policy(PasswordPolicy {
        maximum_failures: 2,
        initial_delay_ticks: 0,
        maximum_delay_ticks: 0,
        ..PasswordPolicy::default()
    });
    let policy = *rt.hosted.kernel.identity.password_policy();
    let _ = rt
        .hosted
        .kernel
        .identity_authenticate_root_credential(&policy, b"wrong once", 0);
    let _ = rt
        .hosted
        .kernel
        .identity_authenticate_root_credential(&policy, b"wrong twice", 1);

    assert_eq!(
        rt.hosted.kernel.identity.root_credential().state(),
        kairos::kernel::identity::RootCredentialState::Locked
    );
    let ledger = rt.hosted.kernel.identity.ledger();
    assert_eq!(ledger.sequence(), 2, "provision + exactly one lock event");
    assert_eq!(ledger.events()[1].kind, IdentityEventKind::CredentialLocked);
}

#[test]
fn identity_ledger_is_checkpointed_into_the_real_statechain() {
    let mut rt = Runtime::with_input([
        "correct horse battery staple one",
        "correct horse battery staple one",
    ]);
    rt.boot().unwrap();
    let kernel_root_before = rt.hosted.kernel.statechain.kernel_root();

    rt.run_command("passwd").unwrap();

    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Syscall, IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID)
        .expect("identity checkpoint chain must exist after a real identity mutation");
    assert!(
        chain
            .events
            .iter()
            .any(|e| e.proof.operation_name == "identity_authority_state_checkpoint"),
        "expected an identity_authority_state_checkpoint event, got {:?}",
        chain
            .events
            .iter()
            .map(|e| e.proof.operation_name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(chain.events.last().unwrap().subsystem, SubsystemKind::Syscall);

    let kernel_root_after = rt.hosted.kernel.statechain.kernel_root();
    assert_ne!(
        kernel_root_before, kernel_root_after,
        "checkpointing the identity ledger must move the real KernelRoot"
    );
}

#[test]
fn ledger_verify_accepts_a_real_multi_op_chain_built_through_the_shell() {
    let mut rt = Runtime::with_input([
        "first genuinely long password one",
        "first genuinely long password one",
    ]);
    rt.boot().unwrap();
    rt.run_command("passwd").unwrap();
    rt.hosted.io.input.extend(
        [
            "first genuinely long password one",
            "second genuinely long password two",
            "second genuinely long password two",
        ]
        .map(|s| s.to_string()),
    );
    rt.run_command("passwd").unwrap();

    let ledger = rt.hosted.kernel.identity.ledger();
    assert_eq!(ledger.sequence(), 2);
    let verify_result: Result<(), LedgerError> = ledger.verify();
    assert_eq!(verify_result, Ok(()));
}
