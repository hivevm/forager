// The reprojection of a shape's lines follows msProjectShapeEx, msProjectShapeLine, msProjectLineEx,
// msProjectSegment, and msTestNeedWrap, src/mapproject.c, of MapServer at the release pinned in
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

//! The lines of a shape reprojected as MapServer reprojects them before it draws them: a line or a
//! ring by `msProjectShapeLine`, its run of points that do not reproject cut at the horizon and
//! its longitudes wrapped across the dateline into longitude and latitude, and the points of a
//! point shape by `msProjectLineEx`. Not ported: the cutting of lines by GEOS
//! (`msProjectGetLineCuttingCase`), which a polar or rotated-pole source into longitude and
//! latitude, or a source of `lon_wrap=0`, takes ([`Reprojection::cuts_lines_with_geos`] tells
//! the first, whose lines the engine draws uncut with a warning); and `USE_PROJ_FASTPATHS`, which
//! the pinned build does not define.

use forager_style_ir::Coord;

use crate::Reprojection;

/// Where `msProjectSegment` stops halving a segment, in the units of its source.
const SEGMENT_TOLERANCE: f64 = 0.01;

/// A line or a ring reprojected: the lines MapServer leaves in its place and after it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShapeLines {
    /// What is left in the line's own place, empty where none of it reprojects or where its points
    /// moved into a line of their own after it.
    pub first: Vec<Coord>,
    /// The lines added after the shape's others, in order: the parts of a line cut at the horizon,
    /// or a ring's points moved there.
    pub added: Vec<Vec<Coord>>,
    /// The points of the line that did not reproject.
    pub failed: usize,
    /// The points the horizon search added (Q-9).
    pub horizon_points: usize,
}

/// The lines of a shape reprojected, as `msProjectShapeEx` leaves them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    /// Each line in its place, those its lines added after them all, a line left without points
    /// kept empty.
    pub lines: Vec<Vec<Coord>>,
    /// The points that did not reproject.
    pub failed: usize,
    /// The points the horizon search added (Q-9).
    pub horizon_points: usize,
}

impl Reprojection {
    /// `msProjectShapeEx` of the `lines` of a line shape, or of a polygon's where `polygon`: each
    /// reprojected by [`Self::shape_line`], the last first, in its place, and the lines each adds
    /// after them all.
    pub fn shape(&self, lines: &[Vec<Coord>], polygon: bool) -> Shape {
        let mut shape = Shape {
            lines: Vec::with_capacity(lines.len()),
            ..Shape::default()
        };
        let mut added = Vec::new();
        for line in lines.iter().rev() {
            let reprojected = self.shape_line(line, polygon);
            shape.failed += reprojected.failed;
            shape.horizon_points += reprojected.horizon_points;
            shape.lines.push(reprojected.first);
            added.extend(reprojected.added);
        }
        shape.lines.reverse();
        shape.lines.extend(added);
        shape
    }

    /// `msProjectShapeLine` of a line, or of a ring where `polygon`, into the projection reprojected
    /// into.
    pub(crate) fn shape_line(&self, points: &[Coord], polygon: bool) -> ShapeLines {
        let mut out = ShapeLines::default();
        // Which line is written to: false for the line's own place, true for the last added.
        let mut moved = false;
        let wrap_test = self.to_geographic && !self.from_geographic;
        // 1 valid, -1 not, 0 unknown, as `valid_flag`.
        let mut valid = 0_i8;
        let mut last = Coord { x: 0.0, y: 0.0 };
        for (i, &this) in points.iter().enumerate() {
            let projected = self.point(this);
            if projected.is_err() {
                out.failed += 1;
            }
            let mut wrk = projected.unwrap_or(this);
            let line_out = |out: &ShapeLines, moved: bool| -> usize {
                if moved {
                    out.added.last().map_or(0, Vec::len)
                } else {
                    out.first.len()
                }
            };
            if wrap_test && i > 0 && projected.is_ok() {
                let written = if moved {
                    out.added.last()
                } else {
                    Some(&out.first)
                };
                let pt1_geo = written.and_then(|l| l.last().copied()).unwrap_or(wrk);
                let dist = wrk.x - pt1_geo.x;
                if dist.abs() > 180.0 && self.need_wrap(this, last, pt1_geo) {
                    wrk.x = wrapped(wrk.x, dist);
                }
            }
            let push = |out: &mut ShapeLines, moved: bool, p: Coord| {
                if moved {
                    if let Some(line) = out.added.last_mut() {
                        line.push(p);
                    }
                } else {
                    out.first.push(p);
                }
            };
            if projected.is_err() {
                if i == 0 {
                    valid = -1;
                } else if valid == 1 {
                    if let Some((_, horizon)) = self.segment(last, this) {
                        push(&mut out, moved, horizon);
                        out.horizon_points += 1;
                    }
                    valid = -1;
                }
            } else if i == 0 || valid == 1 {
                push(&mut out, moved, wrk);
                valid = 1;
            } else {
                if let Some((_, horizon)) = self.segment(this, last) {
                    let written = line_out(&out, moved);
                    if written > 0 && !polygon {
                        // A new part, started without points.
                        out.added.push(Vec::new());
                        moved = true;
                    } else if !moved && written + 2 >= i {
                        // `line->numpoints >= i - 2`: the points so far move into a line of their
                        // own after the shape's others, and the line's place is left empty.
                        out.added.push(std::mem::take(&mut out.first));
                        moved = true;
                    }
                    push(&mut out, moved, horizon);
                    out.horizon_points += 1;
                }
                push(&mut out, moved, wrk);
                valid = 1;
            }
            last = this;
        }
        // A ring is closed, even where the trip over the horizon left it open.
        if polygon {
            let line = if moved {
                out.added.last_mut()
            } else {
                Some(&mut out.first)
            };
            if let Some(line) = line
                && line.len() > 2
                && line.first() != line.last()
            {
                let first = line[0];
                line.push(first);
            }
        }
        out
    }

    /// `msProjectLineEx` of the points of a point shape, in place: where the projection
    /// reprojected into is longitude and latitude and the one reprojected from is not, a point
    /// that does not reproject stays where it was and one that lands more than 180° from the first
    /// is wrapped as `msTestNeedWrap` finds; otherwise a point that does not reproject fails the
    /// line, `Err` with how many did. `Ok` holds how many did not reproject.
    pub fn point_line(&self, points: &mut [Coord]) -> Result<usize, usize> {
        let careful = self.to_geographic && !self.from_geographic;
        let mut failed = 0;
        if careful {
            let Some(&start) = points.first() else {
                return Ok(0);
            };
            for i in 0..points.len() {
                let this = points[i];
                match self.point(this) {
                    Ok(p) => points[i] = p,
                    Err(_) => failed += 1,
                }
                if i > 0 {
                    let dist = points[i].x - points[0].x;
                    if dist.abs() > 180.0 && self.need_wrap(this, start, points[0]) {
                        points[i].x = wrapped(points[i].x, dist);
                    }
                }
            }
            return Ok(failed);
        }
        for point in points.iter_mut() {
            match self.point(*point) {
                Ok(p) => *point = p,
                Err(_) => failed += 1,
            }
        }
        if failed > 0 { Err(failed) } else { Ok(0) }
    }

    /// Whether MapServer cuts the lines of a line shape with GEOS (`msProjectGetLineCuttingCase`):
    /// from a projected source whose origin is the north pole into longitude and latitude, as
    /// `ortho` or `tmerc` of `+lat_0=90` are. Of its other cases, a rotated pole and a source of
    /// `lon_wrap=0` no projection here takes. The third, such a source into a map whose projection
    /// is written `init=epsg:3857`, is reachable but not seen: MapServer tests the map's first
    /// argument as written, and the frontend has expanded `init=` before `proj` reads it, so such
    /// a layer is neither cut nor warned of (docs/semantics/projection.md).
    pub fn cuts_lines_with_geos(&self) -> bool {
        !self.from_geographic && self.to_geographic && self.pole_at_origin() == Some(1.0)
    }

    /// The pole the origin of the projection reprojected from lands on, as `msProjectRect` and
    /// `msProjectGetLineCuttingCase` find it: `Some(1.0)` for the north, `Some(-1.0)` for the
    /// south, each within 1e-8 of a degree.
    pub(crate) fn pole_at_origin(&self) -> Option<f64> {
        let p = self.point(Coord { x: 0.0, y: 0.0 }).ok()?;
        [1.0, -1.0]
            .into_iter()
            .find(|sign: &f64| (p.y - sign * 90.0).abs() < 1e-8)
    }

    /// `msTestNeedWrap`: whether the segment from `pt2` to `pt1`, in the projection reprojected
    /// from, whose start was written at `pt2_geo`, must be wrapped by 360°.
    fn need_wrap(&self, pt1: Coord, pt2: Coord, pt2_geo: Coord) -> bool {
        let middle = Coord {
            x: (pt1.x + pt2.x) * 0.5,
            y: (pt1.y + pt2.y) * 0.5,
        };
        let (Ok(pt1), Ok(pt2), Ok(middle)) = (self.point(pt1), self.point(pt2), self.point(middle))
        else {
            return false;
        };
        // If the last point was moved, a move is due.
        if (pt2_geo.x - pt2.x).abs() > 180.0 {
            return true;
        }
        // Otherwise, wrapped where the middle does not fall between the ends.
        (middle.x < pt1.x && middle.x < pt2_geo.x) || (middle.x > pt1.x && middle.x > pt2_geo.x)
    }

    /// `msProjectSegment`: of `start` and `end`, one reprojecting and the other not, both
    /// reprojected, the one that does not replaced by where the segment crosses the horizon, found
    /// by halving it; `None` where neither reprojects or the halving finds no such point.
    pub(crate) fn segment(&self, start: Coord, end: Coord) -> Option<(Coord, Coord)> {
        if self.point(start).is_err() {
            self.point(end).ok()?;
            let (end, start) = self.segment(end, start)?;
            return Some((start, end));
        }
        let (mut sub_start, mut sub_end) = (start, end);
        while (sub_start.x - sub_end.x).abs() + (sub_start.y - sub_end.y).abs() > SEGMENT_TOLERANCE
        {
            let mid = Coord {
                x: (sub_start.x + sub_end.x) * 0.5,
                y: (sub_start.y + sub_end.y) * 0.5,
            };
            if self.point(mid).is_err() {
                if mid == sub_end {
                    return None;
                }
                sub_end = mid;
            } else {
                if mid == sub_start {
                    return None;
                }
                sub_start = mid;
            }
        }
        let end = self.point(sub_start).ok()?;
        let start = self.point(start).ok()?;
        Some((start, end))
    }
}

/// `x` moved by 360° against the direction of `dist`, as `msProjectShapeLine` and
/// `msProjectLineEx` wrap a longitude.
fn wrapped(x: f64, dist: f64) -> f64 {
    if dist > 0.0 {
        x - 360.0
    } else if dist < 0.0 {
        x + 360.0
    } else {
        x
    }
}
