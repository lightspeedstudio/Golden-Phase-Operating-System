//! run — load and execute a named phase program on a spawned process.

use crate::core::gphase_core::PhaseState;
use crate::isa::assemble_instructions;
use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::{HashMap, String, ToString, Vec};
use crate::userland::syscall;
use crate::vm::{Cpu, Instruction, Register, Trap};
use core::fmt;

const ASM_HELLO: &str = r"
    load a, phi2
    print a
    exit
";

pub struct ProgramLoader {
    programs: HashMap<String, Vec<Instruction>>,
}

impl Default for ProgramLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramLoader {
    pub fn new() -> Self {
        let mut loader = Self {
            programs: HashMap::new(),
        };
        loader.register_defaults();
        loader
    }

    pub fn register(&mut self, name: impl Into<String>, program: Vec<Instruction>) {
        self.programs.insert(name.into(), program);
    }

    pub fn register_asm(&mut self, name: impl Into<String>, source: &str) -> Result<(), RunError> {
        let program = assemble_instructions(source).map_err(RunError::Asm)?;
        self.register(name, program);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&[Instruction]> {
        self.programs.get(name).map(Vec::as_slice)
    }

    fn register_defaults(&mut self) {
        self.register(
            "hello",
            vec![
                Instruction::Load {
                    reg: Register::A,
                    imm: PhaseState::Phi2,
                },
                Instruction::Print { reg: Register::A },
                Instruction::Exit,
            ],
        );
        let _ = self.register_asm("hello_asm", ASM_HELLO);
    }
}

pub fn run_program(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    loader: &ProgramLoader,
    name: &str,
    vms: &mut HashMap<ProcessId, Cpu>,
) -> Result<ProcessId, RunError> {
    let program = loader
        .get(name)
        .ok_or(RunError::UnknownProgram(name.to_string()))?;
    run_assembled(kernel, caller, io, name, program, vms)
}

pub fn run_assembled(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    name: &str,
    program: &[Instruction],
    vms: &mut HashMap<ProcessId, Cpu>,
) -> Result<ProcessId, RunError> {
    #[cfg(feature = "phasesentinel")]
    {
        let bytes = crate::vm::instruction::encode_program(program);
        kernel
            .sentinel_authorize_launch(caller, &bytes)
            .map_err(RunError::Sentinel)?;
    }

    let pid = syscall::spawn(kernel, caller, io).map_err(RunError::Syscall)?;
    syscall::ensure_running(kernel, caller).map_err(RunError::Syscall)?;
    kernel
        .schedule(pid)
        .map_err(|_| RunError::Kernel("schedule child"))?;

    let mut cpu = Cpu::new(256);
    cpu.run(program, 10_000).map_err(RunError::Vm)?;
    #[cfg(any(feature = "std", feature = "baremetal"))]
    commit_vm_statechain(kernel, pid, name, &cpu);

    if let Some(Trap::Exit) = cpu.trap {
        kernel
            .schedule(pid)
            .map_err(|_| RunError::Kernel("schedule for exit"))?;
        kernel
            .syscall(pid)
            .map_err(|_| RunError::Kernel("syscall for exit"))?;
        kernel
            .exit(pid)
            .map_err(|_| RunError::Kernel("exit child"))?;
    }

    vms.insert(pid, cpu);
    let msg = format!("ran {name} on pid {pid}");
    syscall::ensure_running(kernel, caller).map_err(RunError::Syscall)?;
    syscall::write(kernel, caller, io, &msg).map_err(RunError::Syscall)?;
    Ok(pid)
}

#[cfg(any(feature = "std", feature = "baremetal"))]
fn commit_vm_statechain(kernel: &mut Kernel, vm_id: ProcessId, program: &str, cpu: &Cpu) {
    kernel.record_vm_event(
        vm_id,
        PhaseState::Phi0,
        PhaseState::Phi1,
        "vm_start",
        program.as_bytes(),
    );
    for entry in &cpu.topology_log {
        let payload = [
            entry.triple.0.as_u8(),
            entry.triple.1.as_u8(),
            entry.triple.2.as_u8(),
            entry.kind as u8,
        ];
        kernel.record_vm_event(
            vm_id,
            PhaseState::Phi1,
            PhaseState::Phi1,
            "vm_check",
            &payload,
        );
    }
    if let Some(trap) = &cpu.trap {
        let (op, payload) = match trap {
            Trap::Syscall(nr) => ("vm_trap_syscall", nr.to_le_bytes().to_vec()),
            Trap::Yield => ("vm_trap_yield", vec![]),
            Trap::Exit => ("vm_trap_exit", vec![]),
        };
        kernel.record_vm_event(vm_id, PhaseState::Phi1, PhaseState::Phi3, op, &payload);
    }
    if cpu.halted {
        kernel.record_vm_event(vm_id, PhaseState::Phi1, PhaseState::Phi0, "vm_halt", &[]);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    UnknownProgram(String),
    Asm(crate::isa::AsmError),
    Syscall(crate::kernel::syscall::SyscallError),
    Vm(crate::vm::VmError),
    Kernel(&'static str),
    #[cfg(feature = "phasesentinel")]
    Sentinel(crate::kernel::sentinel::SentinelError),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::UnknownProgram(name) => write!(f, "unknown program {name}"),
            RunError::Asm(err) => write!(f, "asm: {err}"),
            RunError::Syscall(err) => write!(f, "{err}"),
            RunError::Vm(err) => write!(f, "{err}"),
            RunError::Kernel(msg) => write!(f, "kernel: {msg}"),
            #[cfg(feature = "phasesentinel")]
            RunError::Sentinel(err) => write!(f, "{}", err.as_str()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RunError {}
