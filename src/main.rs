#[derive(Debug, Clone, Copy)]
enum OpCode {
    Add,    // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 + t2
    Sub,    // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 - t2
    Mul,    // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 * t2
    Div,    // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 / t2
    Eq, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 == t2, r0 = 0 otherwise
    Gt, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 > t2, r0 = 0 otherwise
    Lt, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 < t2, r0 = 0 otherwise
    Gte, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 >= t2, r0 = 0 otherwise
    Lte, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 <= t2, r0 = 0 otherwise
    Jump, // args: i0, jump to i0
    JumpIf, // args: t0, t1, jump to t1 if t0 is not 0
    Move, // args: r0, t1 (t1 can be a register or an immediate value), r0 = t1
    Halt, // args: none, halt the program
}

#[derive(Debug, Clone, Copy)]
enum Operand {
    Register(u64),
    Immediate(u64),
}

#[derive(Debug, Clone, Copy)]
struct Instruction {
    opcode: OpCode,
    operands: [Operand; 4],
}

#[derive(Debug)]
struct Vm {
    program: Vec<Instruction>,
    registers: Vec<u64>,
    pc: usize,
    halted: bool,
}

impl Vm {
    fn new(program: Vec<Instruction>) -> Self {
        Self {
            program,
            registers: Vec::new(),
            pc: 0,
            halted: false,
        }
    }

    fn read_operand(&self, operand: Operand) -> u64 {
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

    fn destination(operand: Operand) -> usize {
        match operand {
            Operand::Register(reg) => reg as usize,
            Operand::Immediate(value) => {
                panic!("destination operand must be register, got immediate {value}")
            }
        }
    }

    fn step(&mut self) {
        if self.halted {
            return;
        }
        if self.pc >= self.program.len() {
            self.halted = true;
            return;
        }

        let instruction = self.program[self.pc];
        let mut advance_pc = true;

        match instruction.opcode {
            OpCode::Add => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, lhs.wrapping_add(rhs));
            }
            OpCode::Sub => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, lhs.wrapping_sub(rhs));
            }
            OpCode::Mul => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, lhs.wrapping_mul(rhs));
            }
            OpCode::Div => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                if rhs == 0 {
                    panic!("division by zero at pc {}", self.pc);
                }
                self.write_register(dst, lhs / rhs);
            }
            OpCode::Eq => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs == rhs));
            }
            OpCode::Gt => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs > rhs));
            }
            OpCode::Lt => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs < rhs));
            }
            OpCode::Gte => {
                let dst = Self::destination(instruction.operands[0]);
                let lhs = self.read_operand(instruction.operands[1]);
                let rhs = self.read_operand(instruction.operands[2]);
                self.write_register(dst, u64::from(lhs >= rhs));
            }
            OpCode::Lte => {
                let dst = Self::destination(instruction.operands[0]);
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
                let dst = Self::destination(instruction.operands[0]);
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
    }

    fn run(&mut self) {
        while !self.halted {
            self.step();
        }
    }
}

fn main() {
    let fib_instructions: Vec<Instruction> = vec![
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
    ];

    let mut vm = Vm::new(fib_instructions);
    vm.run();

    println!("Final registers:");
    println!("r0 = {}", vm.read_operand(Operand::Register(0)));
    println!("r1 = {}", vm.read_operand(Operand::Register(1)));
    println!("r2 = {}", vm.read_operand(Operand::Register(2)));
    println!("r3 = {}", vm.read_operand(Operand::Register(3)));
}
