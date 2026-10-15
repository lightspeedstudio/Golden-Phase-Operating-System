//! isa — static instruction set reference.

use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub struct IsaEntry {
    pub name: &'static str,
    pub opcode: u32,
    pub args: &'static str,
    pub description: &'static str,
}

pub const INSTRUCTION_TABLE: &[IsaEntry] = &[
    IsaEntry {
        name: "LOAD",
        opcode: 0,
        args: "reg, phase",
        description: "load immediate phase into register",
    },
    IsaEntry {
        name: "COMPOSE",
        opcode: 1,
        args: "ra, rb, rc",
        description: "rc = compose(ra, rb)",
    },
    IsaEntry {
        name: "REFLECT",
        opcode: 2,
        args: "ra, rb",
        description: "rb = reflect(ra)",
    },
    IsaEntry {
        name: "CHECK",
        opcode: 3,
        args: "ra, rb, rc",
        description: "topology check on register triple",
    },
    IsaEntry {
        name: "THRESHOLD",
        opcode: 4,
        args: "ra, rb",
        description: "threshold sign compare",
    },
    IsaEntry {
        name: "ENERGY",
        opcode: 5,
        args: "ra, rb",
        description: "threshold energy compare",
    },
    IsaEntry {
        name: "PRINT",
        opcode: 6,
        args: "reg",
        description: "print register phase to output",
    },
    IsaEntry {
        name: "HALT",
        opcode: 7,
        args: "",
        description: "stop VM execution",
    },
    IsaEntry {
        name: "JMP",
        opcode: 8,
        args: "target",
        description: "unconditional branch",
    },
    IsaEntry {
        name: "JZ",
        opcode: 9,
        args: "reg, target",
        description: "branch if register is Phi0",
    },
    IsaEntry {
        name: "JNZ",
        opcode: 10,
        args: "reg, target",
        description: "branch if register is not Phi0",
    },
    IsaEntry {
        name: "CALL",
        opcode: 11,
        args: "target",
        description: "push return address and jump",
    },
    IsaEntry {
        name: "RET",
        opcode: 12,
        args: "",
        description: "pop return address and jump",
    },
    IsaEntry {
        name: "PUSH",
        opcode: 13,
        args: "reg",
        description: "push register to stack",
    },
    IsaEntry {
        name: "POP",
        opcode: 14,
        args: "reg",
        description: "pop stack into register",
    },
    IsaEntry {
        name: "LOAD_MEM",
        opcode: 15,
        args: "reg, addr",
        description: "load phase from memory",
    },
    IsaEntry {
        name: "STORE_MEM",
        opcode: 16,
        args: "addr, reg",
        description: "store phase to memory",
    },
    IsaEntry {
        name: "SYSCALL",
        opcode: 17,
        args: "nr",
        description: "trap to kernel syscall",
    },
    IsaEntry {
        name: "YIELD",
        opcode: 18,
        args: "",
        description: "yield CPU to scheduler",
    },
    IsaEntry {
        name: "EXIT",
        opcode: 19,
        args: "",
        description: "exit user program",
    },
];

pub fn render_table() -> String {
    let mut out = String::from("Golden Phase Operating System instruction set:\n");
    out.push_str("NAME         OPC  ARGS              DESCRIPTION\n");
    for entry in INSTRUCTION_TABLE {
        out.push_str(&format!(
            "{:<12} {:>3}  {:<16} {}\n",
            entry.name, entry.opcode, entry.args, entry.description
        ));
    }
    out
}

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&render_table())?;
    Ok(())
}

pub fn get_instruction_table() -> &'static [IsaEntry] {
    INSTRUCTION_TABLE
}
