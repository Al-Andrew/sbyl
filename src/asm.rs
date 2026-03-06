use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::instruction::{Instruction, OpCode, Operand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsmError {
    InvalidLabel {
        line: usize,
        label: String,
    },
    DuplicateLabel {
        line: usize,
        label: String,
    },
    EmptyInstruction {
        line: usize,
    },
    UnknownOpcode {
        line: usize,
        opcode: String,
    },
    WrongOperandCount {
        line: usize,
        opcode: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidOperand {
        line: usize,
        operand: String,
    },
    DestinationMustBeRegister {
        line: usize,
    },
    JumpTargetMustBeImmediateOrLabel {
        line: usize,
    },
    UnknownLabel {
        line: usize,
        label: String,
    },
}

impl Display for AsmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLabel { line, label } => {
                write!(f, "line {line}: invalid label `{label}`")
            }
            Self::DuplicateLabel { line, label } => {
                write!(f, "line {line}: duplicate label `{label}`")
            }
            Self::EmptyInstruction { line } => write!(f, "line {line}: empty instruction"),
            Self::UnknownOpcode { line, opcode } => {
                write!(f, "line {line}: unknown opcode `{opcode}`")
            }
            Self::WrongOperandCount {
                line,
                opcode,
                expected,
                actual,
            } => write!(
                f,
                "line {line}: opcode `{opcode}` expects {expected} operands, got {actual}"
            ),
            Self::InvalidOperand { line, operand } => {
                write!(f, "line {line}: invalid operand `{operand}`")
            }
            Self::DestinationMustBeRegister { line } => {
                write!(f, "line {line}: destination operand must be a register")
            }
            Self::JumpTargetMustBeImmediateOrLabel { line } => write!(
                f,
                "line {line}: jump target must be an immediate address or label"
            ),
            Self::UnknownLabel { line, label } => {
                write!(f, "line {line}: unknown label `{label}`")
            }
        }
    }
}

impl Error for AsmError {}

#[derive(Debug, Clone)]
struct ParsedInstruction {
    line: usize,
    opcode: OpCode,
    operands: Vec<OperandToken>,
}

#[derive(Debug, Clone)]
enum OperandToken {
    Register(u64),
    Immediate(u64),
    Label(String),
}

pub fn assemble_program(source: &str) -> Result<Vec<Instruction>, AsmError> {
    let mut labels = HashMap::<String, usize>::new();
    let mut parsed = Vec::<ParsedInstruction>::new();
    let mut in_block_comment = false;

    for (idx, raw_line) in source.lines().enumerate() {
        let line = idx + 1;
        let cleaned_line = strip_comments(raw_line, &mut in_block_comment);
        let mut content = cleaned_line.trim();
        if content.is_empty() {
            continue;
        }

        loop {
            let Some(label_token) = content.split_whitespace().next() else {
                break;
            };

            if !label_token.ends_with(':') {
                break;
            }

            let label = label_token.trim_end_matches(':');
            if !is_valid_label(label) {
                return Err(AsmError::InvalidLabel {
                    line,
                    label: label.to_string(),
                });
            }
            if labels.insert(label.to_string(), parsed.len()).is_some() {
                return Err(AsmError::DuplicateLabel {
                    line,
                    label: label.to_string(),
                });
            }

            content = content[label_token.len()..].trim_start();
            if content.is_empty() {
                break;
            }
        }

        if content.is_empty() {
            continue;
        }

        parsed.push(parse_instruction(content, line)?);
    }

    parsed
        .iter()
        .map(|instruction| lower_instruction(instruction, &labels))
        .collect()
}

pub fn disassemble_program(program: &[Instruction]) -> String {
    let labels = build_target_labels(program);
    let mut lines = Vec::new();

    for (pc, instruction) in program.iter().enumerate() {
        if let Some(label) = labels.get(&pc) {
            lines.push(format!("{label}:"));
        }
        lines.push(format_instruction(*instruction, &labels));
    }

    lines.join("\n")
}

fn parse_instruction(content: &str, line: usize) -> Result<ParsedInstruction, AsmError> {
    let mut parts = content.splitn(2, char::is_whitespace);
    let mnemonic = parts.next().ok_or(AsmError::EmptyInstruction { line })?;
    let operand_str = parts.next().unwrap_or("").trim();

    let (opcode, opcode_name, expected) = match mnemonic.to_ascii_lowercase().as_str() {
        "add" => (OpCode::Add, "add", 3),
        "sub" => (OpCode::Sub, "sub", 3),
        "mul" => (OpCode::Mul, "mul", 3),
        "div" => (OpCode::Div, "div", 3),
        "eq" => (OpCode::Eq, "eq", 3),
        "gt" => (OpCode::Gt, "gt", 3),
        "lt" => (OpCode::Lt, "lt", 3),
        "gte" => (OpCode::Gte, "gte", 3),
        "lte" => (OpCode::Lte, "lte", 3),
        "jump" => (OpCode::Jump, "jump", 1),
        "jumpif" => (OpCode::JumpIf, "jumpif", 2),
        "move" => (OpCode::Move, "move", 2),
        "halt" => (OpCode::Halt, "halt", 0),
        _ => {
            return Err(AsmError::UnknownOpcode {
                line,
                opcode: mnemonic.to_string(),
            });
        }
    };

    let operands = if operand_str.is_empty() {
        Vec::new()
    } else {
        operand_str
            .split(',')
            .map(|token| parse_operand_token(token.trim(), line))
            .collect::<Result<Vec<_>, _>>()?
    };

    if operands.len() != expected {
        return Err(AsmError::WrongOperandCount {
            line,
            opcode: opcode_name,
            expected,
            actual: operands.len(),
        });
    }

    Ok(ParsedInstruction {
        line,
        opcode,
        operands,
    })
}

fn parse_operand_token(token: &str, line: usize) -> Result<OperandToken, AsmError> {
    if token.is_empty() {
        return Err(AsmError::InvalidOperand {
            line,
            operand: token.to_string(),
        });
    }

    if let Some(register) = token.strip_prefix('r') {
        let value = register.parse::<u64>().map_err(|_| AsmError::InvalidOperand {
            line,
            operand: token.to_string(),
        })?;
        return Ok(OperandToken::Register(value));
    }

    if let Ok(value) = token.parse::<u64>() {
        return Ok(OperandToken::Immediate(value));
    }

    if is_valid_label(token) {
        return Ok(OperandToken::Label(token.to_string()));
    }

    Err(AsmError::InvalidOperand {
        line,
        operand: token.to_string(),
    })
}

fn lower_instruction(
    parsed: &ParsedInstruction,
    labels: &HashMap<String, usize>,
) -> Result<Instruction, AsmError> {
    let mut operands = [Operand::Register(0); 4];

    for (idx, token) in parsed.operands.iter().enumerate() {
        operands[idx] = lower_operand(parsed, idx, token, labels)?;
    }

    Ok(Instruction {
        opcode: parsed.opcode,
        operands,
    })
}

fn lower_operand(
    parsed: &ParsedInstruction,
    idx: usize,
    token: &OperandToken,
    labels: &HashMap<String, usize>,
) -> Result<Operand, AsmError> {
    let is_destination = matches!(
        parsed.opcode,
        OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::Eq
            | OpCode::Gt
            | OpCode::Lt
            | OpCode::Gte
            | OpCode::Lte
            | OpCode::Move
    ) && idx == 0;

    if is_destination {
        return match token {
            OperandToken::Register(reg) => Ok(Operand::Register(*reg)),
            _ => Err(AsmError::DestinationMustBeRegister { line: parsed.line }),
        };
    }

    let is_jump_target = (parsed.opcode == OpCode::Jump && idx == 0)
        || (parsed.opcode == OpCode::JumpIf && idx == 1);

    if is_jump_target {
        return match token {
            OperandToken::Immediate(value) => Ok(Operand::Immediate(*value)),
            OperandToken::Label(label) => {
                let target =
                    labels
                        .get(label)
                        .ok_or_else(|| AsmError::UnknownLabel {
                            line: parsed.line,
                            label: label.clone(),
                        })?;
                Ok(Operand::Immediate(*target as u64))
            }
            _ => Err(AsmError::JumpTargetMustBeImmediateOrLabel { line: parsed.line }),
        };
    }

    match token {
        OperandToken::Register(value) => Ok(Operand::Register(*value)),
        OperandToken::Immediate(value) => Ok(Operand::Immediate(*value)),
        OperandToken::Label(label) => Err(AsmError::UnknownLabel {
            line: parsed.line,
            label: label.clone(),
        }),
    }
}

fn build_target_labels(program: &[Instruction]) -> BTreeMap<usize, String> {
    let mut labels = BTreeMap::<usize, String>::new();
    for instruction in program {
        match instruction.opcode {
            OpCode::Jump => {
                if let Operand::Immediate(target) = instruction.operands[0] {
                    if (target as usize) < program.len() {
                        labels
                            .entry(target as usize)
                            .or_insert_with(|| format!("L{target}"));
                    }
                }
            }
            OpCode::JumpIf => {
                if let Operand::Immediate(target) = instruction.operands[1] {
                    if (target as usize) < program.len() {
                        labels
                            .entry(target as usize)
                            .or_insert_with(|| format!("L{target}"));
                    }
                }
            }
            _ => {}
        }
    }
    labels
}

fn format_instruction(instruction: Instruction, labels: &BTreeMap<usize, String>) -> String {
    match instruction.opcode {
        OpCode::Add => format!(
            "add {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Sub => format!(
            "sub {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Mul => format!(
            "mul {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Div => format!(
            "div {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Eq => format!(
            "eq {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Gt => format!(
            "gt {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Lt => format!(
            "lt {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Gte => format!(
            "gte {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Lte => format!(
            "lte {}, {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1]),
            format_operand(instruction.operands[2])
        ),
        OpCode::Jump => format!("jump {}", format_jump_target(instruction.operands[0], labels)),
        OpCode::JumpIf => format!(
            "jumpif {}, {}",
            format_operand(instruction.operands[0]),
            format_jump_target(instruction.operands[1], labels)
        ),
        OpCode::Move => format!(
            "move {}, {}",
            format_operand(instruction.operands[0]),
            format_operand(instruction.operands[1])
        ),
        OpCode::Halt => "halt".to_string(),
    }
}

fn format_jump_target(operand: Operand, labels: &BTreeMap<usize, String>) -> String {
    if let Operand::Immediate(target) = operand {
        if let Some(label) = labels.get(&(target as usize)) {
            return label.clone();
        }
    }
    format_operand(operand)
}

fn format_operand(operand: Operand) -> String {
    match operand {
        Operand::Register(reg) => format!("r{reg}"),
        Operand::Immediate(value) => value.to_string(),
    }
}

fn is_valid_label(label: &str) -> bool {
    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn strip_comments(line: &str, in_block_comment: &mut bool) -> String {
    let mut out = String::new();
    let mut idx = 0usize;

    while idx < line.len() {
        let rest = &line[idx..];

        if *in_block_comment {
            if let Some(end) = rest.find("*/") {
                idx += end + 2;
                *in_block_comment = false;
            } else {
                break;
            }
            continue;
        }

        if rest.starts_with("/*") {
            *in_block_comment = true;
            idx += 2;
            continue;
        }
        if rest.starts_with("//") {
            break;
        }

        let ch = rest.chars().next().expect("slice must not be empty");
        if ch == ';' || ch == '#' {
            break;
        }

        out.push(ch);
        idx += ch.len_utf8();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{decode_program, encode_program};

    #[test]
    fn assemble_resolves_labels_for_jumps() {
        let src = r#"
start:
    move r0, 1
    jump done
    add r0, r0, 1
done:
    halt
"#;

        let program = assemble_program(src).expect("assembly should parse");
        assert_eq!(program.len(), 4);
        assert_eq!(program[1].opcode, OpCode::Jump);
        assert_eq!(program[1].operands[0], Operand::Immediate(3));
    }

    #[test]
    fn assemble_rejects_unknown_label() {
        let src = "jump missing";
        let error = assemble_program(src).expect_err("should fail");
        assert!(matches!(error, AsmError::UnknownLabel { .. }));
    }

    #[test]
    fn assemble_supports_single_line_comments() {
        let src = r#"
start: // loop start
    move r0, 1 ; initialize
    add r0, r0, 2 # increment
    jump start
"#;

        let program = assemble_program(src).expect("assembly should parse");
        assert_eq!(program.len(), 3);
        assert_eq!(program[2].opcode, OpCode::Jump);
        assert_eq!(program[2].operands[0], Operand::Immediate(0));
    }

    #[test]
    fn assemble_supports_multiline_comments() {
        let src = r#"
/* header comment
   that spans lines */
entry:
    move r0, 1
    /* skip this whole instruction:
       add r0, r0, 99
    */
    jump entry /* trailing block comment */
"#;

        let program = assemble_program(src).expect("assembly should parse");
        assert_eq!(program.len(), 2);
        assert_eq!(program[1].opcode, OpCode::Jump);
        assert_eq!(program[1].operands[0], Operand::Immediate(0));
    }

    #[test]
    fn disassemble_emits_labels_for_targets() {
        let program = vec![
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
                opcode: OpCode::Jump,
                operands: [
                    Operand::Immediate(0),
                    Operand::Register(0),
                    Operand::Register(0),
                    Operand::Register(0),
                ],
            },
        ];

        let text = disassemble_program(&program);
        assert!(text.contains("L0:"));
        assert!(text.contains("jump L0"));
    }

    #[test]
    fn asm_binary_round_trip() {
        let src = r#"
loop:
    add r0, r0, 1
    lt r1, r0, 10
    jumpif r1, loop
    halt
"#;

        let assembled = assemble_program(src).expect("assemble");
        let bytes = encode_program(&assembled).expect("encode");
        let decoded = decode_program(&bytes).expect("decode");
        assert_eq!(assembled, decoded);

        let disassembled = disassemble_program(&decoded);
        assert!(disassembled.contains("L0:"));
    }
}

