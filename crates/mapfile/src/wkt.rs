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
//! `msGEOSGeometry2Shape` does, an inner ring that is not simple left out. An empty geometry,
//! one with Z or M coordinates, and a `LINEARRING`, which `msGEOSGeometry2Shape` does not
//! convert, are refused. Collections nest at most [`MAX_NESTING`] deep, so that the reader's
//! recursion is bounded whatever the text (Q-9); its memory follows the text's length.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ops::Bound::{Excluded, Unbounded};

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

    /// A polygon's rings, its exterior first, as `msGEOSGeometry2Shape_polygon` and
    /// `_multipolygon` add them: an inner ring that is not simple ([`is_simple_ring`]), where
    /// `GEOSisRing` fails, is read and left out; the exterior is kept whatever it is.
    fn polygon(&mut self, parts: &mut Vec<Vec<[f64; 2]>>) -> Result<(), String> {
        self.open()?;
        parts.push(self.ring()?);
        while self.more()? {
            let ring = self.ring()?;
            if is_simple_ring(&ring) {
                parts.push(ring);
            }
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

/// Whether a closed ring is simple, as GEOS's `isRing` asks of an inner ring: walked from its
/// first point back to it, the ring passes through no point twice. A point repeated in a row is
/// one vertex, not a second pass. So, of the ring's distinct edges, two in a row meet only at
/// the vertex they share, without doubling back along each other, and any other two do not meet
/// at all, not even at an end. A ring that collapses to fewer than three vertices, a point or
/// one edge walked there and back, is not simple.
///
/// The test is a sweep of the edges in the lexicographic order of their points that keeps the
/// edges the sweep line cuts in a tree ordered from bottom to top and tests each edge, as it is
/// inserted or as its neighbours become adjacent, only against its neighbours there: its time
/// is O(n log n) in the ring's vertices and its memory O(n), whatever the ring (Q-9). It stops
/// at the first meeting it finds; until then the edges the line cuts do not cross, so the order
/// of the tree is consistent. Every decision is exact, without a rounding error to depend on a
/// platform (Q-2): points are compared by their coordinates, and which side of a line a point
/// lies on is the sign of a determinant [`orientation`] computes exactly; no point of
/// intersection is ever computed.
fn is_simple_ring(ring: &[[f64; 2]]) -> bool {
    // The ring's vertices, a point repeated in a row once, the closing point not again.
    let mut vertices: Vec<[f64; 2]> = Vec::with_capacity(ring.len());
    for &point in ring {
        if vertices.last() != Some(&point) {
            vertices.push(point);
        }
    }
    while vertices.len() > 1 && vertices.first() == vertices.last() {
        vertices.pop();
    }
    let k = vertices.len();
    if k < 3 {
        return false;
    }
    let at = |i: usize| vertices[i % k];
    // A vertex passed twice: two vertices at one point.
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_unstable_by(|&i, &j| lex(vertices[i], vertices[j]));
    if order
        .windows(2)
        .any(|w| lex(vertices[w[0]], vertices[w[1]]) == Ordering::Equal)
    {
        return false;
    }
    // Two edges in a row that double back: on one line, with their far ends on one side.
    for i in 0..k {
        let (a, b, c) = (at(i + k - 1), at(i), at(i + 1));
        if orientation(a, b, c) == Ordering::Equal && lex(a, b) == lex(c, b) {
            return false;
        }
    }
    // Edge `i` runs from vertex `i` to the next, its ends sorted.
    let edge = |id: usize| {
        let (p, q) = (at(id), at(id + 1));
        if lex(p, q) == Ordering::Less {
            Edge { l: p, r: q, id }
        } else {
            Edge { l: q, r: p, id }
        }
    };
    let in_a_row = |a: &Edge, b: &Edge| (a.id + 1) % k == b.id || (b.id + 1) % k == a.id;
    let mut line: BTreeSet<Edge> = BTreeSet::new();
    for &v in &order {
        let point = vertices[v];
        let incident = [edge((v + k - 1) % k), edge(v)];
        // The edges that start here are inserted before those that end here leave.
        for e in incident.iter().filter(|e| e.l == point) {
            if !line.insert(*e) {
                return false;
            }
            let below = line.range(..*e).next_back();
            let above = line.range((Excluded(*e), Unbounded)).next();
            for n in [below, above].into_iter().flatten() {
                if !in_a_row(e, n) && meets(e, n) {
                    return false;
                }
            }
        }
        for e in incident.iter().filter(|e| e.r == point) {
            let below = line.range(..*e).next_back().copied();
            let above = line.range((Excluded(*e), Unbounded)).next().copied();
            // An edge the tree cannot find means its order broke, which only edges that cross do.
            if !line.remove(e) {
                return false;
            }
            if let (Some(b), Some(a)) = (below, above)
                && !in_a_row(&b, &a)
                && meets(&b, &a)
            {
                return false;
            }
        }
    }
    true
}

/// An edge of a ring, its lexicographically lesser end `l` first, and its place `id` in the ring.
#[derive(Clone, Copy, Debug)]
struct Edge {
    l: [f64; 2],
    r: [f64; 2],
    id: usize,
}

impl Ord for Edge {
    /// The order of two edges the sweep line cuts, from bottom to top: the side of the edge
    /// that starts first on which the other starts, or, where it starts on its line, ends; two
    /// edges on one line by their place in the ring.
    fn cmp(&self, other: &Self) -> Ordering {
        if self.id == other.id {
            return Ordering::Equal;
        }
        let side = |base: &Edge, e: &Edge| match orientation(base.l, base.r, e.l) {
            Ordering::Equal => orientation(base.l, base.r, e.r),
            side => side,
        };
        let order = if lex(other.l, self.l) != Ordering::Greater {
            side(other, self)
        } else {
            side(self, other).reverse()
        };
        order.then(self.id.cmp(&other.id))
    }
}

impl PartialOrd for Edge {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Edge {}

/// Whether two edges have a point in common, their ends included.
fn meets(s: &Edge, t: &Edge) -> bool {
    let d = [
        orientation(s.l, s.r, t.l),
        orientation(s.l, s.r, t.r),
        orientation(t.l, t.r, s.l),
        orientation(t.l, t.r, s.r),
    ];
    let crosses = |a: Ordering, b: Ordering| a != Ordering::Equal && b != Ordering::Equal && a != b;
    // A point on the line of an edge lies on the edge when it lies between its ends.
    let on = |e: &Edge, p: [f64; 2]| {
        lex(e.l, p) != Ordering::Greater && lex(p, e.r) != Ordering::Greater
    };
    crosses(d[0], d[1]) && crosses(d[2], d[3])
        || d[0] == Ordering::Equal && on(s, t.l)
        || d[1] == Ordering::Equal && on(s, t.r)
        || d[2] == Ordering::Equal && on(t, s.l)
        || d[3] == Ordering::Equal && on(t, s.r)
}

/// The lexicographic order of two points, by x and then by y; `-0` and `0` are one coordinate.
fn lex(a: [f64; 2], b: [f64; 2]) -> Ordering {
    let cmp = |x: f64, y: f64| x.partial_cmp(&y).unwrap_or(Ordering::Equal);
    cmp(a[0], b[0]).then(cmp(a[1], b[1]))
}

/// The side of the line from `a` to `b` on which `c` lies: `Greater` to its left, `Less` to its
/// right, `Equal` on it; the sign of `(a - c) × (b - c)`, decided exactly for finite coordinates.
/// It is computed in floating point first, and that sign is taken where the value exceeds a bound
/// on its rounding error (the static filter of Shewchuk's adaptive predicates, widened to cover
/// underflow below the smallest sum it accepts); otherwise [`exact_orientation`] decides.
fn orientation(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Ordering {
    #[cfg(test)]
    tests::ORIENTATIONS.with(|n| n.set(n.get() + 1));
    let left = (a[0] - c[0]) * (b[1] - c[1]);
    let right = (a[1] - c[1]) * (b[0] - c[0]);
    let det = left - right;
    let sum = left.abs() + right.abs();
    // Four units of rounding where Shewchuk proves 3 + 16ε; the unit left over covers the
    // absolute error of an underflow, at most 2^-1074 a product, far below it from 2^-900 on.
    let bound = 4.0 * f64::EPSILON / 2.0 * sum;
    if sum.is_finite() && sum >= MIN_FILTERED && det.abs() > bound {
        return det.partial_cmp(&0.0).unwrap_or(Ordering::Equal);
    }
    exact_orientation(a, b, c)
}

/// The least sum of the determinant's two products whose floating-point sign is trusted, above
/// 2^-900.
const MIN_FILTERED: f64 = 1e-270;

/// The words of the fixed-point integer [`exact_orientation`] sums into: the product of two
/// finite doubles is an integer of at most 106 bits times a power of two from 2^-2148 to 2^1942,
/// so six of them fit in 2148 + 1942 + 106 + 3 bits.
const WORDS: usize = 67;

/// The sign of `(a - c) × (b - c)` in exact integer arithmetic: the determinant expanded into six
/// products of coordinates, each exact as an integer times a power of two, summed into a
/// fixed-point integer wide enough for any of them.
fn exact_orientation(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Ordering {
    let terms = [
        (a[0], b[1], true),
        (a[0], c[1], false),
        (c[0], b[1], false),
        (a[1], b[0], false),
        (a[1], c[0], true),
        (b[0], c[1], true),
    ];
    let mut positive = [0u64; WORDS];
    let mut negative = [0u64; WORDS];
    for (x, y, plus) in terms {
        let ((mx, ex), (my, ey)) = (split(x), split(y));
        if mx == 0 || my == 0 {
            continue;
        }
        let magnitude = u128::from(mx.unsigned_abs()) * u128::from(my.unsigned_abs());
        let sign_positive = (mx < 0) == (my < 0);
        let into = if sign_positive == plus {
            &mut positive
        } else {
            &mut negative
        };
        // Bit 0 of the fixed point is 2^-2148, the least power a product carries.
        let shift = usize::try_from(ex + ey + 2148).unwrap_or(0);
        add_shifted(into, magnitude, shift);
    }
    positive.iter().rev().cmp(negative.iter().rev())
}

/// A finite double as an integer of at most 53 bits and a power of two from 2^-1074 on.
fn split(x: f64) -> (i64, i32) {
    let bits = x.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = (bits & ((1 << 52) - 1)) as i64;
    let (mantissa, power) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    if x.is_sign_negative() {
        (-mantissa, power)
    } else {
        (mantissa, power)
    }
}

/// Adds `value`, shifted left by `shift` bits, to the fixed-point integer `words`.
fn add_shifted(words: &mut [u64; WORDS], value: u128, shift: usize) {
    let (word, bit) = (shift / 64, shift % 64);
    let low = (value as u64 as u128) << bit;
    let high = (value >> 64) << bit;
    for (offset, part) in [
        (0, low as u64),
        (1, (low >> 64) as u64),
        (1, high as u64),
        (2, (high >> 64) as u64),
    ] {
        let mut carry = part;
        let mut i = word + offset;
        while carry != 0 {
            let Some(w) = words.get_mut(i) else { return };
            let (sum, overflow) = w.overflowing_add(carry);
            *w = sum;
            carry = u64::from(overflow);
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    thread_local! {
        /// The orientations this thread has computed, the unit of the simplicity test's cost.
        pub(super) static ORIENTATIONS: Cell<u64> = const { Cell::new(0) };
    }

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

    fn ring(points: &[(f64, f64)]) -> String {
        let coords: Vec<String> = points.iter().map(|(x, y)| format!("{x} {y}")).collect();
        format!("({})", coords.join(","))
    }

    fn parts_of(text: &str) -> Vec<Vec<[f64; 2]>> {
        shape(text).1
    }

    const EXTERIOR: &str = "(0 0,10 0,10 10,0 10,0 0)";
    const SQUARE_HOLE: &str = "(1 1,2 1,2 2,1 2,1 1)";
    const BOW_TIE: &str = "(5 5,7 7,7 5,5 7,5 5)";

    #[test]
    fn an_inner_ring_that_crosses_itself_is_left_out() {
        // Verifies: ADR-0033
        let square = vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0], [1.0, 1.0]];
        let exterior = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        let parts = parts_of(&format!("POLYGON({EXTERIOR},{BOW_TIE},{SQUARE_HOLE})"));
        assert_eq!(parts, [exterior.clone(), square.clone()]);
        let parts = parts_of(&format!(
            "MULTIPOLYGON(({EXTERIOR},{SQUARE_HOLE},{BOW_TIE}),({EXTERIOR},{BOW_TIE}))"
        ));
        assert_eq!(parts, [exterior.clone(), square, exterior]);
        // A polygon of a collection is converted as one alone.
        let parts = parts_of(&format!(
            "GEOMETRYCOLLECTION(POLYGON({EXTERIOR},{BOW_TIE}))"
        ));
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn an_exterior_ring_that_crosses_itself_is_kept() {
        let parts = parts_of(&format!("POLYGON({BOW_TIE},{SQUARE_HOLE})"));
        assert_eq!(parts.len(), 2);
        let parts = parts_of(&format!("MULTIPOLYGON(({BOW_TIE}),({BOW_TIE}))"));
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn a_simple_inner_ring_is_kept() {
        for hole in [
            SQUARE_HOLE,
            // It touches the exterior at a vertex; only the ring itself is tested.
            "(0 0,2 1,1 2,0 0)",
            // A point repeated in a row is one vertex.
            "(1 1,2 1,2 1,2 2,1 2,1 1,1 1)",
            // A vertex in the middle of a straight run.
            "(1 1,2 1,3 1,3 2,1 1)",
            // Vertical edges, and a vertex at the height of another edge's end.
            "(1 1,1 3,2 2,3 3,3 1,1 1)",
        ] {
            assert_eq!(
                parts_of(&format!("POLYGON({EXTERIOR},{hole})")).len(),
                2,
                "{hole}"
            );
        }
    }

    #[test]
    fn an_inner_ring_that_meets_itself_anywhere_else_is_left_out() {
        for hole in [
            BOW_TIE,
            // It passes a vertex twice, a figure eight.
            "(1 1,3 3,5 1,5 5,3 3,1 5,1 1)",
            // A vertex lies on another edge.
            "(1 1,5 1,5 5,3 1,1 5,1 1)",
            // Two edges overlap along a line.
            "(1 1,5 1,5 3,4 1,2 1,2 3,1 1)",
            // Two edges in a row double back.
            "(1 1,4 1,2 1,2 3,1 1)",
            // It collapses to one edge walked there and back, or to a point.
            "(1 1,2 2,1 1,1 1)",
            "(1 1,1 1,1 1,1 1)",
            // Vertical edges that overlap.
            "(1 1,1 4,2 4,2 2,1 2,0 2,0 1,1 1)",
        ] {
            assert_eq!(
                parts_of(&format!("POLYGON({EXTERIOR},{hole})")).len(),
                1,
                "{hole}"
            );
        }
    }

    /// The same definition decided by testing every pair of edges, in integers.
    fn simple_by_pairs(ring: &[(i64, i64)]) -> bool {
        let mut v: Vec<(i64, i64)> = Vec::new();
        for &p in ring {
            if v.last() != Some(&p) {
                v.push(p);
            }
        }
        while v.len() > 1 && v.first() == v.last() {
            v.pop();
        }
        let k = v.len();
        if k < 3 {
            return false;
        }
        let orient = |a: (i64, i64), b: (i64, i64), c: (i64, i64)| {
            ((a.0 - c.0) * (b.1 - c.1) - (a.1 - c.1) * (b.0 - c.0)).signum()
        };
        let on = |a: (i64, i64), b: (i64, i64), p: (i64, i64)| {
            orient(a, b, p) == 0
                && a.0.min(b.0) <= p.0
                && p.0 <= a.0.max(b.0)
                && a.1.min(b.1) <= p.1
                && p.1 <= a.1.max(b.1)
        };
        for i in 0..k {
            for j in i + 1..k {
                let (a, b) = (v[i], v[(i + 1) % k]);
                let (c, d) = (v[j], v[(j + 1) % k]);
                if j == i + 1 {
                    // They share b == c: neither far end may lie on the other edge.
                    if on(a, b, d) || on(c, d, a) {
                        return false;
                    }
                } else if i == 0 && j == k - 1 {
                    // They share a == d.
                    if on(a, b, c) || on(c, d, b) {
                        return false;
                    }
                } else {
                    let (d1, d2) = (orient(a, b, c), orient(a, b, d));
                    let (d3, d4) = (orient(c, d, a), orient(c, d, b));
                    if d1 * d2 < 0 && d3 * d4 < 0
                        || on(a, b, c)
                        || on(a, b, d)
                        || on(c, d, a)
                        || on(c, d, b)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[test]
    fn the_sweep_agrees_with_every_pair_tested() {
        const TINY: f64 = 1.0 / (1u128 << 100) as f64 / (1u128 << 100) as f64;
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = |n: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % n
        };
        let mut simple = 0;
        for case in 0..200_000 {
            // Small grids force collinear edges, shared points, and vertical edges.
            let grid = 3 + next(if case % 4 == 0 { 12 } else { 4 });
            let len = 3 + next(if case % 4 == 0 { 40 } else { 8 }) as usize;
            let mut points: Vec<(i64, i64)> = (0..len)
                .map(|_| (next(grid) as i64, next(grid) as i64))
                .collect();
            points.push(points[0]);
            let floats: Vec<[f64; 2]> = points.iter().map(|&(x, y)| [x as f64, y as f64]).collect();
            let expected = simple_by_pairs(&points);
            assert_eq!(is_simple_ring(&floats), expected, "case {case}: {points:?}");
            // The same ring scaled by a power of two, turned a quarter, and walked backwards.
            let turned: Vec<[f64; 2]> = floats
                .iter()
                .rev()
                .map(|p| [-p[1] * TINY, p[0] * TINY])
                .collect();
            assert_eq!(
                is_simple_ring(&turned),
                expected,
                "case {case}: {points:?} turned"
            );
            simple += usize::from(expected);
        }
        assert!(simple > 10_000, "only {simple} simple rings were drawn");
    }

    #[test]
    fn orientation_is_exact_where_floating_point_rounds() {
        // Integers far apart: the products lose their low bits in floating point.
        let big = 2f64.powi(60);
        assert_eq!(
            orientation([big, big + 1024.0], [-big, -big], [0.0, 0.0]),
            Ordering::Greater
        );
        // Points on the line y = x, a part of an ulp off it.
        let x = 0.1f64;
        let next = f64::from_bits(x.to_bits() + 1);
        assert_eq!(orientation([0.0, 0.0], [1.0, 1.0], [x, x]), Ordering::Equal);
        assert_eq!(
            orientation([0.0, 0.0], [1.0, 1.0], [x, next]),
            Ordering::Greater
        );
        assert_eq!(
            orientation([0.0, 0.0], [1.0, 1.0], [next, x]),
            Ordering::Less
        );
        // Subnormal and huge coordinates, whose differences under- or overflow.
        let tiny = f64::from_bits(1);
        assert_eq!(
            orientation([0.0, 0.0], [tiny, 0.0], [0.0, tiny]),
            Ordering::Greater
        );
        assert_eq!(
            orientation([-f64::MAX, -f64::MAX], [f64::MAX, f64::MAX], [0.0, tiny]),
            Ordering::Greater
        );
        assert_eq!(
            orientation([-f64::MAX, -f64::MAX], [f64::MAX, f64::MAX], [1.0, 1.0]),
            Ordering::Equal
        );
        // The filter never disagrees with the exact sign, near the line or far from it.
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..100_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let t = (state >> 11) as f64 / (1u64 << 53) as f64;
            let scale = 2f64.powi((state % 64) as i32 * 30 - 960);
            let a = [0.3 * scale, 0.7 * scale];
            let b = [1.9 * scale, 3.1 * scale];
            let mut c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if state & 1 == 1 {
                c[1] = f64::from_bits(c[1].to_bits() + 1);
            }
            assert_eq!(
                orientation(a, b, c),
                exact_orientation(a, b, c),
                "{a:?} {b:?} {c:?}"
            );
        }
    }

    /// A zigzag of `teeth` edges across the full width, closed down the left side: every edge is
    /// cut by the sweep line at once, the most a ring can make it hold.
    fn zigzag(teeth: usize, crossing: bool) -> Vec<[f64; 2]> {
        let mut points: Vec<[f64; 2]> = (0..=teeth)
            .map(|i| [if i % 2 == 0 { 0.0 } else { 1000.0 }, i as f64])
            .collect();
        let top = teeth as f64;
        if crossing {
            // Its last edge comes back across the one before it.
            points.push([-1.0, top + 2.0]);
            points.push([500.0, top - 1.5]);
        }
        points.push([-1.0, top]);
        points.push([-1.0, 0.0]);
        points.push([0.0, 0.0]);
        points
    }

    #[test]
    fn a_hostile_ring_of_a_million_vertices_costs_n_log_n() {
        let n = 1_000_000;
        for crossing in [false, true] {
            let hole: Vec<(f64, f64)> = zigzag(n, crossing).iter().map(|p| (p[0], p[1])).collect();
            let text = format!("POLYGON({EXTERIOR},{})", ring(&hole));
            ORIENTATIONS.with(|c| c.set(0));
            let parts = parts_of(&text);
            assert_eq!(parts.len(), if crossing { 1 } else { 2 });
            let spent = ORIENTATIONS.with(Cell::get);
            // Testing every pair of edges would compute n² / 2 of them, 5 * 10^11; the sweep
            // computes a bounded number for each of its n log n steps.
            let bound = 64 * n as u64 * 20;
            assert!(spent < bound, "{spent} orientations, more than {bound}");
        }
    }
}
