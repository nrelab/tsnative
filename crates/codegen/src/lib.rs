use std::{collections::HashMap, fmt, fmt::Write};

use ts_native_hir::Type;
use ts_native_mir::{BasicBlock, Function, InstructionKind, Program, Terminator, ValueId};
use ts_native_syntax::BinaryOperator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenError {
    pub message: String,
}

impl fmt::Display for CodegenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CodegenError {}

pub fn emit(program: &Program) -> Result<String, CodegenError> {
    emit_for_target(program, None)
}

pub fn emit_for_target(
    program: &Program,
    target_triple: Option<&str>,
) -> Result<String, CodegenError> {
    let mut output = String::from("; ModuleID = 'tsnative'\nsource_filename = \"tsnative\"\n");
    if let Some(target_triple) = target_triple {
        writeln!(output, "target triple = \"{target_triple}\"").unwrap();
    }
    output.push('\n');
    for function in &program.functions {
        emit_function(&mut output, function)?;
        output.push('\n');
    }
    Ok(output)
}

fn emit_function(output: &mut String, function: &Function) -> Result<(), CodegenError> {
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("{} %arg{index}", llvm_type(*ty)))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        output,
        "define {} @{}({parameters}) {{",
        llvm_type(function.return_type),
        symbol(&function.name)
    )
    .unwrap();

    let value_types = collect_value_types(function);
    let mut operands = HashMap::new();
    for block in &function.blocks {
        writeln!(output, "bb{}:", block_index(function, block)).unwrap();
        emit_block(output, function, block, &value_types, &mut operands)?;
    }
    output.push_str("}\n");
    Ok(())
}

fn emit_block(
    output: &mut String,
    function: &Function,
    block: &BasicBlock,
    value_types: &HashMap<ValueId, Type>,
    operands: &mut HashMap<ValueId, String>,
) -> Result<(), CodegenError> {
    for instruction in &block.instructions {
        let operand = match &instruction.kind {
            InstructionKind::Number(value) => format_number(*value),
            InstructionKind::Boolean(value) => {
                if *value {
                    "true".to_owned()
                } else {
                    "false".to_owned()
                }
            }
            InstructionKind::LoadParam(index) => format!("%arg{index}"),
            InstructionKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_operand = operand_for(*left, operands)?;
                let right_operand = operand_for(*right, operands)?;
                let left_type = value_types
                    .get(left)
                    .copied()
                    .ok_or_else(|| missing_value(*left))?;
                let instruction_name = format!("%v{}", instruction.result.0);
                match operator {
                    BinaryOperator::Add => writeln!(
                        output,
                        "  {instruction_name} = fadd double {left_operand}, {right_operand}"
                    )
                    .unwrap(),
                    BinaryOperator::Subtract => writeln!(
                        output,
                        "  {instruction_name} = fsub double {left_operand}, {right_operand}"
                    )
                    .unwrap(),
                    BinaryOperator::Multiply => writeln!(
                        output,
                        "  {instruction_name} = fmul double {left_operand}, {right_operand}"
                    )
                    .unwrap(),
                    BinaryOperator::Divide => writeln!(
                        output,
                        "  {instruction_name} = fdiv double {left_operand}, {right_operand}"
                    )
                    .unwrap(),
                    BinaryOperator::LessThan => writeln!(
                        output,
                        "  {instruction_name} = fcmp olt double {left_operand}, {right_operand}"
                    )
                    .unwrap(),
                    BinaryOperator::Equal => writeln!(
                        output,
                        "  {instruction_name} = fcmp oeq {} {left_operand}, {right_operand}",
                        llvm_type(left_type)
                    )
                    .unwrap(),
                }
                instruction_name
            }
            InstructionKind::Call { callee, arguments } => {
                let argument_text = arguments
                    .iter()
                    .map(|value| {
                        let ty = value_types
                            .get(value)
                            .copied()
                            .ok_or_else(|| missing_value(*value))?;
                        Ok(format!(
                            "{} {}",
                            llvm_type(ty),
                            operand_for(*value, operands)?
                        ))
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?
                    .join(", ");
                let instruction_name = format!("%v{}", instruction.result.0);
                writeln!(
                    output,
                    "  {instruction_name} = call {} @{}({argument_text})",
                    llvm_type(instruction.ty),
                    symbol(callee)
                )
                .unwrap();
                instruction_name
            }
        };
        operands.insert(instruction.result, operand);
    }

    match block.terminator.as_ref().ok_or_else(|| CodegenError {
        message: "unterminated MIR block".to_owned(),
    })? {
        Terminator::Return(value) => {
            let ty = value_types
                .get(value)
                .copied()
                .ok_or_else(|| missing_value(*value))?;
            writeln!(
                output,
                "  ret {} {}",
                llvm_type(ty),
                operand_for(*value, operands)?
            )
            .unwrap();
        }
        Terminator::Jump(next) => {
            writeln!(output, "  br label %bb{}", next.0).unwrap();
        }
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            writeln!(
                output,
                "  br i1 {}, label %bb{}, label %bb{}",
                operand_for(*condition, operands)?,
                then_block.0,
                else_block.0
            )
            .unwrap();
        }
    }
    let _ = function;
    Ok(())
}

fn collect_value_types(function: &Function) -> HashMap<ValueId, Type> {
    function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| (instruction.result, instruction.ty))
        })
        .collect()
}

fn operand_for(
    value: ValueId,
    operands: &HashMap<ValueId, String>,
) -> Result<String, CodegenError> {
    operands
        .get(&value)
        .cloned()
        .ok_or_else(|| missing_value(value))
}

fn missing_value(value: ValueId) -> CodegenError {
    CodegenError {
        message: format!("MIR value %v{} is used before it is emitted", value.0),
    }
}

fn symbol(name: &str) -> String {
    format!("tsnative_{name}")
}

fn block_index(function: &Function, block: &BasicBlock) -> usize {
    function
        .blocks
        .iter()
        .position(|candidate| std::ptr::eq(candidate, block))
        .unwrap_or(0)
}

fn llvm_type(ty: Type) -> &'static str {
    match ty {
        Type::Number => "double",
        Type::Boolean => "i1",
    }
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0.000000e+00".to_owned();
    }
    format!("{value:.6e}")
        .replace("e0", "e+00")
        .replace("e-0", "e-0")
}

#[cfg(test)]
mod tests {
    use super::{emit, emit_for_target};
    use ts_native_mir::lower;
    use ts_native_syntax::parse;
    use tsnative_typecheck::check;

    #[test]
    fn emits_recursive_llvm_ir() {
        let source = "function fib(n: number): number { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }";
        let syntax = parse(source).unwrap();
        let typed = check(&syntax).unwrap();
        let mir = lower(&typed).unwrap();
        let llvm = emit(&mir).unwrap();
        assert!(llvm.contains("define double @tsnative_fib(double %arg0)"));
        assert!(llvm.contains("fcmp olt double"));
        assert!(llvm.contains("call double @tsnative_fib"));
        assert!(llvm.contains("br i1"));

        let targeted = emit_for_target(&mir, Some("x86_64-apple-darwin")).unwrap();
        assert!(targeted.contains("target triple = \"x86_64-apple-darwin\""));
    }
}
