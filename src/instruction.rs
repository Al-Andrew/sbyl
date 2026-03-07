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
    Call, // args: i0, i1, i2; call i0 with i1 input bytes and i2 output bytes
    Ret,  // args: i0, i1; return from call frame with i0 input bytes and i1 output bytes
    Move, // args: d0, t1 (exactly one side may be memory)
    Push, // args: t0, push t0 on the VM-managed stack
    Pop,  // args: r0, pop top-of-stack into r0
    Halt, // args: none, halt the program
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBase {
    Absolute,
    Register(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRef {
    pub base: MemoryBase,
    pub offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Register(u64),
    Immediate(u64),
    Memory(MemoryRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: OpCode,
    pub operands: [Operand; 4],
}
