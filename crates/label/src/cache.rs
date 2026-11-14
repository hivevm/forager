// The label cache follows msDrawLabelCache and msCheckLabelMinDistance, src/mapdraw.c,
// msAddLabelGroup, labelInImage, msTestLabelCacheCollisions, and intersectLabelPolygons,
// src/maplabel.c, and msRectOverlap, msPointInRect, msPointInPolygon, and msIntersectSegments,
// src/mapsearch.c, of MapServer at the release pinned in scripts/fetch-msautotest.sh.
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

//! MapServer's label cache for labels of points: which labels are drawn, where, and in what
//! order, from the boxes of their texts and of their `labelpoint` markers.
//!
//! The labels are walked from the highest priority down and, within a priority, from the last
//! added to the first. A label is tested against the image, unless it may be partial, against
//! the markers its priority and higher ones cached, its own apart, and against every label drawn
//! before it, the box of its text and the marker of its `labelpoint` style; such a marker is
//! tested so before the text. A label is drawn where it fits, or where it is forced, and left
//! out where a drawn label of the same text lies within its minimum distance.
//!
//! The tests are MapServer's, pair by pair; which pairs are tested is bounded by a loose grid
//! over the image (Q-9): every marker and every drawn label is held once by its box, and a test
//! visits each entry near its box at most once, and none that cannot meet it by lying inside out
//! along the same axis. Where a member has a `MINDISTANCE`, a drawn label with a text is held a
//! second time, by its point, in a grid of its own, and that test may visit it once in each.

use forager_render::Point;
use forager_style_ir::c::{ms_max, ms_min, rect_overlap};

use crate::place::{Anchor, Bounds, Placement, PointLabel, member_box, place, placed_at};

/// MapServer's `MS_MAX_LABEL_PRIORITY`: priorities run from 1 to it.
pub const MAX_PRIORITY: u8 = 10;

/// The most labels the label cache of one render holds (Q-9, ADR-0032); MapServer holds any
/// number. A label added past it is left out, untested.
pub const MAX_LABELS: usize = 100_000;

/// The side of a cell of the finest level of the grids that bound the tests, in pixels.
const CELL: f64 = 64.0;

/// A label of the cache as it is placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    /// From 1 to [`MAX_PRIORITY`].
    pub priority: u8,
    /// The point the label stands at, in pixels.
    pub point: Point,
    /// The label; `None` for a label without a box of text. With a `label_marker`, it is a label
    /// without text, of its `labelpoint` marker alone: the marker is tested unless `force`, and
    /// the member's box is the marker's. Without one, a text of line breaks only, which is not
    /// drawn, but whose marker is cached.
    pub label: Option<PointLabel>,
    /// `FORCE`: the label is drawn where it does not fit, its `labelpoint` marker untested, and
    /// is not held apart by its minimum distance.
    pub force: bool,
    /// `PARTIALS TRUE`: the label may leave the image.
    pub partials: bool,
    /// The point itself is tested before the text, as for a label neither forced nor offset.
    pub test_point: bool,
    /// The box of the marker drawn at the point, `[min x, min y, max x, max y]`, as the marker
    /// cache holds it.
    pub marker: Option<[f64; 4]>,
    /// The bounds of the marker of the label's first `GEOMTRANSFORM labelpoint` style, at the
    /// point, as `computeMarkerBounds` computes them: tested before the text, and taking room
    /// from the labels after it.
    pub label_marker: Option<Bounds>,
    /// `MINDISTANCE` in pixels; none at 0 or below.
    pub min_distance: f64,
}

/// The box of the marker `msAddLabelGroup` caches beneath a label at `point`, of the width and
/// height `size`: about the point, its far corner `w - 1` and `h - 1` from its near one.
pub fn cached_marker_box(point: Point, (w, h): (f64, f64)) -> [f64; 4] {
    let (min_x, min_y) = (point.x - 0.5 * w, point.y - 0.5 * h);
    [min_x, min_y, min_x + (w - 1.0), min_y + (h - 1.0)]
}

/// `msDrawLabelCache`'s offset of a label from its markers: half the size of the box `marker`
/// the marker cache holds, or of its `labelpoint` marker `label_marker`'s where that is larger,
/// grown by `outline_grow`, half the label's outline, where it has one and the offset across is
/// not 0.
pub fn marker_offset(
    marker: Option<&[f64; 4]>,
    label_marker: Option<&Bounds>,
    outline_grow: Option<f64>,
) -> [f64; 2] {
    let half =
        |[min_x, min_y, max_x, max_y]: &[f64; 4]| [(max_x - min_x) / 2.0, (max_y - min_y) / 2.0];
    let mut offset = marker.map_or([0.0, 0.0], half);
    if let Some(m) = label_marker {
        let [x, y] = half(&m.bbox);
        offset = [ms_max(offset[0], x), ms_max(offset[1], y)];
    }
    match outline_grow {
        Some(grow) if offset[0] != 0.0 => offset.map(|o| o + grow),
        _ => offset,
    }
}

/// The labels of a cache, in the order they were added.
pub trait Members {
    fn count(&self) -> usize;
    fn priority(&self, index: usize) -> u8;
    fn member(&self, index: usize) -> Member;
    /// Whether two labels have the same text, as `strcmp` compares them.
    fn same_text(&self, a: usize, b: usize) -> bool;
}

/// Walks `members` as `msDrawLabelCache` walks its slots, and hands `draw` each label drawn with
/// its placement, in the order MapServer draws them, into an image of `size` pixels with a
/// `gutter` along its edges; see [`walk_label_cache`], whose count of the members left out past
/// [`MAX_LABELS`] it returns.
pub fn draw_label_cache<M: Members + ?Sized>(
    members: &M,
    size: (u32, u32),
    gutter: u32,
    mut draw: impl FnMut(usize, &Placement),
) -> usize {
    walk_label_cache(members, size, gutter, |i, filed| {
        if let Some((placement, _)) = filed {
            draw(i, placement);
        }
    })
}

/// Walks the first [`MAX_LABELS`] of `members` as `msDrawLabelCache` walks its slots, from the
/// highest priority down and within a priority from the last added to the first, into an image of
/// `size` pixels whose `gutter`, `labelcache_map_edge_buffer`, no label reaches into that may not
/// be partial and is tested, and hands `visit` each member it visits, in that order, with its
/// placement and the box it filed it by, its [`member_box`], where it is drawn, and `None` where
/// it is left out. Returns how many members past [`MAX_LABELS`] it left out untested (Q-9).
pub fn walk_label_cache<M: Members + ?Sized>(
    members: &M,
    size: (u32, u32),
    gutter: u32,
    mut visit: impl FnMut(usize, Option<(&Placement, [f64; 4])>),
) -> usize {
    let taken = members.count().min(MAX_LABELS);
    let image = [f64::from(size.0), f64::from(size.1)];
    let mut markers: Vec<Grid<u32>> = (0..MAX_PRIORITY).map(|_| Grid::new(size)).collect();
    let mut min_distance = false;
    for i in 0..taken {
        let member = members.member(i);
        min_distance |= member.min_distance > 0.0;
        if let Some(marker) = member.marker {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a member's index, below MAX_LABELS"
            )]
            markers[slot(member.priority)].insert(&marker, i as u32);
        }
    }
    let mut cache = Cache {
        members,
        image,
        gutter: f64::from(gutter),
        markers,
        boxes: Grid::new(size),
        points: min_distance.then(|| Grid::new(size)),
    };
    for priority in (1..=MAX_PRIORITY).rev() {
        for i in (0..taken).rev() {
            if members.priority(i) != priority {
                continue;
            }
            let member = members.member(i);
            let placement = cache.place(i, &member);
            let filed = placement.as_ref().map(|placement| {
                let bbox = cache.file(i, &member, placement);
                (placement, bbox)
            });
            visit(i, filed);
        }
    }
    members.count() - taken
}

/// The index of a priority's slot.
fn slot(priority: u8) -> usize {
    usize::from(priority.clamp(1, MAX_PRIORITY) - 1)
}

struct Cache<'m, M: ?Sized> {
    members: &'m M,
    /// The image's width and height.
    image: [f64; 2],
    /// The pixels along the image's edges no tested label reaches into that may not be partial.
    gutter: f64,
    /// The markers of each slot, by the member's index.
    markers: Vec<Grid<u32>>,
    /// The labels drawn, by their members' boxes alone (ADR-0032), with the position each stands
    /// at.
    boxes: Grid<(u32, Anchor)>,
    /// The labels drawn with a text, by the points they stand at, for the `MINDISTANCE` test;
    /// none where no member has a minimum distance.
    points: Option<Grid<(u32, Anchor)>>,
}

impl<M: Members + ?Sized> Cache<'_, M> {
    /// Files member `i`, drawn at `placement`, by its box, and by its point where it has a text;
    /// returns its box.
    fn file(&mut self, i: usize, member: &Member, placement: &Placement) -> [f64; 4] {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a member's index, below MAX_LABELS"
        )]
        let entry = (i as u32, placement.anchor);
        let bbox = member_box(&placement.bounds.bbox, member.label_marker.as_ref());
        self.boxes.insert(&bbox, entry);
        if let Some(points) = self.points.as_mut().filter(|_| member.label.is_some()) {
            let p = member.point;
            points.insert(&[p.x, p.y, p.x, p.y], entry);
        }
        bbox
    }

    /// Places member `i`, or `None` where it is left out.
    fn place(&self, i: usize, member: &Member) -> Option<Placement> {
        let Some(label) = member.label else {
            // A label without text, of its marker alone: msDrawLabelCache tests the marker
            // unless the label is forced, and skips the text and the minimum distance
            // (mapdraw.c:3607-3650, 4035-4037).
            let marker = member.label_marker?;
            if !member.force && !self.fits(member, &marker, Some((member.priority, i))) {
                return None;
            }
            return Some(Placement {
                origin: member.point,
                bounds: marker,
                anchor: Anchor::Cc,
            });
        };
        // The marker of a labelpoint style is tested first, at the label's priority, unless the
        // label is forced; where it has one, the point is not tested.
        if let Some(marker) = &member.label_marker {
            if !member.force && !self.fits(member, marker, Some((member.priority, i))) {
                return None;
            }
        } else if member.test_point {
            // The point alone, tested at a priority above every slot's: against no marker.
            let p = member.point;
            let point = Bounds {
                poly: None,
                bbox: [p.x - 0.1, p.y - 0.1, p.x + 0.1, p.y + 0.1],
            };
            if !self.fits(member, &point, None) {
                return None;
            }
        }
        let placement = place(member.point, &label, member.force, |bounds| {
            self.fits(member, bounds, Some((member.priority, i)))
        })?;
        if member.min_distance > 0.0
            && !member.force
            && self.too_close(
                i,
                &member_box(&placement.bounds.bbox, member.label_marker.as_ref()),
                member.min_distance,
            )
        {
            return None;
        }
        Some(placement)
    }

    /// `msTestLabelCacheCollisions`: whether `lb` lies in the image, where the member may not be
    /// partial, and meets no marker of `markers`' priority and above but the member's own, and no
    /// label drawn whose box it meets by its text's box or its `labelpoint` marker.
    fn fits(&self, member: &Member, lb: &Bounds, markers: Option<(u8, usize)>) -> bool {
        if !member.partials && !in_image(self.image, self.gutter, lb) {
            return false;
        }
        if let Some((priority, own)) = markers {
            for p in slot(priority)..usize::from(MAX_PRIORITY) {
                let hit = self.markers[p].any(&lb.bbox, |&j| {
                    let theirs = !(p == slot(priority) && j as usize == own);
                    let Some(marker) = self.members.member(j as usize).marker else {
                        return false;
                    };
                    theirs && intersect(None, &marker, lb.poly.as_ref(), &lb.bbox)
                });
                if hit {
                    return false;
                }
            }
        }
        let meets = |theirs: &Bounds| {
            intersect(
                theirs.poly.as_ref(),
                &theirs.bbox,
                lb.poly.as_ref(),
                &lb.bbox,
            )
        };
        !self.boxes.any(&lb.bbox, |&(j, anchor)| {
            let theirs = self.members.member(j as usize);
            let marker = theirs.label_marker;
            let text = text_bounds(&theirs, anchor);
            let bbox = match (&text, &marker) {
                (Some(text), marker) => member_box(&text.bbox, marker.as_ref()),
                (None, Some(marker)) => marker.bbox,
                (None, None) => return false,
            };
            rect_overlap(bbox, lb.bbox)
                && (text.as_ref().is_some_and(meets) || marker.as_ref().is_some_and(meets))
        })
    }

    /// `msCheckLabelMinDistance`: whether a label drawn with the text of member `i` stands, by its
    /// point or its member's box, within `distance` of `bbox`, member `i`'s box. The drawn labels
    /// are looked up by their points and by their boxes, so one may be visited twice, each lookup
    /// visiting it at most once.
    fn too_close(&self, i: usize, bbox: &[f64; 4], distance: f64) -> bool {
        let buffered = [
            bbox[0] - distance,
            bbox[1] - distance,
            bbox[2] + distance,
            bbox[3] + distance,
        ];
        let close = |&(j, anchor): &(u32, Anchor)| {
            let j = j as usize;
            // A label without text is not measured from (mapdraw.c:3384-3385).
            if !self.members.same_text(i, j) {
                return false;
            }
            let theirs = self.members.member(j);
            let Some(text) = text_bounds(&theirs, anchor) else {
                return false;
            };
            point_in_rect(&theirs.point, &buffered)
                || rect_overlap(
                    member_box(&text.bbox, theirs.label_marker.as_ref()),
                    buffered,
                )
        };
        self.points
            .as_ref()
            .is_some_and(|points| points.any(&buffered, close))
            || self.boxes.any(&buffered, close)
    }
}

/// The box of the text of the drawn `member` at `anchor`, computed again as it was placed; `None`
/// for a label without text.
fn text_bounds(member: &Member, anchor: Anchor) -> Option<Bounds> {
    let label = member.label?;
    Some(placed_at(member.point, &label, anchor).1)
}

/// `labelInImage`: whether the box, or where it is turned its corners, lie in the image at least
/// `gutter` pixels inside each edge (src/maplabel.c:636-663).
fn in_image([width, height]: [f64; 2], gutter: f64, lb: &Bounds) -> bool {
    let [min_x, min_y, max_x, max_y] = lb.bbox;
    let (right, bottom) = (width - gutter, height - gutter);
    if min_x >= gutter && min_y >= gutter && max_x < right && max_y < bottom {
        return true;
    }
    match &lb.poly {
        Some(poly) => poly[1..]
            .iter()
            .all(|p| !(p.x < gutter || p.x >= right || p.y < gutter || p.y >= bottom)),
        None => false,
    }
}

/// `msPointInRect`.
fn point_in_rect(p: &Point, r: &[f64; 4]) -> bool {
    !(p.x < r[0] || p.x > r[2] || p.y < r[1] || p.y > r[3])
}

/// The ring `intersectLabelPolygons` makes of a box without a polygon.
fn ring(r: &[f64; 4]) -> [Point; 5] {
    let p = |x, y| Point { x, y };
    [
        p(r[0], r[1]),
        p(r[0], r[3]),
        p(r[2], r[3]),
        p(r[2], r[1]),
        p(r[0], r[1]),
    ]
}

/// `intersectLabelPolygons`: whether two boxes, each with its turned corners where it has them,
/// meet.
fn intersect(
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

/// `msIntersectSegments`: whether the segments a–b and c–d meet.
fn intersect_segments(a: &Point, b: &Point, c: &Point, d: &Point) -> bool {
    let numerator = (a.y - c.y) * (d.x - c.x) - (a.x - c.x) * (d.y - c.y);
    let denominator = (b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x);
    if denominator == 0.0 && numerator == 0.0 {
        // Coincident: they meet where they overlap.
        return if a.y == c.y {
            (a.x >= ms_min(c.x, d.x) && a.x <= ms_max(c.x, d.x))
                || (b.x >= ms_min(c.x, d.x) && b.x <= ms_max(c.x, d.x))
        } else {
            (a.y >= ms_min(c.y, d.y) && a.y <= ms_max(c.y, d.y))
                || (b.y >= ms_min(c.y, d.y) && b.y <= ms_max(c.y, d.y))
        };
    }
    if denominator == 0.0 {
        return false;
    }
    let r = numerator / denominator;
    if r < 0.0 || r > 1.0 {
        return false;
    }
    let s = ((a.y - c.y) * (b.x - a.x) - (a.x - c.x) * (b.y - a.y)) / denominator;
    !(s < 0.0 || s > 1.0)
}

/// The extent of a box along one axis as a grid files or looks it up: `[lo, hi]`, or for an
/// inside-out box, whose minimum exceeds its maximum, its smaller edge alone, with `true`. A box
/// `msRectOverlap` finds to meet an inside-out one spans it, its smaller edge among it; two boxes
/// inside out along the same axis never meet.
fn extent(min: f64, max: f64) -> (f64, f64, bool) {
    if min <= max {
        (min, max, false)
    } else {
        (max, max, true)
    }
}

/// Entries by where their boxes lie, so that a test visits only the entries that can meet its
/// box, each once, and each entry is held once (Q-9).
///
/// It is a loose grid of levels over the image: level `k` has square cells of side `64 · 2^k`,
/// up to the level of one cell over the image. An entry is held once, at the finest level whose
/// side its box, held to the image, does not exceed, in the cell of its lower corner, so it lies
/// within that cell and the next one along each axis; a box that meets it starts at most a cell
/// before. Coordinates past the image are held to its edge, which keeps every pair that meets.
/// Entries inside out along an axis are held apart and not visited by a box inside out along the
/// same axis, which they cannot meet. An entry with a coordinate that is no number, which
/// `msRectOverlap` finds to meet every box, is held apart and visited by every test, and a test
/// of such a box visits every entry. Nothing is allocated until the first entry.
struct Grid<E> {
    /// The image's width and height, at least a pixel.
    size: [f64; 2],
    /// By whether the box is inside out along x and along y: `x + 2 · y`.
    stores: [Option<Levels<E>>; 4],
    anywhere: Vec<E>,
}

struct Levels<E> {
    levels: Vec<Level<E>>,
}

struct Level<E> {
    side: f64,
    columns: usize,
    rows: usize,
    /// The entries held at this level, so that an empty level is passed over.
    count: usize,
    cells: Vec<Vec<E>>,
}

impl<E: Copy> Grid<E> {
    fn new((width, height): (u32, u32)) -> Self {
        Self {
            size: [f64::from(width).max(1.0), f64::from(height).max(1.0)],
            stores: [None, None, None, None],
            anywhere: Vec::new(),
        }
    }

    /// The box's extents held to the image, `[x0, y0, x1, y1]`, and the store it belongs to;
    /// `None` where a coordinate is no number.
    fn place(&self, bbox: &[f64; 4]) -> Option<([f64; 4], usize)> {
        if bbox.iter().any(|v| v.is_nan()) {
            return None;
        }
        let (x0, x1, x_out) = extent(bbox[0], bbox[2]);
        let (y0, y1, y_out) = extent(bbox[1], bbox[3]);
        let [w, h] = self.size;
        let held = [
            x0.clamp(0.0, w),
            y0.clamp(0.0, h),
            x1.clamp(0.0, w),
            y1.clamp(0.0, h),
        ];
        Some((held, usize::from(x_out) + 2 * usize::from(y_out)))
    }

    /// Holds `entry` for `bbox`.
    fn insert(&mut self, bbox: &[f64; 4], entry: E) {
        let Some((b, store)) = self.place(bbox) else {
            self.anywhere.push(entry);
            return;
        };
        let size = self.size;
        let levels = self.stores[store].get_or_insert_with(|| Levels::new(size));
        let level = levels
            .levels
            .iter_mut()
            .find(|l| b[2] - b[0] <= l.side && b[3] - b[1] <= l.side)
            .expect("the last level spans the image");
        let (x, y) = (
            level.cell(b[0], level.columns),
            level.cell(b[1], level.rows),
        );
        level.cells[y * level.columns + x].push(entry);
        level.count += 1;
    }

    /// Whether `hit` holds for an entry that can meet `bbox`.
    fn any(&self, bbox: &[f64; 4], mut hit: impl FnMut(&E) -> bool) -> bool {
        if self.anywhere.iter().any(&mut hit) {
            return true;
        }
        let Some((b, query)) = self.place(bbox) else {
            return self
                .stores
                .iter()
                .flatten()
                .flat_map(|levels| &levels.levels)
                .flat_map(|level| &level.cells)
                .flatten()
                .any(hit);
        };
        self.stores.iter().enumerate().any(|(store, levels)| {
            // Inside out along an axis, an entry meets no box inside out along it.
            if store & query != 0 {
                return false;
            }
            let Some(levels) = levels else {
                return false;
            };
            levels.levels.iter().filter(|l| l.count > 0).any(|level| {
                let xs = level.cell(b[0], level.columns).saturating_sub(1)
                    ..=level.cell(b[2], level.columns);
                let ys =
                    level.cell(b[1], level.rows).saturating_sub(1)..=level.cell(b[3], level.rows);
                ys.into_iter().any(|y| {
                    xs.clone()
                        .any(|x| level.cells[y * level.columns + x].iter().any(&mut hit))
                })
            })
        })
    }
}

impl<E> Levels<E> {
    fn new([width, height]: [f64; 2]) -> Self {
        let mut levels = Vec::new();
        let mut side = CELL;
        loop {
            // Coordinates run from 0 to the image's edge, that edge included.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a count of cells along the image, held to its side (Q-9)"
            )]
            let count = |extent: f64| (extent / side).floor() as usize + 1;
            let (columns, rows) = (count(width), count(height));
            levels.push(Level {
                side,
                columns,
                rows,
                count: 0,
                cells: (0..columns * rows).map(|_| Vec::new()).collect(),
            });
            if side >= width && side >= height {
                break;
            }
            side *= 2.0;
        }
        Self { levels }
    }
}

impl<E> Level<E> {
    /// The cell of a coordinate held to the image, along an axis of `n` cells.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a cell along the image; a coordinate past it is held to the last"
    )]
    fn cell(&self, v: f64, n: usize) -> usize {
        ((v / self.side).floor() as usize).min(n - 1)
    }
}
