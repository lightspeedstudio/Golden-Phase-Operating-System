//! Isolated PhaseVM runtime wrapping `vm::Cpu`. No host/kernel pointers.

use crate::kernel::phasestack::types::{PhaseStackError, PHASEVM_MAX_MEM, LAUNCH_NONCE_LEN, NONCE_MEM_OFFSET};
use crate::platform::Vec;
use crate::vm::cpu::{Cpu, VmError};
use crate::vm::instruction::{decode_program, Instruction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecReport {
    pub retired: usize,
    pub output: Vec<u8>,
    pub byte_output: Vec<u8>,
}

pub struct PhaseVmRuntime {
    cpu: Cpu,
    program: Vec<Instruction>,
}

impl PhaseVmRuntime {
    pub fn new(code: &[u8], mem_bytes: u32) -> Result<Self, PhaseStackError> {
        if mem_bytes == 0 || mem_bytes > PHASEVM_MAX_MEM {
            return Err(PhaseStackError::ExcessiveMemory);
        }
        let program = decode_program(code).map_err(|_| PhaseStackError::AbiRejected)?;
        Ok(Self {
            cpu: Cpu::new(mem_bytes as usize),
            program,
        })
    }

    pub fn execute(&mut self, fuel: usize) -> Result<ExecReport, PhaseStackError> {
        match self.cpu.run(&self.program, fuel) {
            Ok(()) => {}
            Err(VmError::MaxCyclesExceeded) => return Err(PhaseStackError::FuelAbort),
            Err(VmError::InvalidMemoryAccess { .. }) | Err(VmError::BadProgramIndex { .. }) => {
                return Err(PhaseStackError::VmFault)
            }
            Err(_) => return Err(PhaseStackError::VmFault),
        }
        let retired = self.cpu.trace.len();
        if retired == 0 {
            return Err(PhaseStackError::NoInstruction);
        }
        let output: Vec<u8> = self.cpu.output.iter().map(|p| p.as_u8()).collect();
        Ok(ExecReport {
            retired,
            output,
            byte_output: self.cpu.byte_output.clone(),
        })
    }

    pub fn inject_launch_nonce(&mut self, nonce: &[u8; LAUNCH_NONCE_LEN]) -> Result<(), PhaseStackError> {
        if self.cpu.memory.len() < NONCE_MEM_OFFSET + LAUNCH_NONCE_LEN {
            return Err(PhaseStackError::MemoryEnvelopeInvalid);
        }
        self.cpu.memory[NONCE_MEM_OFFSET..NONCE_MEM_OFFSET + LAUNCH_NONCE_LEN]
            .copy_from_slice(nonce);
        Ok(())
    }

    pub fn cpu_memory_len(&self) -> usize {
        self.cpu.memory.len()
    }
}

pub fn run_plugin(code: &[u8], fuel: usize) -> Result<PhaseVmRuntime, PhaseStackError> {
    let mut rt = PhaseVmRuntime::new(code, PHASEVM_MAX_MEM)?;
    rt.execute(fuel)?;
    Ok(rt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::PhaseState;
    use crate::vm::instruction::{encode_program, Instruction, Register};

    #[test]
    fn halt_executes() {
        let code = encode_program(&[Instruction::Halt]);
        let mut rt = PhaseVmRuntime::new(&code, PHASEVM_MAX_MEM).unwrap();
        let report = rt.execute(16).unwrap();
        assert!(report.retired >= 1);
    }

    #[test]
    fn fuel_abort() {
        let code = encode_program(&[Instruction::Jmp { target: 0 }]);
        let mut rt = PhaseVmRuntime::new(&code, PHASEVM_MAX_MEM).unwrap();
        assert_eq!(rt.execute(8), Err(PhaseStackError::FuelAbort));
    }

    #[test]
    fn oob_is_vm_fault() {
        let code = encode_program(&[Instruction::LoadMem {
            reg: Register::A,
            addr: 5000,
        }]);
        let mut rt = PhaseVmRuntime::new(&code, PHASEVM_MAX_MEM).unwrap();
        assert_eq!(rt.execute(8), Err(PhaseStackError::VmFault));
    }

    #[test]
    fn plugin_memory_is_separate() {
        let code = encode_program(&[Instruction::Halt]);
        let a = PhaseVmRuntime::new(&code, PHASEVM_MAX_MEM).unwrap();
        let b = PhaseVmRuntime::new(&code, PHASEVM_MAX_MEM).unwrap();
        assert_eq!(a.cpu_memory_len(), b.cpu_memory_len());
        assert!(!core::ptr::eq(a.cpu.memory.as_ptr(), b.cpu.memory.as_ptr()));
        let _ = PhaseState::Phi0;
    }

    #[test]
    fn emit_mem_copies_injected_nonce() {
        let mut code = crate::platform::Vec::new();
        for i in 0u16..16 {
            code.push(Instruction::EmitMem { addr: i });
        }
        code.push(Instruction::Halt);
        let bytes = encode_program(&code);
        let mut rt = PhaseVmRuntime::new(&bytes, PHASEVM_MAX_MEM).unwrap();
        let nonce = [0xABu8, 0xCD, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        rt.inject_launch_nonce(&nonce).unwrap();
        let report = rt.execute(32).unwrap();
        assert_eq!(report.byte_output.as_slice(), &nonce);
    }

    #[test]
    fn print_surfaces_in_output() {
        let code = encode_program(&[
            Instruction::Load {
                reg: Register::A,
                imm: PhaseState::Phi1,
            },
            Instruction::Print { reg: Register::A },
            Instruction::Halt,
        ]);
        let mut rt = PhaseVmRuntime::new(&code, PHASEVM_MAX_MEM).unwrap();
        let report = rt.execute(16).unwrap();
        assert_eq!(report.output.as_slice(), &[1]);
        assert!(report.retired >= 3);
    }
}
