// The expression of a layer's GEOMTRANSFORM is read as MapServer's lexer, src/maplexer.l, splits
// it into tokens and its parser, src/mapparser.y, reduces the rules `shape_exp` and `math_exp`, of
// the release pinned in scripts/fetch-msautotest.sh.
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

//! The parse of a layer's `GEOMTRANSFORM` into the calls the engine draws: `[shape]`, and
//! `generalize`, `densify`, `outer`, `inner`, and `smoothsia` of a shape, nested as the grammar
//! nests them, their numbers written, `[map_cellsize]`, or computed of those by `+`, `-`, `*`,
//! `/`, `%`, `^`, and parentheses. Anything else is refused with its reason.

use forager_style_ir::MAX_EXPRESSION_DEPTH;

use super::Undrawn;

/// A shape the expression computes, the rule `shape_exp`.
#[derive(Debug, PartialEq)]
pub(super) enum ShapeExp {
    /// `[shape]`, the feature's shape.
    Shape,
    /// `generalize(shape, tolerance)`.
    Generalize(Box<ShapeExp>, MathExp),
    /// `densify(shape, tolerance)`.
    Densify(Box<ShapeExp>, MathExp),
    /// `outer(shape)` where true, `inner(shape)` where false.
    Rings(Box<ShapeExp>, bool),
    /// `smoothsia(shape[, size[, iterations[, preprocessing]]])`.
    SmoothSia {
        shape: Box<ShapeExp>,
        size: Option<MathExp>,
        iterations: Option<MathExp>,
        preprocessing: Option<String>,
    },
}

/// A number the expression computes, the rule `math_exp`.
#[derive(Debug, PartialEq)]
pub(super) enum MathExp {
    Number(f64),
    /// `[map_cellsize]`, which msGeomTransformShape binds to the cellsize in the layer's units.
    MapCellsize,
    /// `a op b`, `op` one of `+ - * / % ^`.
    Binary(Box<MathExp>, u8, Box<MathExp>),
    /// `- a`, which the rule `'-' math_exp %prec NEG { $$ = $2; }` leaves `a`, not negated.
    Neg(Box<MathExp>),
}

/// A token of the expression, as the lexer's rules of the state `EXPRESSION_STRING` give it.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Text(String),
    Shape,
    MapCellsize,
    /// Any other `[…]`: an attribute's binding, or `[data_cellsize]`.
    Binding(String),
    /// A word of letters, in lower case: the lexer is generated case-insensitive (`flex -i`), as
    /// its lower-case keywords read the mapfile's upper-case ones.
    Word(String),
    Char(u8),
}

/// The functions of MapServer's shape expressions that the engine does not draw, with what they
/// need.
const NOT_DRAWN: &[(&str, &str)] = &[
    ("buffer", "a geometry library"),
    ("difference", "a geometry library"),
    ("simplify", "a geometry library"),
    ("simplifypt", "a geometry library"),
    ("centerline", "a geometry library"),
    ("javascript", "a JavaScript engine"),
    ("fromtext", "a shape read from text"),
];

/// The expression `text` parsed as a shape, or why the engine does not draw it.
pub(super) fn parse(text: &str) -> Result<ShapeExp, Undrawn> {
    let tokens = tokens(text.as_bytes())?;
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        depth: 0,
    };
    let shape = parser.shape()?;
    match parser.peek() {
        None => Ok(shape),
        Some(token) => Err(syntax(&format!("{} after the shape", shown(Some(token))))),
    }
}

fn syntax(what: &str) -> Undrawn {
    Undrawn::Unsupported(format!("does not parse as a shape: {what}"))
}

/// The tokens of `text`; a quoted text with a backslash, or one in backquotes, is refused as a
/// form the engine does not read.
fn tokens(text: &[u8]) -> Result<Vec<Token>, Undrawn> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while let Some(&c) = text.get(i) {
        if c.is_ascii_whitespace() {
            i += 1;
        } else if let Some((value, end)) = number(text, i) {
            tokens.push(Token::Number(value));
            i = end;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let end = run(text, i, |b| b.is_ascii_alphanumeric() || b == b'_');
            let word = String::from_utf8_lossy(&text[i..end]).to_ascii_lowercase();
            tokens.push(Token::Word(word));
            i = end;
        } else if c == b'[' {
            let end = run(text, i + 1, |b| b != b']');
            if end >= text.len() {
                return Err(syntax("a `[` without its `]`"));
            }
            let name = String::from_utf8_lossy(&text[i + 1..end]).into_owned();
            tokens.push(match name.to_ascii_lowercase().as_str() {
                "shape" => Token::Shape,
                "map_cellsize" => Token::MapCellsize,
                _ => Token::Binding(name),
            });
            i = end + 1;
        } else if c == b'\'' || c == b'"' {
            let end = run(text, i + 1, |b| b != c && b != b'\\');
            if text.get(end) != Some(&c) {
                return Err(Undrawn::Unsupported(
                    "holds a quoted text the engine does not read".to_owned(),
                ));
            }
            tokens.push(Token::Text(
                String::from_utf8_lossy(&text[i + 1..end]).into_owned(),
            ));
            i = end + 1;
        } else {
            tokens.push(Token::Char(c));
            i += 1;
        }
    }
    Ok(tokens)
}

/// The end of the run of bytes from `start` that `keep` holds.
fn run(text: &[u8], start: usize, keep: impl Fn(u8) -> bool) -> usize {
    let mut end = start;
    while text.get(end).is_some_and(|&b| keep(b)) {
        end += 1;
    }
    end
}

/// The number at `start` and its end, as the lexer's rule
/// `-?[0-9]+|-?[0-9]+\.[0-9]*|-?\.[0-9]*` with an optional exponent `[eE][+-]?[0-9]+` matches it
/// (src/maplexer.l:583), the longest match; its value as `atof` reads the text: a mantissa of no
/// digit, `.` or `-.`, is 0.
fn number(text: &[u8], start: usize) -> Option<(f64, usize)> {
    let digits = |from| run(text, from, |b| b.is_ascii_digit());
    let mut i = start + usize::from(text.get(start) == Some(&b'-'));
    let whole = digits(i);
    let mut mantissa = whole > i;
    i = whole;
    if text.get(i) == Some(&b'.') {
        let fraction = digits(i + 1);
        mantissa |= fraction > i + 1;
        i = fraction;
    } else if !mantissa {
        return None;
    }
    if matches!(text.get(i), Some(b'e' | b'E')) {
        let sign = i + 1 + usize::from(matches!(text.get(i + 1), Some(b'+' | b'-')));
        let exponent = digits(sign);
        if exponent > sign {
            i = exponent;
        }
    }
    let value = if mantissa {
        std::str::from_utf8(&text[start..i]).ok()?.parse().ok()?
    } else {
        0.0
    };
    Some((value, i))
}

/// The most characters of the expression's own text a reason quotes, so that a diagnostic stays
/// short whatever the expression holds.
const QUOTED: usize = 32;

/// `text` as a reason quotes it: whole up to [`QUOTED`] characters, cut there with an ellipsis
/// past them.
fn cut(text: &str) -> String {
    if text.chars().count() <= QUOTED {
        text.to_owned()
    } else {
        let head: String = text.chars().take(QUOTED).collect();
        format!("{head}\u{2026}")
    }
}

/// A token as a reason names it, readable and bounded; the end of the expression where there is
/// none.
fn shown(token: Option<&Token>) -> String {
    match token {
        None => "the end of the expression".to_owned(),
        Some(Token::Number(n)) => format!("the number {n}"),
        Some(Token::Text(t)) => format!("the text '{}'", cut(t)),
        Some(Token::Shape) => "`[shape]`".to_owned(),
        Some(Token::MapCellsize) => "`[map_cellsize]`".to_owned(),
        Some(Token::Binding(name)) => format!("`[{}]`", cut(name)),
        Some(Token::Word(word)) => format!("`{}`", cut(word)),
        Some(Token::Char(c)) if c.is_ascii_graphic() => format!("`{}`", char::from(*c)),
        Some(Token::Char(c)) => format!("the byte 0x{c:02x}"),
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    /// The levels of nesting entered, held to [`MAX_EXPRESSION_DEPTH`] (Q-9).
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn expect(&mut self, c: u8) -> Result<(), Undrawn> {
        match self.next() {
            Some(Token::Char(got)) if got == c => Ok(()),
            other => Err(syntax(&format!(
                "{} where `{}` is due",
                shown(other.as_ref()),
                char::from(c)
            ))),
        }
    }

    /// Whether the next token is the character `c`, which is then taken.
    fn take(&mut self, c: u8) -> bool {
        let taken = self.peek() == Some(&Token::Char(c));
        self.at += usize::from(taken);
        taken
    }

    fn enter(&mut self) -> Result<(), Undrawn> {
        self.depth += 1;
        if self.depth > MAX_EXPRESSION_DEPTH {
            return Err(Undrawn::Limit(format!(
                "nests deeper than {MAX_EXPRESSION_DEPTH} levels"
            )));
        }
        Ok(())
    }

    /// `shape_exp`: `[shape]`, a shape in parentheses, or a call of a shape function.
    fn shape(&mut self) -> Result<ShapeExp, Undrawn> {
        self.enter()?;
        let shape = match self.next() {
            Some(Token::Shape) => ShapeExp::Shape,
            Some(Token::Char(b'(')) => {
                let shape = self.shape()?;
                self.expect(b')')?;
                shape
            }
            Some(Token::Word(word)) => self.call(&word)?,
            Some(Token::Binding(name)) => {
                return Err(Undrawn::Unsupported(format!(
                    "binds `[{}]` where a shape is due, which the engine does not draw",
                    cut(&name)
                )));
            }
            other => {
                return Err(syntax(&format!(
                    "{} where a shape is due",
                    shown(other.as_ref())
                )));
            }
        };
        self.depth -= 1;
        Ok(shape)
    }

    /// A call of the shape function `name`, its name read.
    fn call(&mut self, name: &str) -> Result<ShapeExp, Undrawn> {
        if let Some((_, needs)) = NOT_DRAWN.iter().find(|(known, _)| *known == name) {
            return Err(Undrawn::Unsupported(format!(
                "calls {name}, which needs {needs} the engine has not"
            )));
        }
        if !["generalize", "densify", "outer", "inner", "smoothsia"].contains(&name) {
            return Err(Undrawn::Unsupported(format!(
                "calls `{}`, which is no shape function the engine draws",
                cut(name)
            )));
        }
        self.expect(b'(')?;
        let shape = Box::new(self.shape()?);
        let mut numbers = Vec::new();
        let mut text = None;
        // generalize and densify take one number, outer and inner none, smoothsia up to two and
        // then a text.
        let most = match name {
            "generalize" | "densify" => 1,
            "smoothsia" => 2,
            _ => 0,
        };
        while self.take(b',') {
            if numbers.len() < most {
                numbers.push(self.math()?);
            } else if name == "smoothsia" && text.is_none() {
                match self.next() {
                    Some(Token::Text(t)) => text = Some(t),
                    other => {
                        return Err(Undrawn::Unsupported(format!(
                            "gives smoothsia {} where a quoted text is due, which the \
                             engine does not read",
                            shown(other.as_ref())
                        )));
                    }
                }
            } else {
                return Err(syntax(&format!("{name} given too many arguments")));
            }
        }
        self.expect(b')')?;
        let mut numbers = numbers.into_iter();
        Ok(match name {
            "generalize" | "densify" => {
                let Some(tolerance) = numbers.next() else {
                    return Err(syntax(&format!("{name} without its tolerance")));
                };
                if name == "generalize" {
                    ShapeExp::Generalize(shape, tolerance)
                } else {
                    ShapeExp::Densify(shape, tolerance)
                }
            }
            "outer" | "inner" => ShapeExp::Rings(shape, name == "outer"),
            _ => ShapeExp::SmoothSia {
                shape,
                size: numbers.next(),
                iterations: numbers.next(),
                preprocessing: text,
            },
        })
    }

    /// `math_exp` at its lowest precedence, `+` and `-`, left-associative.
    fn math(&mut self) -> Result<MathExp, Undrawn> {
        self.enter()?;
        let mut left = self.term()?;
        while let Some(op) = self.binary(b"+-") {
            left = MathExp::Binary(Box::new(left), op, Box::new(self.term()?));
        }
        self.depth -= 1;
        Ok(left)
    }

    /// `*`, `/`, and `%`, left-associative, above `+` and `-`.
    fn term(&mut self) -> Result<MathExp, Undrawn> {
        let mut left = self.unary()?;
        while let Some(op) = self.binary(b"*/%") {
            left = MathExp::Binary(Box::new(left), op, Box::new(self.unary()?));
        }
        Ok(left)
    }

    /// The unary `-` (`NEG`), above `*`, below `^`.
    fn unary(&mut self) -> Result<MathExp, Undrawn> {
        if self.take(b'-') {
            self.enter()?;
            let operand = self.unary()?;
            self.depth -= 1;
            return Ok(MathExp::Neg(Box::new(operand)));
        }
        let base = self.primary()?;
        if self.take(b'^') {
            // Right-associative: its right side may hold a `-` and a `^` of its own.
            self.enter()?;
            let exponent = self.unary()?;
            self.depth -= 1;
            return Ok(MathExp::Binary(Box::new(base), b'^', Box::new(exponent)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<MathExp, Undrawn> {
        match self.next() {
            Some(Token::Number(n)) => Ok(MathExp::Number(n)),
            Some(Token::MapCellsize) => Ok(MathExp::MapCellsize),
            Some(Token::Char(b'(')) => {
                let math = self.math()?;
                self.expect(b')')?;
                Ok(math)
            }
            Some(Token::Binding(name)) => Err(Undrawn::Unsupported(format!(
                "binds `[{}]` where a number is due, which the engine does not draw",
                cut(&name)
            ))),
            other => Err(Undrawn::Unsupported(format!(
                "computes a number of {}, which the engine does not draw",
                shown(other.as_ref())
            ))),
        }
    }

    /// The next token where it is one of the operators `ops`, taken.
    fn binary(&mut self, ops: &[u8]) -> Option<u8> {
        match self.peek() {
            Some(&Token::Char(c)) if ops.contains(&c) => {
                self.at += 1;
                Some(c)
            }
            _ => None,
        }
    }
}
