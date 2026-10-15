use kairos::core::gphase_core::{
    compose, reflect, threshold_energy, threshold_sign, topology_status, PhaseState, TopologyKind,
};
use kairos::vm::{Cpu, CpuMode, Instruction, Register, Trap};

fn run(program: &[Instruction]) -> Cpu {
    let mut cpu = Cpu::new(256);
    cpu.run(program, 10_000).unwrap();
    cpu
}

#[test]
fn load_sets_register() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::A), PhaseState::Phi2);
}

#[test]
fn compose_uses_core_algebra() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Load {
            reg: Register::B,
            imm: PhaseState::Phi2,
        },
        Instruction::Compose {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Halt,
    ]);
    assert_eq!(
        cpu.get(Register::C),
        compose(PhaseState::Phi1, PhaseState::Phi2)
    );
}

#[test]
fn reflect_uses_core_algebra() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi3,
        },
        Instruction::Reflect {
            ra: Register::A,
            rb: Register::B,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::B), reflect(PhaseState::Phi3));
}

#[test]
fn check_logs_topology_without_mutation() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Load {
            reg: Register::B,
            imm: PhaseState::Phi2,
        },
        Instruction::Load {
            reg: Register::C,
            imm: PhaseState::Phi3,
        },
        Instruction::Check {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Halt,
    ]);
    assert_eq!(
        (
            cpu.get(Register::A),
            cpu.get(Register::B),
            cpu.get(Register::C)
        ),
        (PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3)
    );
    assert_eq!(cpu.topology_log.len(), 1);
    let entry = &cpu.topology_log[0];
    let expected = topology_status(PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3);
    assert_eq!(entry.kind, expected.kind);
    assert_eq!(entry.left, expected.left);
    assert_eq!(entry.right, expected.right);
}

#[test]
fn threshold_and_energy_match_core() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Threshold {
            ra: Register::A,
            rb: Register::B,
        },
        Instruction::Energy {
            ra: Register::A,
            rb: Register::C,
        },
        Instruction::Halt,
    ]);
    assert_eq!(
        cpu.get(Register::B).as_u8(),
        threshold_sign(PhaseState::Phi2)
    );
    assert_eq!(
        cpu.get(Register::C).as_u8(),
        threshold_energy(PhaseState::Phi2)
    );
}

#[test]
fn print_and_halt() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Print { reg: Register::A },
        Instruction::Halt,
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi3,
        },
    ]);
    assert_eq!(cpu.output, vec![PhaseState::Phi2]);
    assert!(cpu.halted);
    assert_eq!(cpu.get(Register::A), PhaseState::Phi2);
}

#[test]
fn jmp_branches() {
    let cpu = run(&[
        Instruction::Jmp { target: 2 },
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi3,
        },
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::A), PhaseState::Phi1);
}

#[test]
fn jz_and_jnz() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi0,
        },
        Instruction::Jz {
            reg: Register::A,
            target: 4,
        },
        Instruction::Load {
            reg: Register::B,
            imm: PhaseState::Phi3,
        },
        Instruction::Jmp { target: 6 },
        Instruction::Load {
            reg: Register::B,
            imm: PhaseState::Phi1,
        },
        Instruction::Jnz {
            reg: Register::A,
            target: 2,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::B), PhaseState::Phi1);
}

#[test]
fn call_and_ret() {
    let cpu = run(&[
        Instruction::Call { target: 2 },
        Instruction::Halt,
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Ret,
    ]);
    assert_eq!(cpu.get(Register::A), PhaseState::Phi2);
}

#[test]
fn push_pop_round_trip() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Push { reg: Register::A },
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi0,
        },
        Instruction::Pop { reg: Register::A },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::A), PhaseState::Phi2);
}

#[test]
fn load_mem_and_store_mem() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi3,
        },
        Instruction::StoreMem {
            addr: 10,
            reg: Register::A,
        },
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi0,
        },
        Instruction::LoadMem {
            reg: Register::A,
            addr: 10,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::A), PhaseState::Phi3);
    assert_eq!(cpu.memory[10], 3);
}

#[test]
fn syscall_enters_kernel_mode_and_traps() {
    let mut cpu = Cpu::new(256);
    cpu.run(&[Instruction::Syscall { nr: 42 }, Instruction::Halt], 100)
        .unwrap();
    assert_eq!(cpu.trap, Some(Trap::Syscall(42)));
    assert_eq!(cpu.mode, CpuMode::Kernel);
}

#[test]
fn yield_and_exit_traps() {
    let mut cpu = Cpu::new(256);
    cpu.run(&[Instruction::Yield], 100).unwrap();
    assert_eq!(cpu.trap, Some(Trap::Yield));

    cpu.reset();
    cpu.run(&[Instruction::Exit], 100).unwrap();
    assert_eq!(cpu.trap, Some(Trap::Exit));
    assert!(cpu.halted);
}

#[test]
fn instruction_trace_records_execution() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.trace.len(), 2);
    assert_eq!(cpu.trace[0].pc, 0);
    assert_eq!(cpu.trace[1].pc, 1);
}

#[test]
fn kernel_round_trip_logs_reflection() {
    let cpu = run(&[
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Load {
            reg: Register::B,
            imm: PhaseState::Phi2,
        },
        Instruction::Compose {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Check {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Halt,
    ]);
    assert_eq!(cpu.get(Register::C), PhaseState::Phi3);
    assert_eq!(cpu.topology_log.len(), 1);
    assert_eq!(cpu.topology_log[0].kind, TopologyKind::Reflection);
}

#[test]
fn user_program_loops_yields_and_exits_with_syscall() {
    const LOOP: usize = 1;

    let program = [
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi1,
        },
        Instruction::Yield,
        Instruction::Jnz {
            reg: Register::A,
            target: LOOP,
        },
        Instruction::Syscall { nr: 1 },
        Instruction::Exit,
    ];

    let mut cpu = Cpu::new(256);
    let mut kernel = kairos::Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    kernel.schedule(pid).unwrap();

    let mut yields = 0;
    let mut syscall_handled = false;

    while !matches!(cpu.trap, Some(Trap::Exit)) {
        cpu.run(&program, 100).unwrap();

        match cpu.trap.clone() {
            Some(Trap::Yield) => {
                yields += 1;
                kernel.preempt(pid).unwrap();
                kernel.schedule(pid).unwrap();
                if yields >= 2 {
                    cpu.set(Register::A, PhaseState::Phi0);
                }
                cpu.clear_trap();
            }
            Some(Trap::Syscall(nr)) => {
                assert_eq!(nr, 1);
                kernel.syscall(pid).unwrap();
                kernel.kreturn(pid).unwrap();
                syscall_handled = true;
                cpu.resume_from_syscall();
            }
            Some(Trap::Exit) => break,
            None => break,
        }
    }

    kernel.schedule(pid).unwrap();
    kernel.syscall(pid).unwrap();
    kernel.exit(pid).unwrap();

    assert_eq!(yields, 2);
    assert!(syscall_handled);
    assert_eq!(kernel.get_process(pid).unwrap().state, PhaseState::Phi0);
}
