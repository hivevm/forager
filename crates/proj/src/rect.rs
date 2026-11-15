// The area of a rectangle reprojected follows msProjectRect, msProjectRectAsPolygon,
// msProjectRectGrid, msProjectShapeLine for a polygon, and msProjectSegment, src/mapproject.c, of
// MapServer at the release pinned in scripts/fetch-msautotest.sh.
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

//! The rectangle a source is asked for: a map's extent reprojected into a layer's projection as
//! `msProjectRect` reprojects it. Not ported here: the `+over` MapServer adds to both projections
//! and the wrapping of `msTestNeedWrap`, which change nothing for a rectangle that crosses no
//! antimeridian of its projections (the plan's ticket 05); `lon_wrap`, which no projection here
//! reads; and the memory of a projection's `is_polar`, which MapServer keeps across calls on the
//! same projection.

use forager_style_ir::{Coord, Extent};

use crate::Reprojection;

/// The points along each side of a rectangle, and along each line of its grid:
/// `NUMBER_OF_SAMPLE_POINTS`.
const SAMPLES: i32 = 100;

/// Where `msProjectSegment` stops halving a segment, in the units of its source.
const SEGMENT_TOLERANCE: f64 = 0.01;

/// Web Mercator's half width, in metres, as `msProjectRectAsPolygon` names it.
const WEB_MERCATOR_EDGE: f64 = 20_037_508.342_789_2;

impl Reprojection {
    /// `extent`, in the projection reprojected from, as the rectangle `msProjectRect` gives in the
    /// projection reprojected into: the points of its sides and of a diagonal reprojected, a
    /// side's points that do not reproject replaced by where it crosses the horizon, a grid of
    /// its points where none of them reprojects, and the longitudes all round where a pole of a
    /// polar projection lies inside it; `None` where no point of it reprojects.
    pub fn extent(&self, extent: &Extent) -> Option<Extent> {
        let pole = self.polar_into_longitude_and_latitude();
        let mut rect = self.as_polygon(extent)?;
        if let Some(sign) = pole
            && extent.min_x <= 0.0
            && extent.min_y <= 0.0
            && extent.max_x >= 0.0
            && extent.max_y >= 0.0
        {
            rect.min_x = -180.0;
            rect.max_x = 180.0;
            // As MapServer sets it, for a south pole too.
            rect.max_y = sign * 90.0;
        }
        Some(rect)
    }

    /// The sign of the pole at the origin of a projected `from`, as `msProjectRect` detects polar
    /// stereographic, where `to` is longitude and latitude: `Some(1.0)` for the north, `Some(-1.0)`
    /// for the south.
    fn polar_into_longitude_and_latitude(&self) -> Option<f64> {
        if self.from.is_geographic() || !self.to.is_geographic() {
            return None;
        }
        let p = self.point(Coord { x: 0.0, y: 0.0 }).ok()?;
        [1.0, -1.0]
            .into_iter()
            .find(|sign: &f64| (p.y - sign * 90.0).abs() < 1e-8)
    }

    /// `msProjectRectAsPolygon`.
    fn as_polygon(&self, rect: &Extent) -> Option<Extent> {
        if !self.to.is_geographic()
            && self.from.is_geographic()
            && (rect.min_x + 180.0).abs() < 1e-5
            && (rect.min_y + 90.0).abs() < 1e-5
            && (rect.max_x - 180.0).abs() < 1e-5
            && (rect.max_y - 90.0).abs() < 1e-5
        {
            // A point that does not reproject stays where it was, as MapServer ignores the
            // failure; and its test of the northing is that it differs at all.
            let test = Coord { x: -180.0, y: 85.0 };
            let test = self.point(test).unwrap_or(test);
            let edge =
                if (test.x + WEB_MERCATOR_EDGE).abs() < 1e-5 && test.y != 19_971_868.880_408_6 {
                    WEB_MERCATOR_EDGE
                } else {
                    1e15
                };
            return Some(Extent {
                min_x: -edge,
                min_y: -edge,
                max_x: edge,
                max_y: edge,
            });
        }
        let dx = (rect.max_x - rect.min_x) / f64::from(SAMPLES);
        let dy = (rect.max_y - rect.min_y) / f64::from(SAMPLES);
        if dx == 0.0 && dy == 0.0 {
            let corner = Coord {
                x: rect.min_x,
                y: rect.min_y,
            };
            let p = self.point(corner).unwrap_or(corner);
            return Some(Extent {
                min_x: p.x,
                min_y: p.y,
                max_x: p.x,
                max_y: p.y,
            });
        }
        let ring = ring(rect, dx, dy);
        let Some(line) = self.polygon_line(&ring) else {
            return self.grid(rect, dx, dy);
        };
        let mut bounds = bounds(&line)?;
        if self.to.is_geographic()
            && !self.from.is_geographic()
            && bounds.max_x - bounds.min_x > 360.0
        {
            bounds.min_x = -180.0;
            bounds.max_x = 180.0;
        }
        Some(bounds)
    }

    /// The first line of the ring reprojected as `msProjectShapeLine` reprojects a polygon's:
    /// a run of points that do not reproject is left out, ended and resumed where the ring
    /// crosses the horizon. `None` where that line is left empty, either because no point
    /// reproject or because a run of at most three such points, or one at the ring's start that
    /// a point reprojecting ends within two, makes MapServer move the line into a second one.
    fn polygon_line(&self, ring: &[Coord]) -> Option<Vec<Coord>> {
        let mut line: Vec<Coord> = Vec::with_capacity(ring.len() + 1);
        let mut valid = false;
        let mut last = Coord { x: 0.0, y: 0.0 };
        for (i, &this) in ring.iter().enumerate() {
            match self.point(this) {
                Err(_) => {
                    if i > 0
                        && valid
                        && let Some((_, horizon)) = self.segment(last, this)
                    {
                        line.push(horizon);
                    }
                    valid = false;
                }
                Ok(at) => {
                    if i > 0
                        && !valid
                        && let Some((_, horizon)) = self.segment(this, last)
                    {
                        // `line->numpoints >= i - 2`: the line moves into a second one, and the
                        // first is left empty.
                        if line.len() + 2 >= i {
                            return None;
                        }
                        line.push(horizon);
                    }
                    line.push(at);
                    valid = true;
                }
            }
            last = this;
        }
        (!line.is_empty()).then_some(line)
    }

    /// `msProjectSegment`: of `start` and `end`, one reprojecting and the other not, both
    /// reprojected, the one that does not replaced by where the segment crosses the horizon, found
    /// by halving it; `None` where neither reprojects or the halving finds no such point.
    fn segment(&self, start: Coord, end: Coord) -> Option<(Coord, Coord)> {
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

    /// `msProjectRectGrid`: the rectangle that holds the reprojected points of a grid of
    /// `SAMPLES` + 1 by `SAMPLES` + 1 over `rect`; `None` where none reprojects.
    fn grid(&self, rect: &Extent, dx: f64, dy: f64) -> Option<Extent> {
        let mut points = Vec::new();
        let corner = Coord {
            x: rect.min_x,
            y: rect.min_y,
        };
        points.extend(self.point(corner).ok());
        for ix in 0..=SAMPLES {
            let x = rect.min_x + f64::from(ix) * dx;
            for iy in 0..=SAMPLES {
                let y = rect.min_y + f64::from(iy) * dy;
                points.extend(self.point(Coord { x, y }).ok());
            }
        }
        bounds(&points)
    }
}

/// The ring `msProjectRectAsPolygon` builds around `rect`: along its bottom, its right, its top,
/// and its left side, the corners once, and along its diagonal from the upper right to the lower
/// left without them; a side of no length left out.
fn ring(rect: &Extent, dx: f64, dy: f64) -> Vec<Coord> {
    let at = |x: f64, y: f64| Coord { x, y };
    let step = |i: i32, d: f64| f64::from(i) * d;
    let mut ring = Vec::new();
    if dx != 0.0 {
        ring.extend((0..=SAMPLES).map(|ix| at(rect.min_x + step(ix, dx), rect.min_y)));
    }
    if dy != 0.0 {
        ring.extend((1..=SAMPLES).map(|iy| at(rect.max_x, rect.min_y + step(iy, dy))));
    }
    if dx != 0.0 {
        ring.extend(
            (0..SAMPLES)
                .rev()
                .map(|ix| at(rect.min_x + step(ix, dx), rect.max_y)),
        );
    }
    if dy != 0.0 {
        ring.extend(
            (0..SAMPLES)
                .rev()
                .map(|iy| at(rect.min_x, rect.min_y + step(iy, dy))),
        );
    }
    if dx != 0.0 && dy != 0.0 {
        ring.extend(
            (1..=SAMPLES - 2)
                .rev()
                .map(|i| at(rect.min_x + step(i, dx), rect.min_y + step(i, dy))),
        );
    }
    ring
}

/// The rectangle that holds `points`, as `MS_MIN` and `MS_MAX` grow it; `None` for no points.
fn bounds(points: &[Coord]) -> Option<Extent> {
    let (first, rest) = points.split_first()?;
    let mut b = Extent {
        min_x: first.x,
        min_y: first.y,
        max_x: first.x,
        max_y: first.y,
    };
    for p in rest {
        b.min_x = if b.min_x < p.x { b.min_x } else { p.x };
        b.max_x = if b.max_x > p.x { b.max_x } else { p.x };
        b.min_y = if b.min_y < p.y { b.min_y } else { p.y };
        b.max_y = if b.max_y > p.y { b.max_y } else { p.y };
    }
    Some(b)
}
