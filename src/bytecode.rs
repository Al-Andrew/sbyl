use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::instruction::{Instruction, OpCode, Operand};

const MAGIC: &[u8; 4] = b"RGLB";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 24;
const WORD_SIZE: usize = 8;
const WORDS_PER_INSTRUCTION: usize = 6;
const BYTES_PER_INSTRUCTION: usize = WORDS_PER_INSTRUCTION * WORD_SIZE;

const OPERAND_KIND_REGISTER: u16 = 0;
const OPERAND_KIND_IMMEDIATE: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeError {
    InvalidMagic([u8; 4]),
    UnsupportedVersion(u32),
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    UnknownOpcode(u64),
    UnknownOperandKind(u16),
    InstructionCountTooLarge(u64),
    ProgramTooLarge(usize),
}

impl Display for BytecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic(found) => write!(f, "invalid bytecode magic: {found:?}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported bytecode version: {version}")
            }
            Self::InvalidLength { expected, actual } => {
                write!(
                    f,
                    "invalid bytecode length: expected {expected} bytes, found {actual}"
                )
            }
            Self::UnknownOpcode(opcode) => write!(f, "unknown opcode value: {opcode}"),
            Self::UnknownOperandKind(kind) => write!(f, "unknown operand kind value: {kind}"),
            Self::InstructionCountTooLarge(count) => {
                write!(f, "instruction count too large for this platform: {count}")
            }
            Self::ProgramTooLarge(count) => write!(
                f,
                "program has too many instructions to encode in u64 count: {count}"
            ),
        }
    }
}

impl Error for BytecodeError {}

pub fn encode_program(program: &[Instruction]) -> Result<Vec<u8>, BytecodeError> {
    let instruction_count =
        u64::try_from(program.len()).map_err(|_| BytecodeError::ProgramTooLarge(program.len()))?;
    let mut bytes = Vec::with_capacity(HEADER_SIZE + program.len() * BYTES_PER_INSTRUCTION);

    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&instruction_count.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());

    for instruction in program {
        let opcode = encode_opcode(instruction.opcode);
        let kinds = pack_operand_kinds(instruction.operands);

        bytes.extend_from_slice(&opcode.to_le_bytes());
        bytes.extend_from_slice(&kinds.to_le_bytes());
        for operand in instruction.operands {
            bytes.extend_from_slice(&operand_value(operand).to_le_bytes());
        }
    }

    Ok(bytes)
}

pub fn decode_program(bytes: &[u8]) -> Result<Vec<Instruction>, BytecodeError> {
    if bytes.len() < HEADER_SIZE {
        return Err(BytecodeError::InvalidLength {
            expected: HEADER_SIZE,
            actual: bytes.len(),
        });
    }

    let mut magic = [0u8; 4];
    magic.copy_from_slice(&bytes[0..4]);
    if &magic != MAGIC {
        return Err(BytecodeError::InvalidMagic(magic));
    }

    let version = read_u32(bytes, 4);
    if version != VERSION {
        return Err(BytecodeError::UnsupportedVersion(version));
    }

    let instruction_count_u64 = read_u64(bytes, 8);
    let instruction_count = usize::try_from(instruction_count_u64)
        .map_err(|_| BytecodeError::InstructionCountTooLarge(instruction_count_u64))?;

    let expected = HEADER_SIZE + instruction_count * BYTES_PER_INSTRUCTION;
    if bytes.len() != expected {
        return Err(BytecodeError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }

    let mut program = Vec::with_capacity(instruction_count);
    let mut offset = HEADER_SIZE;
    for _ in 0..instruction_count {
        let opcode_raw = read_u64(bytes, offset);
        let opcode = decode_opcode(opcode_raw)?;
        let operand_kinds = read_u64(bytes, offset + WORD_SIZE);

        let mut operands = [Operand::Register(0); 4];
        for (i, operand) in operands.iter_mut().enumerate() {
            let kind_bits = ((operand_kinds >> (i * 16)) & 0xFFFF) as u16;
            let value = read_u64(bytes, offset + (2 + i) * WORD_SIZE);
            *operand = decode_operand(kind_bits, value)?;
        }

        program.push(Instruction { opcode, operands });
        offset += BYTES_PER_INSTRUCTION;
    }

    Ok(program)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(buf)
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(buf)
}

fn pack_operand_kinds(operands: [Operand; 4]) -> u64 {
    let mut packed = 0u64;
    for (i, operand) in operands.iter().enumerate() {
        let kind = match operand {
            Operand::Register(_) => OPERAND_KIND_REGISTER,
            Operand::Immediate(_) => OPERAND_KIND_IMMEDIATE,
        };
        packed |= u64::from(kind) << (i * 16);
    }
    packed
}

fn decode_operand(kind: u16, value: u64) -> Result<Operand, BytecodeError> {
    match kind {
        OPERAND_KIND_REGISTER => Ok(Operand::Register(value)),
        OPERAND_KIND_IMMEDIATE => Ok(Operand::Immediate(value)),
        unknown => Err(BytecodeError::UnknownOperandKind(unknown)),
    }
}

fn operand_value(operand: Operand) -> u64 {
    match operand {
        Operand::Register(value) | Operand::Immediate(value) => value,
    }
}

fn encode_opcode(opcode: OpCode) -> u64 {
    match opcode {
        OpCode::Add => 0,
        OpCode::Sub => 1,
        OpCode::Mul => 2,
        OpCode::Div => 3,
        OpCode::Eq => 4,
        OpCode::Gt => 5,
        OpCode::Lt => 6,
        OpCode::Gte => 7,
        OpCode::Lte => 8,
        OpCode::Jump => 9,
        OpCode::JumpIf => 10,
        OpCode::Move => 11,
        OpCode::Halt => 12,
    }
}

fn decode_opcode(value: u64) -> Result<OpCode, BytecodeError> {
    match value {
        0 => Ok(OpCode::Add),
        1 => Ok(OpCode::Sub),
        2 => Ok(OpCode::Mul),
        3 => Ok(OpCode::Div),
        4 => Ok(OpCode::Eq),
        5 => Ok(OpCode::Gt),
        6 => Ok(OpCode::Lt),
        7 => Ok(OpCode::Gte),
        8 => Ok(OpCode::Lte),
        9 => Ok(OpCode::Jump),
        10 => Ok(OpCode::JumpIf),
        11 => Ok(OpCode::Move),
        12 => Ok(OpCode::Halt),
        unknown => Err(BytecodeError::UnknownOpcode(unknown)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_program() -> Vec<Instruction> {
        vec![
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
                opcode: OpCode::Add,
                operands: [
                    Operand::Register(1),
                    Operand::Register(0),
                    Operand::Immediate(2),
                    Operand::Register(0),
                ],
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

    #[test]
    fn encode_decode_round_trip() {
        let program = sample_program();
        let bytes = encode_program(&program).expect("encode");
        let decoded = decode_program(&bytes).expect("decode");
        assert_eq!(program, decoded);
    }

    #[test]
    fn decode_rejects_invalid_magic() {
        let program = sample_program();
        let mut bytes = encode_program(&program).expect("encode");
        bytes[0] = b'X';

        let error = decode_program(&bytes).expect_err("should fail");
        assert!(matches!(error, BytecodeError::InvalidMagic(_)));
    }

    #[test]
    fn decode_rejects_unknown_opcode() {
        let program = sample_program();
        let mut bytes = encode_program(&program).expect("encode");
        let unknown_opcode = 99u64.to_le_bytes();
        bytes[24..32].copy_from_slice(&unknown_opcode);

        let error = decode_program(&bytes).expect_err("should fail");
        assert!(matches!(error, BytecodeError::UnknownOpcode(99)));
    }

    #[test]
    fn decode_rejects_unknown_operand_kind() {
        let program = sample_program();
        let mut bytes = encode_program(&program).expect("encode");
        let mut kinds = [0u8; 8];
        kinds.copy_from_slice(&bytes[32..40]);
        let mut kinds_u64 = u64::from_le_bytes(kinds);
        kinds_u64 = (kinds_u64 & !0xFFFF) | 2;
        bytes[32..40].copy_from_slice(&kinds_u64.to_le_bytes());

        let error = decode_program(&bytes).expect_err("should fail");
        assert!(matches!(error, BytecodeError::UnknownOperandKind(2)));
    }
}
