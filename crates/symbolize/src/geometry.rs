// The Douglas-Peucker and the steps of a polygon's simplification follow JTS, the Java Topology
// Suite, at its `master` of 2026-11-15: the sections split and the ring's first point dropped as
// `simplifySection` and `simplifyRingEndpoint` of
// modules/core/src/main/java/org/locationtech/jts/simplify/DouglasPeuckerLineSimplifier.java
// decide, with the distance of `pointToSegment` of
// modules/core/src/main/java/org/locationtech/jts/algorithm/Distance.java; a collapsed ring removed
// and an invalid result repaired as `DPTransformer` of
// modules/core/src/main/java/org/locationtech/jts/simplify/DouglasPeuckerSimplifier.java and
// `transformPolygon` of modules/core/src/main/java/org/locationtech/jts/geom/util/GeometryTransformer.java
// decide. JTS is offered under the Eclipse Public License 2.0 or the Eclipse Distribution License
// 1.0; Forager takes it under the latter:
//
// Copyright (c) 2016 Vivid Solutions.
//
// Eclipse Distribution License - v 1.0
//
// Copyright (c) 2007, Eclipse Foundation, Inc. and its licensors.
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are permitted
// provided that the following conditions are met:
//
//   Redistributions of source code must retain the above copyright notice, this list of conditions
//   and the following disclaimer.
//
//   Redistributions in binary form must reproduce the above copyright notice, this list of
//   conditions and the following disclaimer in the documentation and/or other materials provided
//   with the distribution.
//
//   Neither the name of the Eclipse Foundation, Inc. nor the names of its contributors may be used
//   to endorse or promote products derived from this software without specific prior written
//   permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
// IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
// FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
// CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY
// WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! The geometry of `simplify` and `buffer` (ADR-0041), and of `simplifypt`
//! (ADR-0047) in [`topology`]: a line or a polygon simplified as
//! GEOS's `DouglasPeuckerSimplifier` simplifies it, and a shape buffered by geo's
//! `buffer_with_style` with round joins and caps of 30 segments a quadrant, as `GEOSBuffer_r`
//! buffers it with 30. A line keeps the points of a Douglas-Peucker
//! ported from JTS, which keeps its pending sections on a stack of its own, so no line nests it
//! deeper than the stack allows. A polygon's rings are simplified each as a line, then the steps
//! of JTS follow: a ring's first point dropped, a ring of fewer than 4 coordinates removed, and a
//! result geo's `Validation` finds invalid repaired by geo's `unary_union`.
//!
//! What the functions visit and add is charged to the caller's [`Charge`] as they go (Q-9): each
//! section the Douglas-Peucker scans, before it scans it; the pairs of segments and of rings the
//! validity test compares, and the rings the repair compares, before each runs; and the points the
//! repair returns beyond those it was given; the points a buffer's offset curves may hold, before
//! it runs, and the points its result holds beyond those it was given.

mod topology;

use std::f64::consts::PI;

use forager_style_ir::Coord;
use forager_style_ir::c::{ms_max, ms_min};
use geo::algorithm::buffer::{BufferStyle, LineCap, LineJoin};
use geo::{
    Buffer, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon, Validation,
    unary_union,
};

pub(crate) use topology::{Line, simplify_preserving};

/// The angle of one segment of a buffer's round join or cap: 30 segments a quadrant, as
/// msGEOSBuffer asks of `GEOSBuffer_r`.
const SEGMENT_ANGLE: f64 = PI / 60.0;

/// The segments of a round cap, a half turn, and of a point's circle, a whole one.
const CAP_SEGMENTS: usize = 60;
const CIRCLE_SEGMENTS: usize = 120;

/// A polygon of MapServer's shape as GEOS builds it: a shell and the holes inside it, each ring
/// closed and of 4 coordinates or more.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Area {
    pub(crate) shell: Vec<Coord>,
    pub(crate) holes: Vec<Vec<Coord>>,
}

/// The budget the functions charge what they visit and add to (Q-9).
pub(crate) trait Charge {
    type Error;
    /// `points` visited beyond reading each once.
    fn visit(&mut self, points: usize) -> Result<(), Self::Error>;
    /// `points` added to the shapes.
    fn add(&mut self, points: usize) -> Result<(), Self::Error>;
}

/// `line` simplified by `tolerance`, not below 0, by the Douglas-Peucker: the end points and the
/// points it keeps.
pub(crate) fn simplify_line<C: Charge>(
    line: &[Coord],
    tolerance: f64,
    charge: &mut C,
) -> Result<Vec<Coord>, C::Error> {
    douglas_peucker(line, tolerance, charge)
}

/// `areas` simplified by `tolerance`, not below 0, as GEOS's `DouglasPeuckerSimplifier`
/// simplifies a polygon, one area, or a multipolygon, any other number: the polygons of the
/// result, none where every shell collapses.
///
/// Each ring is simplified as a line, then loses its first point where [`simplified_ring`]
/// decides, and is removed below 4 coordinates. An area whose shell is removed is removed with its
/// holes. The result is kept where geo's `Validation` finds it valid, as a whole for a
/// multipolygon, and is otherwise replaced by geo's `unary_union` of its polygons.
pub(crate) fn simplify_areas<C: Charge>(
    areas: &[Area],
    tolerance: f64,
    charge: &mut C,
) -> Result<Vec<Area>, C::Error> {
    let mut rough = Vec::with_capacity(areas.len());
    for area in areas {
        let Some(shell) = simplified_ring(&area.shell, tolerance, charge)? else {
            continue;
        };
        let mut holes = Vec::with_capacity(area.holes.len());
        for hole in &area.holes {
            holes.extend(simplified_ring(hole, tolerance, charge)?);
        }
        rough.push(Area { shell, holes });
    }
    if rough.is_empty() {
        return Ok(rough);
    }
    // geo's Validation compares each ring's segments pairwise, and each pair of rings.
    let points = count(&rough);
    let rings = rough.iter().map(|area| 1 + area.holes.len()).sum::<usize>();
    let pairs = points.saturating_mul(rings);
    let segments = rough
        .iter()
        .flat_map(|area| std::iter::once(&area.shell).chain(&area.holes))
        .fold(0_usize, |sum, ring| {
            sum.saturating_add(ring.len().saturating_mul(ring.len()))
        });
    charge.visit(segments.saturating_add(pairs))?;
    let polygons = MultiPolygon(rough.iter().map(to_polygon).collect());
    if polygons.is_valid() {
        return Ok(rough);
    }
    charge.visit(pairs)?;
    let repaired: Vec<Area> = unary_union(&polygons.0)
        .0
        .iter()
        .map(from_polygon)
        .collect();
    charge.add(count(&repaired).saturating_sub(points))?;
    Ok(repaired)
}

/// A shape as GEOS builds it from MapServer's, for [`buffer`].
#[derive(Clone, Copy, Debug)]
pub(crate) enum Buffered<'a> {
    /// A point, or the multipoint of more than one.
    Points(&'a [Coord]),
    /// A line, or the multiline of more than one part; a part without points is no line.
    Lines(&'a [Vec<Coord>]),
    /// A polygon, or the multipolygon of more than one area.
    Areas(&'a [Area]),
}

/// `shape` buffered by `distance`, a finite number, by geo's `buffer_with_style` with round
/// joins and round caps of [`SEGMENT_ANGLE`]: the polygons of the result, none where it is empty.
/// A point or a line buffered by a distance not above 0 is empty; a polygon is shrunk by a
/// negative one.
///
/// A tiny buffer, whose edges are too short for the square of their length, is buffered at a
/// scale near 1: the shape and the distance are scaled by the power of two of [`magnification`]
/// and the result by its inverse. A line's distance is raised to its [`floored`] one first.
///
/// Before the buffer runs, its offset curves are charged to `charge`, so that a bound of Q-9
/// stops it before it holds them: the points of their round joins and caps, [`arcs`], as added,
/// and those and the ends of their offset segments, two for each point on each side, as visited.
/// After it, the points of the result beyond those of `shape` and those arcs are charged as
/// added.
pub(crate) fn buffer<C: Charge>(
    shape: Buffered<'_>,
    distance: f64,
    charge: &mut C,
) -> Result<Vec<Area>, C::Error> {
    let (given, sides) = match shape {
        Buffered::Points(points) => (points.len(), 2),
        Buffered::Lines(lines) => (lines.iter().map(Vec::len).sum::<usize>(), 2),
        Buffered::Areas(areas) => (count(areas), 1),
    };
    let arcs = arcs(shape);
    charge.add(arcs)?;
    charge.visit(given.saturating_mul(2 * sides).saturating_add(arcs))?;
    let distance = floored(shape, distance);
    let n = magnification(shape, distance);
    let result = if n == 0 {
        buffer_with_style(shape, distance)
    } else {
        let up = |ring: &[Coord]| ring.iter().map(|p| scaled(p, n)).collect::<Vec<_>>();
        let distance = libm::scalbn(distance, n);
        let mut result = match shape {
            Buffered::Points(points) => buffer_with_style(Buffered::Points(&up(points)), distance),
            Buffered::Lines(lines) => {
                let lines: Vec<Vec<Coord>> = lines.iter().map(|line| up(line)).collect();
                buffer_with_style(Buffered::Lines(&lines), distance)
            }
            Buffered::Areas(areas) => {
                let areas: Vec<Area> = areas
                    .iter()
                    .map(|area| Area {
                        shell: up(&area.shell),
                        holes: area.holes.iter().map(|hole| up(hole)).collect(),
                    })
                    .collect();
                buffer_with_style(Buffered::Areas(&areas), distance)
            }
        };
        for ring in result
            .iter_mut()
            .flat_map(|area| std::iter::once(&mut area.shell).chain(&mut area.holes))
        {
            for p in ring.iter_mut() {
                *p = scaled(p, -n);
            }
        }
        result
    };
    charge.add(count(&result).saturating_sub(given.saturating_add(arcs)))?;
    Ok(result)
}

/// A buffer smaller than 2^`TINY` is scaled, by [`magnification`].
///
/// i_overlay snaps the shape to a grid of more than 2^-31 of its size before it offsets each edge
/// by the edge's direction, which it normalizes by the square of the edge's length. Below a size of
/// 2^-480 that square can fall below the normal numbers, and lose its precision or underflow to 0,
/// which makes the direction infinite. Every buffer of a size of at least 2^-450 is left as it is.
const TINY: i32 = -450;

/// The exponent beyond which a shape scaled by [`magnification`] would lie too far from the
/// origin: some scaled coordinate would reach 2^512.
const LARGEST_SCALED: i32 = 512;

// Guard: i_overlay 4.5.2
// Guard: i_float 1.16.0
/// The power of two that `shape` and `distance` are scaled by before their buffer runs, and its
/// result by the inverse after; 0 for every buffer but a tiny one.
///
/// A buffer's size is the larger of the width and the height of the shape's bounds, and of the
/// distance's magnitude, as i_overlay sizes its grid. Below 2^[`TINY`] the power
/// brings that size to between ½ and 1. A power of two scales a coordinate without rounding, unless
/// the coordinate is subnormal. A point, or a shape of one place, far from the origin and buffered
/// by a tiny distance is not scaled, where a scaled coordinate would lie past
/// 2^[`LARGEST_SCALED`].
///
/// An exact guard (ADR-0049), read from i_overlay 4.5.2 and the i_float 1.16.0 it takes:
/// `FloatPointAdapter::new` sets the grid to 2^(29 - log2 of the size) (i_float's
/// `adapter.rs:37-41`), and the offset of an edge, `outline/builder.rs:192` and `Math::normal` of
/// `mesh/math.rs:10-13` in i_overlay, normalizes the edge by `FloatPointMath::normalize`, which
/// divides by its length without a check (i_float's `float/vector.rs:34-37`). The point offset
/// by that infinite direction fails the debug assertion of `float_to_int` (i_float's
/// `adapter.rs:115-123`), and in a release build the shape vanishes.
fn magnification(shape: Buffered<'_>, distance: f64) -> i32 {
    let Some((low, high)) = bounds(shape) else {
        return 0;
    };
    let size = ms_max(ms_max(high.x - low.x, high.y - low.y), distance.abs());
    if !(size.is_finite() && size > 0.0) {
        return 0;
    }
    let (_, e) = libm::frexp(size);
    if e > TINY {
        return 0;
    }
    let reach = ms_max(
        ms_max(low.x.abs(), low.y.abs()),
        ms_max(high.x.abs(), high.y.abs()),
    );
    if reach > 0.0 && libm::frexp(reach).1 - e > LARGEST_SCALED {
        return 0;
    }
    -e
}

/// A line's buffer narrower than 2^`FLOOR` of the line's size is buffered by that width.
///
/// i_overlay 4.5.2 snaps the shape to a grid of about 2^-30 of its size. A line's stroke whose
/// width is a few steps of that grid leaves its graph inconsistent: `find_contour` walks it
/// without end and grows the contour until an allocation fails, or the stroke returns no polygon,
/// or a debug assertion fails. At 2^-24, 64 or more steps of the grid, it holds.
const FLOOR: i32 = -24;

/// A line's buffer narrower than 2^`REACH_FLOOR` of its reach, the largest magnitude of its
/// coordinates, is buffered by that width.
///
/// Far from the origin the coordinates themselves are coarser than i_overlay's grid: a stroke
/// within some 2^-47 of the reach, a few ulps of its coordinates, walks or vanishes as one of a few
/// grid steps does, however large the line. At 2^-40 it holds.
const REACH_FLOOR: i32 = -40;

// Guard: i_overlay 4.5.2
// Guard: i_float 1.16.0
/// The distance a line's buffer runs by: `distance`, raised to the larger of 2^[`FLOOR`] of the
/// size of the lines' joint bounds, the larger of their width and height, and 2^[`REACH_FLOOR`] of
/// their reach, where it lies below that. The bounds are joint since geo strokes the parts of a
/// multiline as one. A distance not above 0, and that of a point or a polygon, is kept. GEOS keeps
/// the thinner buffer; the two differ by at most the floor.
///
/// A bounded change (ADR-0049), its bound the floor, read from i_overlay 4.5.2 and the i_float
/// 1.16.0 it takes: the grid of `FloatPointAdapter::new`, 2^(29 - log2 of the size) (i_float's
/// `adapter.rs:37-41`), and `find_contour` of i_overlay's `core/extract.rs:181-208`, whose walk
/// of the stroke's graph does not end where the graph is inconsistent.
fn floored(shape: Buffered<'_>, distance: f64) -> f64 {
    if !matches!(shape, Buffered::Lines(_)) || distance <= 0.0 {
        return distance;
    }
    let Some((low, high)) = bounds(shape) else {
        return distance;
    };
    let size = libm::scalbn(ms_max(high.x - low.x, high.y - low.y), FLOOR);
    let reach = libm::scalbn(
        ms_max(
            ms_max(low.x.abs(), low.y.abs()),
            ms_max(high.x.abs(), high.y.abs()),
        ),
        REACH_FLOOR,
    );
    let floor = if size.is_finite() {
        ms_max(size, reach)
    } else {
        reach
    };
    ms_max(distance, floor)
}

/// The lowest and the highest corner of the bounds of `shape`'s points; none where it has no
/// point, or a point of a coordinate that is no finite number.
fn bounds(shape: Buffered<'_>) -> Option<(Coord, Coord)> {
    let points: Box<dyn Iterator<Item = &Coord>> = match shape {
        Buffered::Points(points) => Box::new(points.iter()),
        Buffered::Lines(lines) => Box::new(lines.iter().flatten()),
        Buffered::Areas(areas) => Box::new(
            areas
                .iter()
                .flat_map(|area| std::iter::once(&area.shell).chain(&area.holes))
                .flatten(),
        ),
    };
    let mut corners: Option<(Coord, Coord)> = None;
    for p in points {
        if !(p.x.is_finite() && p.y.is_finite()) {
            return None;
        }
        corners = Some(match corners {
            None => (*p, *p),
            Some((low, high)) => (
                Coord {
                    x: ms_min(low.x, p.x),
                    y: ms_min(low.y, p.y),
                },
                Coord {
                    x: ms_max(high.x, p.x),
                    y: ms_max(high.y, p.y),
                },
            ),
        });
    }
    corners
}

/// `p` scaled by 2^`n`.
fn scaled(p: &Coord, n: i32) -> Coord {
    Coord {
        x: libm::scalbn(p.x, n),
        y: libm::scalbn(p.y, n),
    }
}

/// `shape` buffered by `distance` by geo's `buffer_with_style` with round joins and round caps of
/// [`SEGMENT_ANGLE`], uncharged.
fn buffer_with_style(shape: Buffered<'_>, distance: f64) -> Vec<Area> {
    let style = BufferStyle::new(distance)
        .line_join(LineJoin::Round(SEGMENT_ANGLE))
        .line_cap(LineCap::Round(SEGMENT_ANGLE));
    let buffered = match shape {
        Buffered::Points([point]) => Point::new(point.x, point.y).buffer_with_style(style),
        Buffered::Points(points) => MultiPoint::from(
            points
                .iter()
                .map(|p| Point::new(p.x, p.y))
                .collect::<Vec<_>>(),
        )
        .buffer_with_style(style),
        Buffered::Lines(lines) => MultiLineString(
            lines
                .iter()
                .filter(|line| !line.is_empty())
                .map(|line| to_line_string(line))
                .collect(),
        )
        .buffer_with_style(style),
        Buffered::Areas(areas) => {
            MultiPolygon(areas.iter().map(to_polygon).collect()).buffer_with_style(style)
        }
    };
    buffered.0.iter().map(from_polygon).collect()
}

/// The points the round joins and caps of `shape`'s buffer may add, one for each segment of
/// their arcs: at each vertex the turn between its two segments in steps of [`SEGMENT_ANGLE`],
/// rounded, between points that differ, a ring's first vertex among them, which a polygon's offset
/// curve turns on the one side
/// and a line's on the other, so that a vertex is counted once whatever the distance's sign; a
/// half turn at each end of a line, and a whole turn about a point or a line of one place.
fn arcs(shape: Buffered<'_>) -> usize {
    let turns = |line: &[Coord], closed: bool| -> usize {
        // i_overlay drops a point equal to the one before it before it joins the segments, so
        // the turns are measured between distinct points, a ring's wrapped past its repeats.
        let mut places: Vec<Coord> = Vec::with_capacity(line.len());
        for &p in line {
            if places.last() != Some(&p) {
                places.push(p);
            }
        }
        if closed {
            while places.len() > 1 && places.last() == places.first() {
                places.pop();
            }
        }
        let n = places.len();
        if n < 3 {
            return 0;
        }
        let mut segments = 0_usize;
        let inner = if closed { 0..n } else { 1..n - 1 };
        for i in inner {
            let before = places[(i + n - 1) % n];
            let (at, after) = (places[i], places[(i + 1) % n]);
            let (ax, ay) = (at.x - before.x, at.y - before.y);
            let (bx, by) = (after.x - at.x, after.y - at.y);
            let turn = libm::atan2((ax * by - ay * bx).abs(), ax * bx + ay * by);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole number from 0 to 60"
            )]
            let steps = libm::round(turn / SEGMENT_ANGLE) as usize;
            segments = segments.saturating_add(steps);
        }
        segments
    };
    let line = |line: &Vec<Coord>| match line.as_slice() {
        [] => 0,
        [first, rest @ ..] if rest.iter().all(|p| p == first) => CIRCLE_SEGMENTS,
        _ => turns(line, false).saturating_add(2 * CAP_SEGMENTS),
    };
    match shape {
        Buffered::Points(points) => points.len().saturating_mul(CIRCLE_SEGMENTS),
        Buffered::Lines(lines) => lines.iter().map(line).fold(0, usize::saturating_add),
        Buffered::Areas(areas) => areas
            .iter()
            .flat_map(|area| std::iter::once(&area.shell).chain(&area.holes))
            .map(|ring| turns(ring, true))
            .fold(0, usize::saturating_add),
    }
}

/// The points of the rings of `areas`.
fn count(areas: &[Area]) -> usize {
    areas
        .iter()
        .map(|area| area.shell.len() + area.holes.iter().map(Vec::len).sum::<usize>())
        .sum()
}

/// A closed ring simplified as a line, then without its first point where that point lies
/// within `tolerance` of the segment between its neighbours, as `simplifyRingEndpoint` decides
/// for a ring of 4 coordinates or more; none where fewer than 4 coordinates are left, as
/// `DPTransformer.transformLinearRing` removes such a ring.
fn simplified_ring<C: Charge>(
    ring: &[Coord],
    tolerance: f64,
    charge: &mut C,
) -> Result<Option<Vec<Coord>>, C::Error> {
    let mut kept = douglas_peucker(ring, tolerance, charge)?;
    let n = kept.len();
    if n >= 4 && point_to_segment(kept[0], kept[1], kept[n - 2]) <= tolerance {
        kept.remove(0);
        kept.pop();
        // `CoordinateList.closeRing`: the first point again, unless the ring is closed already.
        if kept.first() != kept.last()
            && let Some(&first) = kept.first()
        {
            kept.push(first);
        }
    }
    Ok((kept.len() >= 4).then_some(kept))
}

/// `simplifySection` of JTS's `DouglasPeuckerLineSimplifier` over the whole line, its recursion
/// turned into a stack of the sections still to scan. A section's inner points are dropped where
/// none lies farther than `tolerance` from the segment between its ends; otherwise the section is
/// split at the first of its farthest points. Each section's inner points are charged to `charge`
/// before they are scanned.
///
/// GEOS's comparisons with a tolerance that is no number are all false, so it splits every section
/// and keeps every point; a section whose distances are all no number is kept whole, where JTS
/// would split it at its first end again without end.
fn douglas_peucker<C: Charge>(
    line: &[Coord],
    tolerance: f64,
    charge: &mut C,
) -> Result<Vec<Coord>, C::Error> {
    let Some(last) = line.len().checked_sub(1) else {
        return Ok(Vec::new());
    };
    let mut kept = vec![true; line.len()];
    let mut sections = vec![(0, last)];
    while let Some((i, j)) = sections.pop() {
        if i + 1 >= j {
            continue;
        }
        charge.visit(j - i - 1)?;
        let mut farthest = -1.0;
        let mut at = i;
        for (k, &p) in line.iter().enumerate().take(j).skip(i + 1) {
            let distance = point_to_segment(p, line[i], line[j]);
            if distance > farthest {
                farthest = distance;
                at = k;
            }
        }
        if farthest <= tolerance {
            kept[i + 1..j].fill(false);
        } else if at > i {
            sections.push((at, j));
            sections.push((i, at));
        }
    }
    Ok(line
        .iter()
        .zip(kept)
        .filter_map(|(&p, keep)| keep.then_some(p))
        .collect())
}

/// `Distance.pointToSegment`: the distance of `p` from the segment from `a` to `b`, by `f64::sqrt`.
fn point_to_segment(p: Coord, a: Coord, b: Coord) -> f64 {
    let distance = |q: Coord| ((p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y)).sqrt();
    if a.x == b.x && a.y == b.y {
        return distance(a);
    }
    let len2 = (b.x - a.x) * (b.x - a.x) + (b.y - a.y) * (b.y - a.y);
    let r = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / len2;
    if r <= 0.0 {
        return distance(a);
    }
    if r >= 1.0 {
        return distance(b);
    }
    let s = ((a.y - p.y) * (b.x - a.x) - (a.x - p.x) * (b.y - a.y)) / len2;
    s.abs() * len2.sqrt()
}

fn to_line_string(ring: &[Coord]) -> LineString {
    LineString::from(ring.iter().map(|c| (c.x, c.y)).collect::<Vec<_>>())
}

fn from_ring(ring: &LineString) -> Vec<Coord> {
    ring.0.iter().map(|c| Coord { x: c.x, y: c.y }).collect()
}

fn to_polygon(area: &Area) -> Polygon {
    Polygon::new(
        to_line_string(&area.shell),
        area.holes.iter().map(|hole| to_line_string(hole)).collect(),
    )
}

fn from_polygon(polygon: &Polygon) -> Area {
    Area {
        shell: from_ring(polygon.exterior()),
        holes: polygon.interiors().iter().map(from_ring).collect(),
    }
}

#[cfg(test)]
mod tests;
