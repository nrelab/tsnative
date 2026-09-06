use std::{collections::HashMap, fmt};

use ts_native_hir as hir;
use ts_native_syntax as syntax;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError {
    pub code: &'static str,
    pub message: String,
    pub span: syntax::Span,
}

impl fmt::Display for TypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}..{}: {}",
            self.code, self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for TypeError {}

#[derive(Debug, Clone)]
struct Signature {
    parameters: Vec<hir::Type>,
    return_type: hir::Type,
}

pub fn check(program: &syntax::Program) -> Result<hir::Program, TypeError> {
    let signatures = collect_signatures(program)?;
    let functions = program
        .functions
        .iter()
        .map(|function| check_function(function, &signatures))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(hir::Program { functions })
}

fn collect_signatures(program: &syntax::Program) -> Result<HashMap<String, Signature>, TypeError> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        if signatures.contains_key(&function.name) {
            return Err(error(
                "E1005",
                format!("duplicate function: {}", function.name),
                function.span,
            ));
        }
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| map_type(&parameter.ty))
            .collect();
        signatures.insert(
            function.name.clone(),
            Signature {
                parameters,
                return_type: map_type(&function.return_type),
            },
        );
    }
    Ok(signatures)
}

fn check_function(
    function: &syntax::Function,
    signatures: &HashMap<String, Signature>,
) -> Result<hir::Function, TypeError> {
    let mut locals = HashMap::new();
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| {
            let ty = map_type(&parameter.ty);
            if locals.insert(parameter.name.clone(), ty).is_some() {
                return Err(error(
                    "E1005",
                    format!("duplicate parameter: {}", parameter.name),
                    parameter.span,
                ));
            }
            Ok(hir::Parameter {
                name: parameter.name.clone(),
                ty,
                span: parameter.span,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = map_type(&function.return_type);
    let body = function
        .body
        .iter()
        .map(|statement| check_statement(statement, &locals, signatures, return_type))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(hir::Function {
        name: function.name.clone(),
        parameters,
        return_type,
        body,
        span: function.span,
    })
}

fn check_statement(
    statement: &syntax::Statement,
    locals: &HashMap<String, hir::Type>,
    signatures: &HashMap<String, Signature>,
    return_type: hir::Type,
) -> Result<hir::Statement, TypeError> {
    match statement {
        syntax::Statement::Return { value, span } => {
            let value = check_expression(value, locals, signatures)?;
            if value.ty != return_type {
                return Err(error(
                    "E1002",
                    format!("return type is {:?}, expected {:?}", value.ty, return_type),
                    *span,
                ));
            }
            Ok(hir::Statement::Return { value, span: *span })
        }
        syntax::Statement::If {
            condition,
            then_body,
            else_body,
            span,
        } => {
            let condition = check_expression(condition, locals, signatures)?;
            if condition.ty != hir::Type::Boolean {
                return Err(error(
                    "E1002",
                    "if condition must be boolean".to_owned(),
                    condition.span,
                ));
            }
            let then_body = then_body
                .iter()
                .map(|statement| check_statement(statement, locals, signatures, return_type))
                .collect::<Result<Vec<_>, _>>()?;
            let else_body = else_body
                .iter()
                .map(|statement| check_statement(statement, locals, signatures, return_type))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(hir::Statement::If {
                condition,
                then_body,
                else_body,
                span: *span,
            })
        }
    }
}

fn check_expression(
    expression: &syntax::Expression,
    locals: &HashMap<String, hir::Type>,
    signatures: &HashMap<String, Signature>,
) -> Result<hir::Expression, TypeError> {
    match expression {
        syntax::Expression::Number { value, span } => Ok(hir::Expression {
            kind: hir::ExpressionKind::Number(*value),
            ty: hir::Type::Number,
            span: *span,
        }),
        syntax::Expression::Boolean { value, span } => Ok(hir::Expression {
            kind: hir::ExpressionKind::Boolean(*value),
            ty: hir::Type::Boolean,
            span: *span,
        }),
        syntax::Expression::Name { value, span } => {
            let Some(ty) = locals.get(value).copied() else {
                return Err(error("E1001", format!("unknown name: {value}"), *span));
            };
            Ok(hir::Expression {
                kind: hir::ExpressionKind::Name(value.clone()),
                ty,
                span: *span,
            })
        }
        syntax::Expression::Binary {
            left,
            operator,
            right,
            span,
        } => {
            let left = check_expression(left, locals, signatures)?;
            let right = check_expression(right, locals, signatures)?;
            let ty = match operator {
                syntax::BinaryOperator::Add
                | syntax::BinaryOperator::Subtract
                | syntax::BinaryOperator::Multiply
                | syntax::BinaryOperator::Divide => hir::Type::Number,
                syntax::BinaryOperator::LessThan | syntax::BinaryOperator::Equal => {
                    hir::Type::Boolean
                }
            };
            let operands_match = left.ty == right.ty;
            let numeric_operands = matches!(
                operator,
                syntax::BinaryOperator::Add
                    | syntax::BinaryOperator::Subtract
                    | syntax::BinaryOperator::Multiply
                    | syntax::BinaryOperator::Divide
                    | syntax::BinaryOperator::LessThan
            );
            if !operands_match || (numeric_operands && left.ty != hir::Type::Number) {
                return Err(error(
                    "E1002",
                    format!(
                        "operator {:?} cannot be applied to {:?} and {:?}",
                        operator, left.ty, right.ty
                    ),
                    *span,
                ));
            }
            Ok(hir::Expression {
                kind: hir::ExpressionKind::Binary {
                    left: Box::new(left),
                    operator: *operator,
                    right: Box::new(right),
                },
                ty,
                span: *span,
            })
        }
        syntax::Expression::Call {
            callee,
            arguments,
            span,
        } => {
            let Some(signature) = signatures.get(callee) else {
                return Err(error("E1003", format!("unknown function: {callee}"), *span));
            };
            if arguments.len() != signature.parameters.len() {
                return Err(error(
                    "E1004",
                    format!(
                        "function {callee} expects {} argument(s), got {}",
                        signature.parameters.len(),
                        arguments.len()
                    ),
                    *span,
                ));
            }
            let arguments = arguments
                .iter()
                .zip(&signature.parameters)
                .map(|(argument, expected)| {
                    let argument = check_expression(argument, locals, signatures)?;
                    if argument.ty != *expected {
                        return Err(error(
                            "E1002",
                            format!(
                                "argument has type {:?}, expected {:?}",
                                argument.ty, expected
                            ),
                            argument.span,
                        ));
                    }
                    Ok(argument)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(hir::Expression {
                kind: hir::ExpressionKind::Call {
                    callee: callee.clone(),
                    arguments,
                },
                ty: signature.return_type,
                span: *span,
            })
        }
    }
}

fn map_type(ty: &syntax::Type) -> hir::Type {
    match ty {
        syntax::Type::Number => hir::Type::Number,
        syntax::Type::Boolean => hir::Type::Boolean,
    }
}

fn error(code: &'static str, message: String, span: syntax::Span) -> TypeError {
    TypeError {
        code,
        message,
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::check;
    use ts_native_syntax::parse;

    #[test]
    fn checks_recursive_fibonacci() {
        let syntax = parse("function fib(n: number): number { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }").unwrap();
        let program = check(&syntax).unwrap();
        assert_eq!(program.functions[0].name, "fib");
    }

    #[test]
    fn rejects_unknown_names_with_stable_code() {
        let syntax = parse("function broken(n: number): number { return missing; }").unwrap();
        let error = check(&syntax).unwrap_err();
        assert_eq!(error.code, "E1001");
    }
}
