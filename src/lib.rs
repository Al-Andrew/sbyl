pub mod asm;
pub mod bytecode;
pub mod instruction;
pub mod vm;

pub use asm::{assemble_program, assemble_program_with_context, disassemble_program};
pub use bytecode::{decode_program, encode_program};
pub use instruction::{Instruction, MemoryBase, MemoryRef, OpCode, Operand};
pub use vm::Vm;
