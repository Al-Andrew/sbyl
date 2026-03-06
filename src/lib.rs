pub mod instruction;
pub mod programs;
pub mod vm;

pub use instruction::{Instruction, OpCode, Operand};
pub use programs::fib_program;
pub use vm::Vm;
