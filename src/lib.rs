pub mod asm;
pub mod bytecode;
pub mod instruction;
pub mod vm;

pub use asm::{assemble_program, disassemble_program};
pub use bytecode::{decode_program, encode_program};
pub use instruction::{Instruction, OpCode, Operand};
pub use vm::Vm;
