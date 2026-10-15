//! Host Gates S0–S12 for PhaseSentinel-gated signed launch.

use super::capability::GrantSet;
use super::cert::is_retired_research_ca;
use super::install;
use super::ticket::VerifiedLaunchTicket;
use super::types::{AppState, InstallScope, PHASEVM_ENTRY, PHASEVM_MAX_MEM};
use super::verifier::tests::signed_gpa;
use crate::kernel::captable::{RIGHT_READ, RIGHT_WRITE};
use crate::kernel::crypto::phase_hash::ph_sentinel_launch_receipt;
use crate::kernel::process::VALID_TRANSITIONS;
use crate::kernel::sentinel::{LiveLaunchView, SentinelLaunchDecision, SentinelRefusal};
use crate::kernel::Kernel;

fn allow_policy(kernel: &mut Kernel) {
    kernel.sentinel.policy.max_rights = u32::MAX;
}

fn install_ok(kernel: &mut Kernel) -> crate::kernel::phasestack::types::AppId {
    allow_policy(kernel);
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    let inst = install::install(kernel, &staged, grants, InstallScope::User).unwrap();
    inst.package().app_id()
}

fn view_from(kernel: &Kernel, id: crate::kernel::phasestack::types::AppId) -> LiveLaunchView {
    let inst = kernel.phasestack.registry.get(id).unwrap();
    super::run::live_view(kernel, inst)
}

#[test]
fn s0_ticket_only_from_installation() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap();
    let t = VerifiedLaunchTicket::from_installation(inst, [1u8; 16]);
    assert_eq!(t.package_id(), id);
    assert_eq!(t.payload_root(), inst.package().code_hash());
}

#[test]
fn s1_valid_signed_launch_allows() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let result = install::launch(&mut kernel, id, 64).unwrap();
    assert!(result.pid > 0 || result.pid == 0);
    assert!(result.retired_instructions >= 1);
    assert_eq!(
        kernel.phasestack.registry.get(id).unwrap().state(),
        AppState::Running
    );
}

#[test]
fn s2_registry_state_refuses() {
    let mut kernel = Kernel::new();
    assert_eq!(
        install::launch(&mut kernel, [0u8; 32], 8),
        Err(super::types::PhaseStackError::RegistryMissing)
    );
    let id = install_ok(&mut kernel);
    install::revoke(&mut kernel, id).unwrap();
    assert_eq!(
        install::launch(&mut kernel, id, 8),
        Err(super::types::PhaseStackError::Revoked)
    );
}

#[test]
fn s2_quarantined_refuses() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    kernel
        .phasestack
        .registry
        .set_state(id, AppState::Quarantined)
        .unwrap();
    assert_eq!(
        install::launch(&mut kernel, id, 8),
        Err(super::types::PhaseStackError::Quarantined)
    );
}

#[test]
fn s3_root_binding() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    let ticket = VerifiedLaunchTicket::from_installation(&inst, [2u8; 16]);
    let live = view_from(&kernel, id);
    let bad_payload = ticket.clone().with_payload_root([0x11u8; 32]);
    match kernel.sentinel.authorize_verified_ticket(&bad_payload, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::PayloadRootMismatch) => {}
        other => panic!("expected PayloadRootMismatch, got {}", other.as_str()),
    }
    let bad_manifest = ticket.with_manifest_root([0x22u8; 32]);
    match kernel.sentinel.authorize_verified_ticket(&bad_manifest, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::ManifestRootMismatch) => {}
        other => panic!("expected ManifestRootMismatch, got {}", other.as_str()),
    }
}

#[test]
fn s4_signer_lifecycle() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    let ticket = VerifiedLaunchTicket::from_installation(&inst, [3u8; 16]);
    let mut live = view_from(&kernel, id);
    live.signer_known = false;
    match kernel.sentinel.authorize_verified_ticket(&ticket, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::SignerUnknown) => {}
        other => panic!("{}", other.as_str()),
    }
    live.signer_known = true;
    live.signer_retired = true;
    match kernel.sentinel.authorize_verified_ticket(&ticket, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::SignerRetired) => {}
        other => panic!("{}", other.as_str()),
    }
    assert!(is_retired_research_ca(
        &crate::kernel::phasestack::cert::RETIRED_RESEARCH_CA_PIN
    ));
}

#[test]
fn s5_capability_subset() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    let ticket = VerifiedLaunchTicket::from_installation(&inst, [4u8; 16]);
    let mut live = view_from(&kernel, id);
    live.policy_caps = RIGHT_READ;
    live.requested_caps = RIGHT_READ;
    let ok = ticket.clone().with_granted_caps(RIGHT_READ);
    let mut live_ok = live;
    live_ok.granted_caps = RIGHT_READ;
    match kernel.sentinel.authorize_verified_ticket(&ok, &live_ok) {
        SentinelLaunchDecision::Allow(_) => {}
        other => panic!("READ/READ should allow: {}", other.as_str()),
    }
    let bad = ticket.clone().with_granted_caps(RIGHT_READ | RIGHT_WRITE);
    match kernel.sentinel.authorize_verified_ticket(&bad, &live_ok) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::CapabilityEscalation) => {}
        other => panic!("{}", other.as_str()),
    }
    live.policy_caps = 0;
    let granted_read = ticket.with_granted_caps(RIGHT_READ);
    match kernel.sentinel.authorize_verified_ticket(&granted_read, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::CapabilityEscalation) => {}
        other => panic!("policy exclude: {}", other.as_str()),
    }
}

#[test]
fn s6_stale_ticket() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    let ticket = VerifiedLaunchTicket::from_installation(&inst, [5u8; 16]);
    let live = view_from(&kernel, id);
    let stale_gen = ticket.clone().with_registry_generation(ticket.registry_generation() + 1);
    match kernel.sentinel.authorize_verified_ticket(&stale_gen, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::RegistryGenerationMismatch) => {}
        other => panic!("{}", other.as_str()),
    }
    let stale_seq = ticket.with_install_sequence(0);
    match kernel.sentinel.authorize_verified_ticket(&stale_seq, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::InstallSequenceMismatch) => {}
        other => panic!("{}", other.as_str()),
    }
}

#[test]
fn s7_permit_replay() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    let ticket = VerifiedLaunchTicket::from_installation(&inst, [6u8; 16]);
    let live = view_from(&kernel, id);
    let permit = match kernel.sentinel.authorize_verified_ticket(&ticket, &live) {
        SentinelLaunchDecision::Allow(p) => p,
        other => panic!("{}", other.as_str()),
    };
    let gen = permit.registry_generation();
    let again = VerifiedLaunchTicket::from_installation(&inst, [6u8; 16]);
    let consumed = kernel.sentinel.consume_permit(permit, gen, false).unwrap();
    assert!(kernel.sentinel.consume_permit(consumed, gen, false).is_err());
    match kernel.sentinel.authorize_verified_ticket(&again, &live) {
        SentinelLaunchDecision::Refuse(SentinelRefusal::NonceReplayed) => {}
        other => panic!("{}", other.as_str()),
    }
}

#[test]
fn s8_toctou_payload_replace() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    let path = format!("{}/code.phase", inst.store_path());
    kernel.fs.unlock_path(&path).unwrap();
    kernel.fs.write_file(&path, b"other-package-bytes").unwrap();
    assert_eq!(
        install::launch(&mut kernel, id, 8),
        Err(super::types::PhaseStackError::PayloadRootMismatch)
    );
}

#[test]
fn s9_revocation() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    install::launch(&mut kernel, id, 64).unwrap();
    let pid = kernel.phasestack.registry.get(id).unwrap().process_id();
    assert!(pid.is_some());
    install::stop(&mut kernel, id).unwrap();
    install::revoke(&mut kernel, id).unwrap();
    let row = kernel.phasestack.registry.get(id).unwrap();
    assert_eq!(row.state(), AppState::Revoked);
    assert_eq!(row.granted_caps(), 0);
    assert!(row.process_id().is_none());
    assert_eq!(
        install::launch(&mut kernel, id, 8),
        Err(super::types::PhaseStackError::Revoked)
    );
}

#[test]
fn s10_legal_transitions() {
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let result = install::launch(&mut kernel, id, 64).unwrap();
    let hist = kernel.get_process(result.pid).unwrap().history.clone();
    for w in hist.windows(2) {
        let allowed = VALID_TRANSITIONS
            .iter()
            .find(|(s, _)| *s == w[0])
            .map(|(_, t)| *t)
            .unwrap();
        assert!(allowed.contains(&w[1]), "illegal {:?} -> {:?}", w[0], w[1]);
    }
}

#[test]
fn s11_evidence_binding() {
    let a = ph_sentinel_launch_receipt(&[&[1u8], &[2u8]]);
    let b = ph_sentinel_launch_receipt(&[&[1u8], &[3u8]]);
    let c = ph_sentinel_launch_receipt(&[&[9u8], &[2u8]]);
    assert_ne!(a, b);
    assert_ne!(a, c);
}

#[test]
fn s12_bounded_tables() {
    use super::types::{MAX_APPS, MAX_NONCES, MAX_PERMITS};
    assert!(MAX_APPS > 0 && MAX_APPS <= 256);
    assert!(MAX_PERMITS > 0 && MAX_NONCES > 0);
    let mut kernel = Kernel::new();
    let id = install_ok(&mut kernel);
    let inst = kernel.phasestack.registry.get(id).unwrap().clone();
    for i in 0..MAX_NONCES {
        let mut nonce = [0u8; 16];
        nonce[0] = (i % 256) as u8;
        nonce[1] = (i / 256) as u8;
        let ticket = VerifiedLaunchTicket::from_installation(&inst, nonce);
        let live = view_from(&kernel, id);
        match kernel.sentinel.authorize_verified_ticket(&ticket, &live) {
            SentinelLaunchDecision::Allow(p) => {
                let gen = p.registry_generation();
                let _ = kernel.sentinel.consume_permit(p, gen, false);
            }
            SentinelLaunchDecision::Isolate(_) => break,
            SentinelLaunchDecision::Refuse(SentinelRefusal::NonceReplayed) => {}
            other => panic!("{}", other.as_str()),
        }
    }
    let ticket = VerifiedLaunchTicket::from_installation(&inst, [0xffu8; 16]);
    let live = view_from(&kernel, id);
    match kernel.sentinel.authorize_verified_ticket(&ticket, &live) {
        SentinelLaunchDecision::Isolate(_) | SentinelLaunchDecision::Refuse(_) => {}
        SentinelLaunchDecision::Allow(_) => panic!("full table must not allow"),
    }
    let _ = PHASEVM_ENTRY;
    let _ = PHASEVM_MAX_MEM;
    let _ = GrantSet::empty();
}
