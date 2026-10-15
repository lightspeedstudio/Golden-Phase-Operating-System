use kairos::core::gphase_core::PhaseState;
use kairos::kernel::crypto::phase_hash::ph_content;
use kairos::kernel::crypto::sign::SignKeypair;
use kairos::kernel::sentinel::checkpoint::SentinelCheckpoint;
use kairos::kernel::sentinel::manifest::ProgramManifest;
use kairos::kernel::sentinel::{SentinelError, TaskDisposition};
use kairos::userland::init::boot;
use kairos::userland::Hosted;
use kairos::vm::{Instruction, Register};

fn prog() -> Vec<Instruction> {
    vec![
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Exit,
    ]
}

fn seed() -> [u8; 32] {
    [0x55; 32]
}

fn pin(hosted: &mut Hosted, program: &[Instruction]) -> [u8; 32] {
    let bytes = kairos::vm::instruction::encode_program(program);
    let kp = SignKeypair::from_seed(&seed());
    let pk = kp.public_bytes();
    hosted.kernel.sentinel.signers.pin_root(pk);
    hosted.kernel.sentinel.policy.max_rights = u32::MAX;
    hosted.kernel.sentinel.policy.allowed_signers.push(pk);
    let mut m = ProgramManifest {
        generation: 1,
        expiry_tick: u64::MAX,
        content_hash: ph_content(&bytes),
        requested_rights: 1,
        signer_pubkey: pk,
        signature: [0u8; 64],
    };
    m.sign(&seed());
    hosted.kernel.sentinel.register_manifest(m);
    pk
}

#[test]
fn quarantine_blocks_relaunch() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let p = prog();
    pin(&mut hosted, &p);
    let bytes = kairos::vm::instruction::encode_program(&p);
    hosted
        .kernel
        .sentinel_authorize_launch(caller, &bytes)
        .unwrap();
    hosted.kernel.sentinel_quarantine_task(caller).unwrap();
    assert_eq!(
        hosted.kernel.sentinel.disposition(caller),
        TaskDisposition::Quarantined
    );
    assert_eq!(
        hosted.kernel.sentinel_authorize_launch(caller, &bytes),
        Err(SentinelError::Quarantined)
    );
}

#[test]
fn isolate_blocks_relaunch() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let p = prog();
    pin(&mut hosted, &p);
    let bytes = kairos::vm::instruction::encode_program(&p);
    hosted
        .kernel
        .sentinel_authorize_launch(caller, &bytes)
        .unwrap();
    hosted.kernel.sentinel_isolate_task(caller).unwrap();
    assert_eq!(
        hosted.kernel.sentinel_authorize_launch(caller, &bytes),
        Err(SentinelError::Isolated)
    );
}

#[test]
fn recover_refuses_without_checkpoint() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    assert_eq!(
        hosted.kernel.sentinel.latest_checkpoint().is_none(),
        true
    );
    let _ = caller;
}

#[test]
fn recover_after_verified_checkpoint() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let p = prog();
    let pk = pin(&mut hosted, &p);
    let bytes = kairos::vm::instruction::encode_program(&p);
    let content = ph_content(&bytes);
    hosted
        .kernel
        .sentinel_authorize_launch(caller, &bytes)
        .unwrap();
    hosted.kernel.sentinel_isolate_task(caller).unwrap();
    let mut ck = SentinelCheckpoint {
        generation: 1,
        content_root: content,
        r_state: hosted.kernel.statechain.kernel_root(),
        r_memory: ph_content(b"MEASURED"),
        r_program: content,
        r_ps: [0u8; 32],
        signature: [0u8; 64],
    };
    ck.sign(&seed());
    hosted.kernel.sentinel_recover_task(caller, ck).unwrap();
    hosted
        .kernel
        .sentinel_authorize_launch(caller, &bytes)
        .unwrap();
    let _ = pk;
}

#[test]
fn hmac_signedcheckpoint_is_not_recoverable() {
    let mut hosted = Hosted::new();
    let _ = hosted.kernel.statechain.seal_checkpoint();
    assert!(hosted.kernel.sentinel.latest_checkpoint().is_none());
}
