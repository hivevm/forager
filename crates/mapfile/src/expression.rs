//! Translates MapServer's expressions into the CQL2 tree of the style IR (ADR-0019).
//!
// The tokens follow the EXPRESSION_STRING rules of src/maplexer.l, which MapServer builds with
// `flex -i`, so its keywords match in any case; the grammar, its precedence, and its typing
// follow src/mapparser.y, and the four kinds of expression follow msEvalExpression in
// src/maputil.c, all of MapServer release 8.6.6. MapServer's syntax decides each operand's type,
// so every attribute is wrapped in a conversion that states it.

use forager_style_ir::{Expr, MAX_EXPRESSION_DEPTH, Op};

use crate::lexer::{Token, TokenKind};

/// Why an expression was not carried over. A warning keeps its class or filter as one that never
/// matches, as MapServer, which parses an expression only when it evaluates it, never matches
/// one it cannot parse; an error fails the load.
#[derive(Debug, PartialEq)]
pub struct Problem {
    pub error: bool,
    pub code: &'static str,
    pub message: String,
}

impl Problem {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            error: false,
            code: "invalid-expression",
            message: message.into(),
        }
    }

    fn unsupported(what: &str) -> Self {
        Self {
            error: false,
            code: "unsupported-expression",
            message: format!("{what} in an expression is not yet supported"),
        }
    }
}

/// The expression a token of a `EXPRESSION` or `FILTER` holds, compared against `item`, the
/// `CLASSITEM` or `FILTERITEM`, where the kind of expression needs one.
pub fn translate(token: &Token, item: Option<&str>) -> Result<Expr, Problem> {
    let against_item = |item: Option<&str>| {
        item.map(|name| Expr::call(Op::ToString, vec![Expr::property(name)]))
            .ok_or_else(|| {
                Problem::invalid("a string, regular expression, or list needs an item to compare")
            })
    };
    match &token.kind {
        TokenKind::Str { value, ignore_case } => {
            let value = Expr::String(value.clone());
            let item = against_item(item)?;
            Ok(if *ignore_case {
                Expr::call(Op::Eq, vec![casei(item), casei(value)])
            } else {
                Expr::call(Op::Eq, vec![item, value])
            })
        }
        TokenKind::Word(value) => Ok(Expr::call(
            Op::Eq,
            vec![against_item(item)?, Expr::String(value.clone())],
        )),
        TokenKind::Regex {
            pattern,
            ignore_case,
        } => Ok(Expr::call(
            Op::RegexpLike,
            vec![
                against_item(item)?,
                Expr::String(pattern.clone()),
                Expr::String(if *ignore_case { "i" } else { "" }.to_owned()),
            ],
        )),
        TokenKind::List(text) => {
            let inner = &text[1..text.len() - 1];
            let values = inner.split(',').map(|v| Expr::String(v.to_owned()));
            Ok(Expr::call(
                Op::In,
                vec![against_item(item)?, Expr::List(values.collect())],
            ))
        }
        TokenKind::Expression(text) => logical(text),
        // MapServer's loadExpression takes no other token: a load error.
        TokenKind::Number(_) | TokenKind::Binding(_) => Err(Problem {
            error: true,
            code: "invalid-value",
            message: "an expression is a string, regular expression, list, or (…)".to_owned(),
        }),
    }
}

/// A logical expression in parentheses, its value taken as MapServer takes it for a class or a
/// filter: a number is true when it is not 0, a string is always true.
fn logical(text: &str) -> Result<Expr, Problem> {
    let tokens = tokenize(text)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        depth: 0,
    };
    let (expr, kind) = parser.expression(0)?;
    if parser.at != parser.tokens.len() {
        return Err(Problem::invalid(format!(
            "unexpected {} in the expression",
            parser.tokens[parser.at].describe()
        )));
    }
    let expr = match kind {
        Kind::Logical => expr,
        Kind::Number => truthy(expr),
        Kind::String => Expr::Bool(true),
        Kind::Time => return Err(Problem::invalid("a time is not a condition")),
    };
    if expr.depth() > MAX_EXPRESSION_DEPTH {
        return Err(too_deep());
    }
    Ok(expr)
}

fn too_deep() -> Problem {
    Problem {
        error: true,
        code: "expression-too-deep",
        message: format!("the expression nests deeper than {MAX_EXPRESSION_DEPTH}"),
    }
}

fn casei(expr: Expr) -> Expr {
    Expr::call(Op::CaseI, vec![expr])
}

fn truthy(number: Expr) -> Expr {
    Expr::call(Op::Ne, vec![number, Expr::Number(0.0)])
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Number(f64),
    Str(String),
    Time(String),
    NumberBinding(String),
    StringBinding(String),
    TimeBinding(String),
    Bool(bool),
    /// A keyword, in lower case.
    Word(String),
    /// An operator or punctuation: `(`, `)`, `,`, `+`, `==`, `~*`, …
    Symbol(&'static str),
}

impl Tok {
    fn describe(&self) -> String {
        match self {
            Tok::Number(n) => format!("number {n}"),
            Tok::Str(s) => format!("string \"{s}\""),
            Tok::Time(t) => format!("time `{t}`"),
            Tok::NumberBinding(b) | Tok::StringBinding(b) | Tok::TimeBinding(b) => {
                format!("attribute [{b}]")
            }
            Tok::Bool(b) => format!("`{b}`"),
            Tok::Word(w) => format!("`{w}`"),
            Tok::Symbol(s) => format!("`{s}`"),
        }
    }
}

/// The symbols of the lexer, longest first, so that each matches as flex's longest match does.
const SYMBOLS: &[&str] = &[
    "||", "&&", "==", "!=", "<>", ">=", "<=", "=*", "~*", "!", "=", ">", "<", "~", "(", ")", ",",
    "+", "-", "*", "/", "%", "^",
];

const KEYWORDS: &[&str] = &[
    "or",
    "and",
    "not",
    "eq",
    "ne",
    "gt",
    "lt",
    "ge",
    "le",
    "in",
    "area",
    "length",
    "tostring",
    "commify",
    "round",
    "upper",
    "lower",
    "initcap",
    "firstcap",
    "buffer",
    "difference",
    "simplify",
    "simplifypt",
    "generalize",
    "smoothsia",
    "centerline",
    "densify",
    "outer",
    "inner",
    "javascript",
    "intersects",
    "disjoint",
    "touches",
    "overlaps",
    "crosses",
    "within",
    "contains",
    "equals",
    "beyond",
    "dwithin",
    "fromtext",
];

fn tokenize(text: &str) -> Result<Vec<Tok>, Problem> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            i += 1;
        } else if let Some(len) = number_len(&chars[i..]) {
            let literal: String = chars[i..i + len].iter().collect();
            tokens.push(Tok::Number(atof(&literal)));
            i += len;
        } else if c == '[' {
            let (name, next) = delimited(&chars, i + 1, ']')?;
            i = next;
            match name.to_ascii_lowercase().as_str() {
                "shape" | "map_cellsize" | "data_cellsize" => {
                    return Err(Problem::unsupported(&format!("[{name}]")));
                }
                _ => tokens.push(Tok::NumberBinding(name)),
            }
        } else if matches!(c, '"' | '\'') {
            if chars.get(i + 1) == Some(&'[')
                && let Ok((name, next)) = delimited(&chars, i + 2, ']')
                && chars.get(next) == Some(&c)
            {
                tokens.push(Tok::StringBinding(name));
                i = next + 1;
                continue;
            }
            let (value, next) = string(&chars, i)?;
            tokens.push(Tok::Str(value));
            // The lexer closes a string on `"i` as well and drops the `i`.
            i = if matches!(chars.get(next), Some('i' | 'I')) {
                next + 1
            } else {
                next
            };
        } else if c == '`' {
            if chars.get(i + 1) == Some(&'[')
                && let Ok((name, next)) = delimited(&chars, i + 2, ']')
                && chars.get(next) == Some(&'`')
            {
                tokens.push(Tok::TimeBinding(name));
                i = next + 1;
                continue;
            }
            let (value, next) = delimited(&chars, i + 1, '`')?;
            tokens.push(Tok::Time(value));
            i = next;
        } else if c.is_ascii_alphabetic() {
            let end = (i..chars.len())
                .find(|&j| !chars[j].is_ascii_alphabetic())
                .unwrap_or(chars.len());
            let word: String = chars[i..end]
                .iter()
                .collect::<String>()
                .to_ascii_lowercase();
            i = end;
            match word.as_str() {
                "true" => tokens.push(Tok::Bool(true)),
                "false" => tokens.push(Tok::Bool(false)),
                w if KEYWORDS.contains(&w) => tokens.push(Tok::Word(word)),
                _ => return Err(Problem::invalid(format!("unknown word `{word}`"))),
            }
        } else if let Some(symbol) = SYMBOLS.iter().find(|s| rest.starts_with(**s)) {
            tokens.push(Tok::Symbol(symbol));
            i += symbol.len();
        } else {
            return Err(Problem::invalid(format!("unexpected `{c}`")));
        }
    }
    Ok(tokens)
}

/// The length of the number at the start of `chars`, by the lexer's rule
/// `-?[0-9]+|-?[0-9]+\.[0-9]*|-?\.[0-9]*|…[eE][+-]?[0-9]+`.
fn number_len(chars: &[char]) -> Option<usize> {
    let digits = |from: usize| {
        (from..=chars.len())
            .find(|&j| !chars.get(j).is_some_and(char::is_ascii_digit))
            .unwrap_or(chars.len())
    };
    let mut i = usize::from(chars.first() == Some(&'-'));
    let int_end = digits(i);
    let has_int = int_end > i;
    i = int_end;
    let has_dot = chars.get(i) == Some(&'.');
    if has_dot {
        i = digits(i + 1);
    }
    if !has_int && !has_dot {
        return None;
    }
    if matches!(chars.get(i), Some('e' | 'E')) {
        let mut j = i + 1;
        if matches!(chars.get(j), Some('+' | '-')) {
            j += 1;
        }
        let end = digits(j);
        if end > j {
            i = end;
        }
    }
    Some(i)
}

/// C's `atof`: the longest numeric prefix, or 0.
pub fn atof(text: &str) -> f64 {
    let text = text.trim_start();
    let chars: Vec<char> = text.chars().collect();
    let sign = usize::from(matches!(chars.first(), Some('+' | '-')));
    let len = number_len(&chars[sign..]).map_or(0, |len| len + sign);
    text[..len].parse().unwrap_or(0.0)
}

/// The text up to `close`, from `from`, with a doubled `close` standing for one; and the index
/// past it.
fn delimited(chars: &[char], from: usize, close: char) -> Result<(String, usize), Problem> {
    let mut value = String::new();
    let mut i = from;
    loop {
        match chars.get(i) {
            None => return Err(Problem::invalid(format!("unterminated `{close}`"))),
            Some(&c) if c == close && chars.get(i + 1) == Some(&close) && close != '`' => {
                value.push(c);
                i += 2;
            }
            Some(&c) if c == close => return Ok((value, i + 1)),
            Some(&c) => {
                value.push(c);
                i += 1;
            }
        }
    }
}

/// A quoted string starting at `from`: a backslash escapes a quote or a backslash.
fn string(chars: &[char], from: usize) -> Result<(String, usize), Problem> {
    let quote = chars[from];
    let mut value = String::new();
    let mut i = from + 1;
    loop {
        match chars.get(i) {
            None => return Err(Problem::invalid("unterminated string")),
            Some('\\') if matches!(chars.get(i + 1), Some('\'' | '"' | '\\')) => {
                value.push(chars[i + 1]);
                i += 2;
            }
            Some(&c) if c == quote => return Ok((value, i + 1)),
            Some(&c) => {
                value.push(c);
                i += 1;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Logical,
    Number,
    String,
    Time,
}

/// Binary operators by precedence, loosest first, as `%left` and `%right` in src/mapparser.y.
#[derive(Clone, Copy, PartialEq)]
enum Binary {
    Or,
    And,
    Compare(Cmp),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Clone, Copy, PartialEq)]
enum Cmp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    In,
    Ieq,
    Re,
    Ire,
}

impl Binary {
    fn precedence(self) -> u8 {
        match self {
            Binary::Or => 1,
            Binary::And => 2,
            // NOT is 3.
            Binary::Compare(_) => 4,
            Binary::Add | Binary::Sub => 5,
            Binary::Mul | Binary::Div | Binary::Mod => 6,
            // Unary minus is 7.
            Binary::Pow => 8,
        }
    }
}

const NOT: u8 = 3;
const NEG: u8 = 7;

struct Parser {
    tokens: Vec<Tok>,
    at: usize,
    /// How deeply the parser has recursed. The limit is the depth of the tree it builds; this
    /// bound, a multiple of it, only keeps parentheses that nest no tree from overflowing the
    /// stack.
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Tok> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn expect(&mut self, symbol: &str) -> Result<(), Problem> {
        match self.next() {
            Some(Tok::Symbol(s)) if s == symbol => Ok(()),
            Some(other) => Err(Problem::invalid(format!(
                "expected `{symbol}`, found {}",
                other.describe()
            ))),
            None => Err(Problem::invalid(format!("expected `{symbol}` at the end"))),
        }
    }

    fn binary(&self) -> Option<Binary> {
        Some(match self.peek()? {
            Tok::Word(w) => match w.as_str() {
                "or" => Binary::Or,
                "and" => Binary::And,
                "eq" => Binary::Compare(Cmp::Eq),
                "ne" => Binary::Compare(Cmp::Ne),
                "lt" => Binary::Compare(Cmp::Lt),
                "gt" => Binary::Compare(Cmp::Gt),
                "le" => Binary::Compare(Cmp::Le),
                "ge" => Binary::Compare(Cmp::Ge),
                "in" => Binary::Compare(Cmp::In),
                _ => return None,
            },
            Tok::Symbol(s) => match *s {
                "||" => Binary::Or,
                "&&" => Binary::And,
                "=" | "==" => Binary::Compare(Cmp::Eq),
                "!=" | "<>" => Binary::Compare(Cmp::Ne),
                "<" => Binary::Compare(Cmp::Lt),
                ">" => Binary::Compare(Cmp::Gt),
                "<=" => Binary::Compare(Cmp::Le),
                ">=" => Binary::Compare(Cmp::Ge),
                "=*" => Binary::Compare(Cmp::Ieq),
                "~" => Binary::Compare(Cmp::Re),
                "~*" => Binary::Compare(Cmp::Ire),
                "+" => Binary::Add,
                "-" => Binary::Sub,
                "*" => Binary::Mul,
                "/" => Binary::Div,
                "%" => Binary::Mod,
                "^" => Binary::Pow,
                _ => return None,
            },
            _ => return None,
        })
    }

    /// An expression whose binary operators bind tighter than `min`.
    fn expression(&mut self, min: u8) -> Result<(Expr, Kind), Problem> {
        self.depth += 1;
        if self.depth > 4 * MAX_EXPRESSION_DEPTH {
            return Err(too_deep());
        }
        let mut left = self.prefix()?;
        while let Some(op) = self.binary() {
            let precedence = op.precedence();
            if precedence <= min {
                break;
            }
            self.at += 1;
            // `^` is right-associative, every other operator left-associative.
            let right = self.expression(if op == Binary::Pow {
                precedence - 1
            } else {
                precedence
            })?;
            left = combine(op, left, right)?;
        }
        self.depth -= 1;
        Ok(left)
    }

    fn prefix(&mut self) -> Result<(Expr, Kind), Problem> {
        match self.peek() {
            Some(Tok::Word(w)) if w == "not" => {
                self.at += 1;
                let operand = self.expression(NOT)?;
                Ok((
                    Expr::call(Op::Not, vec![condition(operand)?]),
                    Kind::Logical,
                ))
            }
            Some(Tok::Symbol("!")) => {
                self.at += 1;
                let operand = self.expression(NOT)?;
                Ok((
                    Expr::call(Op::Not, vec![condition(operand)?]),
                    Kind::Logical,
                ))
            }
            Some(Tok::Symbol("-")) => {
                self.at += 1;
                let (operand, kind) = self.expression(NEG)?;
                number(kind, "-")?;
                Ok((
                    Expr::call(Op::Mul, vec![operand, Expr::Number(-1.0)]),
                    Kind::Number,
                ))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<(Expr, Kind), Problem> {
        let Some(token) = self.next() else {
            return Err(Problem::invalid("the expression ends too early"));
        };
        Ok(match token {
            Tok::Number(n) => (Expr::Number(n), Kind::Number),
            Tok::Str(s) => (Expr::String(s), Kind::String),
            Tok::Bool(b) => (Expr::Bool(b), Kind::Logical),
            Tok::Time(t) => (time(&t)?, Kind::Time),
            Tok::NumberBinding(name) => (
                Expr::call(Op::ToNumber, vec![Expr::property(name)]),
                Kind::Number,
            ),
            Tok::StringBinding(name) => (
                Expr::call(Op::ToString, vec![Expr::property(name)]),
                Kind::String,
            ),
            Tok::TimeBinding(name) => (
                Expr::call(Op::ToTimestamp, vec![Expr::property(name)]),
                Kind::Time,
            ),
            Tok::Symbol("(") => {
                let inner = self.expression(0)?;
                self.expect(")")?;
                inner
            }
            Tok::Word(w) => self.function(&w)?,
            other => {
                return Err(Problem::invalid(format!(
                    "unexpected {} in the expression",
                    other.describe()
                )));
            }
        })
    }

    fn function(&mut self, name: &str) -> Result<(Expr, Kind), Problem> {
        let (op, params, result): (Op, &[Kind], Kind) = match name {
            "length" => (Op::Length, &[Kind::String], Kind::Number),
            "round" => (Op::Round, &[Kind::Number, Kind::Number], Kind::Number),
            "tostring" => (Op::Tostring, &[Kind::Number, Kind::String], Kind::String),
            "commify" => (Op::Commify, &[Kind::String], Kind::String),
            "upper" => (Op::Upper, &[Kind::String], Kind::String),
            "lower" => (Op::Lower, &[Kind::String], Kind::String),
            "initcap" => (Op::Initcap, &[Kind::String], Kind::String),
            "firstcap" => (Op::Firstcap, &[Kind::String], Kind::String),
            other => return Err(Problem::unsupported(&format!("`{other}`"))),
        };
        self.expect("(")?;
        let mut args = Vec::new();
        loop {
            let (arg, kind) = self.expression(0)?;
            let expected = params.get(args.len()).copied();
            if expected != Some(kind) {
                return Err(Problem::invalid(format!(
                    "argument {} of {name} has the wrong type",
                    args.len() + 1
                )));
            }
            args.push(arg);
            match self.next() {
                Some(Tok::Symbol(",")) => {}
                Some(Tok::Symbol(")")) => break,
                _ => return Err(Problem::invalid(format!("expected `,` or `)` in {name}"))),
            }
        }
        // round takes one or two arguments, every other function all of its own.
        if args.len() < params.len() && !(op == Op::Round && args.len() == 1) {
            return Err(Problem::invalid(format!(
                "{name} takes {} arguments",
                params.len()
            )));
        }
        Ok((Expr::call(op, args), result))
    }
}

/// A time literal as CQL2 writes it: `YYYY-MM-DD` as a date, `YYYY-MM-DDThh:mm:ss` with an
/// optional `Z` as a timestamp. MapServer's other time formats are not yet carried over.
fn time(text: &str) -> Result<Expr, Problem> {
    let b = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| range.into_iter().all(|i| b[i].is_ascii_digit());
    let is_date = b.len() >= 10
        && digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10);
    if is_date && b.len() == 10 {
        return Ok(Expr::Date {
            date: text.to_owned(),
        });
    }
    let is_timestamp = is_date
        && (b.len() == 19 || (b.len() == 20 && b[19] == b'Z'))
        && b[10] == b'T'
        && digits(11..13)
        && b[13] == b':'
        && digits(14..16)
        && b[16] == b':'
        && digits(17..19);
    if is_timestamp {
        return Ok(Expr::Timestamp {
            timestamp: text.to_owned(),
        });
    }
    Err(Problem::unsupported(&format!("the time `{text}`")))
}

fn number(kind: Kind, op: &str) -> Result<(), Problem> {
    if kind == Kind::Number {
        Ok(())
    } else {
        Err(Problem::invalid(format!("`{op}` takes numbers")))
    }
}

/// An operand of AND, OR, or NOT: a condition, or a number that is true when it is not 0.
fn condition((expr, kind): (Expr, Kind)) -> Result<Expr, Problem> {
    match kind {
        Kind::Logical => Ok(expr),
        Kind::Number => Ok(truthy(expr)),
        _ => Err(Problem::invalid(
            "AND, OR, and NOT take conditions or numbers",
        )),
    }
}

/// `op` over `left` and `right`, flattening a chain of the same operator into one call.
fn flat(op: Op, left: Expr, right: Expr) -> Expr {
    let mut args = Vec::new();
    for side in [left, right] {
        match side {
            Expr::Call {
                op: inner,
                args: inner_args,
            } if inner == op => args.extend(inner_args),
            other => args.push(other),
        }
    }
    Expr::call(op, args)
}

fn combine(op: Binary, left: (Expr, Kind), right: (Expr, Kind)) -> Result<(Expr, Kind), Problem> {
    let arithmetic = |cql: Op, symbol: &str| {
        number(left.1, symbol)?;
        number(right.1, symbol)?;
        Ok((
            Expr::call(cql, vec![left.0.clone(), right.0.clone()]),
            Kind::Number,
        ))
    };
    match op {
        Binary::Or => Ok((
            flat(Op::Or, condition(left)?, condition(right)?),
            Kind::Logical,
        )),
        Binary::And => Ok((
            flat(Op::And, condition(left)?, condition(right)?),
            Kind::Logical,
        )),
        Binary::Add if left.1 == Kind::String && right.1 == Kind::String => {
            Ok((flat(Op::Concat, left.0, right.0), Kind::String))
        }
        Binary::Add => arithmetic(Op::Add, "+"),
        Binary::Sub => arithmetic(Op::Sub, "-"),
        Binary::Mul => arithmetic(Op::Mul, "*"),
        Binary::Div => arithmetic(Op::Div, "/"),
        Binary::Mod => arithmetic(Op::Mod, "%"),
        Binary::Pow => arithmetic(Op::Pow, "^"),
        Binary::Compare(cmp) => compare(cmp, left, right).map(|e| (e, Kind::Logical)),
    }
}

fn compare(cmp: Cmp, (l, lk): (Expr, Kind), (r, rk): (Expr, Kind)) -> Result<Expr, Problem> {
    let mismatch = || Problem::invalid("a comparison of operands of different types");
    let ordered = |op: Op| Ok(Expr::call(op, vec![l.clone(), r.clone()]));
    match (cmp, lk, rk) {
        (Cmp::In, Kind::String | Kind::Number, Kind::String) => {
            let Expr::String(list) = &r else {
                return Err(Problem::unsupported("IN against a computed string"));
            };
            let values = list.split(',').map(|v| {
                if lk == Kind::Number {
                    Expr::Number(atof(v))
                } else {
                    Expr::String(v.to_owned())
                }
            });
            Ok(Expr::call(Op::In, vec![l, Expr::List(values.collect())]))
        }
        (Cmp::In, ..) => Err(mismatch()),
        (Cmp::Re | Cmp::Ire, Kind::String, Kind::String) => {
            let flags = if cmp == Cmp::Ire { "i" } else { "" };
            Ok(Expr::call(
                Op::RegexpLike,
                vec![l, r, Expr::String(flags.to_owned())],
            ))
        }
        (Cmp::Re | Cmp::Ire, ..) => Err(mismatch()),
        (_, a, b) if a != b => Err(mismatch()),
        (Cmp::Ieq, Kind::String, _) => Ok(Expr::call(Op::Eq, vec![casei(l), casei(r)])),
        (Cmp::Ieq, Kind::Number, _) => ordered(Op::Eq),
        (Cmp::Ieq | Cmp::Eq, Kind::Time, _) => ordered(Op::TEquals),
        (Cmp::Ne, Kind::Time, _) => Ok(Expr::call(
            Op::Not,
            vec![Expr::call(Op::TEquals, vec![l, r])],
        )),
        (Cmp::Lt, Kind::Time, _) => ordered(Op::TBefore),
        (Cmp::Gt, Kind::Time, _) => ordered(Op::TAfter),
        (Cmp::Le, Kind::Time, _) => Ok(Expr::call(
            Op::Not,
            vec![Expr::call(Op::TAfter, vec![l, r])],
        )),
        (Cmp::Ge, Kind::Time, _) => Ok(Expr::call(
            Op::Not,
            vec![Expr::call(Op::TBefore, vec![l, r])],
        )),
        (Cmp::Eq, Kind::Logical, _) => ordered(Op::Eq),
        (_, Kind::Logical, _) => Err(Problem::invalid("conditions compare only with =")),
        (Cmp::Eq, ..) => ordered(Op::Eq),
        (Cmp::Ne, ..) => ordered(Op::Ne),
        (Cmp::Lt, ..) => ordered(Op::Lt),
        (Cmp::Gt, ..) => ordered(Op::Gt),
        (Cmp::Le, ..) => ordered(Op::Le),
        (Cmp::Ge, ..) => ordered(Op::Ge),
    }
}
