//! Tatr Query Language: compile a shell-friendly expression to a postfix program
//! and evaluate it against tasks.
//!
//! Grammar (from the format spec):
//!
//! ```text
//! expr      ::= or
//! or        ::= and *('or' and)
//! and       ::= compare *('and' compare)
//! compare   ::= primary *(cmp primary)
//! cmp       ::= 'lt' | 'le' | 'gt' | 'ge' | 'eq' | 'ne'
//! primary   ::= ':' tag | '[' expr ']' | 'not' primary
//!             | 'any' | 'tagged' | 'priority' | number | huid
//! ```
//!
//! Types are checked at compile time: booleans come from predicates, integers
//! from `priority` and numeric literals, and comparisons accept integers only.
//! That means a compiled [`Query`] can never fail to evaluate.

use std::ops::Range;

use crate::huid::Huid;
use crate::syntax::{SyntaxError, Token, tokenize};
use crate::task::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone)]
enum Op {
    Any,
    Tag(String),
    Tagged,
    Id(Huid),
    Priority,
    Int(i64),
    Not,
    And,
    Or,
    Cmp(CmpOp),
}

#[derive(Debug, Clone)]
pub struct Query {
    ops: Vec<Op>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Bool,
    Int,
}

impl Ty {
    fn name(self) -> &'static str {
        match self {
            Ty::Bool => "boolean",
            Ty::Int => "integer",
        }
    }
}

impl Query {
    /// Matches every task. Used when `ls` is called without a query.
    pub fn any() -> Self {
        Self { ops: vec![Op::Any] }
    }

    pub fn compile(source: &str) -> Result<Self, SyntaxError> {
        let mut parser = Parser {
            tokens: tokenize(source),
            index: 0,
            source,
            ops: Vec::new(),
            types: Vec::new(),
        };

        parser.parse_or()?;

        if let Some(token) = parser.peek() {
            return Err(SyntaxError::new(
                format!(
                    "unexpected `{}`; supported infix operators are `and`, `or`, and the comparisons",
                    token.text
                ),
                token.span.clone(),
            ));
        }

        match parser.types.as_slice() {
            [Ty::Bool] => Ok(Self { ops: parser.ops }),
            [Ty::Int] => Err(SyntaxError::new(
                "query evaluates to an integer; compare it, for example `priority lt 50`",
                0..source.len(),
            )),
            _ => Err(SyntaxError::new(
                "query does not evaluate to a single boolean",
                0..source.len(),
            )),
        }
    }

    pub fn matches(&self, task: &Task) -> bool {
        let mut stack: Vec<Value> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            match op {
                Op::Any => stack.push(Value::Bool(true)),
                Op::Tagged => stack.push(Value::Bool(!task.tags.is_empty())),
                Op::Tag(tag) => stack.push(Value::Bool(task.tags.iter().any(|t| t == tag))),
                Op::Id(id) => stack.push(Value::Bool(task.id == *id)),
                Op::Priority => stack.push(Value::Int(task.priority)),
                Op::Int(number) => stack.push(Value::Int(*number)),
                Op::Not => {
                    let value = pop_bool(&mut stack);
                    stack.push(Value::Bool(!value));
                }
                Op::And => {
                    let right = pop_bool(&mut stack);
                    let left = pop_bool(&mut stack);
                    stack.push(Value::Bool(left && right));
                }
                Op::Or => {
                    let right = pop_bool(&mut stack);
                    let left = pop_bool(&mut stack);
                    stack.push(Value::Bool(left || right));
                }
                Op::Cmp(kind) => {
                    let right = pop_int(&mut stack);
                    let left = pop_int(&mut stack);
                    let result = match kind {
                        CmpOp::Lt => left < right,
                        CmpOp::Le => left <= right,
                        CmpOp::Gt => left > right,
                        CmpOp::Ge => left >= right,
                        CmpOp::Eq => left == right,
                        CmpOp::Ne => left != right,
                    };
                    stack.push(Value::Bool(result));
                }
            }
        }
        debug_assert_eq!(stack.len(), 1, "type checking guarantees one result");
        matches!(stack.pop(), Some(Value::Bool(true)))
    }
}

enum Value {
    Bool(bool),
    Int(i64),
}

fn pop_bool(stack: &mut Vec<Value>) -> bool {
    match stack.pop() {
        Some(Value::Bool(value)) => value,
        _ => unreachable!("type checking guarantees a boolean on the stack"),
    }
}

fn pop_int(stack: &mut Vec<Value>) -> i64 {
    match stack.pop() {
        Some(Value::Int(value)) => value,
        _ => unreachable!("type checking guarantees an integer on the stack"),
    }
}

struct Parser<'a> {
    tokens: Vec<Token>,
    index: usize,
    source: &'a str,
    ops: Vec<Op>,
    types: Vec<Ty>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn expect(&mut self, expected: &str) -> Result<(), SyntaxError> {
        match self.next() {
            Some(token) if token.text == expected => Ok(()),
            Some(token) => Err(SyntaxError::new(
                format!("expected `{expected}` but got `{}`", token.text),
                token.span,
            )),
            None => Err(SyntaxError::new(
                format!("expected `{expected}`"),
                self.source.len()..self.source.len(),
            )),
        }
    }

    fn parse_or(&mut self) -> Result<(), SyntaxError> {
        self.parse_and()?;
        while matches!(self.peek().map(|t| t.text.as_str()), Some("or")) {
            let token = self.next().expect("peeked");
            self.parse_and()?;
            self.emit(Op::Or, &token.span)?;
        }
        Ok(())
    }

    fn parse_and(&mut self) -> Result<(), SyntaxError> {
        self.parse_compare()?;
        while matches!(self.peek().map(|t| t.text.as_str()), Some("and")) {
            let token = self.next().expect("peeked");
            self.parse_compare()?;
            self.emit(Op::And, &token.span)?;
        }
        Ok(())
    }

    fn parse_compare(&mut self) -> Result<(), SyntaxError> {
        self.parse_primary()?;
        loop {
            let op = match self.peek().map(|t| t.text.as_str()) {
                Some("lt") => CmpOp::Lt,
                Some("le") => CmpOp::Le,
                Some("gt") => CmpOp::Gt,
                Some("ge") => CmpOp::Ge,
                Some("eq") => CmpOp::Eq,
                Some("ne") => CmpOp::Ne,
                _ => return Ok(()),
            };
            let token = self.next().expect("peeked");
            self.parse_primary()?;
            self.emit(Op::Cmp(op), &token.span)?;
        }
    }

    fn parse_primary(&mut self) -> Result<(), SyntaxError> {
        let Some(token) = self.next() else {
            return Err(SyntaxError::new(
                "a primary expression is expected here",
                self.source.len()..self.source.len(),
            ));
        };

        match token.text.as_str() {
            "[" => {
                self.parse_or()?;
                self.expect("]")
            }
            "not" => {
                self.parse_primary()?;
                self.emit(Op::Not, &token.span)
            }
            "any" => self.emit(Op::Any, &token.span),
            "tagged" => self.emit(Op::Tagged, &token.span),
            "priority" => self.emit(Op::Priority, &token.span),
            text if text.starts_with(':') => {
                if text.len() == 1 {
                    return Err(SyntaxError::new("empty tag after `:`", token.span));
                }
                self.emit(Op::Tag(text[1..].to_owned()), &token.span)
            }
            text => {
                if let Ok(id) = Huid::parse(text) {
                    return self.emit(Op::Id(id), &token.span);
                }
                if is_integer(text) {
                    let number = text.parse::<i64>().map_err(|_| {
                        SyntaxError::new(
                            format!("integer `{text}` is out of range"),
                            token.span.clone(),
                        )
                    })?;
                    return self.emit(Op::Int(number), &token.span);
                }
                Err(SyntaxError::new(
                    format!("unexpected start of a primary expression `{text}`"),
                    token.span,
                ))
            }
        }
    }

    /// Emit an op and verify its operand types, so evaluation is infallible.
    fn emit(&mut self, op: Op, span: &Range<usize>) -> Result<(), SyntaxError> {
        match &op {
            Op::Any | Op::Tag(_) | Op::Tagged | Op::Id(_) => self.types.push(Ty::Bool),
            Op::Priority | Op::Int(_) => self.types.push(Ty::Int),
            Op::Not => {
                self.pop_expect(Ty::Bool, span, "`not` expects a boolean operand")?;
                self.types.push(Ty::Bool);
            }
            Op::And | Op::Or => {
                self.pop_expect(Ty::Bool, span, "`and`/`or` expect boolean operands")?;
                self.pop_expect(Ty::Bool, span, "`and`/`or` expect boolean operands")?;
                self.types.push(Ty::Bool);
            }
            Op::Cmp(_) => {
                self.pop_expect(Ty::Int, span, "comparisons expect integer operands")?;
                self.pop_expect(Ty::Int, span, "comparisons expect integer operands")?;
                self.types.push(Ty::Bool);
            }
        }
        self.ops.push(op);
        Ok(())
    }

    fn pop_expect(
        &mut self,
        expected: Ty,
        span: &Range<usize>,
        context: &str,
    ) -> Result<(), SyntaxError> {
        match self.types.pop() {
            Some(actual) if actual == expected => Ok(()),
            Some(actual) => Err(SyntaxError::new(
                format!("{context}, got {}", actual.name()),
                span.clone(),
            )),
            None => Err(SyntaxError::new(
                format!("{context}, but the expression is incomplete"),
                span.clone(),
            )),
        }
    }
}

fn is_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(tags: &[&str], priority: i64) -> Task {
        Task::new(
            Huid::parse("20261109-120000").unwrap(),
            "Task",
            tags.iter().map(|tag| (*tag).to_owned()).collect(),
            priority,
        )
    }

    fn matches(query: &str, task: &Task) -> bool {
        Query::compile(query).unwrap().matches(task)
    }

    #[test]
    fn tag_predicates() {
        let task = sample(&["bug", "ui"], 10);
        assert!(matches(":bug", &task));
        assert!(!matches(":backend", &task));
        assert!(matches(":bug and :ui", &task));
        assert!(matches(":bug and not :backend", &task));
        assert!(matches("[:bug or :backend]", &task));
        assert!(matches("tagged", &task));
        assert!(!matches("tagged", &sample(&[], 10)));
    }

    #[test]
    fn and_binds_tighter_than_or() {
        let only_a = sample(&["a"], 0);
        assert!(matches(":a or :b and :c", &only_a));
        assert!(!matches("[:a or :b] and :c", &only_a));
    }

    #[test]
    fn integer_comparisons() {
        let task = sample(&[], 10);
        assert!(matches("priority lt 50", &task));
        assert!(!matches("priority gt 50", &task));
        assert!(matches("priority ge 10 and priority le 10", &task));
        assert!(matches("priority eq 10", &task));
        assert!(matches("priority ne 11", &task));
        assert!(matches("-5 lt priority", &task));
    }

    #[test]
    fn huid_predicate() {
        let task = sample(&[], 0);
        assert!(matches("20261109-120000", &task));
        assert!(!matches("20261109-120001", &task));
    }

    #[test]
    fn not_binds_to_a_primary_not_a_comparison() {
        // `not priority lt 5` is a type error in the upstream grammar too;
        // grouping is required.
        assert!(Query::compile("not priority lt 5").is_err());
        assert!(Query::compile("not [priority lt 5]").is_ok());
    }

    #[test]
    fn rejects_type_errors_and_junk() {
        for bad in [
            "",
            ":bug lt 5",
            "not priority",
            "priority lt",
            "[ :bug",
            ":bug extra",
            "50",
            "priority lt 1 lt 2",
        ] {
            assert!(Query::compile(bad).is_err(), "{bad:?} should not compile");
        }
    }

    #[test]
    fn type_error_mentions_the_type() {
        let error = Query::compile("not priority").unwrap_err();
        assert!(error.message.contains("boolean"), "{error:?}");
        assert!(!error.render("not priority").is_empty());
    }
}
