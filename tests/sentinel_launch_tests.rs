use kairos::core::gphase_core::PhaseState;
use kairos::kernel::crypto::phase_hash::ph_content;
use kairos::kernel::crypto::sign::SignKeypair;
use kairos::kernel::sentinel::manifest::ProgramManifest;
use kairos::kernel::sentinel::SentinelError;
use kairos::runtime::loader::{Loader, LoaderError};
use kairos::userland::init::boot;
use kairos::userland::run::{run_program, ProgramLoader, RunError};
use kairos::userland::Hosted;
use kairos::vm::{Instruction, Register};

fn hello_program() -> Vec<Instruction> {
    vec![
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Print { reg: Register::A },
        Instruction::Exit,
    ]
}

fn seed() -> [u8; 32] {
    [0x44; 32]
}

fn provision_signed(hosted: &mut Hosted, program: &[Instruction]) {
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
}

#[test]
fn unsigned_run_program_refuses() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let err = run_program(
        &mut hosted.kernel,
        caller,
        &mut hosted.io,
        &hosted.programs,
        "hello",
        &mut hosted.vms,
    )
    .unwrap_err();
    assert!(matches!(err, RunError::Sentinel(SentinelError::Unsigned)));
}

#[test]
fn unsigned_attach_program_refuses() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    let err = Loader::attach_program(&mut hosted, pid, "hello").unwrap_err();
    assert!(matches!(err, LoaderError::Sentinel(SentinelError::Unsigned)));
}

#[test]
fn signed_run_and_attach_both_pass() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let prog = hello_program();
    provision_signed(&mut hosted, &prog);
    hosted.programs.register("hello", prog.clone());
    run_program(
        &mut hosted.kernel,
        caller,
        &mut hosted.io,
        &hosted.programs,
        "hello",
        &mut hosted.vms,
    )
    .unwrap();

    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    provision_signed(&mut hosted, &prog);
    hosted.programs.register("hello", prog);
    Loader::attach_program(&mut hosted, pid, "hello").unwrap();
}

#[test]
fn spawn_is_not_a_hidden_run_path() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    let child = hosted.kernel.spawn();
    assert!(hosted.vms.get(&child).is_none());
    let _ = caller;
}

#[test]
fn paste_and_file_stay_refuse_closed() {
    let mut hosted = Hosted::new();
    let pid = boot(&mut hosted);
    hosted.execute_line(pid, "run").unwrap();
    let out = hosted.output_lines().join("\n");
    assert!(out.contains("unsigned paste refused"));
    hosted.execute_line(pid, "run file x.k").unwrap();
    let out = hosted.output_lines().join("\n");
    assert!(out.contains("unsigned file run refused"));
}

#[test]
fn neither_loader_bypasses_the_other() {
    let mut hosted = Hosted::new();
    let caller = boot(&mut hosted);
    assert!(matches!(
        run_program(
            &mut hosted.kernel,
            caller,
            &mut hosted.io,
            &ProgramLoader::new(),
            "hello",
            &mut hosted.vms,
        ),
        Err(RunError::Sentinel(_))
    ));
    assert!(matches!(
        Loader::attach_program(&mut hosted, caller, "hello"),
        Err(LoaderError::Sentinel(_))
    ));
}
