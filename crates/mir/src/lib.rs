use std::{
    collections::{HashMap, HashSet},
    fmt,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyError {
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for VerifyError {}

pub fn verify(program: &Program) -> Result<(), VerifyError> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        if signatures
            .insert(
                function.name.as_str(),
                (&function.parameters, function.return_type),
            )
            .is_some()
        {
            return Err(verify_error(
                "E3001",
                format!("duplicate MIR function: {}", function.name),
            ));
        }
    }
    for function in &program.functions {
        verify_function(function, &signatures)?;
    }
    Ok(())
}

fn verify_function<'a>(
    function: &Function,
    signatures: &HashMap<&'a str, (&'a Vec<hir::Type>, hir::Type)>,
) -> Result<(), VerifyError> {
    if function.blocks.is_empty() || function.entry.0 >= function.blocks.len() {
        return Err(verify_error(
            "E3002",
            format!("invalid entry block in {}", function.name),
        ));
    }

    let reachable = reachable_blocks(function);
    if reachable.len() != function.blocks.len() {
        return Err(verify_error(
            "E3003",
            format!("unreachable block in {}", function.name),
        ));
    }

    let mut definitions = HashMap::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            if definitions
                .insert(
                    instruction.result,
                    (instruction.ty, block_index, instruction_index),
                )
                .is_some()
            {
                return Err(verify_error(
                    "E3004",
                    format!("duplicate SSA value %v{}", instruction.result.0),
                ));
            }
        }
    }

    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            match &instruction.kind {
                InstructionKind::Number(_) if instruction.ty != hir::Type::Number => {
                    return Err(verify_error(
                        "E3005",
                        "number constant has non-number type".to_owned(),
                    ));
                }
                InstructionKind::Boolean(_) if instruction.ty != hir::Type::Boolean => {
                    return Err(verify_error(
                        "E3005",
                        "boolean constant has non-boolean type".to_owned(),
                    ));
                }
                InstructionKind::Number(_) | InstructionKind::Boolean(_) => {}
                InstructionKind::LoadParam(index) => {
                    let Some(parameter_type) = function.parameters.get(*index) else {
                        return Err(verify_error(
                            "E3006",
                            format!("parameter index out of range: {index}"),
                        ));
                    };
                    if instruction.ty != *parameter_type {
                        return Err(verify_error(
                            "E3005",
                            "parameter load has incorrect type".to_owned(),
                        ));
                    }
                }
                InstructionKind::Binary {
                    left,
                    operator,
                    right,
                } => {
                    let left_type =
                        value_type(*left, block_index, instruction_index, &definitions)?;
                    let right_type =
                        value_type(*right, block_index, instruction_index, &definitions)?;
                    let expected = match operator {
                        BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide => {
                            if left_type != hir::Type::Number || right_type != hir::Type::Number {
                                return Err(verify_error(
                                    "E3005",
                                    "arithmetic operands must be numbers".to_owned(),
                                ));
                            }
                            hir::Type::Number
                        }
                        BinaryOperator::LessThan => {
                            if left_type != hir::Type::Number || right_type != hir::Type::Number {
                                return Err(verify_error(
                                    "E3005",
                                    "less-than operands must be numbers".to_owned(),
                                ));
                            }
                            hir::Type::Boolean
                        }
                        BinaryOperator::Equal => {
                            if left_type != right_type {
                                return Err(verify_error(
                                    "E3005",
                                    "equality operands must have matching types".to_owned(),
                                ));
                            }
                            hir::Type::Boolean
                        }
                    };
                    if instruction.ty != expected {
                        return Err(verify_error(
                            "E3005",
                            "binary result has incorrect type".to_owned(),
                        ));
                    }
                }
                InstructionKind::Call { callee, arguments } => {
                    let Some((parameters, return_type)) = signatures.get(callee.as_str()) else {
                        return Err(verify_error(
                            "E3007",
                            format!("unknown MIR call target: {callee}"),
                        ));
                    };
                    if arguments.len() != parameters.len() {
                        return Err(verify_error(
                            "E3008",
                            format!("call to {callee} has incorrect argument count"),
                        ));
                    }
                    for (argument, expected) in arguments.iter().zip(parameters.iter()) {
                        if value_type(*argument, block_index, instruction_index, &definitions)?
                            != *expected
                        {
                            return Err(verify_error(
                                "E3005",
                                format!("call to {callee} has an argument type mismatch"),
                            ));
                        }
                    }
                    if instruction.ty != *return_type {
                        return Err(verify_error(
                            "E3005",
                            format!("call to {callee} has an incorrect result type"),
                        ));
                    }
                }
            }
        }

        let Some(terminator) = &block.terminator else {
            return Err(verify_error(
                "E3002",
                format!("block {block_index} has no terminator"),
            ));
        };
        match terminator {
            Terminator::Return(value) => {
                if value_type(*value, block_index, block.instructions.len(), &definitions)?
                    != function.return_type
                {
                    return Err(verify_error(
                        "E3005",
                        format!("return type mismatch in {}", function.name),
                    ));
                }
            }
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                if value_type(
                    *condition,
                    block_index,
                    block.instructions.len(),
                    &definitions,
                )? != hir::Type::Boolean
                {
                    return Err(verify_error(
                        "E3005",
                        "branch condition must be boolean".to_owned(),
                    ));
                }
                verify_block_reference(function, *then_block)?;
                verify_block_reference(function, *else_block)?;
            }
            Terminator::Jump(next) => verify_block_reference(function, *next)?,
        }
    }
    Ok(())
}

fn value_type(
    value: ValueId,
    block_index: usize,
    instruction_index: usize,
    definitions: &HashMap<ValueId, (hir::Type, usize, usize)>,
) -> Result<hir::Type, VerifyError> {
    let Some((ty, definition_block, definition_index)) = definitions.get(&value).copied() else {
        return Err(verify_error(
            "E3009",
            format!("use of undefined SSA value %v{}", value.0),
        ));
    };
    if definition_block == block_index && definition_index >= instruction_index {
        return Err(verify_error(
            "E3010",
            format!("SSA value %v{} used before definition", value.0),
        ));
    }
    Ok(ty)
}

fn verify_block_reference(function: &Function, block: BlockId) -> Result<(), VerifyError> {
    if block.0 >= function.blocks.len() {
        return Err(verify_error(
            "E3002",
            format!("invalid block reference: {}", block.0),
        ));
    }
    Ok(())
}

fn reachable_blocks(function: &Function) -> HashSet<usize> {
    let mut reachable = HashSet::new();
    let mut pending = vec![function.entry.0];
    while let Some(block) = pending.pop() {
        if !reachable.insert(block) {
            continue;
        }
        let Some(terminator) = function
            .blocks
            .get(block)
            .and_then(|block| block.terminator.as_ref())
        else {
            continue;
        };
        match terminator {
            Terminator::Branch {
                then_block,
                else_block,
                ..
            } => {
                pending.push(then_block.0);
                pending.push(else_block.0);
            }
            Terminator::Jump(next) => pending.push(next.0),
            Terminator::Return(_) => {}
        }
    }
    reachable
}

fn verify_error(code: &'static str, message: String) -> VerifyError {
    VerifyError { code, message }
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
    use super::{
        BasicBlock, BlockId, Function, Program, Terminator, Value, ValueId, evaluate, lower, verify,
    };
    use ts_native_hir::Type;
    use ts_native_syntax::parse;
    use tsnative_typecheck::check;

    #[test]
    fn lowers_fibonacci_to_branching_mir_and_evaluates() {
        let source = "function fib(n: number): number { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }";
        let syntax = parse(source).unwrap();
        let typed = check(&syntax).unwrap();
        let mir = lower(&typed).unwrap();
        verify(&mir).unwrap();
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

    #[test]
    fn rejects_undefined_ssa_values() {
        let program = Program {
            functions: vec![Function {
                name: "bad".to_owned(),
                parameters: Vec::new(),
                return_type: Type::Number,
                blocks: vec![BasicBlock {
                    instructions: Vec::new(),
                    terminator: Some(Terminator::Return(ValueId(0))),
                }],
                entry: BlockId(0),
            }],
        };
        let error = verify(&program).unwrap_err();
        assert_eq!(error.code, "E3009");
    }
}
