use crate::instruction::{Instruction, OpCode, Operand};

pub fn fib_program() -> Vec<Instruction> {
    vec![
        // r0 = 0, r1 = 1, r2 = 0, r2 is the index of the fibonacci number to calculate
        Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(0),
                Operand::Immediate(0),
                Operand::Register(0),
                Operand::Register(0),
            ],
        },
        Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(1),
                Operand::Immediate(1),
                Operand::Register(0),
                Operand::Register(0),
            ],
        },
        Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(2),
                Operand::Immediate(0),
                Operand::Register(0),
                Operand::Register(0),
            ],
        },
        Instruction {
            opcode: OpCode::Gt,
            operands: [
                Operand::Register(3),
                Operand::Register(2),
                Operand::Immediate(10),
                Operand::Register(0),
            ], // r3 = r2 > 10
        },
        Instruction {
            opcode: OpCode::JumpIf,
            operands: [
                Operand::Register(3),
                Operand::Immediate(10),
                Operand::Register(0),
                Operand::Register(0),
            ], // jump to halt when we are done
        },
        // if we're not done, calculate the next fibonacci number
        // make a copy of r0 into r3
        Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(3),
                Operand::Register(0),
                Operand::Register(0),
                Operand::Register(0),
            ], // r3 = r0
        },
        // r0 = r1 + r0 (the new fibonacci number)
        Instruction {
            opcode: OpCode::Add,
            operands: [
                Operand::Register(0),
                Operand::Register(1),
                Operand::Register(3),
                Operand::Register(0),
            ], // r0 = r1 + r3
        },
        // r1 = r3 (the old fibonacci number)
        Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(1),
                Operand::Register(3),
                Operand::Register(0),
                Operand::Register(0),
            ], // r1 = r3
        },
        // increment the index
        Instruction {
            opcode: OpCode::Add,
            operands: [
                Operand::Register(2),
                Operand::Register(2),
                Operand::Immediate(1),
                Operand::Register(0),
            ], // r2 = r2 + 1
        },
        Instruction {
            opcode: OpCode::Jump,
            operands: [
                Operand::Immediate(3),
                Operand::Register(0),
                Operand::Register(0),
                Operand::Register(0),
            ], // jump to the start of the loop
        },
        Instruction {
            opcode: OpCode::Halt,
            operands: [
                Operand::Register(0),
                Operand::Register(0),
                Operand::Register(0),
                Operand::Register(0),
            ],
        },
    ]
}
