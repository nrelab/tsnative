use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Return {
        value: Expression,
        span: Span,
    },
    If {
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Number {
        value: f64,
        span: Span,
    },
    Boolean {
        value: bool,
        span: Span,
    },
    Name {
        value: String,
        span: Span,
    },
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
        span: Span,
    },
    Call {
        callee: String,
        arguments: Vec<Expression>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    LessThan,
    Equal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}..{}: {}",
            self.code, self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for SyntaxError {}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Identifier(String),
    Unsupported(String),
    Number(f64),
    True,
    False,
    Function,
    If,
    Else,
    Return,
    NumberType,
    BooleanType,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Colon,
    Comma,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    LessThan,
    EqualEqual,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    span: Span,
}

fn lex(source: &str) -> Result<Vec<Token>, SyntaxError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut position = 0;

    while position < bytes.len() {
        let byte = bytes[position];
        if byte.is_ascii_whitespace() {
            position += 1;
            continue;
        }

        let start = position;
        let kind = match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                position += 1;
                while position < bytes.len()
                    && (bytes[position].is_ascii_alphanumeric() || bytes[position] == b'_')
                {
                    position += 1;
                }
                let word = &source[start..position];
                match word {
                    "function" => TokenKind::Function,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "return" => TokenKind::Return,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "number" => TokenKind::NumberType,
                    "boolean" => TokenKind::BooleanType,
                    "any" | "eval" | "import" | "export" | "async" | "await" | "throw" | "try"
                    | "catch" | "for" | "while" | "switch" | "class" | "new" | "Proxy"
                    | "Reflect" => TokenKind::Unsupported(word.to_owned()),
                    _ => TokenKind::Identifier(word.to_owned()),
                }
            }
            b'0'..=b'9' => {
                position += 1;
                while position < bytes.len()
                    && (bytes[position].is_ascii_digit() || bytes[position] == b'.')
                {
                    position += 1;
                }
                let text = &source[start..position];
                let value = text.parse::<f64>().map_err(|_| SyntaxError {
                    code: "E0001",
                    message: format!("invalid number literal: {text}"),
                    span: Span::new(start, position),
                })?;
                TokenKind::Number(value)
            }
            b'(' => {
                position += 1;
                TokenKind::LeftParen
            }
            b')' => {
                position += 1;
                TokenKind::RightParen
            }
            b'{' => {
                position += 1;
                TokenKind::LeftBrace
            }
            b'}' => {
                position += 1;
                TokenKind::RightBrace
            }
            b':' => {
                position += 1;
                TokenKind::Colon
            }
            b',' => {
                position += 1;
                TokenKind::Comma
            }
            b';' => {
                position += 1;
                TokenKind::Semicolon
            }
            b'+' => {
                position += 1;
                TokenKind::Plus
            }
            b'-' => {
                position += 1;
                TokenKind::Minus
            }
            b'*' => {
                position += 1;
                TokenKind::Star
            }
            b'/' => {
                position += 1;
                TokenKind::Slash
            }
            b'<' => {
                position += 1;
                TokenKind::LessThan
            }
            b'=' if bytes.get(position + 1) == Some(&b'=') => {
                position += 2;
                TokenKind::EqualEqual
            }
            _ => {
                position += 1;
                return Err(SyntaxError {
                    code: "E0001",
                    message: format!("unexpected character: {}", byte as char),
                    span: Span::new(start, position),
                });
            }
        };
        tokens.push(Token {
            kind,
            span: Span::new(start, position),
        });
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(source.len(), source.len()),
    });
    Ok(tokens)
}

pub fn parse(source: &str) -> Result<Program, SyntaxError> {
    Parser::new(lex(source)?).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse_program(&mut self) -> Result<Program, SyntaxError> {
        let mut functions = Vec::new();
        while !self.at(&TokenKind::Eof) {
            self.reject_unsupported()?;
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, SyntaxError> {
        let start = self.expect(TokenKind::Function)?.span.start;
        let (name, _) = self.expect_identifier()?;
        self.expect(TokenKind::LeftParen)?;
        let mut parameters = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                let (parameter_name, parameter_span) = self.expect_identifier()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                parameters.push(Parameter {
                    name: parameter_name,
                    ty,
                    span: parameter_span,
                });
                if !self.take(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen)?;
        self.expect(TokenKind::Colon)?;
        let return_type = self.parse_type()?;
        self.expect(TokenKind::LeftBrace)?;
        let body = self.parse_statements_until(TokenKind::RightBrace)?;
        let end = self.expect(TokenKind::RightBrace)?.span.end;
        Ok(Function {
            name,
            parameters,
            return_type,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_statements_until(
        &mut self,
        closing: TokenKind,
    ) -> Result<Vec<Statement>, SyntaxError> {
        let mut statements = Vec::new();
        while !self.at(&closing) {
            if self.at(&TokenKind::Eof) {
                return self.error("expected closing brace");
            }
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, SyntaxError> {
        self.reject_unsupported()?;
        if self.take(&TokenKind::Return) {
            let start = self.previous().span.start;
            let value = self.parse_expression()?;
            let end = self.expect(TokenKind::Semicolon)?.span.end;
            return Ok(Statement::Return {
                value,
                span: Span::new(start, end),
            });
        }
        if self.take(&TokenKind::If) {
            let start = self.previous().span.start;
            self.expect(TokenKind::LeftParen)?;
            let condition = self.parse_expression()?;
            self.expect(TokenKind::RightParen)?;
            self.expect(TokenKind::LeftBrace)?;
            let then_body = self.parse_statements_until(TokenKind::RightBrace)?;
            self.expect(TokenKind::RightBrace)?;
            let else_body = if self.take(&TokenKind::Else) {
                self.expect(TokenKind::LeftBrace)?;
                let body = self.parse_statements_until(TokenKind::RightBrace)?;
                self.expect(TokenKind::RightBrace)?;
                body
            } else {
                Vec::new()
            };
            let end = self.previous().span.end;
            return Ok(Statement::If {
                condition,
                then_body,
                else_body,
                span: Span::new(start, end),
            });
        }
        self.error("expected return or if statement")
    }

    fn parse_expression(&mut self) -> Result<Expression, SyntaxError> {
        self.parse_binary_expression(0)
    }

    fn parse_binary_expression(
        &mut self,
        minimum_precedence: u8,
    ) -> Result<Expression, SyntaxError> {
        let mut left = self.parse_primary()?;
        while let Some((operator, precedence)) = self.binary_operator() {
            if precedence < minimum_precedence {
                break;
            }
            self.position += 1;
            let right = self.parse_binary_expression(precedence + 1)?;
            let span = Span::new(expression_span(&left).start, expression_span(&right).end);
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expression, SyntaxError> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Number(value) => {
                self.position += 1;
                Ok(Expression::Number {
                    value,
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.position += 1;
                Ok(Expression::Boolean {
                    value: true,
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.position += 1;
                Ok(Expression::Boolean {
                    value: false,
                    span: token.span,
                })
            }
            TokenKind::Identifier(name) => {
                self.position += 1;
                if self.take(&TokenKind::LeftParen) {
                    let mut arguments = Vec::new();
                    if !self.at(&TokenKind::RightParen) {
                        loop {
                            arguments.push(self.parse_expression()?);
                            if !self.take(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let end = self.expect(TokenKind::RightParen)?.span.end;
                    Ok(Expression::Call {
                        callee: name,
                        arguments,
                        span: Span::new(token.span.start, end),
                    })
                } else {
                    Ok(Expression::Name {
                        value: name,
                        span: token.span,
                    })
                }
            }
            TokenKind::LeftParen => {
                self.position += 1;
                let expression = self.parse_expression()?;
                self.expect(TokenKind::RightParen)?;
                Ok(expression)
            }
            TokenKind::Unsupported(name) => Err(SyntaxError {
                code: "E0003",
                message: format!("unsupported syntax: {name}"),
                span: token.span,
            }),
            _ => self.error("expected expression"),
        }
    }

    fn parse_type(&mut self) -> Result<Type, SyntaxError> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::NumberType => {
                self.position += 1;
                Ok(Type::Number)
            }
            TokenKind::BooleanType => {
                self.position += 1;
                Ok(Type::Boolean)
            }
            TokenKind::Unsupported(name) => Err(SyntaxError {
                code: "E0003",
                message: format!("unsupported type or construct: {name}"),
                span: token.span,
            }),
            _ => self.error("expected number or boolean type"),
        }
    }

    fn binary_operator(&self) -> Option<(BinaryOperator, u8)> {
        match self.current().kind {
            TokenKind::EqualEqual => Some((BinaryOperator::Equal, 1)),
            TokenKind::LessThan => Some((BinaryOperator::LessThan, 1)),
            TokenKind::Plus | TokenKind::Minus => Some((
                if self.current().kind == TokenKind::Plus {
                    BinaryOperator::Add
                } else {
                    BinaryOperator::Subtract
                },
                2,
            )),
            TokenKind::Star | TokenKind::Slash => Some((
                if self.current().kind == TokenKind::Star {
                    BinaryOperator::Multiply
                } else {
                    BinaryOperator::Divide
                },
                3,
            )),
            _ => None,
        }
    }

    fn expect_identifier(&mut self) -> Result<(String, Span), SyntaxError> {
        match self.current().kind.clone() {
            TokenKind::Identifier(name) => {
                let span = self.current().span;
                self.position += 1;
                Ok((name, span))
            }
            _ => self.error("expected identifier"),
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token, SyntaxError> {
        if self.current().kind == expected {
            let token = self.current().clone();
            self.position += 1;
            Ok(token)
        } else {
            self.error(&format!("expected {:?}", expected))
        }
    }

    fn take(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.current().kind == *kind
    }
    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.position - 1]
    }

    fn reject_unsupported(&self) -> Result<(), SyntaxError> {
        if let TokenKind::Unsupported(name) = &self.current().kind {
            return Err(SyntaxError {
                code: "E0003",
                message: format!("unsupported syntax: {name}"),
                span: self.current().span,
            });
        }
        Ok(())
    }

    fn error<T>(&self, message: &str) -> Result<T, SyntaxError> {
        Err(SyntaxError {
            code: "E0002",
            message: message.to_owned(),
            span: self.current().span,
        })
    }
}

fn expression_span(expression: &Expression) -> Span {
    match expression {
        Expression::Number { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::Name { span, .. }
        | Expression::Binary { span, .. }
        | Expression::Call { span, .. } => *span,
    }
}

#[cfg(test)]
mod tests {
    use super::{BinaryOperator, Expression, Statement, Type, parse};

    #[test]
    fn parses_recursive_fibonacci_shape() {
        let program = parse("function fib(n: number): number { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }").unwrap();
        assert_eq!(program.functions[0].name, "fib");
        assert_eq!(program.functions[0].parameters[0].ty, Type::Number);
        assert!(matches!(program.functions[0].body[0], Statement::If { .. }));
        assert!(matches!(
            program.functions[0].body[1],
            Statement::Return {
                value: Expression::Binary {
                    operator: BinaryOperator::Add,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn reports_stable_error_for_invalid_source() {
        let error = parse("function broken(n: number): number { return n }").unwrap_err();
        assert_eq!(error.code, "E0002");
        assert!(error.message.contains("Semicolon"));
    }

    #[test]
    fn reports_unsupported_type_with_stable_code() {
        let error = parse("function dynamic(value: any): number { return 1; }").unwrap_err();
        assert_eq!(error.code, "E0003");
        assert!(error.message.contains("any"));
    }

    #[test]
    fn reports_unsupported_expression_with_stable_code() {
        let error = parse("function dynamic(): number { return eval(1); }").unwrap_err();
        assert_eq!(error.code, "E0003");
        assert!(error.message.contains("eval"));
    }
}
