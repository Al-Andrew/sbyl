use crate::instruction::{Instruction, MemoryBase, MemoryRef, OpCode, Operand};
use anyhow::{Result, bail};

const STACK_CAP_BYTES: usize = 64 * 1024 * 1024;
const WORD_BYTES: usize = 8;
const REGISTER_BANK_SIZE: usize = 32;
const SP_REG: usize = 16;
const BP_REG: usize = 17;
const IP_REG: usize = 18;
const FLAGS_REG: usize = 19;
const RESERVED_START: usize = 16;
const RESERVED_END: usize = 31;

#[derive(Debug)]
pub struct Vm {
    program: Vec<Instruction>,
    registers: Box<[u64]>,
    memory: Vec<u8>,
    pc: usize,
    halted: bool,
}

impl Vm {
    pub fn new(program: Vec<Instruction>) -> Self {
        let mut registers = vec![0; REGISTER_BANK_SIZE].into_boxed_slice();
        registers[SP_REG] = 0;
        registers[BP_REG] = 0;
        registers[IP_REG] = 0;
        registers[FLAGS_REG] = 0;

        Self {
            program,
            registers,
            memory: Vec::new(),
            pc: 0,
            halted: false,
        }
    }

    pub fn read_operand(&self, operand: Operand) -> Result<u64> {
        match operand {
            Operand::Immediate(value) => Ok(value),
            Operand::Register(reg) => self.read_register(reg as usize, self.pc, OpCode::Move),
            Operand::Memory(memory) => {
                let addr = self.resolve_memory(memory, self.pc, OpCode::Move)?;
                self.read_word(addr, self.pc, OpCode::Move)
            }
        }
    }

    fn read_register(&self, idx: usize, pc: usize, opcode: OpCode) -> Result<u64> {
        self.registers
            .get(idx)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("vm runtime error at pc {pc} ({opcode:?}): register index out of range r{idx}"))
    }

    fn write_register(&mut self, reg: usize, value: u64, pc: usize, opcode: OpCode) -> Result<()> {
        self.validate_writable_register(reg, pc, opcode)?;
        if reg >= self.registers.len() {
            bail!("vm runtime error at pc {pc} ({opcode:?}): register index out of range r{reg}");
        }
        self.registers[reg] = value;
        Ok(())
    }

    fn write_system_register(&mut self, reg: usize, value: u64) {
        if reg < self.registers.len() {
            self.registers[reg] = value;
        }
    }

    fn validate_writable_register(&self, reg: usize, pc: usize, opcode: OpCode) -> Result<()> {
        match reg {
            IP_REG => bail!("vm runtime error at pc {pc} ({opcode:?}): cannot write to reserved register ip"),
            FLAGS_REG => bail!("vm runtime error at pc {pc} ({opcode:?}): cannot write to reserved register flags"),
            RESERVED_START..=RESERVED_END if reg > BP_REG => bail!(
                "vm runtime error at pc {pc} ({opcode:?}): cannot write to reserved register r{reg}"
            ),
            _ => Ok(()),
        }
    }

    fn destination(operand: Operand, pc: usize, opcode: OpCode) -> Result<usize> {
        match operand {
            Operand::Register(reg) => Ok(reg as usize),
            Operand::Immediate(value) => bail!(
                "vm runtime error at pc {pc} ({opcode:?}): destination operand must be register, got immediate {value}"
            ),
            Operand::Memory(_) => bail!(
                "vm runtime error at pc {pc} ({opcode:?}): destination operand must be register, got memory"
            ),
        }
    }

    fn resolve_memory(&self, memory: MemoryRef, pc: usize, opcode: OpCode) -> Result<u64> {
        let base = match memory.base {
            MemoryBase::Absolute => 0,
            MemoryBase::Register(reg) => self.read_register(reg as usize, pc, opcode)?,
        };
        self.checked_effective_addr(base, memory.offset, pc, opcode)
    }

    fn checked_effective_addr(&self, base: u64, offset: u64, pc: usize, opcode: OpCode) -> Result<u64> {
        let addr = base.checked_add(offset).ok_or_else(|| {
            anyhow::anyhow!("vm runtime error at pc {pc} ({opcode:?}): address overflow")
        })?;
        if addr % WORD_BYTES as u64 != 0 {
            bail!("vm runtime error at pc {pc} ({opcode:?}): unaligned memory address {addr}");
        }
        Ok(addr)
    }

    fn ensure_memory_len(&mut self, len: usize, pc: usize, opcode: OpCode) -> Result<()> {
        if len > STACK_CAP_BYTES {
            bail!(
                "vm runtime error at pc {pc} ({opcode:?}): memory growth exceeds stack cap of {STACK_CAP_BYTES} bytes"
            );
        }
        if len > self.memory.len() {
            self.memory.resize(len, 0);
        }
        Ok(())
    }

    fn read_word(&self, addr: u64, pc: usize, opcode: OpCode) -> Result<u64> {
        let start = usize::try_from(addr).map_err(|_| {
            anyhow::anyhow!("vm runtime error at pc {pc} ({opcode:?}): address too large")
        })?;
        let end = start.checked_add(WORD_BYTES).ok_or_else(|| {
            anyhow::anyhow!("vm runtime error at pc {pc} ({opcode:?}): address range overflow")
        })?;
        if end > self.memory.len() {
            bail!("vm runtime error at pc {pc} ({opcode:?}): read past allocated memory at address {addr}");
        }

        let mut buf = [0u8; WORD_BYTES];
        buf.copy_from_slice(&self.memory[start..end]);
        Ok(u64::from_le_bytes(buf))
    }

    fn write_word(&mut self, addr: u64, value: u64, pc: usize, opcode: OpCode) -> Result<()> {
        let start = usize::try_from(addr).map_err(|_| {
            anyhow::anyhow!("vm runtime error at pc {pc} ({opcode:?}): address too large")
        })?;
        let end = start.checked_add(WORD_BYTES).ok_or_else(|| {
            anyhow::anyhow!("vm runtime error at pc {pc} ({opcode:?}): address range overflow")
        })?;
        self.ensure_memory_len(end, pc, opcode)?;
        self.memory[start..end].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn sp(&self) -> u64 {
        self.registers[SP_REG]
    }

    fn set_sp(&mut self, value: u64) {
        self.write_system_register(SP_REG, value);
    }

    pub fn step(&mut self) -> Result<()> {
        if self.halted {
            return Ok(());
        }
        if self.pc >= self.program.len() {
            self.halted = true;
            return Ok(());
        }

        self.write_system_register(IP_REG, self.pc as u64);
        let instruction = self.program[self.pc];
        let mut advance_pc = true;

        match instruction.opcode {
            OpCode::Add => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, lhs.wrapping_add(rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Sub => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, lhs.wrapping_sub(rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Mul => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, lhs.wrapping_mul(rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Div => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                if rhs == 0 {
                    bail!("vm runtime error at pc {} (Div): division by zero", self.pc);
                }
                self.write_register(dst, lhs / rhs, self.pc, instruction.opcode)?;
            }
            OpCode::Eq => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, u64::from(lhs == rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Gt => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, u64::from(lhs > rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Lt => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, u64::from(lhs < rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Gte => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, u64::from(lhs >= rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Lte => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let lhs = self.read_operand(instruction.operands[1])?;
                let rhs = self.read_operand(instruction.operands[2])?;
                self.write_register(dst, u64::from(lhs <= rhs), self.pc, instruction.opcode)?;
            }
            OpCode::Jump => {
                let target = self.read_operand(instruction.operands[0])? as usize;
                self.pc = target;
                advance_pc = false;
            }
            OpCode::JumpIf => {
                let condition = self.read_operand(instruction.operands[0])?;
                let target = self.read_operand(instruction.operands[1])? as usize;
                if condition != 0 {
                    self.pc = target;
                    advance_pc = false;
                }
            }
            OpCode::Move => match (instruction.operands[0], instruction.operands[1]) {
                (Operand::Register(dst), src) => {
                    let value = self.read_operand(src)?;
                    self.write_register(dst as usize, value, self.pc, instruction.opcode)?;
                }
                (Operand::Memory(_), Operand::Memory(_)) => bail!(
                    "vm runtime error at pc {} (Move): memory-to-memory transfers are not supported",
                    self.pc
                ),
                (Operand::Memory(memory), src) => {
                    let value = self.read_operand(src)?;
                    let addr = self.resolve_memory(memory, self.pc, instruction.opcode)?;
                    self.write_word(addr, value, self.pc, instruction.opcode)?;
                }
                (Operand::Immediate(value), _) => bail!(
                    "vm runtime error at pc {} (Move): destination operand cannot be immediate {value}",
                    self.pc
                ),
            },
            OpCode::Push => {
                let sp = self.sp();
                if sp % WORD_BYTES as u64 != 0 {
                    bail!("vm runtime error at pc {} (Push): unaligned stack pointer {sp}", self.pc);
                }
                let value = self.read_operand(instruction.operands[0])?;
                self.write_word(sp, value, self.pc, instruction.opcode)?;
                let next_sp = sp.checked_add(WORD_BYTES as u64).ok_or_else(|| {
                    anyhow::anyhow!("vm runtime error at pc {} (Push): stack pointer overflow", self.pc)
                })?;
                self.set_sp(next_sp);
            }
            OpCode::Pop => {
                let dst = Self::destination(instruction.operands[0], self.pc, instruction.opcode)?;
                let sp = self.sp();
                if sp < WORD_BYTES as u64 {
                    bail!("vm runtime error at pc {} (Pop): stack underflow", self.pc);
                }
                let next_sp = sp - WORD_BYTES as u64;
                if next_sp % WORD_BYTES as u64 != 0 {
                    bail!("vm runtime error at pc {} (Pop): unaligned stack pointer {next_sp}", self.pc);
                }
                let value = self.read_word(next_sp, self.pc, instruction.opcode)?;
                self.set_sp(next_sp);
                self.write_register(dst, value, self.pc, instruction.opcode)?;
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
            .map(|(idx, value)| format!("{} = {value}", format_register_name(idx)).len())
            .max()
            .unwrap_or(0);

        let mut lines = Vec::new();
        for chunk in self.registers.chunks(per_line) {
            let mut row = Vec::new();
            for (offset, value) in chunk.iter().enumerate() {
                let idx = lines.len() * per_line + offset;
                row.push(format!(
                    "{} = {value:<width$}",
                    format_register_name(idx),
                    width = width
                ));
            }
            lines.push(row.join("    "));
        }

        lines.join("\n")
    }
}

fn format_register_name(reg: usize) -> String {
    match reg {
        SP_REG => "sp".to_string(),
        BP_REG => "bp".to_string(),
        IP_REG => "ip".to_string(),
        FLAGS_REG => "flags".to_string(),
        _ => format!("r{reg}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_program(program: Vec<Instruction>) -> Result<Vm> {
        let mut vm = Vm::new(program);
        vm.run()?;
        Ok(vm)
    }

    #[test]
    fn move_immediate_to_register_still_works() {
        let vm = run_program(vec![
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Register(0),
                    Operand::Immediate(7),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Halt,
                operands: [Operand::Register(0); 4],
            },
        ])
        .expect("run");

        assert_eq!(vm.read_register(0, 0, OpCode::Move).expect("read"), 7);
    }

    #[test]
    fn move_round_trips_memory() {
        let vm = run_program(vec![
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Memory(MemoryRef {
                        base: MemoryBase::Register(SP_REG as u64),
                        offset: 0,
                    }),
                    Operand::Immediate(42),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Register(1),
                    Operand::Memory(MemoryRef {
                        base: MemoryBase::Register(SP_REG as u64),
                        offset: 0,
                    }),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Halt,
                operands: [Operand::Register(0); 4],
            },
        ])
        .expect("run");

        assert_eq!(vm.read_register(1, 0, OpCode::Move).expect("read"), 42);
    }

    #[test]
    fn bp_relative_memory_round_trips() {
        let vm = run_program(vec![
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Register(BP_REG as u64),
                    Operand::Immediate(8),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Memory(MemoryRef {
                        base: MemoryBase::Register(BP_REG as u64),
                        offset: 8,
                    }),
                    Operand::Immediate(99),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Register(4),
                    Operand::Memory(MemoryRef {
                        base: MemoryBase::Register(BP_REG as u64),
                        offset: 8,
                    }),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Halt,
                operands: [Operand::Register(0); 4],
            },
        ])
        .expect("run");

        assert_eq!(vm.read_register(4, 0, OpCode::Move).expect("read"), 99);
    }

    #[test]
    fn push_and_pop_are_lifo() {
        let vm = run_program(vec![
            Instruction {
                opcode: OpCode::Push,
                operands: [Operand::Immediate(1), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
            },
            Instruction {
                opcode: OpCode::Push,
                operands: [Operand::Immediate(2), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
            },
            Instruction {
                opcode: OpCode::Pop,
                operands: [Operand::Register(0), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
            },
            Instruction {
                opcode: OpCode::Pop,
                operands: [Operand::Register(1), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
            },
            Instruction {
                opcode: OpCode::Halt,
                operands: [Operand::Register(0); 4],
            },
        ])
        .expect("run");

        assert_eq!(vm.read_register(0, 0, OpCode::Move).expect("read"), 2);
        assert_eq!(vm.read_register(1, 0, OpCode::Move).expect("read"), 1);
    }

    #[test]
    fn push_updates_sp() {
        let vm = run_program(vec![
            Instruction {
                opcode: OpCode::Push,
                operands: [Operand::Immediate(7), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
            },
            Instruction {
                opcode: OpCode::Halt,
                operands: [Operand::Register(0); 4],
            },
        ])
        .expect("run");

        assert_eq!(vm.read_register(SP_REG, 0, OpCode::Move).expect("read"), 8);
    }

    #[test]
    fn pop_underflow_fails() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Pop,
            operands: [Operand::Register(0), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("stack underflow"));
    }

    #[test]
    fn unaligned_sp_fails_push() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Push,
            operands: [Operand::Immediate(1), Operand::Register(0), Operand::Register(0), Operand::Register(0)],
        }]);
        vm.write_system_register(SP_REG, 1);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("unaligned stack pointer"));
    }

    #[test]
    fn unaligned_memory_access_fails() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(0),
                Operand::Memory(MemoryRef {
                    base: MemoryBase::Absolute,
                    offset: 1,
                }),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("unaligned memory address"));
    }

    #[test]
    fn reads_past_allocated_memory_fail() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(0),
                Operand::Memory(MemoryRef {
                    base: MemoryBase::Absolute,
                    offset: 0,
                }),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("read past allocated memory"));
    }

    #[test]
    fn memory_growth_beyond_cap_fails() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Memory(MemoryRef {
                    base: MemoryBase::Absolute,
                    offset: STACK_CAP_BYTES as u64,
                }),
                Operand::Immediate(1),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("exceeds stack cap"));
    }

    #[test]
    fn writing_reserved_registers_fails() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(IP_REG as u64),
                Operand::Immediate(1),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("reserved register ip"));

        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(20),
                Operand::Immediate(1),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("reserved register r20"));
    }

    #[test]
    fn writing_register_past_fixed_bank_fails() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(32),
                Operand::Immediate(1),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("register index out of range r32"));
    }

    #[test]
    fn jump_updates_ip() {
        let vm = run_program(vec![
            Instruction {
                opcode: OpCode::Jump,
                operands: [
                    Operand::Immediate(2),
                    Operand::Register(0),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Register(0),
                    Operand::Immediate(1),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Halt,
                operands: [Operand::Register(0); 4],
            },
        ])
        .expect("run");

        assert_eq!(vm.read_register(IP_REG, 0, OpCode::Move).expect("read"), 2);
    }

    #[test]
    fn reading_register_past_fixed_bank_fails() {
        let mut vm = Vm::new(vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(0),
                Operand::Register(32),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }]);
        let error = vm.run().expect_err("should fail");
        assert!(error.to_string().contains("register index out of range r32"));
    }

    #[test]
    fn compact_register_dump_uses_system_aliases() {
        let mut vm = Vm::new(vec![]);
        vm.write_system_register(SP_REG, 8);
        vm.write_system_register(BP_REG, 16);
        vm.write_system_register(IP_REG, 3);
        vm.write_system_register(FLAGS_REG, 1);

        let formatted = vm.format_registers_compact(4);
        assert!(formatted.contains("sp = 8"));
        assert!(formatted.contains("bp = 16"));
        assert!(formatted.contains("ip = 3"));
        assert!(formatted.contains("flags = 1"));
    }
}
