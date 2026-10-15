//! VM instruction set — algebra ops plus control flow, memory, and kernel traps.

use crate::core::gphase_core::PhaseState;

/// Phase register file: A, B, C, D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Register {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
}

impl Register {
    pub const ALL: [Register; 4] = [Register::A, Register::B, Register::C, Register::D];

    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(Register::A),
            1 => Some(Register::B),
            2 => Some(Register::C),
            3 => Some(Register::D),
            _ => None,
        }
    }

    pub const fn as_usize(self) -> usize {
        self as usize
    }
}

/// One VM instruction. Branch targets are program indices (resolved labels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Load {
        reg: Register,
        imm: PhaseState,
    },
    Compose {
        ra: Register,
        rb: Register,
        rc: Register,
    },
    Reflect {
        ra: Register,
        rb: Register,
    },
    Check {
        ra: Register,
        rb: Register,
        rc: Register,
    },
    Threshold {
        ra: Register,
        rb: Register,
    },
    Energy {
        ra: Register,
        rb: Register,
    },
    Print {
        reg: Register,
    },
    Halt,
    Jmp {
        target: usize,
    },
    Jz {
        reg: Register,
        target: usize,
    },
    Jnz {
        reg: Register,
        target: usize,
    },
    Call {
        target: usize,
    },
    Ret,
    Push {
        reg: Register,
    },
    Pop {
        reg: Register,
    },
    LoadMem {
        reg: Register,
        addr: u16,
    },
    StoreMem {
        addr: u16,
        reg: Register,
    },
    Syscall {
        nr: u32,
    },
    Yield,
    Exit,
    /// Copy one memory byte into the VM byte-output buffer.
    EmitMem {
        addr: u16,
    },
}

/// Canonical bytecode encoding used by PhaseStack `CodeHash`.
/// Layout: `u32 LE instruction count` followed by opcode+operands.
/// Branch targets are `u32` program indices, not assembler labels.
pub fn encode_program(program: &[Instruction]) -> crate::platform::Vec<u8> {
    let mut out = crate::platform::Vec::new();
    out.extend_from_slice(&(program.len() as u32).to_le_bytes());
    for insn in program {
        encode_instruction(&mut out, insn);
    }
    out
}

/// Inverse of [`encode_program`]. Rejects unknown opcodes and truncated input.
pub fn decode_program(bytes: &[u8]) -> Result<crate::platform::Vec<Instruction>, ProgramCodecError> {
    if bytes.len() < 4 {
        return Err(ProgramCodecError::Truncated);
    }
    let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let mut off = 4usize;
    let mut out = crate::platform::Vec::new();
    for _ in 0..count {
        out.push(decode_instruction(bytes, &mut off)?);
    }
    if off != bytes.len() {
        return Err(ProgramCodecError::TrailingBytes);
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramCodecError {
    Truncated,
    UnknownOpcode,
    BadRegister,
    BadImmediate,
    TrailingBytes,
}

fn encode_instruction(out: &mut crate::platform::Vec<u8>, insn: &Instruction) {
    match insn {
        Instruction::Load { reg, imm } => {
            out.push(0);
            out.push(*reg as u8);
            out.push(imm.as_u8());
        }
        Instruction::Compose { ra, rb, rc } => {
            out.push(1);
            out.push(*ra as u8);
            out.push(*rb as u8);
            out.push(*rc as u8);
        }
        Instruction::Reflect { ra, rb } => {
            out.push(2);
            out.push(*ra as u8);
            out.push(*rb as u8);
        }
        Instruction::Check { ra, rb, rc } => {
            out.push(3);
            out.push(*ra as u8);
            out.push(*rb as u8);
            out.push(*rc as u8);
        }
        Instruction::Threshold { ra, rb } => {
            out.push(4);
            out.push(*ra as u8);
            out.push(*rb as u8);
        }
        Instruction::Energy { ra, rb } => {
            out.push(5);
            out.push(*ra as u8);
            out.push(*rb as u8);
        }
        Instruction::Print { reg } => {
            out.push(6);
            out.push(*reg as u8);
        }
        Instruction::Halt => out.push(7),
        Instruction::Jmp { target } => {
            out.push(8);
            out.extend_from_slice(&(*target as u32).to_le_bytes());
        }
        Instruction::Jz { reg, target } => {
            out.push(9);
            out.push(*reg as u8);
            out.extend_from_slice(&(*target as u32).to_le_bytes());
        }
        Instruction::Jnz { reg, target } => {
            out.push(10);
            out.push(*reg as u8);
            out.extend_from_slice(&(*target as u32).to_le_bytes());
        }
        Instruction::Call { target } => {
            out.push(11);
            out.extend_from_slice(&(*target as u32).to_le_bytes());
        }
        Instruction::Ret => out.push(12),
        Instruction::Push { reg } => {
            out.push(13);
            out.push(*reg as u8);
        }
        Instruction::Pop { reg } => {
            out.push(14);
            out.push(*reg as u8);
        }
        Instruction::LoadMem { reg, addr } => {
            out.push(15);
            out.push(*reg as u8);
            out.extend_from_slice(&addr.to_le_bytes());
        }
        Instruction::StoreMem { addr, reg } => {
            out.push(16);
            out.extend_from_slice(&addr.to_le_bytes());
            out.push(*reg as u8);
        }
        Instruction::Syscall { nr } => {
            out.push(17);
            out.extend_from_slice(&nr.to_le_bytes());
        }
        Instruction::Yield => out.push(18),
        Instruction::Exit => out.push(19),
        Instruction::EmitMem { addr } => {
            out.push(20);
            out.extend_from_slice(&addr.to_le_bytes());
        }
    }
}

fn take<'a>(bytes: &'a [u8], off: &mut usize, n: usize) -> Result<&'a [u8], ProgramCodecError> {
    let end = off.checked_add(n).ok_or(ProgramCodecError::Truncated)?;
    if end > bytes.len() {
        return Err(ProgramCodecError::Truncated);
    }
    let slice = &bytes[*off..end];
    *off = end;
    Ok(slice)
}

fn take_reg(bytes: &[u8], off: &mut usize) -> Result<Register, ProgramCodecError> {
    let b = take(bytes, off, 1)?[0];
    Register::from_u8(b).ok_or(ProgramCodecError::BadRegister)
}

fn decode_instruction(
    bytes: &[u8],
    off: &mut usize,
) -> Result<Instruction, ProgramCodecError> {
    let op = take(bytes, off, 1)?[0];
    match op {
        0 => {
            let reg = take_reg(bytes, off)?;
            let imm = PhaseState::from_u8(take(bytes, off, 1)?[0])
                .ok_or(ProgramCodecError::BadImmediate)?;
            Ok(Instruction::Load { reg, imm })
        }
        1 => Ok(Instruction::Compose {
            ra: take_reg(bytes, off)?,
            rb: take_reg(bytes, off)?,
            rc: take_reg(bytes, off)?,
        }),
        2 => Ok(Instruction::Reflect {
            ra: take_reg(bytes, off)?,
            rb: take_reg(bytes, off)?,
        }),
        3 => Ok(Instruction::Check {
            ra: take_reg(bytes, off)?,
            rb: take_reg(bytes, off)?,
            rc: take_reg(bytes, off)?,
        }),
        4 => Ok(Instruction::Threshold {
            ra: take_reg(bytes, off)?,
            rb: take_reg(bytes, off)?,
        }),
        5 => Ok(Instruction::Energy {
            ra: take_reg(bytes, off)?,
            rb: take_reg(bytes, off)?,
        }),
        6 => Ok(Instruction::Print {
            reg: take_reg(bytes, off)?,
        }),
        7 => Ok(Instruction::Halt),
        8 => {
            let target = u32::from_le_bytes(take(bytes, off, 4)?.try_into().unwrap()) as usize;
            Ok(Instruction::Jmp { target })
        }
        9 => {
            let reg = take_reg(bytes, off)?;
            let target = u32::from_le_bytes(take(bytes, off, 4)?.try_into().unwrap()) as usize;
            Ok(Instruction::Jz { reg, target })
        }
        10 => {
            let reg = take_reg(bytes, off)?;
            let target = u32::from_le_bytes(take(bytes, off, 4)?.try_into().unwrap()) as usize;
            Ok(Instruction::Jnz { reg, target })
        }
        11 => {
            let target = u32::from_le_bytes(take(bytes, off, 4)?.try_into().unwrap()) as usize;
            Ok(Instruction::Call { target })
        }
        12 => Ok(Instruction::Ret),
        13 => Ok(Instruction::Push {
            reg: take_reg(bytes, off)?,
        }),
        14 => Ok(Instruction::Pop {
            reg: take_reg(bytes, off)?,
        }),
        15 => {
            let reg = take_reg(bytes, off)?;
            let addr = u16::from_le_bytes(take(bytes, off, 2)?.try_into().unwrap());
            Ok(Instruction::LoadMem { reg, addr })
        }
        16 => {
            let addr = u16::from_le_bytes(take(bytes, off, 2)?.try_into().unwrap());
            let reg = take_reg(bytes, off)?;
            Ok(Instruction::StoreMem { addr, reg })
        }
        17 => {
            let nr = u32::from_le_bytes(take(bytes, off, 4)?.try_into().unwrap());
            Ok(Instruction::Syscall { nr })
        }
        18 => Ok(Instruction::Yield),
        19 => Ok(Instruction::Exit),
        20 => {
            let addr = u16::from_le_bytes(take(bytes, off, 2)?.try_into().unwrap());
            Ok(Instruction::EmitMem { addr })
        }
        _ => Err(ProgramCodecError::UnknownOpcode),
    }
}
