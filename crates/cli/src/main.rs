use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use ts_native_driver::Target;
use tsnative_codegen::emit_for_target;
use tsnative_mir::{Program, lower, verify};
use tsnative_syntax::parse;
use tsnative_typecheck::check;

fn print_usage() {
    eprintln!(
        "usage: tsnative <doctor|check|build|run|repro-check|emit-ir> [file.ts] [--target <triple>] [-o <path>]"
    );
}

fn run_doctor() -> ExitCode {
    let mut failed = false;
    println!("tsnative doctor");
    println!("host target: {}", Target::host().triple());

    for (tool, arguments) in [("rustc", ["--version"]), ("clang", ["--version"])] {
        match Command::new(tool).args(arguments).output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                println!(
                    "ok: {tool} {}",
                    version.lines().next().unwrap_or("unknown version")
                );
            }
            Ok(output) => {
                failed = true;
                let details = String::from_utf8_lossy(&output.stderr);
                eprintln!("error: {tool} exited unsuccessfully: {}", details.trim());
            }
            Err(error) => {
                failed = true;
                eprintln!("error: {tool} is unavailable: {error}");
            }
        }
    }

    let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tsnative-support/core/main.c");
    let support_header =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tsnative-support/core/runtime.h");
    for path in [&support, &support_header] {
        if path.is_file() {
            println!("ok: support file {}", path.display());
        } else {
            failed = true;
            eprintln!("error: support file is missing: {}", path.display());
        }
    }

    if failed {
        eprintln!("doctor: failed; install the missing tools and retry");
        ExitCode::from(1)
    } else {
        println!("doctor: ok");
        ExitCode::SUCCESS
    }
}

fn compile_source(source: &str) -> Result<Program, String> {
    let program = parse(source).map_err(|error| error.to_string())?;
    let typed_program = check(&program).map_err(|error| error.to_string())?;
    let mir_program = lower(&typed_program).map_err(|error| error.to_string())?;
    verify(&mir_program).map_err(|error| error.to_string())?;
    Ok(mir_program)
}

fn build_native(
    source_path: &str,
    target: &Target,
    output: Option<&Path>,
) -> Result<PathBuf, String> {
    build_native_in_dir(source_path, target, Path::new("target"), output)
}

fn build_native_in_dir(
    source_path: &str,
    target: &Target,
    output_dir: &Path,
    output: Option<&Path>,
) -> Result<PathBuf, String> {
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
    let llvm =
        emit_for_target(&mir_program, Some(target.triple())).map_err(|error| error.to_string())?;
    let source_name = Path::new(source_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "source path has no valid file name".to_owned())?;
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("cannot create target directory: {error}"))?;
    let ir_path = output_dir.join(format!("{source_name}.ll"));
    let executable = output
        .map(PathBuf::from)
        .unwrap_or_else(|| output_dir.join(source_name));
    if let Some(parent) = executable.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create output directory: {error}"))?;
        }
    }
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

fn run_repro_check(source_path: &str, target: &Target) -> Result<(), String> {
    let source_name = Path::new(source_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "source path has no valid file name".to_owned())?;
    let root = Path::new("target").join("repro-check").join(source_name);
    let first_dir = root.join("first");
    let second_dir = root.join("second");
    fs::create_dir_all(&first_dir).map_err(|error| {
        format!(
            "cannot create reproducibility directory {}: {error}",
            first_dir.display()
        )
    })?;
    fs::create_dir_all(&second_dir).map_err(|error| {
        format!(
            "cannot create reproducibility directory {}: {error}",
            second_dir.display()
        )
    })?;

    let first = build_native_in_dir(source_path, target, &first_dir, None)?;
    let second = build_native_in_dir(source_path, target, &second_dir, None)?;
    let first_ir = fs::read(first.with_extension("ll"))
        .map_err(|error| format!("cannot read first LLVM artifact: {error}"))?;
    let second_ir = fs::read(second.with_extension("ll"))
        .map_err(|error| format!("cannot read second LLVM artifact: {error}"))?;
    let first_executable =
        fs::read(&first).map_err(|error| format!("cannot read first executable: {error}"))?;
    let second_executable =
        fs::read(&second).map_err(|error| format!("cannot read second executable: {error}"))?;
    let ir_equal = first_ir == second_ir;
    let executable_equal = first_executable == second_executable;
    fs::remove_dir_all(&root).map_err(|error| {
        format!(
            "cannot clean reproducibility directory {}: {error}",
            root.display()
        )
    })?;

    if !ir_equal || !executable_equal {
        return Err(format!(
            "reproducibility mismatch: LLVM IR {}, executable {}",
            if ir_equal { "identical" } else { "differing" },
            if executable_equal {
                "identical"
            } else {
                "differing"
            }
        ));
    }
    Ok(())
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
    if command == "doctor" {
        if arguments.next().is_some() {
            eprintln!("error: doctor does not accept positional arguments");
            print_usage();
            return ExitCode::from(2);
        }
        return run_doctor();
    }

    let Some(source_path) = arguments.next() else {
        print_usage();
        return ExitCode::from(2);
    };
    let mut target = Target::host();
    let mut output = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--target" => {
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
            "-o" | "--output" => {
                let Some(path) = arguments.next() else {
                    eprintln!("error: {argument} requires an output path");
                    return ExitCode::from(2);
                };
                output = Some(PathBuf::from(path));
            }
            _ => {
                eprintln!("error: unexpected argument: {argument}");
                return ExitCode::from(2);
            }
        }
    }

    if !matches!(
        command.as_str(),
        "check" | "build" | "run" | "repro-check" | "emit-ir"
    ) {
        eprintln!("error: unknown command: {command}");
        print_usage();
        return ExitCode::from(2);
    }
    if let Err(error) = fs::metadata(&source_path) {
        eprintln!("error: cannot read {source_path}: {error}");
        return ExitCode::from(1);
    }

    if command == "build" || command == "run" {
        let executable = match build_native(&source_path, &target, output.as_deref()) {
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
    if command == "repro-check" {
        if output.is_some() {
            eprintln!("error: repro-check does not accept an output path");
            return ExitCode::from(2);
        }
        return match run_repro_check(&source_path, &target) {
            Ok(()) => {
                println!("reproducible: LLVM IR and executable are byte-identical");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
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
    match emit_for_target(&mir_program, Some(target.triple())) {
        Ok(llvm) => {
            print!("{llvm}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}
