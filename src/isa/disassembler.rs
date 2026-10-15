//! Instruction words → canonical assembly text.

use crate::isa::encode::{decode, DecodeError};
use crate::platform::{String, Vec};
use crate::vm::instruction::{Instruction, Register};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisasmError {
    Decode(DecodeError),
}

impl From<DecodeError> for DisasmError {
    fn from(value: DecodeError) -> Self {
        DisasmError::Decode(value)
    }
}

impl fmt::Display for DisasmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisasmError::Decode(e) => write!(f, "{e:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DisasmError {}

fn reg_name(reg: Register) -> &'static str {
    match reg {
        Register::A => "a",
        Register::B => "b",
        Register::C => "c",
        Register::D => "d",
    }
}

pub fn disassemble_instruction(instr: &Instruction) -> String {
    match instr {
        Instruction::Load { reg, imm } => {
            format!("load {}, phi{}", reg_name(*reg), imm.as_u8())
        }
        Instruction::Compose { ra, rb, rc } => {
            format!(
                "compose {}, {}, {}",
                reg_name(*ra),
                reg_name(*rb),
                reg_name(*rc)
            )
        }
        Instruction::Reflect { ra, rb } => {
            format!("reflect {}, {}", reg_name(*ra), reg_name(*rb))
        }
        Instruction::Check { ra, rb, rc } => {
            format!(
                "check {}, {}, {}",
                reg_name(*ra),
                reg_name(*rb),
                reg_name(*rc)
            )
        }
        Instruction::Threshold { ra, rb } => {
            format!("threshold {}, {}", reg_name(*ra), reg_name(*rb))
        }
        Instruction::Energy { ra, rb } => {
            format!("energy {}, {}", reg_name(*ra), reg_name(*rb))
        }
        Instruction::Print { reg } => format!("print {}", reg_name(*reg)),
        Instruction::Halt => "halt".into(),
        Instruction::Jmp { target } => format!("jmp {target}"),
        Instruction::Jz { reg, target } => format!("jz {}, {target}", reg_name(*reg)),
        Instruction::Jnz { reg, target } => format!("jnz {}, {target}", reg_name(*reg)),
        Instruction::Call { target } => format!("call {target}"),
        Instruction::Ret => "ret".into(),
        Instruction::Push { reg } => format!("push {}", reg_name(*reg)),
        Instruction::Pop { reg } => format!("pop {}", reg_name(*reg)),
        Instruction::LoadMem { reg, addr } => {
            format!("loadmem {}, 0x{addr:x}", reg_name(*reg))
        }
        Instruction::StoreMem { addr, reg } => {
            format!("storemem 0x{addr:x}, {}", reg_name(*reg))
        }
        Instruction::Syscall { nr } => format!("syscall {nr}"),
        Instruction::Yield => "yield".into(),
        Instruction::Exit => "exit".into(),
        Instruction::EmitMem { addr } => format!("emitmem 0x{addr:x}"),
    }
}

pub fn disassemble_word(word: u32) -> Result<String, DisasmError> {
    Ok(disassemble_instruction(&decode(word)?))
}

pub fn disassemble_program(words: &[u32]) -> Result<Vec<String>, DisasmError> {
    words.iter().map(|&w| disassemble_word(w)).collect()
}
