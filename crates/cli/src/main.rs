use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use ts_native_driver::Target;
use tsnative_codegen::emit;
use tsnative_mir::{Program, lower};
use tsnative_syntax::parse;
use tsnative_typecheck::check;

fn print_usage() {
    eprintln!("usage: tsnative <check|build|run|emit-ir> <file.ts> [--target <triple>]");
}

fn compile_source(source: &str) -> Result<Program, String> {
    let program = parse(source).map_err(|error| error.to_string())?;
    let typed_program = check(&program).map_err(|error| error.to_string())?;
    lower(&typed_program).map_err(|error| error.to_string())
}

fn build_native(source_path: &str, target: &Target) -> Result<PathBuf, String> {
    if target.triple() != Target::host().triple() {
        return Err(format!(
            "native build currently supports only {}",
            Target::host().triple()
        ));
    }
    let source = fs::read_to_string(source_path)
        .map_err(|error| format!("cannot read {source_path}: {error}"))?;
    let mir_program = compile_source(&source)?;
    let Some(entry) = mir_program
        .functions
        .iter()
        .find(|function| function.name == "main")
    else {
        return Err("native build requires a function main(): number".to_owned());
    };
    if !entry.parameters.is_empty() || entry.return_type != ts_native_hir::Type::Number {
        return Err("native build requires a function main(): number".to_owned());
    }
    let llvm = emit(&mir_program).map_err(|error| error.to_string())?;
    let source_name = Path::new(source_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "source path has no valid file name".to_owned())?;
    let output_dir = Path::new("target");
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("cannot create target directory: {error}"))?;
    let ir_path = output_dir.join(format!("{source_name}.ll"));
    let executable = output_dir.join(source_name);
    fs::write(&ir_path, llvm)
        .map_err(|error| format!("cannot write {}: {error}", ir_path.display()))?;
    let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tsnative-support/core/main.c");
    let status = Command::new("clang")
        .arg("-x")
        .arg("ir")
        .arg(&ir_path)
        .arg("-x")
        .arg("c")
        .arg(&support)
        .arg("-o")
        .arg(&executable)
        .status()
        .map_err(|error| format!("cannot start clang: {error}"))?;
    if !status.success() {
        return Err(format!(
            "clang failed while building {}",
            executable.display()
        ));
    }
    Ok(executable)
}

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        print_usage();
        return ExitCode::from(2);
    };

    if command == "--version" {
        println!("tsnative 0.1.0");
        return ExitCode::SUCCESS;
    }

    let Some(source_path) = arguments.next() else {
        print_usage();
        return ExitCode::from(2);
    };

    let mut target = Target::host();
    while let Some(argument) = arguments.next() {
        if argument != "--target" {
            eprintln!("error: unexpected argument: {argument}");
            return ExitCode::from(2);
        }
        let Some(triple) = arguments.next() else {
            eprintln!("error: --target requires a target triple");
            return ExitCode::from(2);
        };
        target = match Target::parse(&triple) {
            Ok(target) => target,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(2);
            }
        };
    }

    if !matches!(command.as_str(), "check" | "build" | "run" | "emit-ir") {
        eprintln!("error: unknown command: {command}");
        print_usage();
        return ExitCode::from(2);
    }

    if let Err(error) = fs::metadata(&source_path) {
        eprintln!("error: cannot read {source_path}: {error}");
        return ExitCode::from(1);
    }

    if command == "build" || command == "run" {
        let executable = match build_native(&source_path, &target) {
            Ok(executable) => executable,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(1);
            }
        };
        if command == "build" {
            println!("built {}", executable.display());
            return ExitCode::SUCCESS;
        }
        return match Command::new(&executable).status() {
            Ok(status) => status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .map(ExitCode::from)
                .unwrap_or_else(|| ExitCode::from(1)),
            Err(error) => {
                eprintln!("error: cannot run {}: {error}", executable.display());
                ExitCode::from(1)
            }
        };
    }

    if command == "check" {
        let source = match fs::read_to_string(&source_path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("error: cannot read {source_path}: {error}");
                return ExitCode::from(1);
            }
        };
        let program = match parse(&source) {
            Ok(program) => program,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(1);
            }
        };
        let typed_program = match check(&program) {
            Ok(program) => program,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(1);
            }
        };
        return match lower(&typed_program) {
            Ok(mir_program) => {
                println!(
                    "checked {} function(s), {} MIR block(s) for target {}",
                    mir_program.functions.len(),
                    mir_program
                        .functions
                        .iter()
                        .map(|function| function.blocks.len())
                        .sum::<usize>(),
                    target.triple()
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        };
    }

    if command == "emit-ir" {
        let source = match fs::read_to_string(&source_path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("error: cannot read {source_path}: {error}");
                return ExitCode::from(1);
            }
        };
        let mir_program = match compile_source(&source) {
            Ok(program) => program,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(1);
            }
        };
        return match emit(&mir_program) {
            Ok(llvm) => {
                print!("{llvm}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        };
    }

    eprintln!(
        "{command}: parser not implemented yet (target {})",
        target.triple()
    );
    ExitCode::from(3)
}
