use kairos::core::gphase_core::{compose, PhaseState};
use kairos::isa::{
    assemble, assemble_instructions, decode, decode_program, disassemble_instruction,
    disassemble_program, encode, encode_program, DecodeError, OPCODE_LOAD,
};
use kairos::vm::{Cpu, Instruction, Register, Trap};

#[test]
fn encode_decode_round_trip_all_opcodes() {
    let samples = [
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi2,
        },
        Instruction::Compose {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Reflect {
            ra: Register::A,
            rb: Register::B,
        },
        Instruction::Check {
            ra: Register::A,
            rb: Register::B,
            rc: Register::C,
        },
        Instruction::Threshold {
            ra: Register::A,
            rb: Register::B,
        },
        Instruction::Energy {
            ra: Register::A,
            rb: Register::B,
        },
        Instruction::Print { reg: Register::D },
        Instruction::Halt,
        Instruction::Jmp { target: 4 },
        Instruction::Jz {
            reg: Register::A,
            target: 2,
        },
        Instruction::Call { target: 8 },
        Instruction::Ret,
        Instruction::Push { reg: Register::A },
        Instruction::Pop { reg: Register::A },
        Instruction::LoadMem {
            reg: Register::B,
            addr: 0x20,
        },
        Instruction::StoreMem {
            addr: 0x10,
            reg: Register::C,
        },
        Instruction::Syscall { nr: 3 },
        Instruction::Yield,
        Instruction::Exit,
    ];
    for instr in samples {
        let word = encode(&instr).unwrap();
        let back = decode(word).unwrap();
        assert_eq!(back, instr);
    }
}

#[test]
fn bad_opcode_fails_decode() {
    assert!(matches!(decode(20 << 27), Err(DecodeError::BadOpcode(20))));
}

#[test]
fn assemble_disassemble_round_trip_text() {
    let src = r"
        load a, phi1
        load b, phi2
        compose a, b, c
        print c
        halt
    ";
    let words = assemble(src).unwrap();
    let lines = disassemble_program(&words).unwrap();
    let words2 = assemble(&lines.join("\n")).unwrap();
    assert_eq!(words, words2);
}

#[test]
fn labels_resolve_correctly() {
    let src = r"
        jmp done
        load a, phi0
    done:
        halt
    ";
    let instrs = assemble_instructions(src).unwrap();
    assert!(matches!(instrs[0], Instruction::Jmp { target: 2 }));
    assert!(matches!(
        instrs[1],
        Instruction::Load {
            reg: Register::A,
            imm: PhaseState::Phi0
        }
    ));
    assert!(matches!(instrs[2], Instruction::Halt));
}

#[test]
fn encoded_program_runs_like_handwritten() {
    let src = r"
        load a, phi1
        load b, phi2
        compose a, b, c
        halt
    ";
    let instrs = assemble_instructions(src).unwrap();

    let mut cpu_asm = Cpu::new(256);
    cpu_asm.run(&instrs, 100).unwrap();

    let handwritten = vec![
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
    ];
    let mut cpu_hw = Cpu::new(256);
    cpu_hw.run(&handwritten, 100).unwrap();

    assert_eq!(cpu_asm.get(Register::C), cpu_hw.get(Register::C));
    assert_eq!(
        cpu_asm.get(Register::C),
        compose(PhaseState::Phi1, PhaseState::Phi2)
    );
}

#[test]
fn syscall_word_layout_matches_fpga_decoder() {
    let word = encode(&Instruction::Load {
        reg: Register::A,
        imm: PhaseState::Phi2,
    })
    .unwrap();
    assert_eq!(word >> 27, OPCODE_LOAD);
    assert_eq!((word >> 19) & 0b11, 2);
}

#[test]
fn assemble_loop_and_yield_trap() {
    let src = r"
    loop:
        yield
        jmp loop
    ";
    let instrs = assemble_instructions(src).unwrap();
    let mut cpu = Cpu::new(256);
    cpu.run(&instrs, 3).unwrap();
    assert_eq!(cpu.trap, Some(Trap::Yield));
}

#[test]
fn encode_program_matches_decode_program() {
    let program = vec![Instruction::Exit];
    let words = encode_program(&program).unwrap();
    let back = decode_program(&words).unwrap();
    assert_eq!(back, program);
}

#[test]
fn disassemble_instruction_is_canonical() {
    let line = disassemble_instruction(&Instruction::Load {
        reg: Register::A,
        imm: PhaseState::Phi2,
    });
    assert_eq!(line, "load a, phi2");
}
