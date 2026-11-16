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
//! `msProjectRect` reprojects it, with the `+over` it adds to both projections and its ring
//! reprojected by `msProjectShapeLine` (`shape.rs`). Not ported here: `lon_wrap`, which no
//! projection here reads, and the memory of a projection's `is_polar`. MapServer sets it on the map's
//! projection, which every layer of a render shares, at the first rectangle it projects, so only
//! the first layer of a polar map gets all longitudes and the later ones take the `+over` path;
//! here every layer gets them, as the first does.

use forager_style_ir::{Coord, Extent};

use crate::Reprojection;

/// The points along each side of a rectangle, and along each line of its grid:
/// `NUMBER_OF_SAMPLE_POINTS`.
const SAMPLES: i32 = 100;

/// Web Mercator's half width, in metres, as `msProjectRectAsPolygon` names it.
const WEB_MERCATOR_EDGE: f64 = 20_037_508.342_789_2;

impl Reprojection {
    /// `extent`, in the projection reprojected from, as the rectangle `msProjectRect` gives in the
    /// projection reprojected into: the points of its sides and of a diagonal reprojected, a
    /// side's points that do not reproject replaced by where it crosses the horizon, a grid of
    /// its points where none of them reprojects, and the longitudes all round where a pole of a
    /// polar projection lies inside it; `None` where no point of it reprojects.
    pub fn extent(&self, extent: &Extent) -> Option<Extent> {
        if let Some(sign) = self.polar_into_longitude_and_latitude() {
            if extent.min_x <= 0.0
                && extent.min_y <= 0.0
                && extent.max_x >= 0.0
                && extent.max_y >= 0.0
            {
                let mut rect = self.as_polygon(extent)?;
                rect.min_x = -180.0;
                rect.max_x = 180.0;
                // As MapServer sets it, for a south pole too.
                rect.max_y = sign * 90.0;
                return Some(rect);
            }
            // Where the dateline is surely not enclosed, without `+over`.
            if extent.max_y < 0.0 || extent.max_x < 0.0 || extent.min_x > 0.0 {
                return self.as_polygon(extent);
            }
        } else if self.polar_into_projected()
            && (extent.min_x <= 0.0
                && extent.min_y <= 0.0
                && extent.max_x >= 0.0
                && extent.max_y >= 0.0
                || extent.max_y < 0.0
                || extent.max_x < 0.0
                || extent.min_x > 0.0)
        {
            // About the pole, or where the dateline is surely not enclosed: without `+over`.
            return self.as_polygon(extent);
        }
        self.over().as_polygon(extent)
    }

    /// Whether a projected `from`, into a projected `to`, has the north pole at its origin, as
    /// `msProjectRect` detects polar stereographic through `from`'s own longitude and latitude; it
    /// tests the north pole for either sign.
    fn polar_into_projected(&self) -> bool {
        if self.rotated() || self.from_geographic || self.to_geographic || self.from.is_geographic()
        {
            return false;
        }
        let to_long_lat = Reprojection::new(self.from.clone(), self.from.base_geographic());
        to_long_lat.pole_at_origin() == Some(1.0)
    }

    /// The way with `+over` on each side but a map's `AUTO`, as `msProjectRect` copies a projection
    /// whose first argument is `init=` or `proj=`, as every proj4 string the frontend writes is.
    fn over(&self) -> Self {
        let side = |p: &crate::Projection, geographic: bool| {
            // A map's `AUTO`: longitude and latitude that MapServer's tests do not take for any.
            if p.is_geographic() && !geographic {
                p.clone()
            } else {
                p.with_over()
            }
        };
        Self {
            from: side(&self.from, self.from_geographic),
            to: side(&self.to, self.to_geographic),
            ..self.clone()
        }
    }

    /// The sign of the pole at the origin of a projected `from`, as `msProjectRect` detects polar
    /// stereographic, where `to` is longitude and latitude: `Some(1.0)` for the north, `Some(-1.0)`
    /// for the south. Neither test runs where a side holds a geotransform.
    fn polar_into_longitude_and_latitude(&self) -> Option<f64> {
        if self.rotated() || self.from_geographic || !self.to_geographic {
            return None;
        }
        self.pole_at_origin()
    }

    /// `msProjectRectAsPolygon`.
    fn as_polygon(&self, rect: &Extent) -> Option<Extent> {
        if !self.to_geographic
            && self.from_geographic
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
        let line = self.shape_line(&ring, true).first;
        if line.is_empty() {
            return self.grid(rect, dx, dy);
        }
        let mut bounds = bounds(&line)?;
        if self.to_geographic
            && !self.from_geographic
            && bounds.max_x - bounds.min_x > 360.0
            && self.to_gt.is_none()
        {
            bounds.min_x = -180.0;
            bounds.max_x = 180.0;
        }
        Some(bounds)
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
