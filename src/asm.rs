use std::collections::{BTreeMap, HashMap};

use anyhow::{Result, anyhow, bail};
use miette::{LabeledSpan, NamedSource, miette};

use crate::instruction::{Instruction, MemoryBase, MemoryRef, OpCode, Operand};

const SP_REG: u64 = 16;
const BP_REG: u64 = 17;
const IP_REG: u64 = 18;
const FLAGS_REG: u64 = 19;
const RESERVED_START: u64 = 16;
const RESERVED_END: u64 = 31;

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
    Memory(MemoryToken),
    Label(String),
}

#[derive(Debug, Clone)]
enum MemoryToken {
    Absolute(u64),
    RegisterRelative { base: u64, offset: u64 },
}

pub fn assemble_program(source: &str) -> Result<Vec<Instruction>> {
    assemble_program_with_context("<memory>", source).map_err(|error| anyhow!("{error}"))
}

pub fn assemble_program_with_context(
    source_name: &str,
    source: &str,
) -> miette::Result<Vec<Instruction>> {
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
                return Err(asm_diagnostic(
                    source_name,
                    source,
                    line,
                    &format!("line {line}: invalid label `{label}`"),
                    Some("invalid label"),
                    Some(label),
                ));
            }
            if labels.insert(label.to_string(), parsed.len()).is_some() {
                return Err(asm_diagnostic(
                    source_name,
                    source,
                    line,
                    &format!("line {line}: duplicate label `{label}`"),
                    Some("duplicate label"),
                    Some(label),
                ));
            }

            content = content[label_token.len()..].trim_start();
            if content.is_empty() {
                break;
            }
        }

        if content.is_empty() {
            continue;
        }

        let instruction = parse_instruction(content, line).map_err(|error| {
            let message = error.to_string();
            let highlight = extract_backticked_token(&message);
            asm_diagnostic(
                source_name,
                source,
                line,
                &message,
                Some("invalid instruction"),
                highlight.as_deref(),
            )
        })?;
        parsed.push(instruction);
    }

    let mut program = Vec::with_capacity(parsed.len());
    for instruction in &parsed {
        let lowered = lower_instruction(instruction, &labels).map_err(|error| {
            let message = error.to_string();
            let highlight = extract_backticked_token(&message);
            asm_diagnostic(
                source_name,
                source,
                instruction.line,
                &message,
                Some("invalid operand or target"),
                highlight.as_deref(),
            )
        })?;
        program.push(lowered);
    }

    Ok(program)
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

fn parse_instruction(content: &str, line: usize) -> Result<ParsedInstruction> {
    let mut parts = content.splitn(2, char::is_whitespace);
    let mnemonic = parts
        .next()
        .ok_or_else(|| anyhow!("line {line}: empty instruction"))?;
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
        "push" => (OpCode::Push, "push", 1),
        "pop" => (OpCode::Pop, "pop", 1),
        "halt" => (OpCode::Halt, "halt", 0),
        _ => bail!("line {line}: unknown opcode `{mnemonic}`"),
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
        bail!(
            "line {line}: opcode `{opcode_name}` expects {expected} operands, got {}",
            operands.len()
        );
    }

    Ok(ParsedInstruction {
        line,
        opcode,
        operands,
    })
}

fn parse_operand_token(token: &str, line: usize) -> Result<OperandToken> {
    if token.is_empty() {
        bail!("line {line}: invalid operand `{token}`");
    }

    if token.starts_with('[') || token.ends_with(']') {
        return parse_memory_token(token, line).map(OperandToken::Memory);
    }

    if let Some(register) = parse_register_name(token) {
        return Ok(OperandToken::Register(register));
    }

    if let Ok(value) = token.parse::<u64>() {
        return Ok(OperandToken::Immediate(value));
    }

    if is_valid_label(token) {
        return Ok(OperandToken::Label(token.to_string()));
    }

    bail!("line {line}: invalid operand `{token}`")
}

fn parse_memory_token(token: &str, line: usize) -> Result<MemoryToken> {
    if !(token.starts_with('[') && token.ends_with(']')) {
        bail!("line {line}: invalid operand `{token}`");
    }

    let inner = token[1..token.len() - 1].trim();
    if inner.is_empty() {
        bail!("line {line}: invalid operand `{token}`");
    }

    if inner.contains('-') {
        bail!("line {line}: negative offsets are not supported in `{token}`");
    }

    if let Ok(value) = inner.parse::<u64>() {
        return Ok(MemoryToken::Absolute(value));
    }

    if let Some(base) = parse_register_name(inner) {
        return Ok(MemoryToken::RegisterRelative { base, offset: 0 });
    }

    if let Some((base_str, offset_str)) = inner.split_once('+') {
        let base_str = base_str.trim();
        let offset_str = offset_str.trim();
        let Some(base) = parse_register_name(base_str) else {
            bail!("line {line}: invalid operand `{token}`");
        };
        let offset = offset_str
            .parse::<u64>()
            .map_err(|_| anyhow!("line {line}: invalid operand `{token}`"))?;
        return Ok(MemoryToken::RegisterRelative { base, offset });
    }

    bail!("line {line}: invalid operand `{token}`")
}

fn lower_instruction(
    parsed: &ParsedInstruction,
    labels: &HashMap<String, usize>,
) -> Result<Instruction> {
    validate_instruction(parsed)?;

    let mut operands = [Operand::Register(0); 4];
    for (idx, token) in parsed.operands.iter().enumerate() {
        operands[idx] = lower_operand(parsed, idx, token, labels)?;
    }

    Ok(Instruction {
        opcode: parsed.opcode,
        operands,
    })
}

fn validate_instruction(parsed: &ParsedInstruction) -> Result<()> {
    match parsed.opcode {
        OpCode::Move => {
            let dst = parsed.operands.first().expect("validated arity");
            let src = parsed.operands.get(1).expect("validated arity");

            if matches!(dst, OperandToken::Immediate(_) | OperandToken::Label(_)) {
                bail!("line {}: destination operand must be register or memory", parsed.line);
            }
            if matches!(dst, OperandToken::Memory(_)) && matches!(src, OperandToken::Memory(_)) {
                bail!("line {}: move does not support memory-to-memory transfers", parsed.line);
            }
            if let OperandToken::Register(reg) = dst {
                validate_writable_register(*reg, parsed.line)?;
            }
        }
        OpCode::Pop => match parsed.operands.first().expect("validated arity") {
            OperandToken::Register(reg) => validate_writable_register(*reg, parsed.line)?,
            _ => bail!("line {}: destination operand must be a register", parsed.line),
        },
        OpCode::Add
        | OpCode::Sub
        | OpCode::Mul
        | OpCode::Div
        | OpCode::Eq
        | OpCode::Gt
        | OpCode::Lt
        | OpCode::Gte
        | OpCode::Lte => match parsed.operands.first().expect("validated arity") {
            OperandToken::Register(reg) => validate_writable_register(*reg, parsed.line)?,
            _ => bail!("line {}: destination operand must be a register", parsed.line),
        },
        _ => {}
    }

    Ok(())
}

fn lower_operand(
    parsed: &ParsedInstruction,
    idx: usize,
    token: &OperandToken,
    labels: &HashMap<String, usize>,
) -> Result<Operand> {
    let is_jump_target = (parsed.opcode == OpCode::Jump && idx == 0)
        || (parsed.opcode == OpCode::JumpIf && idx == 1);

    if is_jump_target {
        return match token {
            OperandToken::Immediate(value) => Ok(Operand::Immediate(*value)),
            OperandToken::Label(label) => {
                let target = labels
                    .get(label)
                    .ok_or_else(|| anyhow!("line {}: unknown label `{label}`", parsed.line))?;
                Ok(Operand::Immediate(*target as u64))
            }
            _ => bail!(
                "line {}: jump target must be an immediate address or label",
                parsed.line
            ),
        };
    }

    match token {
        OperandToken::Register(value) => Ok(Operand::Register(*value)),
        OperandToken::Immediate(value) => Ok(Operand::Immediate(*value)),
        OperandToken::Memory(MemoryToken::Absolute(offset)) => Ok(Operand::Memory(MemoryRef {
            base: MemoryBase::Absolute,
            offset: *offset,
        })),
        OperandToken::Memory(MemoryToken::RegisterRelative { base, offset }) => {
            Ok(Operand::Memory(MemoryRef {
                base: MemoryBase::Register(*base),
                offset: *offset,
            }))
        }
        OperandToken::Label(label) => bail!("line {}: unknown label `{label}`", parsed.line),
    }
}

fn validate_writable_register(reg: u64, line: usize) -> Result<()> {
    match reg {
        IP_REG => bail!("line {line}: cannot write to reserved register `ip`"),
        FLAGS_REG => bail!("line {line}: cannot write to reserved register `flags`"),
        RESERVED_START..=RESERVED_END if reg > BP_REG => {
            bail!("line {line}: cannot write to reserved register `r{reg}`")
        }
        _ => Ok(()),
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
        OpCode::Push => format!("push {}", format_operand(instruction.operands[0])),
        OpCode::Pop => format!("pop {}", format_operand(instruction.operands[0])),
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
        Operand::Register(reg) => format_register(reg),
        Operand::Immediate(value) => value.to_string(),
        Operand::Memory(memory) => format_memory(memory),
    }
}

fn format_memory(memory: MemoryRef) -> String {
    match memory.base {
        MemoryBase::Absolute => format!("[{}]", memory.offset),
        MemoryBase::Register(base) if memory.offset == 0 => format!("[{}]", format_register(base)),
        MemoryBase::Register(base) => format!("[{}+{}]", format_register(base), memory.offset),
    }
}

fn format_register(reg: u64) -> String {
    match reg {
        SP_REG => "sp".to_string(),
        BP_REG => "bp".to_string(),
        IP_REG => "ip".to_string(),
        FLAGS_REG => "flags".to_string(),
        _ => format!("r{reg}"),
    }
}

fn parse_register_name(token: &str) -> Option<u64> {
    match token.to_ascii_lowercase().as_str() {
        "sp" => Some(SP_REG),
        "bp" => Some(BP_REG),
        "ip" => Some(IP_REG),
        "flags" => Some(FLAGS_REG),
        _ => token.strip_prefix('r')?.parse::<u64>().ok(),
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

fn asm_diagnostic(
    source_name: &str,
    source: &str,
    line: usize,
    message: &str,
    label: Option<&str>,
    highlight: Option<&str>,
) -> miette::Report {
    let (offset, len) = line_span(source, line, highlight);
    let mut labels = Vec::new();
    if let Some(text) = label {
        labels.push(LabeledSpan::at((offset, len), text));
    }

    miette!(labels = labels, "{message}")
        .with_source_code(NamedSource::new(source_name.to_string(), source.to_string()))
}

fn line_span(source: &str, line: usize, highlight: Option<&str>) -> (usize, usize) {
    if line == 0 {
        return (0, source.len().max(1));
    }

    let mut start = 0usize;
    let mut current = 1usize;
    for segment in source.split_inclusive('\n') {
        if current == line {
            let line_content = segment.trim_end_matches('\n');
            if let Some(target) = highlight {
                if !target.is_empty() {
                    if let Some(column) = line_content.find(target) {
                        return (start + column, target.len().max(1));
                    }
                }
            }
            let len = line_content.len().max(1);
            return (start, len);
        }
        start += segment.len();
        current += 1;
    }

    if current == line {
        return (start, 1);
    }

    (source.len().saturating_sub(1), 1)
}

fn extract_backticked_token(message: &str) -> Option<String> {
    let start = message.find('`')?;
    let rest = &message[start + 1..];
    let end = rest.find('`')?;
    let token = &rest[..end];
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
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
        assert!(error.to_string().contains("unknown label `missing`"));
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
    fn assemble_supports_memory_operands_and_aliases() {
        let src = "move [bp+8], 42\nmove r1, [sp]\npush r2\npop bp";
        let program = assemble_program(src).expect("assembly should parse");

        assert_eq!(
            program[0].operands[0],
            Operand::Memory(MemoryRef {
                base: MemoryBase::Register(BP_REG),
                offset: 8,
            })
        );
        assert_eq!(
            program[1].operands[1],
            Operand::Memory(MemoryRef {
                base: MemoryBase::Register(SP_REG),
                offset: 0,
            })
        );
        assert_eq!(program[2].opcode, OpCode::Push);
        assert_eq!(program[3].operands[0], Operand::Register(BP_REG));
    }

    #[test]
    fn assemble_rejects_invalid_memory_usage() {
        let error = assemble_program("move [r0], [r1]").expect_err("should fail");
        assert!(error.to_string().contains("memory-to-memory"));

        let error = assemble_program("move r0, [bp-8]").expect_err("should fail");
        assert!(error.to_string().contains("negative offsets"));
    }

    #[test]
    fn assemble_rejects_reserved_register_writes() {
        let error = assemble_program("move ip, 1").expect_err("should fail");
        assert!(error.to_string().contains("reserved register `ip`"));

        let error = assemble_program("pop flags").expect_err("should fail");
        assert!(error.to_string().contains("reserved register `flags`"));

        let error = assemble_program("move r20, 1").expect_err("should fail");
        assert!(error.to_string().contains("reserved register `r20`"));
    }

    #[test]
    fn disassemble_emits_labels_for_targets_and_aliases() {
        let program = vec![
            Instruction {
                opcode: OpCode::Move,
                operands: [
                    Operand::Register(SP_REG),
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
        assert!(text.contains("move sp, 0"));
    }

    #[test]
    fn asm_binary_round_trip() {
        let src = r#"
loop:
    add r0, r0, 1
    lt r1, r0, 10
    jumpif r1, loop
    move [bp+8], r0
    push 7
    pop bp
    halt
"#;

        let assembled = assemble_program(src).expect("assemble");
        let bytes = encode_program(&assembled).expect("encode");
        let decoded = decode_program(&bytes).expect("decode");
        assert_eq!(assembled, decoded);

        let disassembled = disassemble_program(&decoded);
        assert!(disassembled.contains("L0:"));
        assert!(disassembled.contains("move [bp+8], r0"));
    }
}
