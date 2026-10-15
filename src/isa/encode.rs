//! Fixed 32-bit instruction word codec — matches `fpga/phase_decoder.v`.

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;
use crate::vm::instruction::{Instruction, Register};

pub const OPCODE_LOAD: u32 = 0;
pub const OPCODE_COMPOSE: u32 = 1;
pub const OPCODE_REFLECT: u32 = 2;
pub const OPCODE_CHECK: u32 = 3;
pub const OPCODE_THRESHOLD: u32 = 4;
pub const OPCODE_ENERGY: u32 = 5;
pub const OPCODE_PRINT: u32 = 6;
pub const OPCODE_HALT: u32 = 7;
pub const OPCODE_JMP: u32 = 8;
pub const OPCODE_JZ: u32 = 9;
pub const OPCODE_JNZ: u32 = 10;
pub const OPCODE_CALL: u32 = 11;
pub const OPCODE_RET: u32 = 12;
pub const OPCODE_PUSH: u32 = 13;
pub const OPCODE_POP: u32 = 14;
pub const OPCODE_LOAD_MEM: u32 = 15;
pub const OPCODE_STORE_MEM: u32 = 16;
pub const OPCODE_SYSCALL: u32 = 17;
pub const OPCODE_YIELD: u32 = 18;
pub const OPCODE_EXIT: u32 = 19;
pub const OPCODE_EMIT_MEM: u32 = 20;

const RESERVED_MASK: u32 = 0x7 << 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    ReservedBitsSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    BadOpcode(u32),
    BadRegister(u8),
    ReservedBitsSet,
}

fn reg_bits(reg: Register) -> u32 {
    reg as u32
}

fn phase_bits(phase: PhaseState) -> u32 {
    phase.as_u8() as u32
}

pub fn encode(instr: &Instruction) -> Result<u32, EncodeError> {
    let word = match instr {
        Instruction::Load { reg, imm } => {
            (OPCODE_LOAD << 27) | (reg_bits(*reg) << 25) | (phase_bits(*imm) << 19)
        }
        Instruction::Compose { ra, rb, rc } => {
            (OPCODE_COMPOSE << 27)
                | (reg_bits(*ra) << 25)
                | (reg_bits(*rb) << 23)
                | (reg_bits(*rc) << 21)
        }
        Instruction::Reflect { ra, rb } => {
            (OPCODE_REFLECT << 27) | (reg_bits(*ra) << 25) | (reg_bits(*rb) << 23)
        }
        Instruction::Check { ra, rb, rc } => {
            (OPCODE_CHECK << 27)
                | (reg_bits(*ra) << 25)
                | (reg_bits(*rb) << 23)
                | (reg_bits(*rc) << 21)
        }
        Instruction::Threshold { ra, rb } => {
            (OPCODE_THRESHOLD << 27) | (reg_bits(*ra) << 25) | (reg_bits(*rb) << 23)
        }
        Instruction::Energy { ra, rb } => {
            (OPCODE_ENERGY << 27) | (reg_bits(*ra) << 25) | (reg_bits(*rb) << 23)
        }
        Instruction::Print { reg } => (OPCODE_PRINT << 27) | (reg_bits(*reg) << 25),
        Instruction::Halt => OPCODE_HALT << 27,
        Instruction::Jmp { target } => (OPCODE_JMP << 27) | (*target as u32 & 0x3f_ffff),
        Instruction::Jz { reg, target } => {
            (OPCODE_JZ << 27) | (reg_bits(*reg) << 25) | (*target as u32 & 0x3f_ffff)
        }
        Instruction::Jnz { reg, target } => {
            (OPCODE_JNZ << 27) | (reg_bits(*reg) << 25) | (*target as u32 & 0x3f_ffff)
        }
        Instruction::Call { target } => (OPCODE_CALL << 27) | (*target as u32 & 0x3f_ffff),
        Instruction::Ret => OPCODE_RET << 27,
        Instruction::Push { reg } => (OPCODE_PUSH << 27) | (reg_bits(*reg) << 25),
        Instruction::Pop { reg } => (OPCODE_POP << 27) | (reg_bits(*reg) << 25),
        Instruction::LoadMem { reg, addr } => {
            (OPCODE_LOAD_MEM << 27) | (reg_bits(*reg) << 25) | (*addr as u32)
        }
        Instruction::StoreMem { addr, reg } => {
            (OPCODE_STORE_MEM << 27) | (reg_bits(*reg) << 25) | (*addr as u32)
        }
        Instruction::Syscall { nr } => (OPCODE_SYSCALL << 27) | (*nr & 0x3f_ffff),
        Instruction::Yield => OPCODE_YIELD << 27,
        Instruction::Exit => OPCODE_EXIT << 27,
        Instruction::EmitMem { addr } => (OPCODE_EMIT_MEM << 27) | (*addr as u32),
    };
    if word & RESERVED_MASK != 0
        && !matches!(
            instr,
            Instruction::LoadMem { .. }
                | Instruction::StoreMem { .. }
                | Instruction::Jmp { .. }
                | Instruction::Jz { .. }
                | Instruction::Jnz { .. }
                | Instruction::Call { .. }
                | Instruction::Syscall { .. }
                | Instruction::EmitMem { .. }
        )
    {
        // reserved bits must be zero for register/immediate ops
        if (word & RESERVED_MASK) != 0 {
            return Err(EncodeError::ReservedBitsSet);
        }
    }
    Ok(word)
}

fn decode_reg(bits: u32) -> Result<Register, DecodeError> {
    Register::from_u8(bits as u8).ok_or(DecodeError::BadRegister(bits as u8))
}

pub fn decode(word: u32) -> Result<Instruction, DecodeError> {
    let opcode = word >> 27;
    let ra = (word >> 25) & 0b11;
    let rb = (word >> 23) & 0b11;
    let rc = (word >> 21) & 0b11;
    let imm = (word >> 19) & 0b11;
    let target = (word & 0x3f_ffff) as usize;
    let addr = (word & 0xffff) as u16;

    let instr = match opcode {
        OPCODE_LOAD => Instruction::Load {
            reg: decode_reg(ra)?,
            imm: PhaseState::from_u8(imm as u8).ok_or(DecodeError::BadRegister(imm as u8))?,
        },
        OPCODE_COMPOSE => Instruction::Compose {
            ra: decode_reg(ra)?,
            rb: decode_reg(rb)?,
            rc: decode_reg(rc)?,
        },
        OPCODE_REFLECT => Instruction::Reflect {
            ra: decode_reg(ra)?,
            rb: decode_reg(rb)?,
        },
        OPCODE_CHECK => Instruction::Check {
            ra: decode_reg(ra)?,
            rb: decode_reg(rb)?,
            rc: decode_reg(rc)?,
        },
        OPCODE_THRESHOLD => Instruction::Threshold {
            ra: decode_reg(ra)?,
            rb: decode_reg(rb)?,
        },
        OPCODE_ENERGY => Instruction::Energy {
            ra: decode_reg(ra)?,
            rb: decode_reg(rb)?,
        },
        OPCODE_PRINT => Instruction::Print {
            reg: decode_reg(ra)?,
        },
        OPCODE_HALT => Instruction::Halt,
        OPCODE_JMP => Instruction::Jmp { target },
        OPCODE_JZ => Instruction::Jz {
            reg: decode_reg(ra)?,
            target,
        },
        OPCODE_JNZ => Instruction::Jnz {
            reg: decode_reg(ra)?,
            target,
        },
        OPCODE_CALL => Instruction::Call { target },
        OPCODE_RET => Instruction::Ret,
        OPCODE_PUSH => Instruction::Push {
            reg: decode_reg(ra)?,
        },
        OPCODE_POP => Instruction::Pop {
            reg: decode_reg(ra)?,
        },
        OPCODE_LOAD_MEM => Instruction::LoadMem {
            reg: decode_reg(ra)?,
            addr,
        },
        OPCODE_STORE_MEM => Instruction::StoreMem {
            addr,
            reg: decode_reg(ra)?,
        },
        OPCODE_SYSCALL => Instruction::Syscall {
            nr: word & 0x3f_ffff,
        },
        OPCODE_YIELD => Instruction::Yield,
        OPCODE_EXIT => Instruction::Exit,
        OPCODE_EMIT_MEM => Instruction::EmitMem { addr },
        other => return Err(DecodeError::BadOpcode(other)),
    };

    if (word & RESERVED_MASK) != 0
        && matches!(
            instr,
            Instruction::Load { .. }
                | Instruction::Compose { .. }
                | Instruction::Reflect { .. }
                | Instruction::Check { .. }
                | Instruction::Threshold { .. }
                | Instruction::Energy { .. }
                | Instruction::Print { .. }
                | Instruction::Halt
                | Instruction::Ret
                | Instruction::Push { .. }
                | Instruction::Pop { .. }
                | Instruction::Yield
                | Instruction::Exit
                | Instruction::EmitMem { .. }
        )
    {
        return Err(DecodeError::ReservedBitsSet);
    }

    Ok(instr)
}

pub fn encode_program(program: &[Instruction]) -> Result<Vec<u32>, EncodeError> {
    program.iter().map(encode).collect()
}

pub fn decode_program(words: &[u32]) -> Result<Vec<Instruction>, DecodeError> {
    words.iter().map(|&w| decode(w)).collect()
}
