#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Add, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 + t2
    Sub, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 - t2
    Mul, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 * t2
    Div, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = r1 / t2
    Eq,  // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 == t2, r0 = 0 otherwise
    Gt,  // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 > t2, r0 = 0 otherwise
    Lt,  // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 < t2, r0 = 0 otherwise
    Gte, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 >= t2, r0 = 0 otherwise
    Lte, // args: r0, r1, t2 (t2 can be a register or an immediate value), r0 = 1 if r1 <= t2, r0 = 0 otherwise
    Jump, // args: i0, jump to i0
    JumpIf, // args: t0, t1, jump to t1 if t0 is not 0
    Move, // args: r0, t1 (t1 can be a register or an immediate value), r0 = t1
    Halt, // args: none, halt the program
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Register(u64),
    Immediate(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: OpCode,
    pub operands: [Operand; 4],
}
