use std::{collections::HashMap, fmt};

use ts_native_hir as hir;
use ts_native_syntax::{BinaryOperator, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<hir::Type>,
    pub return_type: hir::Type,
    pub blocks: Vec<BasicBlock>,
    pub entry: BlockId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub instructions: Vec<Instruction>,
    pub terminator: Option<Terminator>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instruction {
    pub result: ValueId,
    pub ty: hir::Type,
    pub kind: InstructionKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstructionKind {
    Number(f64),
    Boolean(bool),
    LoadParam(usize),
    Binary {
        left: ValueId,
        operator: BinaryOperator,
        right: ValueId,
    },
    Call {
        callee: String,
        arguments: Vec<ValueId>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Return(ValueId),
    Branch {
        condition: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Jump(BlockId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

impl fmt::Display for LowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}..{}: {}",
            self.code, self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for LowerError {}

pub fn lower(program: &hir::Program) -> Result<Program, LowerError> {
    Ok(Program {
        functions: program
            .functions
            .iter()
            .map(lower_function)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_function(function: &hir::Function) -> Result<Function, LowerError> {
    let mut builder = Builder::new();
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    let parameter_indices = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.name.clone(), index))
        .collect::<HashMap<_, _>>();
    lower_statements(&mut builder, &function.body, &parameter_indices)?;
    if builder.current_terminator().is_none() {
        return Err(LowerError {
            code: "E2001",
            message: "function does not return on every path".to_owned(),
            span: function.span,
        });
    }
    Ok(Function {
        name: function.name.clone(),
        parameters,
        return_type: function.return_type,
        blocks: builder.blocks,
        entry: BlockId(0),
    })
}

fn lower_statements(
    builder: &mut Builder,
    statements: &[hir::Statement],
    parameter_indices: &HashMap<String, usize>,
) -> Result<(), LowerError> {
    for statement in statements {
        if builder.current_terminator().is_some() {
            break;
        }
        match statement {
            hir::Statement::Return { value, .. } => {
                let value = lower_expression(builder, value, parameter_indices)?;
                builder.terminate(Terminator::Return(value));
            }
            hir::Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let condition = lower_expression(builder, condition, parameter_indices)?;
                let then_block = builder.new_block();
                let else_block = builder.new_block();
                let join_block = builder.new_block();
                builder.terminate(Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                });

                builder.set_current(then_block);
                lower_statements(builder, then_body, parameter_indices)?;
                if builder.current_terminator().is_none() {
                    builder.terminate(Terminator::Jump(join_block));
                }

                builder.set_current(else_block);
                lower_statements(builder, else_body, parameter_indices)?;
                if builder.current_terminator().is_none() {
                    builder.terminate(Terminator::Jump(join_block));
                }
                builder.set_current(join_block);
            }
        }
    }
    Ok(())
}

fn lower_expression(
    builder: &mut Builder,
    expression: &hir::Expression,
    parameter_indices: &HashMap<String, usize>,
) -> Result<ValueId, LowerError> {
    let kind = match &expression.kind {
        hir::ExpressionKind::Number(value) => InstructionKind::Number(*value),
        hir::ExpressionKind::Boolean(value) => InstructionKind::Boolean(*value),
        hir::ExpressionKind::Name(name) => {
            let Some(index) = parameter_indices.get(name).copied() else {
                return Err(LowerError {
                    code: "E2002",
                    message: format!("unknown MIR name: {name}"),
                    span: expression.span,
                });
            };
            InstructionKind::LoadParam(index)
        }
        hir::ExpressionKind::Binary {
            left,
            operator,
            right,
        } => {
            let left = lower_expression(builder, left, parameter_indices)?;
            let right = lower_expression(builder, right, parameter_indices)?;
            InstructionKind::Binary {
                left,
                operator: *operator,
                right,
            }
        }
        hir::ExpressionKind::Call { callee, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| lower_expression(builder, argument, parameter_indices))
                .collect::<Result<Vec<_>, _>>()?;
            InstructionKind::Call {
                callee: callee.clone(),
                arguments,
            }
        }
    };
    Ok(builder.emit(expression.ty, kind, expression.span))
}

struct Builder {
    blocks: Vec<BasicBlock>,
    current: BlockId,
    next_value: usize,
}

impl Builder {
    fn new() -> Self {
        Self {
            blocks: vec![BasicBlock {
                instructions: Vec::new(),
                terminator: None,
            }],
            current: BlockId(0),
            next_value: 0,
        }
    }

    fn new_block(&mut self) -> BlockId {
        let block = BlockId(self.blocks.len());
        self.blocks.push(BasicBlock {
            instructions: Vec::new(),
            terminator: None,
        });
        block
    }

    fn set_current(&mut self, block: BlockId) {
        self.current = block;
    }

    fn emit(&mut self, ty: hir::Type, kind: InstructionKind, span: Span) -> ValueId {
        let result = ValueId(self.next_value);
        self.next_value += 1;
        self.blocks[self.current.0].instructions.push(Instruction {
            result,
            ty,
            kind,
            span,
        });
        result
    }

    fn terminate(&mut self, terminator: Terminator) {
        self.blocks[self.current.0].terminator = Some(terminator);
    }

    fn current_terminator(&self) -> Option<&Terminator> {
        self.blocks[self.current.0].terminator.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub fn evaluate(
    program: &Program,
    function: &str,
    arguments: &[Value],
) -> Result<Value, RuntimeError> {
    let functions = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    evaluate_function(&functions, function, arguments)
}

fn evaluate_function(
    functions: &HashMap<&str, &Function>,
    name: &str,
    arguments: &[Value],
) -> Result<Value, RuntimeError> {
    let function = functions
        .get(name)
        .ok_or_else(|| runtime_error(format!("unknown function: {name}")))?;
    if arguments.len() != function.parameters.len() {
        return Err(runtime_error(format!(
            "function {name} expects {} argument(s), got {}",
            function.parameters.len(),
            arguments.len()
        )));
    }
    let mut values = HashMap::new();
    let mut block = function.entry;
    loop {
        let current = &function.blocks[block.0];
        for instruction in &current.instructions {
            let value = match &instruction.kind {
                InstructionKind::Number(value) => Value::Number(*value),
                InstructionKind::Boolean(value) => Value::Boolean(*value),
                InstructionKind::LoadParam(index) => arguments[*index],
                InstructionKind::Binary {
                    left,
                    operator,
                    right,
                } => evaluate_binary(*operator, values[left], values[right])?,
                InstructionKind::Call {
                    callee,
                    arguments: argument_ids,
                } => {
                    let arguments = argument_ids.iter().map(|id| values[id]).collect::<Vec<_>>();
                    evaluate_function(functions, callee, &arguments)?
                }
            };
            values.insert(instruction.result, value);
        }
        match current
            .terminator
            .as_ref()
            .ok_or_else(|| runtime_error("unterminated MIR block".to_owned()))?
        {
            Terminator::Return(value) => return Ok(values[value]),
            Terminator::Jump(next) => block = *next,
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                block = match values[condition] {
                    Value::Boolean(true) => *then_block,
                    Value::Boolean(false) => *else_block,
                    Value::Number(_) => {
                        return Err(runtime_error("branch condition is not boolean".to_owned()));
                    }
                };
            }
        }
    }
}

fn evaluate_binary(
    operator: BinaryOperator,
    left: Value,
    right: Value,
) -> Result<Value, RuntimeError> {
    match (operator, left, right) {
        (BinaryOperator::Add, Value::Number(left), Value::Number(right)) => {
            Ok(Value::Number(left + right))
        }
        (BinaryOperator::Subtract, Value::Number(left), Value::Number(right)) => {
            Ok(Value::Number(left - right))
        }
        (BinaryOperator::Multiply, Value::Number(left), Value::Number(right)) => {
            Ok(Value::Number(left * right))
        }
        (BinaryOperator::Divide, Value::Number(left), Value::Number(right)) => {
            Ok(Value::Number(left / right))
        }
        (BinaryOperator::LessThan, Value::Number(left), Value::Number(right)) => {
            Ok(Value::Boolean(left < right))
        }
        (BinaryOperator::Equal, left, right) => Ok(Value::Boolean(left == right)),
        _ => Err(runtime_error("invalid MIR operands".to_owned())),
    }
}

fn runtime_error(message: String) -> RuntimeError {
    RuntimeError { message }
}

#[cfg(test)]
mod tests {
    use super::{Terminator, Value, evaluate, lower};
    use ts_native_syntax::parse;
    use tsnative_typecheck::check;

    #[test]
    fn lowers_fibonacci_to_branching_mir_and_evaluates() {
        let source = "function fib(n: number): number { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }";
        let syntax = parse(source).unwrap();
        let typed = check(&syntax).unwrap();
        let mir = lower(&typed).unwrap();
        assert!(
            mir.functions[0]
                .blocks
                .iter()
                .any(|block| matches!(block.terminator, Some(Terminator::Branch { .. })))
        );
        assert_eq!(
            evaluate(&mir, "fib", &[Value::Number(10.0)]).unwrap(),
            Value::Number(55.0)
        );
    }
}
