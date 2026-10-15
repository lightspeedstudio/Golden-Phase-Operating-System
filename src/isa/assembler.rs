//! Text → instruction words with label resolution.

use crate::core::gphase_core::PhaseState;
use crate::isa::encode::{encode, EncodeError};
use crate::platform::{HashMap, String, ToString, Vec};
use crate::vm::instruction::{Instruction, Register};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsmError {
    Encode(EncodeError),
    UnknownOpcode(String),
    BadOperand(String),
    UnknownLabel(String),
    DuplicateLabel(String),
}

impl From<EncodeError> for AsmError {
    fn from(value: EncodeError) -> Self {
        AsmError::Encode(value)
    }
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsmError::Encode(e) => write!(f, "encode: {e:?}"),
            AsmError::UnknownOpcode(op) => write!(f, "unknown opcode {op}"),
            AsmError::BadOperand(msg) => write!(f, "bad operand: {msg}"),
            AsmError::UnknownLabel(l) => write!(f, "unknown label {l}"),
            AsmError::DuplicateLabel(l) => write!(f, "duplicate label {l}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AsmError {}

fn parse_reg(token: &str) -> Result<Register, AsmError> {
    match token.to_ascii_lowercase().as_str() {
        "a" => Ok(Register::A),
        "b" => Ok(Register::B),
        "c" => Ok(Register::C),
        "d" => Ok(Register::D),
        _ => Err(AsmError::BadOperand(format!("register {token}"))),
    }
}

fn parse_phase(token: &str) -> Result<PhaseState, AsmError> {
    match token.to_ascii_lowercase().as_str() {
        "phi0" | "kairos0" | "0" => Ok(PhaseState::Phi0),
        "phi1" | "kairos1" | "1" => Ok(PhaseState::Phi1),
        "phi2" | "kairos2" | "2" => Ok(PhaseState::Phi2),
        "phi3" | "kairos3" | "3" => Ok(PhaseState::Phi3),
        _ => Err(AsmError::BadOperand(format!("kairos state {token}"))),
    }
}

fn parse_usize(token: &str) -> Result<usize, AsmError> {
    if let Some(hex) = token.strip_prefix("0x") {
        usize::from_str_radix(hex, 16).map_err(|_| AsmError::BadOperand(token.into()))
    } else {
        token
            .parse()
            .map_err(|_| AsmError::BadOperand(token.into()))
    }
}

fn parse_u16(token: &str) -> Result<u16, AsmError> {
    Ok(parse_usize(token)? as u16)
}

fn split_operands(rest: &str) -> Vec<String> {
    rest.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_line(
    line: &str,
    labels: &HashMap<String, usize>,
) -> Result<Option<Instruction>, AsmError> {
    let line = line.split(';').next().unwrap_or("").trim();
    if line.is_empty() {
        return Ok(None);
    }
    if line.ends_with(':') {
        return Ok(None);
    }

    let mut parts = line.split_whitespace();
    let op = parts
        .next()
        .ok_or_else(|| AsmError::UnknownOpcode(String::new()))?;
    let op_lc = op.to_ascii_lowercase();
    let rest: Vec<String> = parts.map(str::to_string).collect();
    let joined = rest.join(" ");
    let ops = split_operands(&joined);

    let resolve_label = |name: &str| -> Result<usize, AsmError> {
        labels
            .get(&name.to_ascii_lowercase())
            .copied()
            .ok_or_else(|| AsmError::UnknownLabel(name.into()))
    };

    let instr = match op_lc.as_str() {
        "load" => {
            let reg = parse_reg(&ops[0])?;
            let imm = parse_phase(&ops[1])?;
            Instruction::Load { reg, imm }
        }
        "compose" => Instruction::Compose {
            ra: parse_reg(&ops[0])?,
            rb: parse_reg(&ops[1])?,
            rc: parse_reg(&ops[2])?,
        },
        "reflect" => Instruction::Reflect {
            ra: parse_reg(&ops[0])?,
            rb: parse_reg(&ops[1])?,
        },
        "check" => Instruction::Check {
            ra: parse_reg(&ops[0])?,
            rb: parse_reg(&ops[1])?,
            rc: parse_reg(&ops[2])?,
        },
        "threshold" => Instruction::Threshold {
            ra: parse_reg(&ops[0])?,
            rb: parse_reg(&ops[1])?,
        },
        "energy" => Instruction::Energy {
            ra: parse_reg(&ops[0])?,
            rb: parse_reg(&ops[1])?,
        },
        "print" => Instruction::Print {
            reg: parse_reg(&ops[0])?,
        },
        "halt" => Instruction::Halt,
        "jmp" => Instruction::Jmp {
            target: resolve_label(&ops[0])?,
        },
        "jz" => Instruction::Jz {
            reg: parse_reg(&ops[0])?,
            target: resolve_label(&ops[1])?,
        },
        "jnz" => Instruction::Jnz {
            reg: parse_reg(&ops[0])?,
            target: resolve_label(&ops[1])?,
        },
        "call" => Instruction::Call {
            target: resolve_label(&ops[0])?,
        },
        "ret" => Instruction::Ret,
        "push" => Instruction::Push {
            reg: parse_reg(&ops[0])?,
        },
        "pop" => Instruction::Pop {
            reg: parse_reg(&ops[0])?,
        },
        "loadmem" => Instruction::LoadMem {
            reg: parse_reg(&ops[0])?,
            addr: parse_u16(&ops[1])?,
        },
        "storemem" => Instruction::StoreMem {
            addr: parse_u16(&ops[0])?,
            reg: parse_reg(&ops[1])?,
        },
        "syscall" => Instruction::Syscall {
            nr: parse_usize(&ops[0])? as u32,
        },
        "yield" => Instruction::Yield,
        "exit" => Instruction::Exit,
        "emitmem" => Instruction::EmitMem {
            addr: parse_u16(&ops[0])?,
        },
        other => return Err(AsmError::UnknownOpcode(other.into())),
    };
    Ok(Some(instr))
}

pub fn assemble(source: &str) -> Result<Vec<u32>, AsmError> {
    let mut labels: HashMap<String, usize> = HashMap::new();
    let mut pc = 0usize;
    for line in source.lines() {
        let trimmed = line.split(';').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(label) = trimmed.strip_suffix(':') {
            let name = label.trim().to_ascii_lowercase();
            if labels.insert(name.clone(), pc).is_some() {
                return Err(AsmError::DuplicateLabel(name));
            }
            if !trimmed[..trimmed.len() - 1].trim().is_empty() && trimmed.contains(':') {
                // label-only line
                if trimmed.split_whitespace().count() == 1 {
                    continue;
                }
            }
            continue;
        }
        pc += 1;
    }

    let mut program = Vec::new();
    for line in source.lines() {
        if let Some(instr) = parse_line(line, &labels)? {
            program.push(encode(&instr)?);
        }
    }
    Ok(program)
}

pub fn assemble_instructions(source: &str) -> Result<Vec<Instruction>, AsmError> {
    let words = assemble(source)?;
    words
        .iter()
        .map(|w| crate::isa::encode::decode(*w).map_err(|_| AsmError::BadOperand("decode".into())))
        .collect()
}
