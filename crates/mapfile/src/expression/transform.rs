// A GEOMTRANSFORM expression is read as MapServer's lexer, src/maplexer.l, splits it into tokens
// and its parser, src/mapparser.y, reduces the rules `shape_exp`, `math_exp`, and `string_exp`, of
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

//! Translates a `GEOMTRANSFORM` expression into its transform tree
//! (ADR-0037): `[shape]`, and `generalize`, `densify`, `outer`, `inner`, and
//! `smoothsia` of a shape, their numbers written, `[map_cellsize]`, or computed of those, each
//! operation without `[map_cellsize]` folded at load. An expression the grammar parses that holds
//! any other construct is the tree `not_drawn`, naming the first such construct in reading order;
//! one it does not parse is `not_drawn: syntax`, with a warning at its place.
//!
//! The parser reads the whole expression, the arguments of a function the tree has no node for
//! among it, so that a syntax error anywhere is found. It counts each node of the tree it builds
//! as one level, each operator of a chain among them, and refuses a tree past
//! [`MAX_EXPRESSION_DEPTH`] before it recurses deeper (Q-9).

use forager_style_ir::c::int_cast;
use forager_style_ir::{
    MAX_EXPRESSION_DEPTH, NotDrawn, NumberOp, Preprocessing, TransformNumber, TransformShape,
    TransformTree,
};

use super::{Lexeme, Problem, Scanner, Tok, too_deep};

/// A transform tree, and where the expression has a syntax error, why, at the character of the
/// mapfile's token, its opening parenthesis at 0.
#[derive(Debug, PartialEq)]
pub struct Translated {
    pub tree: TransformTree,
    pub syntax: Option<(String, usize)>,
}

/// The tree of a `GEOMTRANSFORM` expression, the token's text with its parentheses, which
/// loadLayer and loadStyle strip; the load error `expression-too-deep` for one that nests past
/// [`MAX_EXPRESSION_DEPTH`].
pub fn geom_transform(text: &str) -> Result<Translated, Problem> {
    let inner = text
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(text);
    let offset = usize::from(inner.len() != text.len());
    let mut parser = Parser {
        tokens: Scanner::new(inner)
            .map(|(lexeme, span)| (lexeme, span.start))
            .collect(),
        text: inner,
        at: 0,
        calls: 0,
        first: None,
    };
    let parsed = parser.shape(1).and_then(|(shape, _)| match parser.peek() {
        None => Ok(shape),
        Some(_) => Err(parser.syntax_at(parser.at, "after the shape")),
    });
    let syntax = |(what, at): (String, usize)| {
        let character = inner.get(..at).unwrap_or(inner).chars().count() + offset;
        Ok(Translated {
            tree: TransformTree::NotDrawn(NotDrawn::Syntax),
            syntax: Some((what, character)),
        })
    };
    match parsed {
        Err(Fail::TooDeep) => Err(too_deep()),
        Err(Fail::Syntax(what, at)) => syntax((what, at)),
        Ok(shape) => {
            let tree = match (parser.first, shape) {
                (Some(reason), _) => TransformTree::NotDrawn(reason),
                (None, Some(shape)) => TransformTree::Drawn(shape),
                (None, None) => TransformTree::NotDrawn(NotDrawn::Syntax),
            };
            Ok(Translated { tree, syntax: None })
        }
    }
}

/// Why the parse stopped: a syntax error, with what was found and its byte in the expression, or
/// a tree past the nesting bound.
#[derive(Debug)]
enum Fail {
    Syntax(String, usize),
    TooDeep,
}

/// A number argument as the parser builds it: the number, or none where it holds a construct the
/// tree has no node for; and its height, the levels of the tree below and at it.
type Number = (Option<TransformNumber>, usize);

/// A text argument: a quoted string as written, or one computed; none where it holds a
/// construct the tree has no node for.
enum Text {
    Literal(String),
    Computed,
    NotDrawn,
}

/// The most characters of the expression a diagnostic quotes.
const QUOTED: usize = 32;

fn cut(text: &str) -> String {
    if text.chars().count() <= QUOTED {
        text.to_owned()
    } else {
        let head: String = text.chars().take(QUOTED).collect();
        format!("{head}\u{2026}")
    }
}

/// The binding precedences of `math_exp`'s operators, as `%left` and `%right` order them.
const ADD: u8 = 1;
const MUL: u8 = 2;
const NEG: u8 = 3;
const POW: u8 = 4;

struct Parser<'a> {
    tokens: Vec<(Lexeme, usize)>,
    text: &'a str,
    at: usize,
    /// How deeply the parser has recursed: the bound, a multiple of the tree's, keeps
    /// parentheses and unary minuses, which nest no node, from overflowing the stack.
    calls: usize,
    /// The first construct, in reading order, the tree has no node for.
    first: Option<NotDrawn>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Lexeme> {
        self.tokens.get(self.at).map(|(lexeme, _)| lexeme)
    }

    fn next(&mut self) -> Option<Lexeme> {
        let lexeme = self.tokens.get(self.at).map(|(lexeme, _)| lexeme.clone());
        self.at += 1;
        lexeme
    }

    /// The byte at which the token at `at` starts, the expression's end past the last.
    fn byte(&self, at: usize) -> usize {
        self.tokens
            .get(at)
            .map_or(self.text.len(), |&(_, start)| start)
    }

    /// The token at `at` as a diagnostic names it.
    fn shown(&self, at: usize) -> String {
        let Some((lexeme, start)) = self.tokens.get(at) else {
            return "the end of the expression".to_owned();
        };
        let end = self.byte(at + 1).min(self.text.len());
        let written = self.text.get(*start..end).unwrap_or_default().trim_end();
        match lexeme {
            Lexeme::Unterminated => "an unterminated string".to_owned(),
            _ => format!("`{}`", cut(written)),
        }
    }

    /// A syntax error at the token at `at`, `what` saying why.
    fn syntax_at(&self, at: usize, what: &str) -> Fail {
        Fail::Syntax(format!("{} {what}", self.shown(at)), self.byte(at))
    }

    fn record(&mut self, reason: NotDrawn) {
        self.first.get_or_insert(reason);
    }

    /// Enters a node of the tree at `level`, the root at 1, refused past the bound.
    fn enter(&mut self, level: usize) -> Result<(), Fail> {
        self.calls += 1;
        if level > MAX_EXPRESSION_DEPTH || self.calls > 4 * MAX_EXPRESSION_DEPTH {
            return Err(Fail::TooDeep);
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.calls -= 1;
    }

    /// Whether the next token is the symbol `symbol`, which is then taken.
    fn take(&mut self, symbol: &str) -> bool {
        let taken = matches!(self.peek(), Some(Lexeme::Tok(Tok::Symbol(s))) if *s == symbol);
        self.at += usize::from(taken);
        taken
    }

    fn expect(&mut self, symbol: &str) -> Result<(), Fail> {
        if self.take(symbol) {
            return Ok(());
        }
        Err(self.syntax_at(self.at, &format!("where `{symbol}` is due")))
    }

    /// `shape_exp` at `level`: the shape, none where it holds a construct the tree has no node
    /// for, and its height.
    fn shape(&mut self, level: usize) -> Result<(Option<TransformShape>, usize), Fail> {
        self.enter(level)?;
        let at = self.at;
        let shape = match self.next() {
            Some(Lexeme::Special(name)) if name.eq_ignore_ascii_case("shape") => {
                (Some(TransformShape::Shape), 1)
            }
            Some(Lexeme::Tok(Tok::Symbol("("))) => {
                let shape = self.shape(level)?;
                self.expect(")")?;
                shape
            }
            Some(Lexeme::Tok(Tok::Word(word))) => self.call(&word, at, level)?,
            _ => return Err(self.syntax_at(at, "where a shape is due")),
        };
        self.leave();
        Ok(shape)
    }

    /// A call of the shape function `name`, its name read at the token `at`.
    fn call(
        &mut self,
        name: &str,
        at: usize,
        level: usize,
    ) -> Result<(Option<TransformShape>, usize), Fail> {
        let below = level + 1;
        let not_drawn = match name {
            "buffer" => Some(NotDrawn::Buffer),
            "centerline" => Some(NotDrawn::Centerline),
            "difference" => Some(NotDrawn::Difference),
            "simplify" => Some(NotDrawn::Simplify),
            "simplifypt" => Some(NotDrawn::Simplifypt),
            "javascript" => Some(NotDrawn::Javascript),
            "generalize" | "densify" | "outer" | "inner" | "smoothsia" => None,
            // msTokenizeExpression reads `fromtext('WKT')` into a shape of the mapfile's text,
            // which the tree cannot hold: `(`, a string, and `)`, or it fails the parse.
            "fromtext" => {
                self.record(NotDrawn::Fromtext);
                self.expect("(")?;
                let text = self.at;
                if !matches!(self.next(), Some(Lexeme::Tok(Tok::Str(_)))) {
                    return Err(self.syntax_at(text, "where fromtext's text is due"));
                }
                self.expect(")")?;
                return Ok((None, 1));
            }
            _ => return Err(self.syntax_at(at, "where a shape is due")),
        };
        if let Some(reason) = not_drawn {
            self.record(reason);
        }
        self.expect("(")?;
        let (shape, shape_height) = self.shape(below)?;
        let shape = shape.map(Box::new);
        let mut height = shape_height;
        let built = match name {
            "generalize" | "densify" | "buffer" | "simplify" | "simplifypt" => {
                self.expect(",")?;
                let (number, number_height) = self.math(below, 0)?;
                height = height.max(number_height);
                match (name, shape, number) {
                    ("generalize", Some(s), Some(n)) => Some(TransformShape::Generalize(s, n)),
                    ("densify", Some(s), Some(n)) => Some(TransformShape::Densify(s, n)),
                    _ => None,
                }
            }
            "outer" => shape.map(TransformShape::Outer),
            "inner" => shape.map(TransformShape::Inner),
            "centerline" => None,
            "difference" => {
                self.expect(",")?;
                let (_, other) = self.shape(below)?;
                height = height.max(other);
                None
            }
            "javascript" => {
                self.expect(",")?;
                let (_, text) = self.text(below, 0)?;
                height = height.max(text);
                None
            }
            _ => {
                let (smoothed, args) = self.smoothsia_args(below)?;
                height = height.max(args);
                shape
                    .zip(smoothed)
                    .map(
                        |(shape, (size, iterations, preprocessing))| TransformShape::SmoothSia {
                            shape,
                            size,
                            iterations,
                            preprocessing,
                        },
                    )
            }
        };
        self.expect(")")?;
        Ok((built, height + 1))
    }

    /// The arguments of `smoothsia` after its shape, of the rules `SMOOTHSIA '(' shape_exp [','
    /// math_exp [',' math_exp [',' string_exp]]] ')'`: its size and iterations, 3 and 1 where
    /// they are not written as msSmoothShapeSIA(s, 3, 1, NULL) and (s, size, 1, NULL) take them,
    /// and its preprocessing, `all` and `angle` read without regard to case as strcasecmp reads
    /// them, any other text none; none where one holds a construct the tree has no node for.
    #[expect(
        clippy::type_complexity,
        reason = "the three arguments, as the variant holds them"
    )]
    fn smoothsia_args(
        &mut self,
        level: usize,
    ) -> Result<
        (
            Option<(TransformNumber, TransformNumber, Option<Preprocessing>)>,
            usize,
        ),
        Fail,
    > {
        let mut numbers: Vec<Option<TransformNumber>> = Vec::new();
        let mut height = 0;
        let mut preprocessing = None;
        let mut drawn = true;
        while numbers.len() < 2 && self.take(",") {
            let (number, number_height) = self.math(level, 0)?;
            height = height.max(number_height);
            numbers.push(number);
        }
        if numbers.len() == 2 && self.take(",") {
            let (text, text_height) = self.text(level, 0)?;
            height = height.max(text_height);
            match text {
                Text::Literal(text) if text.eq_ignore_ascii_case("all") => {
                    preprocessing = Some(Preprocessing::All);
                }
                Text::Literal(text) if text.eq_ignore_ascii_case("angle") => {
                    preprocessing = Some(Preprocessing::Angle);
                }
                Text::Literal(_) => {}
                Text::Computed => {
                    self.record(NotDrawn::Text);
                    drawn = false;
                }
                Text::NotDrawn => drawn = false,
            }
        }
        let mut numbers = numbers.into_iter();
        let size = numbers.next().unwrap_or(Some(TransformNumber::Number(3.0)));
        let iterations = numbers.next().unwrap_or(Some(TransformNumber::Number(1.0)));
        let args = size
            .zip(iterations)
            .filter(|_| drawn)
            .map(|(size, iterations)| (size, iterations, preprocessing));
        Ok((args, height))
    }

    /// `math_exp` at `level`, its operators binding tighter than `min`.
    fn math(&mut self, level: usize, min: u8) -> Result<Number, Fail> {
        self.enter(level)?;
        let mut left = self.prefix(level)?;
        while let Some((op, precedence)) = self.operator() {
            if precedence <= min {
                break;
            }
            let at = self.at;
            self.at += 1;
            // `^` is right-associative, every other operator left-associative.
            let right = self.math(
                level + 1,
                if op == NumberOp::Pow { NEG } else { precedence },
            )?;
            let height = left.1.max(right.1) + 1;
            // The chain's left side moved one level down: its deepest node sits at
            // `level - 1 + height`.
            if level - 1 + height > MAX_EXPRESSION_DEPTH {
                return Err(Fail::TooDeep);
            }
            left = (self.combine(op, left.0, right.0, at)?, height);
        }
        self.leave();
        Ok(left)
    }

    /// The next token where it is an operator of `math_exp`, with its precedence.
    fn operator(&self) -> Option<(NumberOp, u8)> {
        let Some(Lexeme::Tok(Tok::Symbol(s))) = self.peek() else {
            return None;
        };
        Some(match *s {
            "+" => (NumberOp::Add, ADD),
            "-" => (NumberOp::Sub, ADD),
            "*" => (NumberOp::Mul, MUL),
            "/" => (NumberOp::Div, MUL),
            "%" => (NumberOp::Mod, MUL),
            "^" => (NumberOp::Pow, POW),
            _ => return None,
        })
    }

    /// `left op right`, folded to one number where neither holds `[map_cellsize]`, as the rules
    /// of `math_exp` compute it: `%` of the operands cut to C's `int`s, `/` by 0 a parse error
    /// (`yyerror(p, "Division by zero.")`) at the operator, the token `at`. A remainder by 0, or of
    /// `INT_MIN` by -1, on which the x86-64 build traps, and a result that is not finite, which
    /// the style IR cannot hold, fold to no number: the construct `literal`.
    fn combine(
        &mut self,
        op: NumberOp,
        left: Option<TransformNumber>,
        right: Option<TransformNumber>,
        at: usize,
    ) -> Result<Option<TransformNumber>, Fail> {
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(None);
        };
        let (TransformNumber::Number(a), TransformNumber::Number(b)) = (&left, &right) else {
            return Ok(Some(TransformNumber::Op(
                op,
                Box::new(left),
                Box::new(right),
            )));
        };
        let (a, b) = (*a, *b);
        let value = match op {
            NumberOp::Add => a + b,
            NumberOp::Sub => a - b,
            NumberOp::Mul => a * b,
            NumberOp::Div if b == 0.0 => return Err(self.syntax_at(at, "divides by zero")),
            NumberOp::Div => a / b,
            NumberOp::Mod => match int_cast(a).checked_rem(int_cast(b)) {
                Some(r) => f64::from(r),
                None => {
                    self.record(NotDrawn::Literal);
                    return Ok(None);
                }
            },
            NumberOp::Pow => libm::pow(a, b),
        };
        if !value.is_finite() {
            self.record(NotDrawn::Literal);
            return Ok(None);
        }
        Ok(Some(TransformNumber::Number(value)))
    }

    /// An operand of `math_exp`: a number, `[map_cellsize]`, a number in parentheses, the unary
    /// `-`, or a function of a number.
    fn prefix(&mut self, level: usize) -> Result<Number, Fail> {
        let at = self.at;
        Ok(match self.next() {
            Some(Lexeme::Tok(Tok::Symbol("-"))) => {
                // `'-' math_exp %prec NEG { $$ = $2; }`: the operand, not negated.
                let operand = self.math(level, NEG)?;
                return Ok(operand);
            }
            Some(Lexeme::Tok(Tok::Number(n))) if n.is_finite() => {
                (Some(TransformNumber::Number(n)), 1)
            }
            // A literal past a double, which atof reads as infinite: no finite JSON number.
            Some(Lexeme::Tok(Tok::Number(_))) => {
                self.record(NotDrawn::Literal);
                (None, 1)
            }
            Some(Lexeme::Special(name)) if name.eq_ignore_ascii_case("map_cellsize") => {
                (Some(TransformNumber::MapCellsize), 1)
            }
            // msGeomTransformShape binds `[data_cellsize]` to 0 on every layer but a contour
            // layer (src/mapgeomtransform.c:285-291), which Forager does not carry.
            Some(Lexeme::Special(name)) if name.eq_ignore_ascii_case("data_cellsize") => {
                (Some(TransformNumber::Number(0.0)), 1)
            }
            Some(Lexeme::Tok(Tok::NumberBinding(_))) => {
                self.record(NotDrawn::Binding);
                (None, 1)
            }
            Some(Lexeme::Tok(Tok::Symbol("("))) => {
                let number = self.math(level, 0)?;
                self.expect(")")?;
                number
            }
            Some(Lexeme::Tok(Tok::Word(word))) => {
                let below = level + 1;
                let reason = match word.as_str() {
                    "length" => NotDrawn::Length,
                    "area" => NotDrawn::Area,
                    "round" => NotDrawn::Round,
                    _ => return Err(self.syntax_at(at, "where a number is due")),
                };
                self.record(reason);
                self.expect("(")?;
                let mut height = match reason {
                    NotDrawn::Length => self.text(below, 0)?.1,
                    NotDrawn::Area => self.shape(below)?.1,
                    _ => self.math(below, 0)?.1,
                };
                if reason == NotDrawn::Round && self.take(",") {
                    height = height.max(self.math(below, 0)?.1);
                }
                self.expect(")")?;
                (None, height + 1)
            }
            _ => return Err(self.syntax_at(at, "where a number is due")),
        })
    }

    /// `string_exp` at `level`, its `+` taken where `min` is 0.
    fn text(&mut self, level: usize, min: u8) -> Result<(Text, usize), Fail> {
        self.enter(level)?;
        let mut left = self.text_operand(level)?;
        while min == 0 && self.take("+") {
            let (right, right_height) = self.text(level + 1, ADD)?;
            let height = left.1.max(right_height) + 1;
            if level - 1 + height > MAX_EXPRESSION_DEPTH {
                return Err(Fail::TooDeep);
            }
            let text = match (left.0, right) {
                (Text::NotDrawn, _) | (_, Text::NotDrawn) => Text::NotDrawn,
                _ => Text::Computed,
            };
            left = (text, height);
        }
        self.leave();
        Ok(left)
    }

    /// An operand of `string_exp`: a string, a string in parentheses, or a function of one.
    fn text_operand(&mut self, level: usize) -> Result<(Text, usize), Fail> {
        let at = self.at;
        Ok(match self.next() {
            Some(Lexeme::Tok(Tok::Str(text))) => (Text::Literal(text), 1),
            Some(Lexeme::Tok(Tok::StringBinding(_))) => {
                self.record(NotDrawn::Binding);
                (Text::NotDrawn, 1)
            }
            Some(Lexeme::Tok(Tok::Symbol("("))) => {
                let text = self.text(level, 0)?;
                self.expect(")")?;
                text
            }
            Some(Lexeme::Tok(Tok::Word(word))) => {
                let below = level + 1;
                if !matches!(
                    word.as_str(),
                    "tostring" | "commify" | "upper" | "lower" | "initcap" | "firstcap"
                ) {
                    return Err(self.syntax_at(at, "where a text is due"));
                }
                self.expect("(")?;
                let mut height = 0;
                let mut drawn = true;
                if word == "tostring" {
                    let (number, number_height) = self.math(below, 0)?;
                    drawn &= number.is_some();
                    height = number_height;
                    self.expect(",")?;
                }
                let (text, text_height) = self.text(below, 0)?;
                drawn &= !matches!(text, Text::NotDrawn);
                self.expect(")")?;
                let text = if drawn {
                    Text::Computed
                } else {
                    Text::NotDrawn
                };
                (text, height.max(text_height) + 1)
            }
            _ => return Err(self.syntax_at(at, "where a text is due")),
        })
    }
}

#[cfg(test)]
mod tests;
