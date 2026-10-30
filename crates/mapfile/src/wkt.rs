// The reader re-implements the grammar GEOS's WKTReader accepts, no GEOS code. The conversion
// follows MapServer's msGEOSGeometry2Shape (src/mapgeos.c) of the release pinned in
// scripts/fetch-msautotest.sh.
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

//! The frontend's own reader of the WKT of an inline feature (ADR-0033): it re-implements the
//! grammar GEOS's `WKTReader` accepts for two-dimensional points, lines, polygons, their multi
//! forms, and collections, no GEOS code, and converts what it reads into a shape as
//! `msGEOSGeometry2Shape` does. An empty geometry,
//! one with Z or M coordinates, and a `LINEARRING`, which `msGEOSGeometry2Shape` does not
//! convert, are refused. Collections nest at most [`MAX_NESTING`] deep, so that the reader's
//! recursion is bounded whatever the text (Q-9); its memory follows the text's length.

use std::borrow::Cow;

use forager_style_ir::ShapeType;

/// The deepest a `GEOMETRYCOLLECTION` nests in another.
pub const MAX_NESTING: usize = 32;

/// A shape as `msGEOSGeometry2Shape` makes it: its type and its parts.
#[derive(Debug, PartialEq)]
pub struct Shape {
    pub shape_type: ShapeType,
    pub parts: Vec<Vec<[f64; 2]>>,
}

/// Reads `text` into a shape, or says why it cannot.
pub fn read(text: &str) -> Result<Shape, String> {
    let mut reader = Reader { text, at: 0 };
    let shape = reader.tagged(0)?;
    match reader.next()? {
        Token::End => Ok(shape),
        token => Err(format!(
            "unexpected {} after the end of the geometry",
            token.name()
        )),
    }
}

#[derive(Debug, PartialEq)]
enum Token<'a> {
    Open,
    Close,
    Comma,
    Number(f64),
    /// A word in upper case.
    Word(String),
    /// A token that only `strtod` reads as a number, kept as written.
    Unread(&'a str),
    End,
}

impl Token<'_> {
    fn name(&self) -> String {
        match self {
            Token::Open => "`(`".to_owned(),
            Token::Close => "`)`".to_owned(),
            Token::Comma => "`,`".to_owned(),
            Token::Number(n) => format!("the number {n}"),
            Token::Word(w) => format!("`{}`", excerpt(w)),
            Token::Unread(t) => format!("`{}`", excerpt(t)),
            Token::End => "the end of the text".to_owned(),
        }
    }
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}

/// The characters that end a word or a number in the grammar GEOS accepts.
fn delimits(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\t' | ' ' | '(' | ')' | ',')
}

impl<'a> Reader<'a> {
    /// The next token of the grammar GEOS accepts: `(`, `)`, `,`, or a run of other characters up
    /// to one of them or a space, tab, or line break, a number where `strtod` reads it whole and a
    /// word otherwise. A number that is not finite, or that only `strtod` reads, such
    /// as a hexadecimal one, is refused where it is read.
    fn next(&mut self) -> Result<Token<'a>, String> {
        let rest = &self.text[self.at..];
        let trimmed = rest.trim_start_matches(['\n', '\r', '\t', ' ']);
        self.at += rest.len() - trimmed.len();
        let Some(c) = trimmed.chars().next() else {
            return Ok(Token::End);
        };
        let punct = match c {
            '(' => Some(Token::Open),
            ')' => Some(Token::Close),
            ',' => Some(Token::Comma),
            _ => None,
        };
        if let Some(token) = punct {
            self.at += 1;
            return Ok(token);
        }
        let len = trimmed.find(delimits).unwrap_or(trimmed.len());
        let raw = &trimmed[..len];
        self.at += len;
        Ok(match raw.parse::<f64>() {
            Ok(n) if n.is_finite() => Token::Number(n),
            Ok(_) => Token::Unread(raw),
            Err(_) if strtod_reads(raw) => Token::Unread(raw),
            Err(_) => Token::Word(raw.to_ascii_uppercase()),
        })
    }

    fn peek(&mut self) -> Result<Token<'a>, String> {
        let at = self.at;
        let token = self.next();
        self.at = at;
        token
    }

    fn number(&mut self) -> Result<f64, String> {
        match self.next()? {
            Token::Number(n) => Ok(n),
            Token::Unread(raw) => Err(format!("the number `{}` is not read", excerpt(raw))),
            token => Err(format!("expected a number, found {}", token.name())),
        }
    }

    /// The `(` that opens a geometry's text. `EMPTY`, `Z`, `M`, and
    /// `ZM` are refused.
    fn open(&mut self) -> Result<(), String> {
        match self.next()? {
            Token::Open => Ok(()),
            Token::Word(w) if w == "EMPTY" => Err("an empty geometry is not read".to_owned()),
            Token::Word(w) if matches!(w.as_str(), "Z" | "M" | "ZM") => {
                Err("Z and M coordinates are not read".to_owned())
            }
            token => Err(format!("expected `(`, found {}", token.name())),
        }
    }

    /// Whether another element follows: `,`, or `)` to end the list.
    fn more(&mut self) -> Result<bool, String> {
        match self.next()? {
            Token::Comma => Ok(true),
            Token::Close => Ok(false),
            token => Err(format!("expected `,` or `)`, found {}", token.name())),
        }
    }

    /// A coordinate of two numbers; a third, an undeclared Z, is refused.
    fn coordinate(&mut self) -> Result<[f64; 2], String> {
        let xy = [self.number()?, self.number()?];
        if matches!(self.peek()?, Token::Number(_) | Token::Unread(_)) {
            return Err("Z and M coordinates are not read".to_owned());
        }
        Ok(xy)
    }

    /// A parenthesized list of coordinates.
    fn coordinates(&mut self) -> Result<Vec<[f64; 2]>, String> {
        self.open()?;
        let mut coords = vec![self.coordinate()?];
        while self.more()? {
            coords.push(self.coordinate()?);
        }
        Ok(coords)
    }

    /// A line string's coordinates: GEOS refuses one of a single point.
    fn line(&mut self) -> Result<Vec<[f64; 2]>, String> {
        let coords = self.coordinates()?;
        if coords.len() < 2 {
            return Err("a line needs at least two points".to_owned());
        }
        Ok(coords)
    }

    /// A linear ring's coordinates: GEOS refuses one that is not closed or has fewer than four.
    fn ring(&mut self) -> Result<Vec<[f64; 2]>, String> {
        let coords = self.coordinates()?;
        if coords.len() < 4 {
            return Err("a ring needs at least four points".to_owned());
        }
        if coords.first() != coords.last() {
            return Err("a ring must end where it starts".to_owned());
        }
        Ok(coords)
    }

    /// A polygon's rings, its exterior first, as `msGEOSGeometry2Shape_polygon` adds them. It
    /// skips an inner ring GEOS does not call a ring, one that crosses itself; this reader keeps
    /// it, as it does not test whether a ring crosses itself.
    fn polygon(&mut self, parts: &mut Vec<Vec<[f64; 2]>>) -> Result<(), String> {
        self.open()?;
        parts.push(self.ring()?);
        while self.more()? {
            parts.push(self.ring()?);
        }
        Ok(())
    }

    /// A geometry with its type name, converted as `msGEOSGeometry2Shape` converts it; `depth`
    /// collections enclose it.
    fn tagged(&mut self, depth: usize) -> Result<Shape, String> {
        let tag = match self.next()? {
            Token::Word(w) => w,
            token => return Err(format!("expected a geometry type, found {}", token.name())),
        };
        // GEOS matches a type by its start, and reads Z, M, or ZM at its end as dimensions.
        if tag.ends_with('Z') || tag.ends_with('M') {
            return Err("Z and M coordinates are not read".to_owned());
        }
        let mut parts = Vec::new();
        let shape_type = if tag.starts_with("POINT") {
            // msGEOSGeometry2Shape_point: one part of one point.
            parts.push(self.coordinates_of_one()?);
            ShapeType::Point
        } else if tag.starts_with("LINESTRING") {
            parts.push(self.line()?);
            ShapeType::Line
        } else if tag.starts_with("POLYGON") {
            self.polygon(&mut parts)?;
            ShapeType::Polygon
        } else if tag.starts_with("MULTIPOINT") {
            // msGEOSGeometry2Shape_multipoint: one part of all the points.
            parts.push(self.multipoint()?);
            ShapeType::Point
        } else if tag.starts_with("MULTILINESTRING") {
            // msGEOSGeometry2Shape_multiline: a part for each line.
            self.open()?;
            parts.push(self.line()?);
            while self.more()? {
                parts.push(self.line()?);
            }
            ShapeType::Line
        } else if tag.starts_with("MULTIPOLYGON") {
            // msGEOSGeometry2Shape_multipolygon: every ring of every polygon, a part each.
            self.open()?;
            self.polygon(&mut parts)?;
            while self.more()? {
                self.polygon(&mut parts)?;
            }
            ShapeType::Polygon
        } else if tag.starts_with("GEOMETRYCOLLECTION") {
            // The parts of every member gathered into one line shape.
            if depth >= MAX_NESTING {
                return Err(format!("collections nest more than {MAX_NESTING} deep"));
            }
            self.open()?;
            parts.extend(self.tagged(depth + 1)?.parts);
            while self.more()? {
                parts.extend(self.tagged(depth + 1)?.parts);
            }
            ShapeType::Line
        } else if tag.starts_with("LINEARRING") {
            return Err("a LINEARRING is not converted into a shape".to_owned());
        } else {
            return Err(format!("unknown geometry type `{}`", excerpt(&tag)));
        };
        Ok(Shape { shape_type, parts })
    }

    /// A point's text: one coordinate in parentheses.
    fn coordinates_of_one(&mut self) -> Result<Vec<[f64; 2]>, String> {
        let coords = self.coordinates()?;
        if coords.len() != 1 {
            return Err("a point has one coordinate".to_owned());
        }
        Ok(coords)
    }

    /// A multipoint's text: its points bare, `(0 0, 1 1)`, or each in parentheses,
    /// `((0 0), (1 1))`.
    fn multipoint(&mut self) -> Result<Vec<[f64; 2]>, String> {
        self.open()?;
        let mut points = Vec::new();
        if matches!(self.peek()?, Token::Number(_) | Token::Unread(_)) {
            points.push(self.coordinate()?);
            while self.more()? {
                points.push(self.coordinate()?);
            }
        } else {
            points.extend(self.coordinates_of_one()?);
            while self.more()? {
                points.extend(self.coordinates_of_one()?);
            }
        }
        Ok(points)
    }
}

/// The first [`EXCERPT_CHARS`] characters of `text` a message quotes, with `…` where it goes on,
/// so that a message stays short whatever the text.
pub fn excerpt(text: &str) -> Cow<'_, str> {
    match text.char_indices().nth(EXCERPT_CHARS) {
        Some((at, _)) => Cow::Owned(format!("{}…", &text[..at])),
        None => Cow::Borrowed(text),
    }
}

/// The most characters of a text a message quotes.
pub const EXCERPT_CHARS: usize = 80;

/// Whether C's `strtod` reads `raw` whole where Rust's parser does not: a hexadecimal number,
/// or `nan` with its parenthesized tag.
fn strtod_reads(raw: &str) -> bool {
    let unsigned = raw.strip_prefix(['+', '-']).unwrap_or(raw);
    let lower = unsigned.to_ascii_lowercase();
    lower.starts_with("0x") && lower.len() > 2 || lower.starts_with("nan(")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(text: &str) -> (ShapeType, Vec<Vec<[f64; 2]>>) {
        let s = read(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        (s.shape_type, s.parts)
    }

    #[test]
    fn each_type_converts_as_msgeosgeometry2shape_converts_it() {
        assert_eq!(
            shape("POINT(1 2)"),
            (ShapeType::Point, vec![vec![[1.0, 2.0]]])
        );
        assert_eq!(
            shape("MULTIPOINT(1 2, 3 4)"),
            (ShapeType::Point, vec![vec![[1.0, 2.0], [3.0, 4.0]]])
        );
        assert_eq!(
            shape("multipoint ((1 2),(3 4))"),
            (ShapeType::Point, vec![vec![[1.0, 2.0], [3.0, 4.0]]])
        );
        assert_eq!(
            shape("LINESTRING (0 0, 1 1)"),
            (ShapeType::Line, vec![vec![[0.0, 0.0], [1.0, 1.0]]])
        );
        assert_eq!(
            shape("MULTILINESTRING((0 0,1 1),(2 2,3 3))"),
            (
                ShapeType::Line,
                vec![vec![[0.0, 0.0], [1.0, 1.0]], vec![[2.0, 2.0], [3.0, 3.0]]]
            )
        );
        let square = |o: f64| vec![[o, o], [o + 1.0, o], [o + 1.0, o + 1.0], [o, o]];
        assert_eq!(
            shape("POLYGON((0 0,1 0,1 1,0 0),(5 5,6 5,6 6,5 5))"),
            (ShapeType::Polygon, vec![square(0.0), square(5.0)])
        );
        assert_eq!(
            shape("MULTIPOLYGON(((0 0,1 0,1 1,0 0)),((5 5,6 5,6 6,5 5)))"),
            (ShapeType::Polygon, vec![square(0.0), square(5.0)])
        );
    }

    #[test]
    fn a_collection_gathers_its_members_parts_into_a_line_shape() {
        assert_eq!(
            shape(
                "GEOMETRYCOLLECTION(POINT(1 2), LINESTRING(0 0, 1 1), \
                 GEOMETRYCOLLECTION(POLYGON((0 0,1 0,1 1,0 0))))"
            ),
            (
                ShapeType::Line,
                vec![
                    vec![[1.0, 2.0]],
                    vec![[0.0, 0.0], [1.0, 1.0]],
                    vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]],
                ]
            )
        );
    }

    #[test]
    fn numbers_and_words_are_read_as_geoss_tokenizer_reads_them() {
        assert_eq!(
            shape("\tPoint\n( -1.5e1\r+.5 )  "),
            (ShapeType::Point, vec![vec![[-15.0, 0.5]]])
        );
        // GEOS matches a type by its start.
        assert_eq!(shape("POINTS(1 2)").0, ShapeType::Point);
    }

    #[test]
    fn what_msshapefromwkt_would_not_convert_is_refused() {
        for text in [
            "",
            "POINT EMPTY",
            "MULTIPOINT(EMPTY, (1 1))",
            "GEOMETRYCOLLECTION EMPTY",
            "POINT Z (1 2 3)",
            "POINTZ(1 2 3)",
            "POINT(1 2 3)",
            "POINTM(1 2 3)",
            "LINESTRING(1 2 3, 4 5 6)",
            "LINESTRING(1 2)",
            "POLYGON((0 0,1 0,0 0))",
            "POLYGON((0 0,1 0,1 1,0 1))",
            "LINEARRING(0 0,1 0,1 1,0 0)",
            "TRIANGLE((0 0,1 0,1 1,0 0))",
            "POINT(1 2) POINT(3 4)",
            "POINT(1 2",
            "POINT(1)",
            "POINT(1 2, 3 4)",
            "POINT(1 inf)",
            "POINT(1 nan)",
            "POINT(0x10 2)",
            "POINT(1e999 2)",
            "(1 2)",
            "12",
        ] {
            assert!(read(text).is_err(), "{text:?} was read");
        }
    }

    #[test]
    fn a_message_quotes_a_long_token_in_part() {
        let long = "X".repeat(1000);
        let message = read(&format!("{long}(1 2)")).unwrap_err();
        assert!(message.len() < 200, "{message}");
        assert!(message.contains(&format!("{}…", "X".repeat(EXCERPT_CHARS))));
    }

    #[test]
    fn nesting_is_bounded_whatever_the_text() {
        // Verifies: Q-9
        let nested = |depth: usize| {
            format!(
                "{}POINT(1 2){}",
                "GEOMETRYCOLLECTION(".repeat(depth),
                ")".repeat(depth)
            )
        };
        assert!(read(&nested(MAX_NESTING)).is_ok());
        let refused = read(&nested(MAX_NESTING + 1)).unwrap_err();
        assert!(refused.contains("nest"), "{refused}");
        // Far past the bound, the reader stops at it rather than recursing on.
        assert!(read(&nested(100_000)).is_err());
    }
}
