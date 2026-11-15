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

//! The geometry of `simplify` (ADR-draft-geometry-through-geo): a line or a polygon simplified as
//! GEOS's `DouglasPeuckerSimplifier` simplifies it. A line keeps the points of a Douglas-Peucker
//! ported from JTS, which keeps its pending sections on a stack of its own, so no line nests it
//! deeper than the stack allows. A polygon's rings are simplified each as a line, then the steps
//! of JTS follow: a ring's first point dropped, a ring of fewer than 4 coordinates removed, and a
//! result geo's `Validation` finds invalid repaired by geo's `unary_union`.
//!
//! What the functions visit and add is charged to the caller's [`Charge`] as they go (Q-9): each
//! section the Douglas-Peucker scans, before it scans it; the pairs of segments and of rings the
//! validity test compares, and the rings the repair compares, before each runs; and the points the
//! repair returns beyond those it was given.

use forager_style_ir::Coord;
use geo::{LineString, MultiPolygon, Polygon, Validation, unary_union};

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

fn to_ring(ring: &[Coord]) -> LineString {
    LineString::from(ring.iter().map(|c| (c.x, c.y)).collect::<Vec<_>>())
}

fn from_ring(ring: &LineString) -> Vec<Coord> {
    ring.0.iter().map(|c| Coord { x: c.x, y: c.y }).collect()
}

fn to_polygon(area: &Area) -> Polygon {
    Polygon::new(
        to_ring(&area.shell),
        area.holes.iter().map(|hole| to_ring(hole)).collect(),
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
