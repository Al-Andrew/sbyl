use anyhow::{Result, anyhow, bail, ensure};

use crate::instruction::{Instruction, MemoryBase, MemoryRef, OpCode, Operand};

const MAGIC: &[u8; 4] = b"RGLB";
const VERSION: u32 = 2;
const HEADER_SIZE: usize = 24;
const WORD_SIZE: usize = 8;
const WORDS_PER_INSTRUCTION: usize = 6;
const BYTES_PER_INSTRUCTION: usize = WORDS_PER_INSTRUCTION * WORD_SIZE;

const OPERAND_KIND_REGISTER: u16 = 0;
const OPERAND_KIND_IMMEDIATE: u16 = 1;
const OPERAND_KIND_MEMORY_ABSOLUTE: u16 = 2;
const OPERAND_KIND_MEMORY_REGISTER_RELATIVE: u16 = 3;

const MEMORY_SUBTYPE_ABSOLUTE: u64 = 0;
const MEMORY_SUBTYPE_REGISTER_RELATIVE: u64 = 1;

pub fn encode_program(program: &[Instruction]) -> Result<Vec<u8>> {
    let instruction_count = u64::try_from(program.len()).map_err(|_| {
        anyhow!(
            "program has too many instructions to encode in u64 count: {}",
            program.len()
        )
    })?;
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
            bytes.extend_from_slice(&operand_value(operand)?.to_le_bytes());
        }
    }

    Ok(bytes)
}

pub fn decode_program(bytes: &[u8]) -> Result<Vec<Instruction>> {
    ensure!(
        bytes.len() >= HEADER_SIZE,
        "invalid bytecode length: expected at least {HEADER_SIZE} bytes, found {}",
        bytes.len()
    );

    let mut magic = [0u8; 4];
    magic.copy_from_slice(&bytes[0..4]);
    ensure!(&magic == MAGIC, "invalid bytecode magic: {magic:?}");

    let version = read_u32(bytes, 4);
    ensure!(version == VERSION, "unsupported bytecode version: {version}");

    let instruction_count_u64 = read_u64(bytes, 8);
    let instruction_count = usize::try_from(instruction_count_u64).map_err(|_| {
        anyhow!("instruction count too large for this platform: {instruction_count_u64}")
    })?;

    let expected = HEADER_SIZE + instruction_count * BYTES_PER_INSTRUCTION;
    ensure!(
        bytes.len() == expected,
        "invalid bytecode length: expected {expected} bytes, found {}",
        bytes.len()
    );

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
            Operand::Memory(MemoryRef {
                base: MemoryBase::Absolute,
                ..
            }) => OPERAND_KIND_MEMORY_ABSOLUTE,
            Operand::Memory(MemoryRef {
                base: MemoryBase::Register(_),
                ..
            }) => OPERAND_KIND_MEMORY_REGISTER_RELATIVE,
        };
        packed |= u64::from(kind) << (i * 16);
    }
    packed
}

fn decode_operand(kind: u16, value: u64) -> Result<Operand> {
    match kind {
        OPERAND_KIND_REGISTER => Ok(Operand::Register(value)),
        OPERAND_KIND_IMMEDIATE => Ok(Operand::Immediate(value)),
        OPERAND_KIND_MEMORY_ABSOLUTE => {
            ensure!(
                value >> 56 == MEMORY_SUBTYPE_ABSOLUTE,
                "invalid absolute memory encoding"
            );
            Ok(Operand::Memory(MemoryRef {
                base: MemoryBase::Absolute,
                offset: value & ((1u64 << 56) - 1),
            }))
        }
        OPERAND_KIND_MEMORY_REGISTER_RELATIVE => {
            ensure!(
                value >> 56 == MEMORY_SUBTYPE_REGISTER_RELATIVE,
                "invalid register-relative memory encoding"
            );
            let base = (value >> 32) & 0x00FF_FFFF;
            let offset = value & 0xFFFF_FFFF;
            Ok(Operand::Memory(MemoryRef {
                base: MemoryBase::Register(base),
                offset,
            }))
        }
        unknown => bail!("unknown operand kind value: {unknown}"),
    }
}

fn operand_value(operand: Operand) -> Result<u64> {
    match operand {
        Operand::Register(value) | Operand::Immediate(value) => Ok(value),
        Operand::Memory(MemoryRef {
            base: MemoryBase::Absolute,
            offset,
        }) => {
            ensure!(offset < (1u64 << 56), "absolute memory address too large: {offset}");
            Ok((MEMORY_SUBTYPE_ABSOLUTE << 56) | offset)
        }
        Operand::Memory(MemoryRef {
            base: MemoryBase::Register(base),
            offset,
        }) => {
            ensure!(base < (1u64 << 24), "memory base register too large: {base}");
            ensure!(
                u32::try_from(offset).is_ok(),
                "memory offset too large for encoding: {offset}"
            );
            Ok((MEMORY_SUBTYPE_REGISTER_RELATIVE << 56) | (base << 32) | offset)
        }
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
        OpCode::Push => 13,
        OpCode::Pop => 14,
    }
}

fn decode_opcode(value: u64) -> Result<OpCode> {
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
        13 => Ok(OpCode::Push),
        14 => Ok(OpCode::Pop),
        unknown => bail!("unknown opcode value: {unknown}"),
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
                    Operand::Memory(MemoryRef {
                        base: MemoryBase::Absolute,
                        offset: 8,
                    }),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Push,
                operands: [
                    Operand::Immediate(7),
                    Operand::Register(0),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
            Instruction {
                opcode: OpCode::Pop,
                operands: [
                    Operand::Register(17),
                    Operand::Register(0),
                    Operand::Register(0),
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
        assert!(error.to_string().contains("invalid bytecode magic"));
    }

    #[test]
    fn decode_rejects_unknown_opcode() {
        let program = sample_program();
        let mut bytes = encode_program(&program).expect("encode");
        let unknown_opcode = 99u64.to_le_bytes();
        bytes[24..32].copy_from_slice(&unknown_opcode);

        let error = decode_program(&bytes).expect_err("should fail");
        assert!(error.to_string().contains("unknown opcode value: 99"));
    }

    #[test]
    fn decode_rejects_unknown_operand_kind() {
        let program = sample_program();
        let mut bytes = encode_program(&program).expect("encode");
        let mut kinds = [0u8; 8];
        kinds.copy_from_slice(&bytes[32..40]);
        let mut kinds_u64 = u64::from_le_bytes(kinds);
        kinds_u64 = (kinds_u64 & !0xFFFF) | 99;
        bytes[32..40].copy_from_slice(&kinds_u64.to_le_bytes());

        let error = decode_program(&bytes).expect_err("should fail");
        assert!(error.to_string().contains("unknown operand kind value: 99"));
    }

    #[test]
    fn decode_rejects_unsupported_version() {
        let program = sample_program();
        let mut bytes = encode_program(&program).expect("encode");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());

        let error = decode_program(&bytes).expect_err("should fail");
        assert!(error.to_string().contains("unsupported bytecode version: 1"));
    }

    #[test]
    fn encode_rejects_large_memory_offset() {
        let program = vec![Instruction {
            opcode: OpCode::Move,
            operands: [
                Operand::Register(0),
                Operand::Memory(MemoryRef {
                    base: MemoryBase::Register(1),
                    offset: u64::from(u32::MAX) + 1,
                }),
                Operand::Register(0),
                Operand::Register(0),
            ],
        }];

        let error = encode_program(&program).expect_err("should fail");
        assert!(error.to_string().contains("memory offset too large"));
    }
}
