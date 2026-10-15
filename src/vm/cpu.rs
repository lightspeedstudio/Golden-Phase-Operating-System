//! Phase CPU — registers, memory, stack, control flow, traps.

use crate::core::gphase_core::{
    compose, reflect, threshold_energy, threshold_sign, topology_status, PhaseState, TopologyKind,
};
use crate::platform::Vec;
use crate::vm::instruction::{Instruction, Register};
use core::fmt;

pub const DEFAULT_MEMORY_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuMode {
    User,
    Kernel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trap {
    Syscall(u32),
    Yield,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    InvalidRegister,
    InvalidMemoryAccess { addr: u16 },
    StackUnderflow,
    StackOverflow,
    EmptyCallStack,
    BadProgramIndex { pc: usize },
    MaxCyclesExceeded,
    Trapped(Trap),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::InvalidRegister => write!(f, "invalid register"),
            VmError::InvalidMemoryAccess { addr } => {
                write!(f, "invalid memory access at {addr}")
            }
            VmError::StackUnderflow => write!(f, "stack underflow"),
            VmError::StackOverflow => write!(f, "stack overflow"),
            VmError::EmptyCallStack => write!(f, "empty call stack"),
            VmError::BadProgramIndex { pc } => write!(f, "bad program index {pc}"),
            VmError::MaxCyclesExceeded => write!(f, "max cycles exceeded"),
            VmError::Trapped(trap) => write!(f, "trapped: {trap:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VmError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyLogEntry {
    pub pc: usize,
    pub triple: (PhaseState, PhaseState, PhaseState),
    pub kind: TopologyKind,
    pub left: PhaseState,
    pub right: PhaseState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub pc: usize,
    pub instruction: Instruction,
}

pub struct Cpu {
    pub registers: [PhaseState; 4],
    pub pc: usize,
    pub sp: usize,
    pub mode: CpuMode,
    pub memory: Vec<u8>,
    pub halted: bool,
    pub trap: Option<Trap>,
    pub output: Vec<PhaseState>,
    pub byte_output: Vec<u8>,
    pub topology_log: Vec<TopologyLogEntry>,
    pub trace: Vec<TraceEntry>,
    call_stack: Vec<usize>,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new(DEFAULT_MEMORY_SIZE)
    }
}

impl Cpu {
    pub fn new(memory_size: usize) -> Self {
        Self {
            registers: [PhaseState::Phi0; 4],
            pc: 0,
            sp: memory_size,
            mode: CpuMode::User,
            memory: vec![0; memory_size],
            halted: false,
            trap: None,
            output: Vec::new(),
            byte_output: Vec::new(),
            topology_log: Vec::new(),
            trace: Vec::new(),
            call_stack: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        let size = self.memory.len();
        *self = Self::new(size);
    }

    pub fn get(&self, reg: Register) -> PhaseState {
        self.registers[reg.as_usize()]
    }

    pub fn set(&mut self, reg: Register, value: PhaseState) {
        self.registers[reg.as_usize()] = value;
    }

    fn push_stack(&mut self, value: PhaseState) -> Result<(), VmError> {
        if self.sp == 0 {
            return Err(VmError::StackOverflow);
        }
        self.sp -= 1;
        self.memory[self.sp] = value.as_u8();
        Ok(())
    }

    fn pop_stack(&mut self) -> Result<PhaseState, VmError> {
        if self.sp >= self.memory.len() {
            return Err(VmError::StackUnderflow);
        }
        let byte = self.memory[self.sp];
        self.sp += 1;
        PhaseState::from_u8(byte).ok_or(VmError::InvalidRegister)
    }

    fn load_mem_byte(&self, addr: u16) -> Result<u8, VmError> {
        let idx = addr as usize;
        if idx >= self.memory.len() {
            return Err(VmError::InvalidMemoryAccess { addr });
        }
        Ok(self.memory[idx])
    }

    fn store_mem_byte(&mut self, addr: u16, value: u8) -> Result<(), VmError> {
        let idx = addr as usize;
        if idx >= self.memory.len() {
            return Err(VmError::InvalidMemoryAccess { addr });
        }
        self.memory[idx] = value;
        Ok(())
    }

    /// Execute one instruction. Updates `pc` unless the opcode branches.
    pub fn step(&mut self, program: &[Instruction]) -> Result<(), VmError> {
        if self.halted {
            return Ok(());
        }
        if self.pc >= program.len() {
            return Ok(());
        }

        let pc = self.pc;
        let instruction = program[pc].clone();
        self.trace.push(TraceEntry {
            pc,
            instruction: instruction.clone(),
        });

        match instruction {
            Instruction::Load { reg, imm } => {
                self.set(reg, imm);
                self.pc += 1;
            }
            Instruction::Compose { ra, rb, rc } => {
                let result = compose(self.get(ra), self.get(rb));
                self.set(rc, result);
                self.pc += 1;
            }
            Instruction::Reflect { ra, rb } => {
                let result = reflect(self.get(ra));
                self.set(rb, result);
                self.pc += 1;
            }
            Instruction::Check { ra, rb, rc } => {
                let a = self.get(ra);
                let b = self.get(rb);
                let c = self.get(rc);
                let result = topology_status(a, b, c);
                self.topology_log.push(TopologyLogEntry {
                    pc,
                    triple: (a, b, c),
                    kind: result.kind,
                    left: result.left,
                    right: result.right,
                });
                self.pc += 1;
            }
            Instruction::Threshold { ra, rb } => {
                let bit = threshold_sign(self.get(ra));
                self.set(rb, PhaseState::from_u8(bit).expect("threshold bit"));
                self.pc += 1;
            }
            Instruction::Energy { ra, rb } => {
                let bit = threshold_energy(self.get(ra));
                self.set(rb, PhaseState::from_u8(bit).expect("energy bit"));
                self.pc += 1;
            }
            Instruction::Print { reg } => {
                self.output.push(self.get(reg));
                self.pc += 1;
            }
            Instruction::Halt => {
                self.halted = true;
            }
            Instruction::Jmp { target } => {
                if target >= program.len() {
                    return Err(VmError::BadProgramIndex { pc: target });
                }
                self.pc = target;
            }
            Instruction::Jz { reg, target } => {
                if self.get(reg) == PhaseState::Phi0 {
                    if target >= program.len() {
                        return Err(VmError::BadProgramIndex { pc: target });
                    }
                    self.pc = target;
                } else {
                    self.pc += 1;
                }
            }
            Instruction::Jnz { reg, target } => {
                if self.get(reg) != PhaseState::Phi0 {
                    if target >= program.len() {
                        return Err(VmError::BadProgramIndex { pc: target });
                    }
                    self.pc = target;
                } else {
                    self.pc += 1;
                }
            }
            Instruction::Call { target } => {
                if target >= program.len() {
                    return Err(VmError::BadProgramIndex { pc: target });
                }
                self.call_stack.push(self.pc + 1);
                self.pc = target;
            }
            Instruction::Ret => {
                let ret = self.call_stack.pop().ok_or(VmError::EmptyCallStack)?;
                if ret >= program.len() {
                    return Err(VmError::BadProgramIndex { pc: ret });
                }
                self.pc = ret;
            }
            Instruction::Push { reg } => {
                self.push_stack(self.get(reg))?;
                self.pc += 1;
            }
            Instruction::Pop { reg } => {
                let value = self.pop_stack()?;
                self.set(reg, value);
                self.pc += 1;
            }
            Instruction::LoadMem { reg, addr } => {
                let byte = self.load_mem_byte(addr)?;
                let value = PhaseState::from_u8(byte).ok_or(VmError::InvalidRegister)?;
                self.set(reg, value);
                self.pc += 1;
            }
            Instruction::StoreMem { addr, reg } => {
                self.store_mem_byte(addr, self.get(reg).as_u8())?;
                self.pc += 1;
            }
            Instruction::Syscall { nr } => {
                self.mode = CpuMode::Kernel;
                self.trap = Some(Trap::Syscall(nr));
                self.pc += 1;
            }
            Instruction::Yield => {
                self.trap = Some(Trap::Yield);
                self.pc += 1;
            }
            Instruction::Exit => {
                self.trap = Some(Trap::Exit);
                self.halted = true;
            }
            Instruction::EmitMem { addr } => {
                const MAX_BYTE_OUTPUT: usize = 32;
                let byte = self.load_mem_byte(addr)?;
                if self.byte_output.len() >= MAX_BYTE_OUTPUT {
                    return Err(VmError::InvalidMemoryAccess { addr });
                }
                self.byte_output.push(byte);
                self.pc += 1;
            }
        }

        Ok(())
    }

    /// Run until halt, trap, or end of program.
    pub fn run(&mut self, program: &[Instruction], max_cycles: usize) -> Result<(), VmError> {
        let mut cycles = 0;
        while self.pc < program.len() && !self.halted && self.trap.is_none() {
            if cycles >= max_cycles {
                return Err(VmError::MaxCyclesExceeded);
            }
            self.step(program)?;
            cycles += 1;
        }
        Ok(())
    }

    /// Clear a yield trap so execution can continue.
    pub fn clear_trap(&mut self) {
        self.trap = None;
    }

    /// Resume after the host handles a syscall trap. Returns to user mode.
    pub fn resume_from_syscall(&mut self) {
        self.mode = CpuMode::User;
        self.trap = None;
    }
}
