//! Phase CPU — Layer 3.

pub mod cpu;
pub mod instruction;

pub use cpu::{Cpu, CpuMode, TopologyLogEntry, TraceEntry, Trap, VmError, DEFAULT_MEMORY_SIZE};
pub use instruction::{Instruction, Register};
