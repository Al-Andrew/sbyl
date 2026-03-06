use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result as MietteResult, WrapErr, miette};
use reglang::{
    Vm, assemble_program_with_context, decode_program, disassemble_program, encode_program,
};

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
        #[arg(long, default_value_t = false)]
        print_registers: bool,
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

fn main() -> MietteResult<()> {
    run_cli()
}

fn run_cli() -> MietteResult<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            input,
            input_flag,
            print_registers,
        } => {
            let input = required_arg(input, input_flag, "input")?;
            run_command(&input, print_registers)?;
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
) -> MietteResult<PathBuf> {
    positional
        .or(named)
        .ok_or_else(|| miette!("missing required argument: {arg_name}"))
}

fn optional_arg(positional: Option<PathBuf>, named: Option<PathBuf>) -> Option<PathBuf> {
    positional.or(named)
}

fn run_command(input: &Path, print_registers: bool) -> MietteResult<()> {
    let extension = extension(input)?;
    let program = match extension {
        "sby" => {
            let bytes = fs::read(input)
                .into_diagnostic()
                .wrap_err_with(|| format!("failed reading bytecode file: {}", input.display()))?;
            decode_program(&bytes)
                .map_err(|error| miette!("{error:#}"))
                .wrap_err_with(|| format!("failed decoding bytecode file: {}", input.display()))?
        }
        "sas" => {
            let source = fs::read_to_string(input)
                .into_diagnostic()
                .wrap_err_with(|| format!("failed reading assembly source: {}", input.display()))?;
            assemble_program_with_context(&input.display().to_string(), &source)
                .wrap_err_with(|| format!("failed assembling source file: {}", input.display()))?
        }
        _ => {
            return Err(miette!(
                "unsupported input extension `{extension}`, expected .sas or .sby"
            ));
        }
    };

    let mut vm = Vm::new(program);
    vm.run()
        .map_err(|error| miette!("{error:#}"))
        .wrap_err_with(|| format!("failed executing program: {}", input.display()))?;

    if print_registers {
        println!("Final registers:");
        println!("{}", vm.format_registers_compact(3));
    }

    Ok(())
}

fn assemble_command(input: &Path, output: &Path) -> MietteResult<()> {
    ensure_extension(input, "sas")?;
    ensure_extension(output, "sby")?;

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed reading assembly source: {}", input.display()))?;
    let program = assemble_program_with_context(&input.display().to_string(), &source)
        .wrap_err_with(|| format!("failed assembling source file: {}", input.display()))?;
    let bytes = encode_program(&program)
        .map_err(|error| miette!("{error:#}"))
        .wrap_err_with(|| format!("failed encoding bytecode for: {}", input.display()))?;
    fs::write(output, bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed writing bytecode output: {}", output.display()))?;
    Ok(())
}

fn disassemble_command(input: &Path, output: Option<&Path>) -> MietteResult<()> {
    ensure_extension(input, "sby")?;

    let bytes = fs::read(input)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed reading bytecode file: {}", input.display()))?;
    let program = decode_program(&bytes)
        .map_err(|error| miette!("{error:#}"))
        .wrap_err_with(|| format!("failed decoding bytecode file: {}", input.display()))?;
    let mut assembly = disassemble_program(&program);
    if !assembly.is_empty() {
        assembly.push('\n');
    }

    if let Some(output_path) = output {
        ensure_extension(output_path, "sas")?;
        fs::write(output_path, assembly)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "failed writing disassembled source file: {}",
                    output_path.display()
                )
            })?;
    } else {
        print!("{assembly}");
    }

    Ok(())
}

fn extension(path: &Path) -> MietteResult<&str> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| miette!("file has no valid extension: {}", path.display()))
}

fn ensure_extension(path: &Path, expected: &str) -> MietteResult<()> {
    let ext = extension(path)?;
    if ext == expected {
        Ok(())
    } else {
        Err(miette!(
            "expected .{expected} file, got .{ext}: {}",
            path.display()
        ))
    }
}
