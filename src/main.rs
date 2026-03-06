use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use reglang::{Operand, Vm, assemble_program, decode_program, disassemble_program, encode_program};

#[derive(Parser, Debug)]
#[command(name = "reglang")]
#[command(about = "RegLang VM tools", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Run {
        input: Option<PathBuf>,
        #[arg(long, value_name = "INPUT", conflicts_with = "input")]
        input_flag: Option<PathBuf>,
    },
    Assemble {
        input: Option<PathBuf>,
        output: Option<PathBuf>,
        #[arg(long, value_name = "INPUT", conflicts_with = "input")]
        input_flag: Option<PathBuf>,
        #[arg(long, value_name = "OUTPUT", conflicts_with = "output")]
        output_flag: Option<PathBuf>,
    },
    Disassemble {
        input: Option<PathBuf>,
        output: Option<PathBuf>,
        #[arg(long, value_name = "INPUT", conflicts_with = "input")]
        input_flag: Option<PathBuf>,
        #[arg(long, value_name = "OUTPUT", conflicts_with = "output")]
        output_flag: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { input, input_flag } => {
            let input = required_arg(input, input_flag, "input")?;
            run_command(&input)?;
        }
        Commands::Assemble {
            input,
            output,
            input_flag,
            output_flag,
        } => {
            let input = required_arg(input, input_flag, "input")?;
            let output = required_arg(output, output_flag, "output")?;
            assemble_command(&input, &output)?;
        }
        Commands::Disassemble {
            input,
            output,
            input_flag,
            output_flag,
        } => {
            let input = required_arg(input, input_flag, "input")?;
            let output = optional_arg(output, output_flag);
            disassemble_command(&input, output.as_deref())?;
        }
    }

    Ok(())
}

fn required_arg(
    positional: Option<PathBuf>,
    named: Option<PathBuf>,
    arg_name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    positional
        .or(named)
        .ok_or_else(|| format!("missing required argument: {arg_name}").into())
}

fn optional_arg(positional: Option<PathBuf>, named: Option<PathBuf>) -> Option<PathBuf> {
    positional.or(named)
}

fn run_command(input: &Path) -> Result<(), Box<dyn Error>> {
    let extension = extension(input)?;
    let program = match extension {
        "sby" => {
            let bytes = fs::read(input)?;
            decode_program(&bytes)?
        }
        "sas" => {
            let source = fs::read_to_string(input)?;
            assemble_program(&source)?
        }
        _ => {
            return Err(
                format!("unsupported input extension `{extension}`, expected .sas or .sby").into(),
            );
        }
    };

    let mut vm = Vm::new(program);
    vm.run();

    println!("Final registers:");
    for reg in 0..=3 {
        println!("r{reg} = {}", vm.read_operand(Operand::Register(reg)));
    }

    Ok(())
}

fn assemble_command(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    ensure_extension(input, "sas")?;
    ensure_extension(output, "sby")?;

    let source = fs::read_to_string(input)?;
    let program = assemble_program(&source)?;
    let bytes = encode_program(&program)?;
    fs::write(output, bytes)?;
    Ok(())
}

fn disassemble_command(input: &Path, output: Option<&Path>) -> Result<(), Box<dyn Error>> {
    ensure_extension(input, "sby")?;

    let bytes = fs::read(input)?;
    let program = decode_program(&bytes)?;
    let mut assembly = disassemble_program(&program);
    if !assembly.is_empty() {
        assembly.push('\n');
    }

    if let Some(output_path) = output {
        ensure_extension(output_path, "sas")?;
        fs::write(output_path, assembly)?;
    } else {
        print!("{assembly}");
    }

    Ok(())
}

fn extension(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| format!("file has no valid extension: {}", path.display()).into())
}

fn ensure_extension(path: &Path, expected: &str) -> Result<(), Box<dyn Error>> {
    let ext = extension(path)?;
    if ext == expected {
        Ok(())
    } else {
        Err(format!(
            "expected .{expected} file, got .{ext}: {}",
            path.display()
        )
        .into())
    }
}
