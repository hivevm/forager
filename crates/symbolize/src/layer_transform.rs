// A layer's GEOMTRANSFORM follows msGeomTransformShape, src/mapgeomtransform.c, and the functions
// its parser calls, src/mapparser.y: msGeneralize, src/maputil.c; msDensify, msRings2Shape,
// msGetOuterList, and msIsOuterRing, src/mapprimitive.cpp; msSmoothShapeSIA and its line window
// and preprocessing, src/mapsmoothing.c; and msPointInPolygon and msDistancePointToPoint,
// src/mapsearch.c; of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2008 Regents of the University of Minnesota (src/mapprimitive.cpp).
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

//! A layer's `GEOMTRANSFORM` (RFC 89): its transform tree (ADR-draft-typed-geomtransform)
//! replaces the shape of each feature that passes the layer's filter before it is classed and
//! drawn. The tree holds `generalize`, `densify`, `outer`, `inner`, and `smoothsia` of `[shape]`,
//! nested; the frontend has translated MapServer's text into it.
//!
//! A shape is MapServer's: a type, point, line, or polygon, and a list of parts, a polygon's
//! rings listed flat, as a source delivers them. What the transforms add or visit is bounded per
//! layer (Q-9, [`Budget`]).

use forager_style_ir::c::int_cast;
use forager_style_ir::{Coord, Geometry, NumberOp, Preprocessing, TransformNumber, TransformShape};

/// The most points one layer's transforms add to its shapes, `densify`'s and `smoothsia`'s
/// preprocessing (Q-9): past it, the layer draws nothing.
pub const MAX_ADDED_POINTS: usize = 1_000_000;

/// The most points one layer's transforms visit beyond reading each once, `smoothsia`'s windows
/// over its passes and the rings `outer` and `inner` test each ring against (Q-9): past it, the
/// layer draws nothing.
pub const MAX_TRANSFORM_VISITS: usize = 100_000_000;

/// Why a layer's `GEOMTRANSFORM` draws nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum Undrawn {
    /// MapServer fails the transform on the shape, for the reason given.
    Failed(String),
    /// The transforms pass a bound of Q-9, which the reason names.
    Limit(String),
}

/// What one layer's transforms have added and visited, held to [`MAX_ADDED_POINTS`] and
/// [`MAX_TRANSFORM_VISITS`]; a layer takes a new one.
#[derive(Debug, Default)]
pub struct Budget {
    added: usize,
    visited: usize,
}

impl Budget {
    fn add(&mut self, points: usize) -> Result<(), Undrawn> {
        self.added = self.added.saturating_add(points);
        if self.added > MAX_ADDED_POINTS {
            return Err(Undrawn::Limit(format!(
                "adds more than {MAX_ADDED_POINTS} points to the layer's shapes"
            )));
        }
        Ok(())
    }

    fn visit(&mut self, points: usize) -> Result<(), Undrawn> {
        self.visited = self.visited.saturating_add(points);
        if self.visited > MAX_TRANSFORM_VISITS {
            return Err(Undrawn::Limit(format!(
                "visits more than {MAX_TRANSFORM_VISITS} points of the layer's shapes"
            )));
        }
        Ok(())
    }
}

/// `geometry` replaced by the transform `shape`, as msGeomTransformShape evaluates it with
/// `[map_cellsize]` at `cellsize`, the map's cellsize in the layer's units; the reason where
/// MapServer's parser fails it, or where the engine cannot hold the geometry as MapServer's
/// shape: a collection, or a multipoint, whose parts it does not know.
pub fn transformed(
    shape: &TransformShape,
    geometry: &Geometry,
    cellsize: f64,
    budget: &mut Budget,
) -> Result<Geometry, Undrawn> {
    let read = Shape::of(geometry)?;
    let mut eval = Eval { cellsize, budget };
    Ok(eval.shape(shape, read)?.into_geometry())
}

/// MapServer's shape type.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Point,
    Line,
    Polygon,
}

/// MapServer's `shapeObj`: its type and its parts, a polygon's rings listed flat.
#[derive(Clone, Debug, PartialEq)]
struct Shape {
    kind: Kind,
    parts: Vec<Vec<Coord>>,
}

impl Shape {
    fn of(geometry: &Geometry) -> Result<Self, Undrawn> {
        let (kind, parts) = match geometry {
            Geometry::Point(c) => (Kind::Point, vec![vec![*c]]),
            Geometry::LineString(line) => (Kind::Line, vec![line.clone()]),
            Geometry::MultiLineString(lines) => (Kind::Line, lines.clone()),
            Geometry::Polygon(rings) => (Kind::Polygon, rings.clone()),
            Geometry::MultiPolygon(polygons) => (Kind::Polygon, polygons.concat()),
            Geometry::MultiPoint(_) => {
                return Err(Undrawn::Failed(
                    "is not drawn on a multipoint, whose parts the engine does not keep".to_owned(),
                ));
            }
            Geometry::Collection(_) => {
                return Err(Undrawn::Failed(
                    "is not drawn on a geometry collection, which is no shape of MapServer's"
                        .to_owned(),
                ));
            }
        };
        Ok(Self { kind, parts })
    }

    /// The geometry of the shape: a polygon of all its rings, a line string of a line of one
    /// part, a multi-line string of any other, a point of one point, a multipoint of more.
    fn into_geometry(self) -> Geometry {
        let mut parts = self.parts;
        match self.kind {
            Kind::Polygon => Geometry::Polygon(parts),
            Kind::Line if parts.len() == 1 => Geometry::LineString(parts.pop().unwrap_or_default()),
            Kind::Line => Geometry::MultiLineString(parts),
            Kind::Point => match parts.as_slice() {
                [part] if part.len() == 1 => Geometry::Point(part[0]),
                _ => Geometry::MultiPoint(parts.concat()),
            },
        }
    }
}

/// The evaluation of a transform tree on one shape.
struct Eval<'a> {
    cellsize: f64,
    budget: &'a mut Budget,
}

impl Eval<'_> {
    /// The rule `shape_exp` reduced: the inner shape first, then the function's numbers.
    fn shape(&mut self, transform: &TransformShape, shape: Shape) -> Result<Shape, Undrawn> {
        match transform {
            TransformShape::Shape => Ok(shape),
            TransformShape::Generalize(inner, tolerance) => {
                let shape = self.shape(inner, shape)?;
                Ok(generalize(&shape, self.number(tolerance)?))
            }
            TransformShape::Densify(inner, tolerance) => {
                let shape = self.shape(inner, shape)?;
                densify(&shape, self.number(tolerance)?, self.budget)
            }
            TransformShape::Outer(inner) => {
                let shape = self.shape(inner, shape)?;
                rings(&shape, true, self.budget)
            }
            TransformShape::Inner(inner) => {
                let shape = self.shape(inner, shape)?;
                rings(&shape, false, self.budget)
            }
            TransformShape::SmoothSia {
                shape: inner,
                size,
                iterations,
                preprocessing,
            } => {
                let shape = self.shape(inner, shape)?;
                // msSmoothShapeSIA takes its size and iterations as `int`s.
                let size = int_cast(self.number(size)?);
                let iterations = int_cast(self.number(iterations)?);
                smooth_sia(shape, size, iterations, *preprocessing, self.budget)
            }
        }
    }

    /// The rule `math_exp` reduced, in doubles.
    fn number(&mut self, number: &TransformNumber) -> Result<f64, Undrawn> {
        Ok(match number {
            TransformNumber::Number(n) => *n,
            TransformNumber::MapCellsize => self.cellsize,
            TransformNumber::Op(op, left, right) => {
                let (a, b) = (self.number(left)?, self.number(right)?);
                match op {
                    NumberOp::Add => a + b,
                    NumberOp::Sub => a - b,
                    NumberOp::Mul => a * b,
                    // `yyerror(p, "Division by zero.")`.
                    NumberOp::Div if b == 0.0 => {
                        return Err(Undrawn::Failed("divides by zero".to_owned()));
                    }
                    NumberOp::Div => a / b,
                    NumberOp::Mod => {
                        // `(int)$1 % (int)$3`: the x86-64 build traps on a divisor of 0 and on
                        // INT_MIN % -1, which the engine refuses instead.
                        let (a, b) = (int_cast(a), int_cast(b));
                        match a.checked_rem(b) {
                            Some(r) => f64::from(r),
                            None => {
                                return Err(Undrawn::Failed(format!(
                                    "takes {a} % {b}, which MapServer's build traps on"
                                )));
                            }
                        }
                    }
                    NumberOp::Pow => libm::pow(a, b),
                }
            }
        })
    }
}

/// msDistancePointToPoint: the square root of the sum of the squared differences.
fn distance(a: Coord, b: Coord) -> f64 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    (dx * dx + dy * dy).sqrt()
}

/// msGeneralize (src/maputil.c:2823-2871): the first part alone, of its points the first, each
/// at least `tolerance` from the last kept, and the last; the shape's other parts are dropped. A
/// shape without parts is kept, one whose first part has no point becomes that empty part.
fn generalize(shape: &Shape, tolerance: f64) -> Shape {
    let sq_tolerance = tolerance * tolerance;
    let Some(first) = shape.parts.first() else {
        return shape.clone();
    };
    let mut kept = Vec::new();
    if let Some(&start) = first.first() {
        kept.push(start);
        let mut last = start;
        for (i, &p) in first.iter().enumerate().skip(1) {
            let (dx, dy) = (p.x - last.x, p.y - last.y);
            if i == first.len() - 1 || dx * dx + dy * dy >= sq_tolerance {
                kept.push(p);
                last = p;
            }
        }
    }
    Shape {
        kind: shape.kind,
        parts: vec![kept],
    }
}

/// msDensify (src/mapprimitive.cpp:2627-2687): a line or a polygon with each segment longer than
/// `tolerance` cut into `floor(length / tolerance) + 1` equal ones. It fails a point shape and a
/// tolerance not above 0; the engine refuses a part without points, which C reads past, and
/// charges the points it adds to `budget` before it makes them.
fn densify(shape: &Shape, tolerance: f64, budget: &mut Budget) -> Result<Shape, Undrawn> {
    if shape.kind == Kind::Point {
        return Err(Undrawn::Failed(
            "calls densify on a point shape, which fails in MapServer".to_owned(),
        ));
    }
    if tolerance <= 0.0 {
        return Err(Undrawn::Failed(format!(
            "calls densify with the tolerance {tolerance}, not above 0, which fails in MapServer"
        )));
    }
    let mut parts = Vec::with_capacity(shape.parts.len());
    for part in &shape.parts {
        let Some(&last) = part.last() else {
            return Err(Undrawn::Failed(
                "calls densify on a part without points".to_owned(),
            ));
        };
        let mut line = Vec::with_capacity(part.len());
        for pair in part.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            line.push(a);
            let d = distance(a, b);
            if d > tolerance {
                let n = (d / tolerance).floor();
                // At most MAX_ADDED_POINTS, so the cast and `int`s below are exact.
                if n > MAX_ADDED_POINTS as f64 {
                    budget.add(usize::MAX)?;
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a whole number from 1 to MAX_ADDED_POINTS"
                )]
                let n = n as usize;
                budget.add(n)?;
                let length = d / (n as f64 + 1.0);
                line.reserve(n);
                for k in 0..n {
                    let c = (k as f64 + 1.0) * length / d;
                    line.push(Coord {
                        x: a.x + c * (b.x - a.x),
                        y: a.y + c * (b.y - a.y),
                    });
                }
            }
        }
        line.push(last);
        parts.push(line);
    }
    Ok(Shape {
        kind: shape.kind,
        parts,
    })
}

/// msRings2Shape (src/mapprimitive.cpp:2588-2625): the rings of a polygon that msGetOuterList
/// finds outer where `outer`, inner otherwise, in their order. It fails any other shape. The
/// tests of each ring against every other are charged to `budget` first.
fn rings(shape: &Shape, outer: bool, budget: &mut Budget) -> Result<Shape, Undrawn> {
    let name = if outer { "outer" } else { "inner" };
    if shape.kind != Kind::Polygon {
        return Err(Undrawn::Failed(format!(
            "calls {name} on a shape that is no polygon, which fails in MapServer"
        )));
    }
    let points: usize = shape.parts.iter().map(Vec::len).sum();
    budget.visit(points.saturating_mul(shape.parts.len()))?;
    let mut kept = Vec::new();
    for r in 0..shape.parts.len() {
        if is_outer_ring(&shape.parts, r)? == outer {
            kept.push(shape.parts[r].clone());
        }
    }
    Ok(Shape {
        kind: Kind::Polygon,
        parts: kept,
    })
}

/// msIsOuterRing (src/mapprimitive.cpp:289-326): the ring alone is outer; otherwise it is outer
/// where its first point lies inside an even number of the other rings, a ring on whose edge
/// the first or the second point lies tested by the third instead. C reads past a ring of too
/// few points for these tests, which the engine refuses.
fn is_outer_ring(rings: &[Vec<Coord>], r: usize) -> Result<bool, Undrawn> {
    if rings.len() == 1 {
        return Ok(true);
    }
    let ring = &rings[r];
    let point = |i: usize| {
        ring.get(i).copied().ok_or_else(|| {
            Undrawn::Failed(format!(
                "tests a ring of {} points, too few to tell whether it is outer",
                ring.len()
            ))
        })
    };
    let mut status = true;
    for (i, other) in rings.iter().enumerate() {
        if i == r {
            continue;
        }
        let first = point_in_polygon(point(0)?, other);
        let second = point_in_polygon(point(1)?, other);
        if first == second {
            if first {
                status = !status;
            }
        } else if point_in_polygon(point(2)?, other) {
            status = !status;
        }
    }
    Ok(status)
}

/// `msPointInPolygon` (src/mapsearch.c:173-185), W. Randolph Franklin's pnpoly, under his notice,
/// which `src/mapsearch.c` keeps beside it and THIRD-PARTY-NOTICES.md reproduces for the binaries:
///
/// Copyright (c) 1970-2003, Wm. Randolph Franklin
///
/// Permission is hereby granted, free of charge, to any person obtaining a copy of this software
/// and associated documentation files (the "Software"), to deal in the Software without
/// restriction, including without limitation the rights to use, copy, modify, merge, publish,
/// distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
/// Software is furnished to do so, subject to the following conditions:
///
/// 1. Redistributions of source code must retain the above copyright notice, this list of
///    conditions and the following disclaimers.
/// 2. Redistributions in binary form must reproduce the above copyright notice in the
///    documentation and/or other materials provided with the distribution.
/// 3. The name of W. Randolph Franklin may not be used to endorse or promote products derived from
///    this Software without specific prior written permission.
///
/// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
/// BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
/// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
/// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
/// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
fn point_in_polygon(p: Coord, c: &[Coord]) -> bool {
    let mut status = false;
    let Some(mut j) = c.len().checked_sub(1) else {
        return false;
    };
    for i in 0..c.len() {
        let (ci, cj) = (c[i], c[j]);
        if ((ci.y <= p.y && p.y < cj.y) || (cj.y <= p.y && p.y < ci.y))
            && (p.x < (cj.x - ci.x) * (p.y - ci.y) / (cj.y - ci.y) + ci.x)
        {
            status = !status;
        }
        j = i;
    }
    status
}

/// MapServer's `FP_EQ` of src/mapsmoothing.c: within 1e-12.
fn fp_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

/// What [`Window::next`] gives: MapServer's `MS_DONE`, `MS_FALSE`, and `MS_TRUE`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Done,
    Invalid,
    Valid,
}

/// The `lineWindow` of src/mapsmoothing.c: the indices of the `size` points about the current
/// one, wrapped around a closed ring.
struct Window<'a> {
    line: &'a [Coord],
    pos: usize,
    started: bool,
    index: usize,
    ring: bool,
    points: Vec<usize>,
}

impl<'a> Window<'a> {
    /// initLineWindow (src/mapsmoothing.c:46-65), `size` at least 1: the line is a ring where it
    /// has two points or more and its first and last are equal within `FP_EQ`.
    fn new(line: &'a [Coord], size: usize) -> Self {
        let size = size.max(1);
        let ring = line.len() >= 2 && {
            let (first, last) = (line[0], line[line.len() - 1]);
            fp_eq(first.x, last.x) && fp_eq(first.y, last.y)
        };
        Self {
            line,
            pos: 0,
            started: false,
            index: size / 2,
            ring,
            points: vec![0; size],
        }
    }

    /// nextLineWindow (src/mapsmoothing.c:69-97): the next point; the window is invalid where it
    /// runs off a line that is no ring, or off a ring too short for it.
    fn next(&mut self) -> Step {
        if self.started {
            self.pos += 1;
        }
        self.started = true;
        let n = self.line.len();
        if self.pos >= n {
            return Step::Done;
        }
        self.points[self.index] = self.pos;
        for i in 0..self.index {
            let mut r = self.pos.checked_sub(i + 1);
            let mut l = Some(self.pos + i + 1).filter(|&l| l < n);
            if self.ring {
                // `r = numpoints - (i + 2)`, `l = 1 + (l - numpoints)`.
                r = r.or_else(|| n.checked_sub(i + 2));
                l = l.or_else(|| Some(1 + (self.pos + i + 1 - n)).filter(|&l| l < n));
            }
            let (Some(r), Some(l)) = (r, l) else {
                return Step::Invalid;
            };
            self.points[self.index - (i + 1)] = r;
            self.points[self.index + i + 1] = l;
        }
        Step::Valid
    }

    fn point(&self, k: usize) -> Coord {
        self.line[self.points[k]]
    }
}

/// msSmoothShapeSIA (src/mapsmoothing.c:183-312): each part of three points or more smoothed by
/// a window of `size` points, odd and at most one below the part's count, weighed by halves away
/// from its centre; a point whose window runs off the part kept. The passes are as many as
/// `iterations`, less one past one: the C smooths the shape it was given on both the first and
/// the second pass. The preprocessing `all` or `angle` runs first ([`preprocess`]). The work of
/// the passes is charged to `budget` first.
fn smooth_sia(
    mut shape: Shape,
    size: i32,
    iterations: i32,
    preprocessing: Option<Preprocessing>,
    budget: &mut Budget,
) -> Result<Shape, Undrawn> {
    let size = usize::try_from(size.max(3)).unwrap_or(3);
    let iterations = iterations.max(1);
    match preprocessing {
        Some(Preprocessing::All) => shape = preprocess(&shape, true, budget)?,
        Some(Preprocessing::Angle) => shape = preprocess(&shape, false, budget)?,
        None => {}
    }
    let passes = usize::try_from(if iterations > 1 { iterations - 1 } else { 1 }).unwrap_or(1);
    let points: usize = shape.parts.iter().map(Vec::len).sum();
    let per_pass = points.saturating_mul(size.min(points)).max(1);
    budget.visit(per_pass.saturating_mul(passes))?;
    for _ in 0..passes {
        shape.parts = shape
            .parts
            .iter()
            .map(|part| smooth_part(part, size))
            .collect();
    }
    Ok(shape)
}

/// One part smoothed by one pass of msSmoothShapeSIA (src/mapsmoothing.c:230-300).
fn smooth_part(part: &[Coord], size: usize) -> Vec<Coord> {
    let n = part.len();
    if n < 3 {
        return part.to_vec();
    }
    let mut ws = size.min(n - 1).max(1);
    if ws.is_multiple_of(2) {
        ws -= 1;
    }
    let mut window = Window::new(part, ws);
    let index = window.index;
    let mut coeff = vec![0.0; ws];
    coeff[index] = 1.0;
    for k in 0..index {
        coeff[index + (k + 1)] = coeff[index - k] / 2.0;
        coeff[index - (k + 1)] = coeff[index + k] / 2.0;
    }
    let mut smoothed = Vec::with_capacity(n);
    loop {
        match window.next() {
            Step::Done => break,
            Step::Invalid => smoothed.push(window.point(index)),
            Step::Valid => {
                let (mut sum, mut sum_x, mut sum_y) = (0.0, 0.0, 0.0);
                for (k, &c) in coeff.iter().enumerate() {
                    let p = window.point(k);
                    let (x, y) = if k == index {
                        (p.x, p.y)
                    } else {
                        (p.x * c, p.y * c)
                    };
                    sum += c;
                    sum_x += x;
                    sum_y += y;
                }
                smoothed.push(Coord {
                    x: sum_x / sum,
                    y: sum_y / sum,
                });
            }
        }
    }
    smoothed
}

/// processShapePathDistance (src/mapsmoothing.c:114-181): each point of a window of five whose
/// path is longer than 1.3 times the distance between its ends, or every such point where
/// `force`, gets the midpoints towards its neighbours on either side; the last point of a ring
/// becomes the first of the new one. The points added are charged to `budget`.
fn preprocess(shape: &Shape, force: bool, budget: &mut Budget) -> Result<Shape, Undrawn> {
    const WINDOW: usize = 5;
    let mut parts = Vec::with_capacity(shape.parts.len());
    for part in &shape.parts {
        let mut window = Window::new(part, WINDOW);
        let index = window.index;
        let mut line: Vec<Coord> = Vec::with_capacity(part.len());
        let mid = |a: Coord, b: Coord| Coord {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        };
        loop {
            let step = window.next();
            if step == Step::Done {
                break;
            }
            if window.ring && window.pos == part.len() - 1 {
                line.push(line[0]);
                continue;
            }
            if step == Step::Invalid {
                line.push(window.point(index));
                continue;
            }
            let looped = force || {
                let path: f64 = (1..WINDOW)
                    .map(|i| distance(window.point(i - 1), window.point(i)))
                    .sum();
                path / distance(window.point(0), window.point(WINDOW - 1)) > 1.3
            };
            let p = part[window.pos];
            if looped {
                line.push(mid(p, window.point(index - 1)));
            }
            line.push(p);
            if looped {
                line.push(mid(p, window.point(index + 1)));
            }
        }
        budget.add(line.len() - part.len())?;
        parts.push(line);
    }
    Ok(Shape {
        kind: shape.kind,
        parts,
    })
}

#[cfg(test)]
mod tests;
