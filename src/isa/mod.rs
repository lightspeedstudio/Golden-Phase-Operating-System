//! Assembler / disassembler — shared 32-bit ISA with VM and FPGA decoder.

pub mod assembler;
pub mod disassembler;
pub mod encode;

pub use assembler::{assemble, assemble_instructions, AsmError};
pub use disassembler::{
    disassemble_instruction, disassemble_program, disassemble_word, DisasmError,
};
pub use encode::{
    decode, decode_program, encode, encode_program, DecodeError, EncodeError, OPCODE_COMPOSE,
    OPCODE_LOAD,
};
