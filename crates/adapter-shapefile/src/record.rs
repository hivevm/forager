// A record of the `.shp` follows `msSHPReadShape` and `ReadRect` of MapServer's src/mapshape.c of
// the release pinned in scripts/fetch-msautotest.sh, and the boxes `msRectOverlap` and
// `msRectContained` of its src/mapsearch.c.
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

//! One record of the `.shp`: checked as a view into its bytes, built into a geometry only once
//! it reads as a shape.

use forager_ports::{Coord, Geometry};

/// The most points and parts a record may state.
const MAX_POINTS: i32 = 50_000_000;
const MAX_PARTS: i32 = 10_000_000;

/// A box, compared as MapServer compares its `rectObj`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Rect {
    pub const EMPTY: Rect = Rect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    };

    /// Four little-endian doubles at `at`: the least x and y, then the greatest.
    pub fn read(bytes: &[u8], at: usize) -> Option<Rect> {
        let f = |i: usize| le_f64(bytes, at.checked_add(i * 8)?);
        Some(Rect {
            min_x: f(0)?,
            min_y: f(1)?,
            max_x: f(2)?,
            max_y: f(3)?,
        })
    }

    /// `msRectOverlap`: only a comparison that holds separates two boxes, so a NaN overlaps.
    pub fn overlaps(&self, other: &Rect) -> bool {
        if self.min_x > other.max_x || self.max_x < other.min_x {
            return false;
        }
        if self.min_y > other.max_y || self.max_y < other.min_y {
            return false;
        }
        true
    }

    /// `msRectContained`: whether this box lies inside `other`; a NaN never does.
    pub fn contained_in(&self, other: &Rect) -> bool {
        self.min_x >= other.min_x
            && self.max_x <= other.max_x
            && self.min_y >= other.min_y
            && self.max_y <= other.max_y
    }
}

/// A record that reads as a shape, still in the bytes of the `.shp`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Shape<'a> {
    Point(Coord),
    MultiPoint(&'a [[u8; 16]]),
    /// An arc or a polygon: its parts, checked against its points.
    Lines {
        polygon: bool,
        parts: &'a [[u8; 4]],
        points: &'a [[u8; 16]],
    },
}

/// Whether `msSHPReadBounds` reads a record of this shape type as a point.
pub(crate) fn is_point(shape_type: u8) -> bool {
    matches!(shape_type, 1 | 11 | 21)
}

/// The record `bytes`, its 8-byte header included, read as a shape of the file's `shape_type`,
/// or `None` where `msSHPReadShape` reads it as no shape. The record's own shape type is not read.
pub(crate) fn parse(shape_type: u8, bytes: &[u8]) -> Option<Shape<'_>> {
    match shape_type {
        1 | 11 | 21 => {
            if bytes.len() < 28 {
                return None;
            }
            coord(bytes, 12).map(Shape::Point)
        }
        8 | 18 | 28 => multipoint(bytes, shape_type != 8),
        3 | 13 | 23 => lines(bytes, false),
        5 | 15 | 25 => lines(bytes, true),
        _ => None,
    }
}

fn multipoint(bytes: &[u8], measured: bool) -> Option<Shape<'_>> {
    if bytes.len() < 48 {
        return None;
    }
    let count = le_i32(bytes, 44)?;
    if !(0..=MAX_POINTS).contains(&count) {
        return None;
    }
    let count = u64::from(count.unsigned_abs());
    // With a measure or a height, their range and one value per point follow the points.
    let mut required = 48 + 16 * count;
    if measured {
        required += 16 + 8 * count;
    }
    if required > bytes.len() as u64 {
        return None;
    }
    let points = bytes.get(48..48 + 16 * usize::try_from(count).ok()?)?;
    Some(Shape::MultiPoint(points.as_chunks().0))
}

fn lines(bytes: &[u8], polygon: bool) -> Option<Shape<'_>> {
    if bytes.len() < 52 {
        return None;
    }
    let parts = le_i32(bytes, 44)?;
    let points = le_i32(bytes, 48)?;
    if !(0..=MAX_POINTS).contains(&points) || !(0..=MAX_PARTS).contains(&parts) {
        return None;
    }
    let parts = usize::try_from(parts).ok()?;
    let points = usize::try_from(points).ok()?;
    // 52 + 4 × parts + 16 × points: at most 840,000,052, within reach of every usize.
    let parts_end = 52 + 4 * parts;
    let points_end = parts_end + 16 * points;
    if points_end > bytes.len() {
        return None;
    }
    let shape = Shape::Lines {
        polygon,
        parts: bytes.get(52..parts_end)?.as_chunks().0,
        points: bytes.get(parts_end..points_end)?.as_chunks().0,
    };
    let whole = shape.ranges().all(|r| r.is_some());
    whole.then_some(shape)
}

impl Shape<'_> {
    /// The start and end of each part, or `None` for a part that starts below 0, ends past the
    /// points, or holds none.
    fn ranges(&self) -> impl Iterator<Item = Option<(usize, usize)>> + '_ {
        let (parts, points): (&[[u8; 4]], usize) = match self {
            Shape::Lines { parts, points, .. } => (parts, points.len()),
            _ => (&[], 0),
        };
        let start = |p: &[u8; 4]| i32::from_le_bytes(*p);
        parts.iter().enumerate().map(move |(i, part)| {
            let begin = usize::try_from(start(part)).ok()?;
            let end = match parts.get(i + 1) {
                Some(next) => usize::try_from(start(next)).ok()?,
                None => points,
            };
            (begin < end && end <= points).then_some((begin, end))
        })
    }

    /// The geometry the port delivers: a point, a multipoint, an arc as a line string of one
    /// part or a multi-line string of more, and a polygon as one polygon of all its parts.
    pub fn geometry(&self) -> Geometry {
        match self {
            Shape::Point(c) => Geometry::Point(*c),
            Shape::MultiPoint(points) => Geometry::MultiPoint(points.iter().map(xy).collect()),
            Shape::Lines {
                polygon,
                parts,
                points,
            } => {
                // The points are taken in sequence from the first, whatever a part's start.
                let mut next = points.iter().map(xy);
                let mut lines: Vec<Vec<Coord>> = Vec::with_capacity(parts.len());
                for (begin, end) in self.ranges().flatten() {
                    lines.push(next.by_ref().take(end - begin).collect());
                }
                if *polygon {
                    Geometry::Polygon(lines)
                } else if lines.len() == 1 {
                    Geometry::LineString(lines.pop().unwrap_or_default())
                } else {
                    Geometry::MultiLineString(lines)
                }
            }
        }
    }
}

fn xy(point: &[u8; 16]) -> Coord {
    let (x, y) = point.split_at(8);
    let f = |b: &[u8]| f64::from_le_bytes(b.try_into().unwrap_or([0; 8]));
    Coord { x: f(x), y: f(y) }
}

/// The point of two little-endian doubles at `at`.
pub(crate) fn coord(bytes: &[u8], at: usize) -> Option<Coord> {
    Some(Coord {
        x: le_f64(bytes, at)?,
        y: le_f64(bytes, at.checked_add(8)?)?,
    })
}

fn le_f64(bytes: &[u8], at: usize) -> Option<f64> {
    let b = bytes.get(at..at.checked_add(8)?)?;
    Some(f64::from_le_bytes(b.try_into().ok()?))
}

fn le_i32(bytes: &[u8], at: usize) -> Option<i32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(i32::from_le_bytes(b.try_into().ok()?))
}

/// The big-endian `int` at `at`, as the file headers and the `.shx` entries store them.
pub(crate) fn be_i32(bytes: &[u8], at: usize) -> Option<i32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(i32::from_be_bytes(b.try_into().ok()?))
}
