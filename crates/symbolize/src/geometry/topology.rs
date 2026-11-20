// The topology-preserving simplification follows JTS, the Java Topology Suite, at its `master`
// commit 722a943c of 2026-11-14: the sections, the minimum size, the indexes, and the ring's first
// point as
// modules/core/src/main/java/org/locationtech/jts/simplify/TopologyPreservingSimplifier.java,
// TaggedLinesSimplifier.java, TaggedLineStringSimplifier.java, and TaggedLineString.java decide;
// the jump test as ComponentJumpChecker.java of the same package decides; the interior intersection
// as `computeIntersect`, `computeCollinearIntersection`, and `isInteriorIntersection` of
// modules/core/src/main/java/org/locationtech/jts/algorithm/RobustLineIntersector.java and
// LineIntersector.java decide; the crossing count as `countSegment` of
// modules/core/src/main/java/org/locationtech/jts/algorithm/RayCrossingCounter.java decides; and
// `equalsTopo`, `distance`, and `orientationIndex` of
// modules/core/src/main/java/org/locationtech/jts/geom/LineSegment.java, `pointToSegment` of
// modules/core/src/main/java/org/locationtech/jts/algorithm/Distance.java, `isRing` of
// modules/core/src/main/java/org/locationtech/jts/geom/CoordinateArrays.java, and the tests of an
// envelope of modules/core/src/main/java/org/locationtech/jts/geom/Envelope.java. JTS is offered
// under the Eclipse Public License 2.0 or the Eclipse Distribution License 1.0; Forager takes it
// under the latter:
//
// Copyright (c) 2016 Vivid Solutions.
// Copyright (c) 2016 Vivid Solutions, and others.
// Copyright (c) 2023 Martin Davis.
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

//! The geometry of `simplifypt` (GEO-simplifypt-ported-from-jts): the lines of one shape
//! simplified together as JTS's `TopologyPreservingSimplifier` simplifies them, a Douglas-Peucker
//! that refuses to flatten a section where the new segment would meet the rest of the shape in
//! an interior point, lie on an equal segment, or jump another line.
//!
//! The port departs from JTS where the ADR decides: the lines are taken in the shape's order,
//! not a `HashMap`'s; the pending sections are kept on a stack of their own, not by recursion;
//! every orientation is the sign of geo's `RobustKernel`; each index answers a query by examining
//! every segment it holds; and a section whose distances are all no number is kept whole.
//!
//! What it visits is charged to the caller's [`Charge`] before it is visited (Q-9): the inner
//! points of each section it scans, the segments each query of an index examines, and the other
//! lines the jump test compares and the segments whose crossings it counts.

use forager_style_ir::Coord;
use geo::kernels::{Kernel, Orientation, RobustKernel};

use super::{Charge, point_to_segment};

/// One line of a shape for [`simplify_preserving`]: its points, and whether it is a ring of a
/// polygon, JTS's `LinearRing`, which may lose its first point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line<'a> {
    pub(crate) points: &'a [Coord],
    pub(crate) ring: bool,
}

/// `lines` simplified together by `tolerance`, as JTS's `TopologyPreservingSimplifier`
/// simplifies the lines of one geometry: the points each keeps, in the order of `lines`. A line
/// keeps at least 2 points, a closed one at least 4; one of fewer than 2, which GEOS cannot build,
/// is kept as it is.
pub(crate) fn simplify_preserving<C: Charge>(
    lines: &[Line<'_>],
    tolerance: f64,
    charge: &mut C,
) -> Result<Vec<Vec<Coord>>, C::Error> {
    let mut first = Vec::with_capacity(lines.len());
    let mut input = Pool::default();
    let mut id = 0;
    for (l, line) in lines.iter().enumerate() {
        first.push(id);
        for (index, pair) in line.points.windows(2).enumerate() {
            input.insert(id, Held::new(pair[0], pair[1], l, index));
            id += 1;
        }
    }
    let mut simplifier = Simplifier {
        lines,
        tolerance,
        first,
        input,
        output: Pool::default(),
        outputs: 0,
        results: vec![Vec::new(); lines.len()],
        charge,
    };
    for l in 0..lines.len() {
        simplifier.simplify(l)?;
    }
    Ok(simplifier
        .results
        .iter()
        .zip(lines)
        .map(|(result, line)| match result.last() {
            None => line.points.to_vec(),
            Some(last) => {
                let mut points: Vec<Coord> = result.iter().map(|kept| kept.segment.p0).collect();
                points.push(last.segment.p1);
                points
            }
        })
        .collect())
}

/// JTS's `LineSegment`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    p0: Coord,
    p1: Coord,
}

/// A segment an index holds, with the line and the place of an input segment.
#[derive(Clone, Copy, Debug)]
struct Held {
    segment: Segment,
    line: usize,
    index: usize,
}

impl Held {
    fn new(p0: Coord, p1: Coord, line: usize, index: usize) -> Self {
        Held {
            segment: Segment { p0, p1 },
            line,
            index,
        }
    }
}

/// An index without a spatial structure: the segments it holds, each found by its id.
#[derive(Default)]
struct Pool {
    held: Vec<(usize, Held)>,
    at: Vec<Option<usize>>,
}

impl Pool {
    fn insert(&mut self, id: usize, held: Held) {
        if self.at.len() <= id {
            self.at.resize(id + 1, None);
        }
        self.at[id] = Some(self.held.len());
        self.held.push((id, held));
    }

    fn remove(&mut self, id: usize) {
        let Some(place) = self.at.get_mut(id).and_then(Option::take) else {
            return;
        };
        self.held.swap_remove(place);
        if let Some(&(moved, _)) = self.held.get(place) {
            self.at[moved] = Some(place);
        }
    }
}

/// Where a segment of a line's result is held: an input segment kept unchanged, which stays in
/// the input index, or a new segment, which the output index holds.
#[derive(Clone, Copy, Debug)]
enum Origin {
    Input(usize),
    Output(usize),
}

#[derive(Clone, Copy, Debug)]
struct Kept {
    segment: Segment,
    origin: Origin,
}

/// `TaggedLinesSimplifier` and `TaggedLineStringSimplifier` over the lines of one shape.
struct Simplifier<'a, 'c, C> {
    lines: &'a [Line<'a>],
    tolerance: f64,
    /// The id of each line's first input segment.
    first: Vec<usize>,
    input: Pool,
    output: Pool,
    /// The ids the output index has given.
    outputs: usize,
    /// Each line's result, `TaggedLineString`'s `resultSegs`.
    results: Vec<Vec<Kept>>,
    charge: &'c mut C,
}

impl<C: Charge> Simplifier<'_, '_, C> {
    /// `TaggedLineStringSimplifier.simplify`: `simplifySection` over the whole line, its
    /// recursion turned into a stack that takes the left section first, then the ring's first
    /// point.
    fn simplify(&mut self, l: usize) -> Result<(), C::Error> {
        let points = self.lines[l].points;
        // GEOS builds no line of fewer than 2 points; such a line is kept as it is.
        if points.len() < 2 {
            return Ok(());
        }
        let last = points.len() - 1;
        // `TopologyPreservingSimplifier`: a closed line keeps 4 points, an open one 2.
        let minimum = if points.first() == points.last() {
            4
        } else {
            2
        };
        let mut sections = vec![(0, last, 0_usize)];
        while let Some((i, j, depth)) = sections.pop() {
            let depth = depth + 1;
            if i + 1 == j {
                self.keep_input(l, i);
                continue;
            }
            let mut valid = true;
            if self.result_size(l) < minimum && depth + 1 < minimum {
                valid = false;
            }
            self.charge.visit(j - i - 1)?;
            let flat = Segment {
                p0: points[i],
                p1: points[j],
            };
            let mut farthest = -1.0;
            let mut at = i;
            for (k, &p) in points.iter().enumerate().take(j).skip(i + 1) {
                let distance = point_to_segment(p, flat.p0, flat.p1);
                if distance > farthest {
                    farthest = distance;
                    at = k;
                }
            }
            if farthest > self.tolerance {
                valid = false;
            }
            if valid {
                valid = self.section_valid(l, i, j, flat)?;
            }
            if valid {
                for k in i..j {
                    self.input.remove(self.first[l] + k);
                }
                let id = self.add_output(flat);
                self.results[l].push(Kept {
                    segment: flat,
                    origin: Origin::Output(id),
                });
            } else if at == i {
                // Every distance is no number: JTS would split the section at its first end again
                // without end, so it is kept whole.
                for k in i..j {
                    self.keep_input(l, k);
                }
            } else {
                sections.push((at, j, depth));
                sections.push((i, at, depth));
            }
        }
        if self.lines[l].ring && points.len() >= 4 && points.first() == points.last() {
            self.simplify_ring_endpoint(l, minimum)?;
        }
        Ok(())
    }

    /// `simplifyRingEndpoint`: the ring's first point dropped where it lies within the tolerance
    /// of the segment between its neighbours, the ring holds more than its minimum, and the new
    /// segment keeps the topology.
    fn simplify_ring_endpoint(&mut self, l: usize, minimum: usize) -> Result<(), C::Error> {
        if self.result_size(l) <= minimum {
            return Ok(());
        }
        let result = &self.results[l];
        let (first, last) = (result[0], result[result.len() - 1]);
        let flat = Segment {
            p0: last.segment.p0,
            p1: first.segment.p1,
        };
        if !(point_to_segment(first.segment.p0, flat.p0, flat.p1) <= self.tolerance
            && self.endpoint_valid(l, first.segment, last.segment, flat)?)
        {
            return Ok(());
        }
        for kept in [first, last] {
            match kept.origin {
                Origin::Input(id) => self.input.remove(id),
                Origin::Output(id) => self.output.remove(id),
            }
        }
        // `removeRingEndpoint`: the first segment starts where the last did, which is dropped.
        let id = self.add_output(flat);
        let result = &mut self.results[l];
        result.pop();
        result[0] = Kept {
            segment: flat,
            origin: Origin::Output(id),
        };
        Ok(())
    }

    /// `getResultSize`: the points of the line's result so far.
    fn result_size(&self, l: usize) -> usize {
        match self.results[l].len() {
            0 => 0,
            n => n + 1,
        }
    }

    fn keep_input(&mut self, l: usize, index: usize) {
        let points = self.lines[l].points;
        self.results[l].push(Kept {
            segment: Segment {
                p0: points[index],
                p1: points[index + 1],
            },
            origin: Origin::Input(self.first[l] + index),
        });
    }

    fn add_output(&mut self, segment: Segment) -> usize {
        let id = self.outputs;
        self.outputs += 1;
        self.output.insert(
            id,
            Held {
                segment,
                line: usize::MAX,
                index: usize::MAX,
            },
        );
        id
    }

    /// `isTopologyValid` of a section from `i` to `j` of line `l` flattened to `flat`.
    fn section_valid(
        &mut self,
        l: usize,
        i: usize,
        j: usize,
        flat: Segment,
    ) -> Result<bool, C::Error> {
        Ok(!self.has_output_intersection(flat)?
            && !self.has_input_intersection(Some((l, i, j)), flat)?
            && !self.has_section_jump(l, i, j, flat)?)
    }

    /// `isTopologyValid` of the ring's first and last segments flattened to `flat`.
    fn endpoint_valid(
        &mut self,
        l: usize,
        first: Segment,
        last: Segment,
        flat: Segment,
    ) -> Result<bool, C::Error> {
        // Segments already flat keep the topology, which the intersection tests would deny.
        if orientation_index(flat.p0, flat.p1, first.p0) == 0 {
            return Ok(true);
        }
        Ok(!self.has_output_intersection(flat)?
            && !self.has_input_intersection(None, flat)?
            && !self.has_pair_jump(l, first, last, flat)?)
    }

    fn has_output_intersection(&mut self, flat: Segment) -> Result<bool, C::Error> {
        self.charge.visit(self.output.held.len())?;
        Ok(self.output.held.iter().any(|(_, held)| {
            envelopes_intersect(held.segment, flat) && has_invalid_intersection(held.segment, flat)
        }))
    }

    /// `hasInputIntersection`, a segment of the excluded section, `(line, start, end)`, ignored.
    fn has_input_intersection(
        &mut self,
        excluded: Option<(usize, usize, usize)>,
        flat: Segment,
    ) -> Result<bool, C::Error> {
        self.charge.visit(self.input.held.len())?;
        Ok(self.input.held.iter().any(|(_, held)| {
            envelopes_intersect(held.segment, flat)
                && has_invalid_intersection(held.segment, flat)
                && !excluded.is_some_and(|(line, start, end)| {
                    held.line == line && held.index >= start && held.index < end
                })
        }))
    }

    /// `ComponentJumpChecker.hasJump` of a section from `i` to `j` of line `l`.
    fn has_section_jump(
        &mut self,
        l: usize,
        i: usize,
        j: usize,
        flat: Segment,
    ) -> Result<bool, C::Error> {
        let others = self.lines.len() - 1;
        if others == 0 {
            return Ok(false);
        }
        self.charge.visit(others)?;
        let points = self.lines[l].points;
        let mut envelope = Envelope::default();
        for &p in &points[i..=j] {
            envelope.expand(p);
        }
        for c in 0..self.lines.len() {
            if c == l {
                continue;
            }
            let Some(point) = self.component_point(c) else {
                continue;
            };
            if envelope.intersects(point) {
                self.charge.visit(j - i + 1)?;
                let mut section = 0;
                for k in i..j {
                    count_segment(point, points[k], points[k + 1], &mut section);
                }
                let mut flattened = 0;
                count_segment(point, flat.p0, flat.p1, &mut flattened);
                if section % 2 != flattened % 2 {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// `ComponentJumpChecker.hasJump` of the ring's first and last segments.
    fn has_pair_jump(
        &mut self,
        l: usize,
        first: Segment,
        last: Segment,
        flat: Segment,
    ) -> Result<bool, C::Error> {
        let others = self.lines.len() - 1;
        if others == 0 {
            return Ok(false);
        }
        self.charge.visit(others)?;
        let mut envelope = Envelope::default();
        for p in [first.p0, first.p1, last.p0, last.p1] {
            envelope.expand(p);
        }
        for c in 0..self.lines.len() {
            if c == l {
                continue;
            }
            let Some(point) = self.component_point(c) else {
                continue;
            };
            if envelope.intersects(point) {
                self.charge.visit(3)?;
                let mut section = 0;
                count_segment(point, first.p0, first.p1, &mut section);
                count_segment(point, last.p0, last.p1, &mut section);
                let mut flattened = 0;
                count_segment(point, flat.p0, flat.p1, &mut flattened);
                if section % 2 != flattened % 2 {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// `TaggedLineString.getComponentPoint`: the first point of the line's result, or its second
    /// point before it is simplified; none for a line of fewer than 2 points, which GEOS cannot
    /// build.
    fn component_point(&self, c: usize) -> Option<Coord> {
        match self.results[c].first() {
            Some(kept) => Some(kept.segment.p0),
            None => self.lines[c].points.get(1).copied(),
        }
    }
}

/// `Orientation.index`: 1 where `q` lies to the left of the line from `p1` to `p2`, -1 to its
/// right, 0 on it, by the sign of geo's `RobustKernel`.
fn orientation_index(p1: Coord, p2: Coord, q: Coord) -> i32 {
    let c = |p: Coord| geo::Coord { x: p.x, y: p.y };
    match RobustKernel::orient2d(c(p1), c(p2), c(q)) {
        Orientation::CounterClockwise => 1,
        Orientation::Clockwise => -1,
        Orientation::Collinear => 0,
    }
}

/// Java's `Math.min`: no number where either is none.
fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a <= b {
        a
    } else {
        b
    }
}

/// Java's `Math.max`: no number where either is none.
fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a >= b {
        a
    } else {
        b
    }
}

/// `Envelope.intersects(p1, p2, q1, q2)`: whether the envelopes of two segments meet, the
/// visitor's filter of `LineSegmentIndex.query`.
fn envelopes_intersect(p: Segment, q: Segment) -> bool {
    let (p1, p2, q1, q2) = (p.p0, p.p1, q.p0, q.p1);
    if min(p1.x, p2.x) > max(q1.x, q2.x) || max(p1.x, p2.x) < min(q1.x, q2.x) {
        return false;
    }
    !(min(p1.y, p2.y) > max(q1.y, q2.y) || max(p1.y, p2.y) < min(q1.y, q2.y))
}

/// `Envelope.intersects(p1, p2, q)`: whether `q` lies in the envelope of `p1` and `p2`.
fn envelope_holds(p1: Coord, p2: Coord, q: Coord) -> bool {
    (q.x >= if p1.x < p2.x { p1.x } else { p2.x })
        && (q.x <= if p1.x > p2.x { p1.x } else { p2.x })
        && (q.y >= if p1.y < p2.y { p1.y } else { p2.y })
        && (q.y <= if p1.y > p2.y { p1.y } else { p2.y })
}

/// JTS's `Envelope`, null until it takes a point.
struct Envelope {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl Default for Envelope {
    fn default() -> Self {
        Envelope {
            min_x: 0.0,
            max_x: -1.0,
            min_y: 0.0,
            max_y: -1.0,
        }
    }
}

impl Envelope {
    fn is_null(&self) -> bool {
        self.max_x < self.min_x
    }

    /// `expandToInclude`.
    fn expand(&mut self, p: Coord) {
        if self.is_null() {
            *self = Envelope {
                min_x: p.x,
                max_x: p.x,
                min_y: p.y,
                max_y: p.y,
            };
            return;
        }
        if p.x < self.min_x {
            self.min_x = p.x;
        }
        if p.x > self.max_x {
            self.max_x = p.x;
        }
        if p.y < self.min_y {
            self.min_y = p.y;
        }
        if p.y > self.max_y {
            self.max_y = p.y;
        }
    }

    /// `intersects(Coordinate)`.
    fn intersects(&self, p: Coord) -> bool {
        if self.is_null() {
            return false;
        }
        !(p.x > self.max_x || p.x < self.min_x || p.y > self.max_y || p.y < self.min_y)
    }
}

/// `hasInvalidIntersection`: the segments are equal, or meet in a point interior to either.
fn has_invalid_intersection(a: Segment, b: Segment) -> bool {
    // `LineSegment.equalsTopo`.
    if (a.p0 == b.p0 && a.p1 == b.p1) || (a.p0 == b.p1 && a.p1 == b.p0) {
        return true;
    }
    is_interior_intersection(a.p0, a.p1, b.p0, b.p1)
}

/// `RobustLineIntersector.computeIntersect` followed by `LineIntersector.isInteriorIntersection`:
/// whether the segments `p` and `q` meet in a point that is no end point of one of them.
fn is_interior_intersection(p1: Coord, p2: Coord, q1: Coord, q2: Coord) -> bool {
    if !envelopes_intersect(Segment { p0: p1, p1: p2 }, Segment { p0: q1, p1: q2 }) {
        return false;
    }
    let pq1 = orientation_index(p1, p2, q1);
    let pq2 = orientation_index(p1, p2, q2);
    if (pq1 > 0 && pq2 > 0) || (pq1 < 0 && pq2 < 0) {
        return false;
    }
    let qp1 = orientation_index(q1, q2, p1);
    let qp2 = orientation_index(q1, q2, p2);
    if (qp1 > 0 && qp2 > 0) || (qp1 < 0 && qp2 < 0) {
        return false;
    }
    let mut points = [p1; 2];
    let count;
    if pq1 == 0 && pq2 == 0 && qp1 == 0 && qp2 == 0 {
        // `computeCollinearIntersection`.
        let q1_in_p = envelope_holds(p1, p2, q1);
        let q2_in_p = envelope_holds(p1, p2, q2);
        let p1_in_q = envelope_holds(q1, q2, p1);
        let p2_in_q = envelope_holds(q1, q2, p2);
        let (a, b, single) = if q1_in_p && q2_in_p {
            (q1, q2, false)
        } else if p1_in_q && p2_in_q {
            (p1, p2, false)
        } else if q1_in_p && p1_in_q {
            (q1, p1, q1 == p1 && !q2_in_p && !p2_in_q)
        } else if q1_in_p && p2_in_q {
            (q1, p2, q1 == p2 && !q2_in_p && !p1_in_q)
        } else if q2_in_p && p1_in_q {
            (q2, p1, q2 == p1 && !q1_in_p && !p2_in_q)
        } else if q2_in_p && p2_in_q {
            (q2, p2, q2 == p2 && !q1_in_p && !p1_in_q)
        } else {
            return false;
        };
        points = [a, b];
        count = if single { 1 } else { 2 };
    } else if pq1 == 0 || pq2 == 0 || qp1 == 0 || qp2 == 0 {
        // An end point on the other segment, two equal end points tested first.
        points[0] = if p1 == q1 || p1 == q2 {
            p1
        } else if p2 == q1 || p2 == q2 {
            p2
        } else if pq1 == 0 {
            q1
        } else if pq2 == 0 {
            q2
        } else if qp1 == 0 {
            p1
        } else {
            p2
        };
        count = 1;
    } else {
        // A proper intersection lies in the interior of both segments.
        return true;
    }
    [(p1, p2), (q1, q2)].iter().any(|&(a, b)| {
        points[..count]
            .iter()
            .any(|&point| !(point == a || point == b))
    })
}

/// `RayCrossingCounter.countSegment`, the count alone: whether the segment from `p1` to `p2`
/// crosses the ray from `p` in the direction of x, an upward segment with its first point and a
/// downward one with its last, counted in `count`.
fn count_segment(p: Coord, p1: Coord, p2: Coord, count: &mut u32) {
    if p1.x < p.x && p2.x < p.x {
        return;
    }
    if p.x == p2.x && p.y == p2.y {
        return;
    }
    if p1.y == p.y && p2.y == p.y {
        return;
    }
    if (p1.y > p.y && p2.y <= p.y) || (p2.y > p.y && p1.y <= p.y) {
        let mut orientation = orientation_index(p1, p2, p);
        if orientation == 0 {
            return;
        }
        if p2.y < p1.y {
            orientation = -orientation;
        }
        if orientation == 1 {
            *count += 1;
        }
    }
}

#[cfg(test)]
mod tests;
