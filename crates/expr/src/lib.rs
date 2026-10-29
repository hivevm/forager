// Evaluates the expressions of the style IR as msEvalExpression, src/maputil.c, and the grammar of
// src/mapparser.y evaluate MapServer's, of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The evaluator of the style IR's expressions (ADR-0031): synchronous, without I/O, reading a
//! feature's properties through [`Lookup`]. Each operator and function means what MapServer's
//! grammar means by it; `docs/semantics/expression.md` states each.
//!
//! The grammar evaluates every operand before an operator, so a failure anywhere in a tree, a
//! division by zero say, fails the whole expression, which then does not match.

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod c;
mod pattern;
mod time;

use std::borrow::Cow;
use std::collections::BTreeMap;

use forager_style_ir::{Expr, MAX_EXPRESSION_DEPTH, Op};

pub use crate::c::{atof, atoi};
pub use crate::pattern::SIZE_LIMIT;

/// The properties of one feature, as the evaluator reads them.
pub trait Lookup {
    /// The text of the property `name`, or `None` where the feature has none of that name or its
    /// value is null; both read as the empty text, as MapServer reads an attribute a shape has no
    /// value for.
    fn text(&self, name: &str) -> Option<Cow<'_, str>>;
}

/// Why an expression has no value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// MapServer's evaluation fails with a parse error there, or C leaves the result undefined: a
    /// division or a `%` by zero, a `tostring` msToString refuses, a time msParseTime rejects,
    /// operands of types the grammar does not combine, a tree past the depth the IR allows.
    Undefined,
    /// A pattern with a construct PCRE2 reads and the `regex` crate cannot match as PCRE2 does
    /// (ADR-0031), to be reported once per render with what carries it.
    Pattern {
        pattern: String,
        construct: &'static str,
    },
    /// A text past [`MAX_TEXT_BYTES`] (Q-9), to be reported once per render with what carries
    /// it.
    TooLong,
}

/// The longest text an expression reads or computes, in bytes (Q-9); one longer fails the
/// expression.
pub const MAX_TEXT_BYTES: usize = 16 << 20;

/// `value`, or [`Failure::TooLong`] for a text past [`MAX_TEXT_BYTES`].
fn bounded(value: Val) -> Result<Val, Failure> {
    match &value {
        Val::Str(s) if s.len() > MAX_TEXT_BYTES => Err(Failure::TooLong),
        _ => Ok(value),
    }
}

/// How many compiled patterns a render keeps, beyond which one is compiled each time it is used,
/// so that the cache stays bounded (Q-9).
const MAX_CACHED_PATTERNS: usize = 1024;

/// An evaluator for one render: it compiles each pattern a tree holds as a literal once and keeps
/// it; a pattern computed per feature, from a property say, is compiled where it is used and
/// dropped, as mapparser.y compiles and frees one for each `~`, so that features cannot fill the
/// cache (Q-9).
#[derive(Debug, Default)]
pub struct Evaluator {
    patterns: BTreeMap<(Vec<u8>, bool), pattern::Compiled>,
}

/// A value of the grammar: a condition, a number, a string as C's bytes, a time, a list.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    Bool(bool),
    Num(f64),
    Str(Vec<u8>),
    Time(time::Time),
    List(Vec<Val>),
}

/// A C string: the bytes up to the first NUL.
fn c_string(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())].to_vec()
}

/// A number as `%g` prints it.
fn g(number: f64) -> Vec<u8> {
    c::to_string(b"%g", number).expect("%g is a valid format")
}

impl Evaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many compiled patterns the evaluator keeps.
    #[cfg(test)]
    fn cached(&self) -> usize {
        self.patterns.len()
    }

    /// Whether `expr` matches the feature, as msEvalExpression decides for a class or a filter:
    /// a condition as it is, a number when it is not 0, a string always.
    pub fn matches(&mut self, expr: &Expr, feature: &dyn Lookup) -> Result<bool, Failure> {
        match self.eval(expr, feature, 1)? {
            Val::Bool(b) => Ok(b),
            Val::Num(n) => Ok(n != 0.0),
            Val::Str(_) => Ok(true),
            Val::Time(_) | Val::List(_) => Err(Failure::Undefined),
        }
    }

    /// The value of `expr` as a number, as msEvalDoubleExpression reads it: a string with `atof`.
    pub fn number(&mut self, expr: &Expr, feature: &dyn Lookup) -> Result<f64, Failure> {
        match self.eval(expr, feature, 1)? {
            Val::Num(n) => Ok(n),
            Val::Str(s) => Ok(atof(&s)),
            Val::Bool(_) => Ok(0.0),
            Val::Time(_) | Val::List(_) => Err(Failure::Undefined),
        }
    }

    /// The value of `expr` as text, as msEvalTextExpression gives it: a number with `%g`, a
    /// condition as `true` or `false`.
    pub fn text(&mut self, expr: &Expr, feature: &dyn Lookup) -> Result<Vec<u8>, Failure> {
        let value = self.eval(expr, feature, 1)?;
        text_of(value).ok_or(Failure::Undefined)
    }

    fn eval(&mut self, expr: &Expr, feature: &dyn Lookup, depth: usize) -> Result<Val, Failure> {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(Failure::Undefined);
        }
        Ok(match expr {
            Expr::Bool(b) => Val::Bool(*b),
            Expr::Number(n) => Val::Num(*n),
            Expr::String(s) => Val::Str(c_string(s)),
            Expr::Property { property } => bounded(Val::Str(
                feature
                    .text(property)
                    .map(|text| c_string(&text))
                    .unwrap_or_default(),
            ))?,
            Expr::Date { date: text } | Expr::Timestamp { timestamp: text } => {
                Val::Time(time::parse(&c_string(text)).ok_or(Failure::Undefined)?)
            }
            Expr::List(items) => Val::List(
                items
                    .iter()
                    .map(|item| self.eval(item, feature, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            Expr::Call { op, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.eval(arg, feature, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                let literal_pattern = matches!(args_of(expr).get(1), Some(Expr::String(_)));
                bounded(self.apply(*op, args, literal_pattern)?)?
            }
        })
    }

    /// `op` over `args`; `literal_pattern` tells that a pattern is a literal of the tree.
    fn apply(&mut self, op: Op, args: Vec<Val>, literal_pattern: bool) -> Result<Val, Failure> {
        use Val::{Bool, List, Num, Str, Time};
        let undefined = Err(Failure::Undefined);
        Ok(match (op, args.as_slice()) {
            (Op::Eq, [a, b]) => Bool(equal(a, b)?),
            (Op::Ne, [a, b]) => Bool(!equal(a, b)?),
            (Op::Lt | Op::Gt | Op::Le | Op::Ge, [a, b]) => Bool(ordered(op, a, b)?),
            (Op::In, [Str(s), List(items)]) => {
                let mut found = false;
                for item in items {
                    match item {
                        Str(item) => found |= item == s,
                        _ => return undefined,
                    }
                }
                Bool(found)
            }
            (Op::In, [Num(n), List(items)]) => {
                let mut found = false;
                for item in items {
                    match item {
                        Num(item) => found |= item == n,
                        _ => return undefined,
                    }
                }
                Bool(found)
            }
            (Op::And, args) if !args.is_empty() => Bool(conditions(args)?.iter().all(|&b| b)),
            (Op::Or, args) if !args.is_empty() => Bool(conditions(args)?.iter().any(|&b| b)),
            (Op::Not, [Bool(b)]) => Bool(!b),
            (Op::Add, [Num(a), Num(b)]) => Num(a + b),
            (Op::Sub, [Num(a), Num(b)]) => Num(a - b),
            (Op::Mul, [Num(a), Num(b)]) => Num(a * b),
            (Op::Div, [Num(a), Num(b)]) => {
                if *b == 0.0 {
                    return undefined;
                }
                Num(a / b)
            }
            (Op::Mod, [Num(a), Num(b)]) => Num(f64::from(modulo(*a, *b)?)),
            (Op::Pow, [Num(a), Num(b)]) => Num(libm::pow(*a, *b)),
            (Op::CaseI, [Str(s)]) => Str(s.to_ascii_lowercase()),
            (Op::TBefore, [Time(a), Time(b)]) => Bool(a < b),
            (Op::TAfter, [Time(a), Time(b)]) => Bool(a > b),
            (Op::TEquals, [Time(a), Time(b)]) => Bool(a == b),
            (Op::ToNumber, [Num(n)]) => Num(*n),
            (Op::ToNumber, [Str(s)]) => Num(atof(s)),
            (Op::ToString, [value]) => Str(text_of(value.clone()).ok_or(Failure::Undefined)?),
            (Op::ToTimestamp, [Time(t)]) => Time(*t),
            (Op::ToTimestamp, [Str(s)]) => Time(time::parse(s).ok_or(Failure::Undefined)?),
            (Op::RegexpLike, [Str(value), Str(pattern), Str(flags)]) => {
                let caseless = match flags.as_slice() {
                    b"" => false,
                    b"i" => true,
                    _ => return undefined,
                };
                Bool(self.regexp_like(value, pattern, caseless, literal_pattern)?)
            }
            (Op::Concat, args) => {
                let length = args
                    .iter()
                    .map(|arg| match arg {
                        Str(s) => s.len(),
                        _ => 0,
                    })
                    .fold(0usize, usize::saturating_add);
                if length > MAX_TEXT_BYTES {
                    return Err(Failure::TooLong);
                }
                let mut joined = Vec::with_capacity(length);
                for arg in args {
                    match arg {
                        Str(s) => joined.extend_from_slice(s),
                        _ => return undefined,
                    }
                }
                Str(joined)
            }
            (Op::Upper, [Str(s)]) => Str(s.to_ascii_uppercase()),
            (Op::Lower, [Str(s)]) => Str(s.to_ascii_lowercase()),
            (Op::Initcap, [Str(s)]) => {
                let mut s = s.clone();
                c::initcap(&mut s);
                Str(s)
            }
            (Op::Firstcap, [Str(s)]) => {
                let mut s = s.clone();
                c::firstcap(&mut s);
                Str(s)
            }
            (Op::Commify, [Str(s)]) => Str(c::commify(s)),
            (Op::Length, [Str(s)]) => Num(s.len() as f64),
            (Op::Round, [Num(x)]) => Num(lrint(*x)?),
            (Op::Round, [Num(x), Num(n)]) => Num(lrint(x / n)? * n),
            (Op::Tostring, [Num(x), Str(format)]) => {
                Str(c::to_string(format, *x).ok_or(Failure::Undefined)?)
            }
            _ => return undefined,
        })
    }

    /// `~` and `~*` of mapparser.y: false for an empty value, before the pattern is compiled;
    /// false for a pattern PCRE2 refuses.
    fn regexp_like(
        &mut self,
        value: &[u8],
        pattern: &[u8],
        caseless: bool,
        literal: bool,
    ) -> Result<bool, Failure> {
        if value.is_empty() {
            return Ok(false);
        }
        let key = (pattern.to_vec(), caseless);
        let fresh;
        let compiled = match self.patterns.get(&key) {
            Some(compiled) => compiled,
            None => {
                fresh = pattern::compile(pattern, caseless);
                if literal && self.patterns.len() < MAX_CACHED_PATTERNS {
                    self.patterns.insert(key.clone(), fresh.clone());
                }
                &fresh
            }
        };
        match compiled {
            pattern::Compiled::Matcher(regex) => Ok(regex.is_match(value)),
            pattern::Compiled::Refused => Ok(false),
            pattern::Compiled::Unsupported(construct) => Err(Failure::Pattern {
                pattern: String::from_utf8_lossy(pattern).into_owned(),
                construct,
            }),
        }
    }
}

/// The arguments of a call, none for any other node.
fn args_of(expr: &Expr) -> &[Expr] {
    match expr {
        Expr::Call {
            op: Op::RegexpLike,
            args,
        } => args,
        _ => &[],
    }
}

/// A value as text, where the grammar gives one.
fn text_of(value: Val) -> Option<Vec<u8>> {
    match value {
        Val::Str(s) => Some(s),
        Val::Num(n) => Some(g(n)),
        Val::Bool(b) => Some(if b {
            b"true".to_vec()
        } else {
            b"false".to_vec()
        }),
        Val::Time(_) | Val::List(_) => None,
    }
}

/// `=` of the grammar: numbers as doubles, strings as `strcmp`, conditions alike.
fn equal(a: &Val, b: &Val) -> Result<bool, Failure> {
    match (a, b) {
        (Val::Num(a), Val::Num(b)) => Ok(a == b),
        (Val::Str(a), Val::Str(b)) => Ok(a == b),
        (Val::Bool(a), Val::Bool(b)) => Ok(a == b),
        _ => Err(Failure::Undefined),
    }
}

/// `<`, `>`, `<=`, `>=` of numbers as doubles, of strings as `strcmp` orders them, by unsigned
/// bytes.
fn ordered(op: Op, a: &Val, b: &Val) -> Result<bool, Failure> {
    let ordering = match (a, b) {
        (Val::Num(a), Val::Num(b)) => {
            return Ok(match op {
                Op::Lt => a < b,
                Op::Gt => a > b,
                Op::Le => a <= b,
                _ => a >= b,
            });
        }
        (Val::Str(a), Val::Str(b)) => a.cmp(b),
        _ => return Err(Failure::Undefined),
    };
    Ok(match op {
        Op::Lt => ordering.is_lt(),
        Op::Gt => ordering.is_gt(),
        Op::Le => ordering.is_le(),
        _ => ordering.is_ge(),
    })
}

fn conditions(args: &[Val]) -> Result<Vec<bool>, Failure> {
    args.iter()
        .map(|arg| match arg {
            Val::Bool(b) => Ok(*b),
            _ => Err(Failure::Undefined),
        })
        .collect()
}

/// C's `(int)a % (int)b`, undefined for a zero divisor, an operand past an `int`, and the one
/// quotient past it.
fn modulo(a: f64, b: f64) -> Result<i32, Failure> {
    let int = |x: f64| {
        let truncated = x.trunc();
        (truncated >= f64::from(i32::MIN) && truncated <= f64::from(i32::MAX))
            .then_some(truncated as i32)
            .ok_or(Failure::Undefined)
    };
    int(a)?.checked_rem(int(b)?).ok_or(Failure::Undefined)
}

/// MS_NINT, `lrint` at the pin: half to even, undefined past a `long`.
fn lrint(x: f64) -> Result<f64, Failure> {
    let rounded = x.round_ties_even();
    const LONG: f64 = 9_223_372_036_854_775_808.0;
    if (-LONG..LONG).contains(&rounded) {
        Ok(rounded)
    } else {
        Err(Failure::Undefined)
    }
}

#[cfg(test)]
mod tests;
