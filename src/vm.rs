use crate::instruction::{Instruction, OpCode, Operand};
use anyhow::{Result, bail};

#[derive(Debug)]
pub struct Vm {
    program: Vec<Instruction>,
    registers: Vec<u64>,
    pc: usize,
    halted: bool,
}

impl Vm {
    pub fn new(program: Vec<Instruction>) -> Self {
        Self {
            program,
            registers: Vec::new(),
            pc: 0,
            halted: false,
        }
    }

    pub fn read_operand(&self, operand: Operand) -> u64 {
        match operand {
            Operand::Immediate(value) => value,
            Operand::Register(reg) => {
                let idx = reg as usize;
                if idx < self.registers.len() {
                    self.registers[idx]
                } else {
                    0
                }
            }
        }
    }

    fn write_register(&mut self, reg: usize, value: u64) {
        if reg >= self.registers.len() {
            self.registers.resize(reg + 1, 0);
        }
        self.registers[reg] = value;
    }

    fn destination(operand: Operand, pc: usize, opcode: OpCode) -> Result<usize> {
        match operand {
            Operand::Register(reg) => Ok(reg as usize),
            Operand::Immediate(value) => bail!(
                "vm runtime error at pc {pc} ({opcode:?}): destination operand must be register, got immediate {value}"
            ),
        }
    }

    pub fn step(&mut self) -> Result<()> {
        if self.halted {
            return Ok(());
        }
        if self.pc >= self.program.len() {
            self.halted = true;
            return Ok(());
        }

        let instruction = self.program[self.pc];
        let mut advance_pc = true;

        match instruction.opcode {
            OpCode::Add => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, lhs.wrapping_add(rhs));
            }
            OpCode::Sub => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, lhs.wrapping_sub(rhs));
            }
            OpCode::Mul => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, lhs.wrapping_mul(rhs));
            }
            OpCode::Div => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                if rhs == 0 {
                    bail!("vm runtime error at pc {} (Div): division by zero", self.pc);
                }
                self.write_register(dst, lhs / rhs);
            }
            OpCode::Eq => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs == rhs));
            }
            OpCode::Gt => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs > rhs));
            }
            OpCode::Lt => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs < rhs));
            }
            OpCode::Gte => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs >= rhs));
            }
            OpCode::Lte => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs <= rhs));
            }
            OpCode::Jump => {
                let target = self.read_operand(instruction.operands[0]) as usize;
                self.pc = target;
                advance_pc = false;
            }
            OpCode::JumpIf => {
                let condition = self.read_operand(instruction.operands[0]);
                let target = self.read_operand(instruction.operands[1]) as usize;
                if condition != 0 {
                    self.pc = target;
                    advance_pc = false;
                }
            }
            OpCode::Move => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let value = self.read_operand(instruction.operands[1]);
                self.write_register(dst, value);
            }
            OpCode::Halt => {
                self.halted = true;
            }
        }

        if advance_pc {
            self.pc += 1;
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        while !self.halted {
            self.step()?;
        }
        Ok(())
    }

    pub fn format_registers_compact(&self, per_line: usize) -> String {
        if self.registers.is_empty() {
            return "(no registers)".to_string();
        }

        let per_line = per_line.max(1);
        let width = self
            .registers
            .iter()
            .enumerate()
            .map(|(idx, value)| format!("r{idx} = {value}").len())
            .max()
            .unwrap_or(0);

        let mut lines = Vec::new();
        for chunk in self.registers.chunks(per_line) {
            let mut row = Vec::new();
            for (offset, value) in chunk.iter().enumerate() {
                let idx = lines.len() * per_line + offset;
                row.push(format!("r{idx} = {value:<width$}", width = width));
            }
            lines.push(row.join("    "));
        }

        lines.join("\n")
    }
}
