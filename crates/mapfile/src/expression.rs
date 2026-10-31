// The expression grammar and its evaluation follow MapServer's src/maplexer.l, src/mapparser.y,
// and msEvalExpression of the release pinned in scripts/fetch-msautotest.sh.
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

//! Translates MapServer's expressions into the CQL2 tree of the style IR (ADR-0030).
//!
// The tokens follow the EXPRESSION_STRING rules of src/maplexer.l, which MapServer builds with
// `flex -i`, so its keywords match in any case; the grammar, its precedence, and its typing
// follow src/mapparser.y, and the four kinds of expression follow msEvalExpression in
// src/maputil.c, all of the MapServer release pinned in scripts/fetch-msautotest.sh. MapServer's
// syntax decides each operand's type, so every attribute is wrapped in a conversion that states
// it.

use forager_style_ir::{Expr, MAX_EXPRESSION_DEPTH, Op};

use crate::lexer::{Token, TokenKind};

/// Why an expression was not carried over. A warning keeps its class, label, or filter as one that
/// never matches, as MapServer, which parses an expression only when it evaluates it, never matches
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
    // msEvalExpression takes an empty expression as always true, before it looks at the item.
    let empty = match &token.kind {
        TokenKind::Str { value, .. } => value.is_empty(),
        TokenKind::List(text) | TokenKind::Expression(text) => text.len() == 2,
        _ => false,
    };
    if empty {
        return Ok(Expr::Bool(true));
    }
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

/// An expression in parentheses that a style property binds, such as `SIZE ([a] * 2)`, its value
/// taken as MapServer's `msEvalDoubleExpression` takes it: evaluated as text, a number printed
/// with `%g`, and the text read back with `atof`.
pub fn number_value(text: &str) -> Result<Expr, Problem> {
    let (expr, kind) = parse(text)?;
    let text = match kind {
        Kind::Number => Expr::call(Op::Tostring, vec![expr, Expr::String("%g".to_owned())]),
        Kind::String => expr,
        Kind::Logical | Kind::Time => {
            return Err(Problem::invalid("a condition or a time is not a number"));
        }
    };
    let expr = Expr::call(Op::ToNumber, vec![text]);
    if expr.depth() > MAX_EXPRESSION_DEPTH {
        return Err(too_deep());
    }
    Ok(expr)
}

/// The text of a label's or class's `TEXT`, as msEvalTextExpression evaluates it: a string with
/// each `[name]` the expression lexer finds in it replaced by the property's value, or an
/// expression in parentheses taken as text, a number printed with `%g`, a condition as `true` or
/// `false`.
pub fn text(token: &Token) -> Result<Expr, Problem> {
    let expr = match &token.kind {
        TokenKind::Str { value, .. } | TokenKind::Word(value) => substituted(value)?,
        TokenKind::Expression(text) => {
            let (expr, kind) = parse(text)?;
            match kind {
                Kind::String => expr,
                Kind::Number => Expr::call(Op::Tostring, vec![expr, Expr::String("%g".to_owned())]),
                // mapparser.y gives a logical expression's value as the string "true" or "false"
                // where a string is asked for; to_string of a condition reads the same.
                Kind::Logical => Expr::call(Op::ToString, vec![expr]),
                Kind::Time => {
                    return Err(Problem::unsupported("a time as a label's text"));
                }
            }
        }
        _ => {
            return Err(Problem {
                error: true,
                code: "invalid-value",
                message: "TEXT expects a string or an expression".to_owned(),
            });
        }
    };
    if expr.depth() > MAX_EXPRESSION_DEPTH {
        return Err(too_deep());
    }
    Ok(expr)
}

/// `text` with the bindings [`text_bindings`] finds replaced by the property's values, as
/// msEvalTextExpression replaces them: for each binding in the lexer's order, every occurrence of
/// `[name]` in the text left so far, left to right, so that a later pass sees what an earlier one
/// left. A value that holds a later binding's `[name]` is not searched again, as it is in
/// MapServer. A literal, or a `concat`.
fn substituted(text: &str) -> Result<Expr, Problem> {
    enum Part {
        Literal(String),
        Value(String),
    }
    let mut parts = vec![Part::Literal(text.to_owned())];
    for name in text_bindings(text)? {
        let target = format!("[{name}]");
        parts = parts
            .into_iter()
            .flat_map(|part| match part {
                Part::Literal(literal) if literal.contains(&target) => {
                    let mut split = Vec::new();
                    for (i, piece) in literal.split(target.as_str()).enumerate() {
                        if i > 0 {
                            split.push(Part::Value(name.clone()));
                        }
                        split.push(Part::Literal(piece.to_owned()));
                    }
                    split
                }
                part => vec![part],
            })
            .collect();
    }
    let mut exprs: Vec<Expr> = parts
        .into_iter()
        .filter_map(|part| match part {
            Part::Literal(literal) if literal.is_empty() => None,
            Part::Literal(literal) => Some(Expr::String(literal)),
            Part::Value(name) => Some(Expr::call(Op::ToString, vec![Expr::property(&name)])),
        })
        .collect();
    Ok(match exprs.len() {
        0 => Expr::String(String::new()),
        1 => exprs.pop().expect("one part"),
        _ => Expr::call(Op::Concat, exprs),
    })
}

/// The attributes msTokenizeExpression finds in a plain `TEXT`, in order: the bindings among the
/// tokens [`Scanner`] reads, as msEvalTextExpression replaces only those. `[shape]`,
/// `[map_cellsize]`, and `[data_cellsize]` are no attribute, a string holds no binding, and a
/// string that does not close ends the text.
///
/// MapServer also stops at a token it cannot read: a time literal `msParseTime` rejects, or
/// `fromtext` not followed by a WKT string. An empty time literal is one; where the frontend
/// cannot tell, before a `[` that could be a binding, the text is not carried over.
fn text_bindings(text: &str) -> Result<Vec<String>, Problem> {
    let undecidable = |what: &str, at: usize| {
        if text.as_bytes()[at..].contains(&b'[') {
            Err(Problem::unsupported(&format!(
                "{what} before a binding in a plain TEXT, where MapServer may stop reading,"
            )))
        } else {
            Ok(())
        }
    };
    let mut names = Vec::new();
    for (lexeme, span) in Scanner::new(text) {
        match lexeme {
            Lexeme::Tok(
                Tok::NumberBinding(name) | Tok::StringBinding(name) | Tok::TimeBinding(name),
            ) => {
                names.push(name);
            }
            // An empty time literal, which msParseTime rejects.
            Lexeme::Tok(Tok::Time(time)) if time.is_empty() => break,
            Lexeme::Tok(Tok::Time(_)) => undecidable("a time literal", span.end)?,
            Lexeme::Tok(Tok::Word(word)) if word == "fromtext" => {
                undecidable("`fromtext`", span.end)?;
            }
            Lexeme::Unterminated => break,
            _ => {}
        }
    }
    Ok(names)
}

/// The tree of an expression in parentheses and the type of its value. The outer parentheses
/// are the mapfile token's, which loadExpression strips: they bracket nothing in the expression.
fn parse(text: &str) -> Result<(Expr, Kind), Problem> {
    let inner = text
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(text);
    let tokens = tokenize(inner)?;
    let mut parser = Parser {
        tokens,
        at: 0,
        depth: 0,
    };
    let parsed = parser.expression(0)?;
    if parser.at != parser.tokens.len() {
        return Err(Problem::invalid(format!(
            "unexpected {} in the expression",
            parser.tokens[parser.at].describe()
        )));
    }
    Ok(parsed)
}

/// A logical expression in parentheses, its value taken as MapServer takes it for a class or a
/// filter: a number is true when it is not 0, a string is always true.
fn logical(text: &str) -> Result<Expr, Problem> {
    let (expr, kind) = parse(text)?;
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

/// A token the EXPRESSION_STRING rules of maplexer.l read, where the parser and a plain `TEXT`
/// take it differently.
#[derive(Debug, PartialEq)]
enum Lexeme {
    Tok(Tok),
    /// `[shape]`, `[map_cellsize]`, or `[data_cellsize]`, as written: a binding to no attribute.
    Special(String),
    /// A character the rule `.` returns as itself: a `[` or a backtick that opens no binding and
    /// no time, a letter that starts no keyword, a character of no other rule.
    Char(char),
    /// A string the text ends in before its quote closes it, the last token.
    Unterminated,
}

/// The tokens msTokenizeExpression reads from `text` with the EXPRESSION_STRING rules of
/// maplexer.l, which MapServer builds with `flex -i`, matched case-insensitively and longest
/// first, each with its span. White space is `[ \t\r]+`; a newline, a NUL, and a `|` that is no
/// `||` end the tokens, as the rule `[\r|\n|\0]` returns 0 there. MapServer holds the text as a C
/// string, so a NUL inside a token ends the text too: the scanner reads only what comes before
/// the first one.
struct Scanner<'a> {
    text: &'a str,
    at: usize,
    /// The last `]` of the text: a `[` past it opens no binding, and knowing it keeps many `[`
    /// linear.
    last_close: Option<usize>,
    done: bool,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        let text = text.split('\0').next().unwrap_or_default();
        Self {
            text,
            at: 0,
            last_close: text.bytes().rposition(|b| b == b']'),
            done: false,
        }
    }

    /// The token at `start`, and the index past it.
    fn lexeme(&self, start: usize) -> Option<(Lexeme, usize)> {
        let bytes = self.text.as_bytes();
        let c = bytes[start];
        if c == b'\n' || c == 0 || (c == b'|' && bytes.get(start + 1) != Some(&b'|')) {
            return None;
        }
        if let Some(len) = number_len(&bytes[start..]) {
            let literal = &self.text[start..start + len];
            return Some((Lexeme::Tok(Tok::Number(atof(literal))), start + len));
        }
        let lexeme = match c {
            b'[' if self.last_close.is_some_and(|last| last > start) => {
                bracketed(self.text, start + 1, b']', b"]").map(|(name, next)| {
                    let token = &self.text[start..next];
                    let special = ["[shape]", "[map_cellsize]", "[data_cellsize]"]
                        .iter()
                        .any(|special| token.eq_ignore_ascii_case(special));
                    if special {
                        (Lexeme::Special(name), next)
                    } else {
                        (Lexeme::Tok(Tok::NumberBinding(name)), next)
                    }
                })
            }
            quote @ (b'"' | b'\'' | b'`') if bytes.get(start + 1) == Some(&b'[') => {
                bracketed(self.text, start + 2, quote, &[b']', quote]).map(|(name, next)| {
                    let token = if quote == b'`' {
                        Tok::TimeBinding(name)
                    } else {
                        Tok::StringBinding(name)
                    };
                    (Lexeme::Tok(token), next)
                })
            }
            _ => None,
        };
        if lexeme.is_some() {
            return lexeme;
        }
        Some(match c {
            b'"' | b'\'' => match string(self.text, start) {
                Some((value, next)) => (Lexeme::Tok(Tok::Str(value)), next),
                None => (Lexeme::Unterminated, bytes.len()),
            },
            b'`' => match bytes[start + 1..].iter().position(|&b| b == b'`') {
                Some(len) => {
                    let time = self.text[start + 1..start + 1 + len].to_owned();
                    (Lexeme::Tok(Tok::Time(time)), start + len + 2)
                }
                None => (Lexeme::Char('`'), start + 1),
            },
            c if c.is_ascii_alphabetic() => {
                // The lexer has no rule for a word, only one for each keyword: of `andnot` it
                // reads `and` and then `not`, the longest keyword each time.
                let ahead = &bytes[start..bytes.len().min(start + 16)];
                let keyword = KEYWORDS
                    .iter()
                    .chain(&["true", "false"])
                    .filter(|k| {
                        ahead.len() >= k.len()
                            && ahead[..k.len()].eq_ignore_ascii_case(k.as_bytes())
                    })
                    .max_by_key(|k| k.len());
                match keyword {
                    Some(&"true") => (Lexeme::Tok(Tok::Bool(true)), start + 4),
                    Some(&"false") => (Lexeme::Tok(Tok::Bool(false)), start + 5),
                    Some(&word) => (Lexeme::Tok(Tok::Word(word.to_owned())), start + word.len()),
                    None => (Lexeme::Char(char::from(c)), start + 1),
                }
            }
            _ => {
                let rest = &self.text[start..];
                match SYMBOLS.iter().find(|s| rest.starts_with(**s)) {
                    Some(symbol) => (Lexeme::Tok(Tok::Symbol(symbol)), start + symbol.len()),
                    None => {
                        let c = rest.chars().next().expect("a character at the start");
                        (Lexeme::Char(c), start + c.len_utf8())
                    }
                }
            }
        })
    }
}

impl Iterator for Scanner<'_> {
    type Item = (Lexeme, std::ops::Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.text.as_bytes();
        while bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r'))
        {
            self.at += 1;
        }
        if self.done || self.at >= bytes.len() {
            return None;
        }
        let start = self.at;
        let Some((lexeme, next)) = self.lexeme(start) else {
            self.done = true;
            return None;
        };
        self.done = lexeme == Lexeme::Unterminated;
        self.at = next;
        Some((lexeme, start..next))
    }
}

/// The tokens of an expression in parentheses, without them, for the parser: a token it has no
/// use for is a problem.
fn tokenize(text: &str) -> Result<Vec<Tok>, Problem> {
    let mut tokens = Vec::new();
    for (lexeme, span) in Scanner::new(text) {
        match lexeme {
            Lexeme::Tok(Tok::Number(n)) => tokens.push(Tok::Number(finite(&text[span], n)?)),
            Lexeme::Tok(token) => tokens.push(token),
            Lexeme::Special(name) => return Err(Problem::unsupported(&format!("[{name}]"))),
            Lexeme::Char('[') => return Err(Problem::invalid("unterminated `[`")),
            Lexeme::Char('`') => return Err(Problem::invalid("unterminated '`'")),
            Lexeme::Char(c) if c.is_ascii_alphabetic() => {
                let rest = &text[span.start..];
                let end = rest
                    .find(|c: char| !c.is_ascii_alphabetic())
                    .unwrap_or(rest.len());
                let word = &rest[..end];
                return Err(Problem::invalid(format!("unknown word `{word}`")));
            }
            Lexeme::Char(c) => return Err(Problem::invalid(format!("unexpected `{c}`"))),
            Lexeme::Unterminated => return Err(Problem::invalid("unterminated string")),
        }
    }
    Ok(tokens)
}

/// The length of the number at the start of `bytes`, by the lexer's rule
/// `-?[0-9]+|-?[0-9]+\.[0-9]*|-?\.[0-9]*|…[eE][+-]?[0-9]+`.
fn number_len(bytes: &[u8]) -> Option<usize> {
    let digits = |from: usize| {
        (from..=bytes.len())
            .find(|&j| !bytes.get(j).is_some_and(u8::is_ascii_digit))
            .unwrap_or(bytes.len())
    };
    let mut i = usize::from(bytes.first() == Some(&b'-'));
    let int_end = digits(i);
    let has_int = int_end > i;
    i = int_end;
    let has_dot = bytes.get(i) == Some(&b'.');
    if has_dot {
        i = digits(i + 1);
    }
    if !has_int && !has_dot {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
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
    let bytes = text.as_bytes();
    let sign = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let len = number_len(&bytes[sign..]).map_or(0, |len| len + sign);
    text[..len].parse().unwrap_or(0.0)
}

/// A number literal, which the IR can hold only when it is finite: JSON has no infinity.
fn finite(literal: &str, n: f64) -> Result<f64, Problem> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Problem::unsupported(&format!("the number `{literal}`")))
    }
}

/// The name of an attribute binding whose name starts at `from`, as the lexer's rules
/// `\[(?:\]\]|[^\]])*\]`, `"\[(?:""|[^"])*\]"`, and `` `\[(?:``|[^`])*\]` `` match it: a name
/// of any character but `escape`, which it holds doubled, up to the last `end` flex's longest
/// match reaches, the doubled `escape` read as one, as msStringUnescape reads it; and the index
/// past the match.
fn bracketed(text: &str, from: usize, escape: u8, end: &[u8]) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut last = None;
    let mut i = from;
    loop {
        if bytes[i..].starts_with(end) {
            last = Some(i);
        }
        match bytes.get(i) {
            None => break,
            Some(&b) if b == escape => {
                if bytes.get(i + 1) != Some(&escape) {
                    break;
                }
                i += 2;
            }
            Some(_) => i += 1,
        }
    }
    let close = last?;
    let escape = char::from(escape);
    let name = text[from..close].replace(&format!("{escape}{escape}"), &escape.to_string());
    Some((name, close + end.len()))
}

/// The string the quote at `from` opens, as the MSSTRING rules of maplexer.l read it, and the
/// index past it: a backslash escapes a quote or itself, the other quote is held, and the same
/// quote, with an `i` after it or not, closes it, the `i` dropped. `None` when it does not close.
fn string(text: &str, from: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let quote = bytes[from];
    let mut value = String::new();
    let mut held = from + 1;
    let mut i = from + 1;
    loop {
        match bytes.get(i) {
            None => return None,
            Some(b'\\') if matches!(bytes.get(i + 1), Some(b'\'' | b'"' | b'\\')) => {
                value.push_str(&text[held..i]);
                held = i + 1;
                i += 2;
            }
            Some(&b) if b == quote => {
                value.push_str(&text[held..i]);
                let icase = bytes
                    .get(i + 1)
                    .is_some_and(|c| c.eq_ignore_ascii_case(&b'i'));
                return Some((value, i + 1 + usize::from(icase)));
            }
            Some(_) => i += 1,
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
                // src/mapparser.y reads `'-' math_exp %prec NEG { $$ = $2; }`: the operand
                // unchanged, not negated.
                let (operand, kind) = self.expression(NEG)?;
                number(kind, "-")?;
                Ok((operand, Kind::Number))
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
                Ok(if lk == Kind::Number {
                    Expr::Number(finite(v, atof(v))?)
                } else {
                    Expr::String(v.to_owned())
                })
            });
            let values = values.collect::<Result<_, Problem>>()?;
            Ok(Expr::call(Op::In, vec![l, Expr::List(values)]))
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

#[cfg(test)]
mod tests {
    use super::{Tok, text_bindings, tokenize};

    #[test]
    fn a_binding_with_a_doubled_bracket_reads_the_same_in_an_expression_and_a_text() {
        // One scanner reads both: `]]` in a binding is one `]`, a doubled quote in a quoted one
        // is one quote, and the binding runs to the last end the longest match reaches.
        for (binding, name) in [
            ("[a]]b]", "a]b"),
            ("[a]]]", "a]"),
            ("[a]] = 1 OR [b]", "a] = 1 OR [b"),
            (r#""[a]]b]""#, "a]]b"),
            ("'[a''b]'", "a'b"),
            ("`[t``x]`", "t`x"),
        ] {
            let expression = tokenize(binding).unwrap();
            let read = match expression.as_slice() {
                [Tok::NumberBinding(n) | Tok::StringBinding(n) | Tok::TimeBinding(n)] => n,
                other => panic!("{binding}: {other:?}"),
            };
            assert_eq!(read, name, "{binding} in an expression");
            assert_eq!(
                text_bindings(binding).unwrap(),
                [name],
                "{binding} in a TEXT"
            );
        }
    }
}
