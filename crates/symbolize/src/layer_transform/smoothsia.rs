// MapServer's smoothing of a shape follows msSmoothShapeSIA, its line window, and its
// preprocessing, src/mapsmoothing.c, of the release pinned in scripts/fetch-msautotest.sh.
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

//! `smoothsia`, the port of src/mapsmoothing.c: the line window, the passes of
//! [`smooth_sia`], and its preprocessing, [`preprocess`]. The transform tree of the parent module
//! calls the two, with its budget and its shape.

use forager_style_ir::{Coord, Preprocessing};

use super::{Budget, Shape, Undrawn, distance};

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
    /// has two points or more and its first and last are equal within `FP_EQ`. The window holds
    /// its indices in `points`, a buffer of an earlier window or an empty one.
    fn new(line: &'a [Coord], size: usize, mut points: Vec<usize>) -> Self {
        let size = size.max(1);
        points.clear();
        points.resize(size, 0);
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
            points,
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
/// the passes is charged to `budget` first, every pass MapServer runs, though the passes here go
/// over the parts of three points or more alone, the others kept as they are.
pub(super) fn smooth_sia(
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
    // `Buffers::smooth` keeps a part of fewer than three points as it is, so a pass that skips them
    // costs what it is charged, however many short parts the shape holds.
    let smoothed: Vec<usize> = (0..shape.parts.len())
        .filter(|&i| shape.parts[i].len() >= 3)
        .collect();
    if smoothed.is_empty() {
        return Ok(shape);
    }
    // The passes reuse one set of buffers, so that a pass after the first allocates nothing: under
    // the fuzz build's sanitizer, a pass's own allocations cost more than the window walk it is
    // charged for.
    let mut buffers = Buffers::default();
    for &i in &smoothed {
        for _ in 0..passes {
            buffers.smooth(&mut shape.parts[i], size);
        }
    }
    Ok(shape)
}

/// What one pass of [`smooth_sia`] writes besides the part, kept for the next pass.
#[derive(Default)]
struct Buffers {
    smoothed: Vec<Coord>,
    window: Vec<usize>,
    coeff: Vec<f64>,
}

impl Buffers {
    /// `part` smoothed by one pass of msSmoothShapeSIA (src/mapsmoothing.c:230-300).
    fn smooth(&mut self, part: &mut Vec<Coord>, size: usize) {
        let n = part.len();
        if n < 3 {
            return;
        }
        let mut ws = size.min(n - 1).max(1);
        if ws.is_multiple_of(2) {
            ws -= 1;
        }
        let mut window = Window::new(part, ws, std::mem::take(&mut self.window));
        let index = window.index;
        let coeff = &mut self.coeff;
        coeff.clear();
        coeff.resize(ws, 0.0);
        coeff[index] = 1.0;
        for k in 0..index {
            coeff[index + (k + 1)] = coeff[index - k] / 2.0;
            coeff[index - (k + 1)] = coeff[index + k] / 2.0;
        }
        let smoothed = &mut self.smoothed;
        smoothed.clear();
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
        self.window = window.points;
        std::mem::swap(part, smoothed);
    }
}

/// processShapePathDistance (src/mapsmoothing.c:114-181): each point of a window of five whose
/// path is longer than 1.3 times the distance between its ends, or every such point where
/// `force`, gets the midpoints towards its neighbours on either side; the last point of a ring
/// becomes the first of the new one. The points added are charged to `budget`.
fn preprocess(shape: &Shape, force: bool, budget: &mut Budget) -> Result<Shape, Undrawn> {
    const WINDOW: usize = 5;
    let mut parts = Vec::with_capacity(shape.parts.len());
    for part in &shape.parts {
        let mut window = Window::new(part, WINDOW, Vec::new());
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
