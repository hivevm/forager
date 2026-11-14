// The pairwise reference follows msDrawLabelCache and msCheckLabelMinDistance, src/mapdraw.c,
// labelInImage, msTestLabelCacheCollisions, and intersectLabelPolygons, src/maplabel.c, and
// msRectOverlap, msPointInRect, msPointInPolygon, and msIntersectSegments, src/mapsearch.c, of
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

//! The label cache against MapServer's pairwise test (ADR-0032): labels of arbitrary boxes, of no
//! number, infinite, inside out, and larger than a cell among them, go through the cache and
//! through a reference that tests every marker and every drawn label, pair by pair, as
//! `msTestLabelCacheCollisions` does, and the two must draw the same labels at the same positions
//! in the same order.

use forager_label::{
    Anchor, Bounds, MAX_PRIORITY, Member, Members, Offsets, PointLabel, Position, TextBox,
    draw_label_cache, marker_bounds, place,
};
use forager_render::Point;
use forager_style_ir::LabelMarker;
use forager_style_ir::c::{ms_max, ms_min, rect_overlap};

/// The image: 5 by 4 cells of the grid's finest level, and four levels.
const SIZE: (u32, u32) = (300, 200);

/// Inputs drawn, and the seed they are drawn from.
const INPUTS: usize = 10_000;
const SEED: u64 = 0x2026_1031_0024;

/// Labels with the text each draws, as an index into a few texts, and the gutter in pixels.
struct Cache(Vec<(Member, u8)>, u32);

impl Members for Cache {
    fn count(&self) -> usize {
        self.0.len()
    }
    fn priority(&self, index: usize) -> u8 {
        self.0[index].0.priority
    }
    fn member(&self, index: usize) -> Member {
        self.0[index].0
    }
    fn same_text(&self, a: usize, b: usize) -> bool {
        self.0[a].1 == self.0[b].1
    }
}

/// SplitMix64: a fixed seed's inputs, the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// A number below `n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// Whether an event of `percent` in a hundred happens.
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    /// A number from `lo` to `hi`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
    /// A coordinate that is no number or infinite.
    fn special(&mut self) -> f64 {
        [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, -1e300][self.below(5) as usize]
    }
    /// `value`, or in `percent` of a hundred a special one.
    fn or_special(&mut self, percent: u64, value: f64) -> f64 {
        if self.chance(percent) {
            self.special()
        } else {
            value
        }
    }
}

const ANCHORS: [Anchor; 10] = [
    Anchor::Ul,
    Anchor::Uc,
    Anchor::Ur,
    Anchor::Cl,
    Anchor::Cc,
    Anchor::Cr,
    Anchor::Ll,
    Anchor::Lc,
    Anchor::Lr,
    Anchor::Other,
];

/// A label of an arbitrary box: mostly small and near the image, and now and then of no number,
/// infinite, turned, inside out by a negative buffer, or larger than a cell or the image.
fn member(rng: &mut Rng) -> (Member, u8) {
    let point = Point {
        x: {
            let x = rng.range(-40.0, 340.0);
            rng.or_special(3, x)
        },
        y: {
            let y = rng.range(-40.0, 240.0);
            rng.or_special(3, y)
        },
    };
    // A negative width or height turns the box inside out along that axis alone, and the other
    // axis is then long, so that boxes inside out along different axes come to meet.
    let (width, height) = match rng.below(20) {
        0..=2 => (rng.range(-200.0, -1.0), rng.range(30.0, 400.0)),
        3..=5 => (rng.range(30.0, 400.0), rng.range(-200.0, -1.0)),
        6..=13 => (rng.range(0.0, 60.0), rng.range(0.0, 30.0)),
        14..=16 => (rng.range(60.0, 300.0), rng.range(0.0, 30.0)),
        _ => (rng.range(300.0, 2000.0), rng.range(60.0, 1000.0)),
    };
    let (min_x, max_y) = (rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
    let (min_x, max_y) = (rng.or_special(2, min_x), rng.or_special(2, max_y));
    let text = TextBox {
        min_x,
        min_y: rng.or_special(2, max_y - height),
        max_x: rng.or_special(2, min_x + width),
        max_y,
        num_lines: 1 + rng.below(3) as usize,
    };
    let offsets = Offsets {
        marker: if rng.chance(30) {
            [rng.range(0.0, 10.0), rng.range(0.0, 10.0)]
        } else {
            [0.0, 0.0]
        },
        label: if rng.chance(30) {
            [rng.below(21) as i32 - 10, rng.below(21) as i32 - 10]
        } else {
            [0, 0]
        },
    };
    let rotation = match rng.below(20) {
        0..=15 => 0.0,
        16..=18 => rng.range(0.0, std::f64::consts::TAU),
        _ => rng.special(),
    };
    let buffer = match rng.below(10) {
        0..=6 => 0,
        7..=8 => rng.below(6) as i32,
        _ => -(1 + rng.below(300) as i32),
    };
    let position = if rng.chance(30) {
        Position::Auto
    } else {
        Position::At(ANCHORS[rng.below(10) as usize])
    };
    let label = (!rng.chance(5)).then_some(PointLabel {
        position,
        text,
        offsets,
        rotation,
        buffer,
    });
    // FORCE: the label's draw, taken by a member with a label; both drawn to keep the cases.
    let label_force = rng.chance(15);
    let marker = rng.chance(40).then(|| {
        let (w, h) = (rng.range(0.0, 40.0), rng.range(0.0, 40.0));
        let mut m = [
            point.x - w / 2.0,
            point.y - h / 2.0,
            point.x + w / 2.0,
            point.y + h / 2.0,
        ];
        // Inside out along one axis, a marker is long along the other.
        let long = rng.range(20.0, 300.0);
        match rng.below(10) {
            0 => m[rng.below(4) as usize] = rng.special(),
            1..=2 => m = [m[2], point.y - long, m[0], point.y + long],
            3..=4 => m = [point.x - long, m[3], point.x + long, m[1]],
            5 => m = [-1e4, -1e4, 1e4, 1e4],
            _ => {}
        }
        m
    });
    let label_marker = label_marker(rng, point);
    let member = Member {
        priority: 1 + rng.below(3) as u8,
        point,
        label,
        // A member without a label takes its own draw, drawn first so that it is always drawn.
        force: rng.chance(15) && label.is_none() || label_force && label.is_some(),
        partials: rng.chance(50),
        test_point: rng.chance(40),
        marker,
        label_marker,
        min_distance: if rng.chance(20) {
            rng.range(1.0, 100.0)
        } else {
            0.0
        },
    };
    (member, rng.below(3) as u8)
}

/// The marker of a labelpoint style at `point`, or none: a box about the point, now and then
/// turned, of no number, or inside out.
fn label_marker(rng: &mut Rng, point: Point) -> Option<Bounds> {
    rng.chance(30).then(|| {
        let size = (rng.range(0.0, 40.0), rng.range(0.0, 40.0));
        let anchor = [rng.range(0.0, 1.0), rng.range(0.0, 1.0)];
        let angle = if rng.chance(20) {
            rng.range(0.0, 360.0)
        } else {
            0.0
        };
        let offset = [rng.range(-10.0, 10.0), rng.range(-10.0, 10.0)];
        let marker = LabelMarker {
            size,
            anchor_point: anchor,
            angle,
            offset,
        };
        let mut bounds = marker_bounds(point, &marker);
        match rng.below(10) {
            0 => bounds.bbox[rng.below(4) as usize] = rng.special(),
            1 => bounds.bbox.swap(0, 2),
            _ => {}
        }
        bounds
    })
}

/// The labels the cache draws, in order, with the position each takes.
fn through_cache(cache: &Cache) -> Vec<(usize, Anchor)> {
    let mut out = Vec::new();
    draw_label_cache(cache, SIZE, cache.1, |i, p| out.push((i, p.anchor)));
    out
}

/// The labels MapServer draws, in order: `msDrawLabelCache`'s walk from the highest priority
/// down and, within a priority, from the last added, with every test pair by pair.
fn pairwise(cache: &Cache) -> Vec<(usize, Anchor)> {
    let members = &cache.0;
    let mut drawn: Vec<(usize, Option<Bounds>)> = Vec::new();
    let mut out = Vec::new();
    for priority in (1..=MAX_PRIORITY).rev() {
        for i in (0..members.len()).rev() {
            let m = members[i].0;
            if m.priority != priority {
                continue;
            }
            let current = usize::from(priority) - 1;
            let Some(label) = m.label else {
                // A label without text, of its labelpoint marker alone: the marker tested unless
                // the label is forced, the text and the minimum distance skipped.
                let Some(marker) = &m.label_marker else {
                    continue;
                };
                if m.force || collisions(cache, &drawn, &m, marker, current, i) {
                    drawn.push((i, None));
                    out.push((i, Anchor::Cc));
                }
                continue;
            };
            // The marker of a labelpoint style, unless the label is forced; else the label
            // point, tested at MS_MAX_LABEL_PRIORITY: against no marker.
            if let Some(marker) = &m.label_marker {
                if !m.force && !collisions(cache, &drawn, &m, marker, current, i) {
                    continue;
                }
            } else if m.test_point {
                let p = m.point;
                let point = Bounds {
                    poly: None,
                    bbox: [p.x - 0.1, p.y - 0.1, p.x + 0.1, p.y + 0.1],
                };
                if !collisions(cache, &drawn, &m, &point, usize::from(MAX_PRIORITY), i) {
                    continue;
                }
            }
            let Some(placement) = place(m.point, &label, m.force, |lb| {
                collisions(cache, &drawn, &m, lb, current, i)
            }) else {
                continue;
            };
            let bbox = global_bbox(&placement.bounds, m.label_marker.as_ref());
            if min_distance(cache, &drawn, i, &bbox) {
                continue;
            }
            drawn.push((i, Some(placement.bounds)));
            out.push((i, placement.anchor));
        }
    }
    out
}

/// msDrawLabelCache's global label bbox of a label of one text: its text's box, then by `MS_MIN`
/// and `MS_MAX` its labelpoint marker's.
fn global_bbox(text: &Bounds, marker: Option<&Bounds>) -> [f64; 4] {
    let mut bbox = text.bbox;
    if let Some(m) = marker {
        let min = |a: f64, b: f64| if a < b { a } else { b };
        let max = |a: f64, b: f64| if a > b { a } else { b };
        bbox = [
            min(bbox[0], m.bbox[0]),
            min(bbox[1], m.bbox[1]),
            max(bbox[2], m.bbox[2]),
            max(bbox[3], m.bbox[3]),
        ];
    }
    bbox
}

/// `msTestLabelCacheCollisions` with the cache's gutter and no leader: false where `lb` leaves the
/// image of a label not partial, meets a marker of slot `current_priority` or above but the
/// label's own, or, within a drawn label's global bbox, meets its text or its labelpoint marker.
fn collisions(
    cache: &Cache,
    drawn: &[(usize, Option<Bounds>)],
    m: &Member,
    lb: &Bounds,
    current_priority: usize,
    current_label: usize,
) -> bool {
    if !m.partials && !label_in_image(SIZE, cache.1, lb) {
        return false;
    }
    for p in current_priority..usize::from(MAX_PRIORITY) {
        for (id, (other, _)) in cache.0.iter().enumerate() {
            if usize::from(other.priority) - 1 != p {
                continue;
            }
            let Some(marker) = other.marker else {
                continue;
            };
            if !(p == current_priority && current_label == id)
                && intersect_label_polygons(None, &marker, lb.poly.as_ref(), &lb.bbox)
            {
                return false;
            }
        }
    }
    for (j, theirs) in drawn {
        let marker = cache.0[*j].0.label_marker;
        // A label without text has its marker's box.
        let bbox = match (theirs, &marker) {
            (Some(theirs), marker) => global_bbox(theirs, marker.as_ref()),
            (None, Some(marker)) => marker.bbox,
            (None, None) => continue,
        };
        if !rect_overlap(bbox, lb.bbox) {
            continue;
        }
        if let Some(theirs) = theirs
            && intersect_label_polygons(
                theirs.poly.as_ref(),
                &theirs.bbox,
                lb.poly.as_ref(),
                &lb.bbox,
            )
        {
            return false;
        }
        if let Some(m) = &marker
            && intersect_label_polygons(m.poly.as_ref(), &m.bbox, lb.poly.as_ref(), &lb.bbox)
        {
            return false;
        }
    }
    true
}

/// `msCheckLabelMinDistance` for a label of a point: whether a drawn label of the same text
/// stands, by its point or its box, within the label's minimum distance of `bbox`.
fn min_distance(
    cache: &Cache,
    drawn: &[(usize, Option<Bounds>)],
    i: usize,
    bbox: &[f64; 4],
) -> bool {
    let m = cache.0[i].0;
    assert!(m.label.is_some(), "a placed label has a text");
    if m.min_distance <= 0.0 || m.force {
        return false;
    }
    let d = m.min_distance;
    let buffered = [bbox[0] - d, bbox[1] - d, bbox[2] + d, bbox[3] + d];
    drawn.iter().any(|(j, theirs)| {
        // A label without text is not measured from.
        let Some(theirs) = theirs else {
            return false;
        };
        let bbox = global_bbox(theirs, cache.0[*j].0.label_marker.as_ref());
        cache.same_text(i, *j)
            && (point_in_rect(&cache.0[*j].0.point, &buffered)
                || intersect_label_polygons(None, &bbox, None, &buffered))
    })
}

/// `labelInImage` (src/maplabel.c:636-663), its int `gutter` compared with the doubles of the box
/// and taken from the image's int width and height.
fn label_in_image((width, height): (u32, u32), gutter: u32, lb: &Bounds) -> bool {
    let g = f64::from(gutter);
    let (right, bottom) = (f64::from(width) - g, f64::from(height) - g);
    let b = &lb.bbox;
    if b[0] >= g && b[1] >= g && b[2] < right && b[3] < bottom {
        return true;
    }
    let Some(poly) = &lb.poly else {
        return false;
    };
    for p in &poly[1..] {
        if p.x < g || p.x >= right || p.y < g || p.y >= bottom {
            return false;
        }
    }
    true
}

/// `msPointInRect`.
fn point_in_rect(p: &Point, rect: &[f64; 4]) -> bool {
    !(p.x < rect[0] || p.x > rect[2] || p.y < rect[1] || p.y > rect[3])
}

/// `intersectLabelPolygons`.
fn intersect_label_polygons(
    l1: Option<&[Point; 5]>,
    r1: &[f64; 4],
    l2: Option<&[Point; 5]>,
    r2: &[f64; 4],
) -> bool {
    if !rect_overlap(*r1, *r2) {
        return false;
    }
    if l1.is_none() && l2.is_none() {
        return true;
    }
    let ring = |r: &[f64; 4]| {
        let p = |x, y| Point { x, y };
        [
            p(r[0], r[1]),
            p(r[0], r[3]),
            p(r[2], r[3]),
            p(r[2], r[1]),
            p(r[0], r[1]),
        ]
    };
    let p1 = l1.copied().unwrap_or_else(|| ring(r1));
    let p2 = l2.copied().unwrap_or_else(|| ring(r2));
    for v1 in 1..p1.len() {
        for v2 in 1..p2.len() {
            if intersect_segments(&p1[v1 - 1], &p1[v1], &p2[v2 - 1], &p2[v2]) {
                return true;
            }
        }
    }
    point_in_polygon(&p2[0], &p1) || point_in_polygon(&p1[0], &p2)
}

/// `msPointInPolygon`, W. Randolph Franklin's pnpoly, under his notice, which `src/mapsearch.c`
/// keeps beside it and THIRD-PARTY-NOTICES.md reproduces for the binaries:
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
fn point_in_polygon(p: &Point, c: &[Point; 5]) -> bool {
    let mut status = false;
    let mut j = c.len() - 1;
    for i in 0..c.len() {
        if ((c[i].y <= p.y && p.y < c[j].y) || (c[j].y <= p.y && p.y < c[i].y))
            && p.x < (c[j].x - c[i].x) * (p.y - c[i].y) / (c[j].y - c[i].y) + c[i].x
        {
            status = !status;
        }
        j = i;
    }
    status
}

/// `msIntersectSegments`.
fn intersect_segments(a: &Point, b: &Point, c: &Point, d: &Point) -> bool {
    let numerator = (a.y - c.y) * (d.x - c.x) - (a.x - c.x) * (d.y - c.y);
    let denominator = (b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x);
    if denominator == 0.0 && numerator == 0.0 {
        if a.y == c.y {
            return (a.x >= ms_min(c.x, d.x) && a.x <= ms_max(c.x, d.x))
                || (b.x >= ms_min(c.x, d.x) && b.x <= ms_max(c.x, d.x));
        }
        return (a.y >= ms_min(c.y, d.y) && a.y <= ms_max(c.y, d.y))
            || (b.y >= ms_min(c.y, d.y) && b.y <= ms_max(c.y, d.y));
    }
    if denominator == 0.0 {
        return false;
    }
    let r = numerator / denominator;
    if r < 0.0 || r > 1.0 {
        return false;
    }
    let numerator = (a.y - c.y) * (b.x - a.x) - (a.x - c.x) * (b.y - a.y);
    let s = numerator / denominator;
    !(s < 0.0 || s > 1.0)
}

#[test]
fn the_cache_draws_what_the_pairwise_test_draws_on_arbitrary_labels() {
    // Verifies: ADR-0032
    let mut rng = Rng(SEED);
    let mut drawn = 0;
    for input in 0..INPUTS {
        let count = 1 + rng.below(16) as usize;
        let members = (0..count).map(|_| member(&mut rng)).collect();
        // Half the inputs have no gutter, the others one of 0 to 40 pixels.
        let gutter = if rng.chance(50) {
            0
        } else {
            rng.below(41) as u32
        };
        let cache = Cache(members, gutter);
        let expected = pairwise(&cache);
        assert_eq!(
            through_cache(&cache),
            expected,
            "input {input} of seed {SEED:#x}: {:?}",
            cache.0
        );
        drawn += expected.len();
    }
    // The inputs draw labels and leave labels out, so the comparison decides something.
    assert!(drawn > INPUTS && drawn < INPUTS * 8, "{drawn} labels drawn");
}

/// The seed of the inputs some of whose labels lie spans of the image away from it.
const FAR_SEED: u64 = 0x2026_1114_0025;

/// `member` moved, in half the cases along each axis, by 1 to 4 spans of the image to either side
/// and a remainder that is no whole cell: its point, its marker's box, and its labelpoint
/// marker's bounds alike, so that it stands as it stood, far from the image. Whether it moved.
fn far(rng: &mut Rng, mut member: Member) -> (Member, bool) {
    let mut along = |span: u32| {
        if rng.chance(50) {
            return 0.0;
        }
        let k = 1.0 + rng.below(4) as f64;
        let sign = if rng.chance(50) { 1.0 } else { -1.0 };
        sign * k * f64::from(span) + rng.range(1.0, 63.0)
    };
    let (dx, dy) = (along(SIZE.0), along(SIZE.1));
    let shift = |b: &mut [f64; 4]| {
        b[0] += dx;
        b[1] += dy;
        b[2] += dx;
        b[3] += dy;
    };
    member.point = Point {
        x: member.point.x + dx,
        y: member.point.y + dy,
    };
    if let Some(marker) = member.marker.as_mut() {
        shift(marker);
    }
    if let Some(bounds) = member.label_marker.as_mut() {
        shift(&mut bounds.bbox);
        for p in bounds.poly.iter_mut().flatten() {
            *p = Point {
                x: p.x + dx,
                y: p.y + dy,
            };
        }
    }
    (member, dx != 0.0 || dy != 0.0)
}

#[test]
fn the_cache_draws_what_the_pairwise_test_draws_on_labels_spans_of_the_image_away() {
    // Verifies: ADR-0039
    // The grid wraps around the image, so labels several spans of it away share its cells with
    // labels near it; the pairs that meet must still be found, those that do not still not.
    let mut rng = Rng(FAR_SEED);
    let (mut drawn, mut drawn_far) = (0, 0);
    for input in 0..INPUTS {
        let count = 1 + rng.below(16) as usize;
        let mut moved = Vec::with_capacity(count);
        let members = (0..count)
            .map(|_| {
                let (member, text) = member(&mut rng);
                let (member, away) = far(&mut rng, member);
                moved.push(away);
                (member, text)
            })
            .collect();
        let gutter = if rng.chance(50) {
            0
        } else {
            rng.below(41) as u32
        };
        let cache = Cache(members, gutter);
        let expected = pairwise(&cache);
        assert_eq!(
            through_cache(&cache),
            expected,
            "input {input} of seed {FAR_SEED:#x}: {:?}",
            cache.0
        );
        drawn += expected.len();
        drawn_far += expected.iter().filter(|(i, _)| moved[*i]).count();
    }
    // Labels far from the image are drawn and left out among the others, so the comparison
    // decides something for them.
    assert!(drawn > INPUTS && drawn < INPUTS * 8, "{drawn} labels drawn");
    assert!(drawn_far > INPUTS, "{drawn_far} labels drawn far away");
}
