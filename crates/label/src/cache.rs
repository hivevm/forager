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
//! order, from the boxes of their texts alone.
//!
//! The labels are walked from the highest priority down and, within a priority, from the last
//! added to the first. A label is tested against the image, unless it may be partial, against
//! the markers its priority and higher ones cached, its own apart, and against every label drawn
//! before it; it is drawn where it fits, or where it is forced, and left out where a drawn label
//! of the same text lies within its minimum distance.
//!
//! The tests are MapServer's, pair by pair; which pairs are tested is bounded by a loose grid
//! over the image (Q-9): every label and marker is held once, and a test visits each entry near
//! its box at most once, and none that cannot meet it by lying inside out along the same axis.

use forager_render::Point;

use crate::place::{Anchor, Bounds, Placement, PointLabel, place, placed_at};

/// MapServer's `MS_MAX_LABEL_PRIORITY`: priorities run from 1 to it.
pub const MAX_PRIORITY: u8 = 10;

/// The most labels the label cache of one render holds (Q-9); MapServer holds any
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
    /// The label; `None` for a text of line breaks only, which takes no box and is not drawn, but
    /// whose marker is cached.
    pub label: Option<PointLabel>,
    /// `PARTIALS TRUE`: the label may leave the image.
    pub partials: bool,
    /// The point itself is tested before the text, as for a label neither forced nor offset.
    pub test_point: bool,
    /// The box of the marker drawn at the point, `[min x, min y, max x, max y]`, as the marker
    /// cache holds it.
    pub marker: Option<[f64; 4]>,
    /// `MINDISTANCE` in pixels; none at 0 or below.
    pub min_distance: f64,
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
/// its placement, in the order MapServer draws them, into an image of `size` pixels. The caller
/// holds at most [`MAX_LABELS`] members.
pub fn draw_label_cache<M: Members + ?Sized>(
    members: &M,
    size: (u32, u32),
    mut draw: impl FnMut(usize, &Placement),
) {
    let image = [f64::from(size.0), f64::from(size.1)];
    let mut markers: Vec<Grid<u32>> = (0..MAX_PRIORITY).map(|_| Grid::new(size)).collect();
    for i in 0..members.count() {
        let member = members.member(i);
        if let Some(marker) = member.marker {
            markers[slot(member.priority)].insert(&marker, None, i as u32);
        }
    }
    let mut cache = Cache {
        members,
        image,
        markers,
        drawn: Grid::new(size),
    };
    for priority in (1..=MAX_PRIORITY).rev() {
        for i in (0..members.count()).rev() {
            if members.priority(i) != priority {
                continue;
            }
            if let Some(placement) = cache.place(i) {
                cache.drawn.insert(
                    &placement.bounds.bbox,
                    Some(members.member(i).point),
                    (i as u32, placement.anchor),
                );
                draw(i, &placement);
            }
        }
    }
}

/// The index of a priority's slot.
fn slot(priority: u8) -> usize {
    usize::from(priority.clamp(1, MAX_PRIORITY) - 1)
}

struct Cache<'m, M: ?Sized> {
    members: &'m M,
    /// The image's width and height.
    image: [f64; 2],
    /// The markers of each slot.
    markers: Vec<Grid<u32>>,
    /// The labels drawn, with the position each stands at.
    drawn: Grid<(u32, Anchor)>,
}

impl<M: Members + ?Sized> Cache<'_, M> {
    /// Places member `i`, or `None` where it is left out.
    fn place(&self, i: usize) -> Option<Placement> {
        let member = self.members.member(i);
        let label = member.label?;
        if member.test_point {
            // The point alone, tested at a priority above every slot's: against no marker.
            let p = member.point;
            let point = Bounds {
                poly: None,
                bbox: [p.x - 0.1, p.y - 0.1, p.x + 0.1, p.y + 0.1],
            };
            if !self.fits(&member, &point, None) {
                return None;
            }
        }
        let placement = place(member.point, &label, |bounds| {
            self.fits(&member, bounds, Some((member.priority, i)))
        })?;
        if member.min_distance > 0.0
            && !label.force
            && self.too_close(i, &placement.bounds.bbox, member.min_distance)
        {
            return None;
        }
        Some(placement)
    }

    /// `msTestLabelCacheCollisions`: whether `lb` lies in the image, where the member may not be
    /// partial, and meets no marker of `markers`' priority and above but the member's own, and no
    /// label drawn.
    fn fits(&self, member: &Member, lb: &Bounds, markers: Option<(u8, usize)>) -> bool {
        if !member.partials && !in_image(self.image, lb) {
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
        !self.drawn.any(&lb.bbox, |&(j, anchor)| {
            let theirs = self.bounds(j as usize, anchor);
            rect_overlap(&theirs.bbox, &lb.bbox)
                && intersect(
                    theirs.poly.as_ref(),
                    &theirs.bbox,
                    lb.poly.as_ref(),
                    &lb.bbox,
                )
        })
    }

    /// `msCheckLabelMinDistance`: whether a label drawn with the text of member `i` stands, by its
    /// point or its box, within `distance` of `bbox`.
    fn too_close(&self, i: usize, bbox: &[f64; 4], distance: f64) -> bool {
        let buffered = [
            bbox[0] - distance,
            bbox[1] - distance,
            bbox[2] + distance,
            bbox[3] + distance,
        ];
        self.drawn.any(&buffered, |&(j, anchor)| {
            let j = j as usize;
            self.members.same_text(i, j)
                && (point_in_rect(&self.members.member(j).point, &buffered)
                    || rect_overlap(&self.bounds(j, anchor).bbox, &buffered))
        })
    }

    /// The box of the drawn member `j` at `anchor`, computed again as it was placed.
    fn bounds(&self, j: usize, anchor: Anchor) -> Bounds {
        let member = self.members.member(j);
        let label = member.label.expect("a drawn label has a box");
        placed_at(member.point, &label, anchor).1
    }
}

/// `labelInImage` with no gutter: whether the box, or where it is turned its corners, lie in the
/// image.
fn in_image([width, height]: [f64; 2], lb: &Bounds) -> bool {
    let [min_x, min_y, max_x, max_y] = lb.bbox;
    if min_x >= 0.0 && min_y >= 0.0 && max_x < width && max_y < height {
        return true;
    }
    match &lb.poly {
        Some(poly) => poly[1..]
            .iter()
            .all(|p| !(p.x < 0.0 || p.x >= width || p.y < 0.0 || p.y >= height)),
        None => false,
    }
}

/// `msRectOverlap`.
fn rect_overlap(a: &[f64; 4], b: &[f64; 4]) -> bool {
    !(a[0] > b[2] || a[2] < b[0] || a[1] > b[3] || a[3] < b[1])
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
    if !rect_overlap(r1, r2) {
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

/// `msPointInPolygon`.
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

/// MapServer's `MS_MIN` and `MS_MAX`.
fn ms_min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

fn ms_max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
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

    /// Holds `entry` for `bbox`, grown to `point` where one is given.
    fn insert(&mut self, bbox: &[f64; 4], point: Option<Point>, entry: E) {
        let placed = self
            .place(bbox)
            .filter(|_| point.is_none_or(|p| !p.x.is_nan() && !p.y.is_nan()));
        let Some((mut b, store)) = placed else {
            self.anywhere.push(entry);
            return;
        };
        if let Some(p) = point {
            let [w, h] = self.size;
            let (px, py) = (p.x.clamp(0.0, w), p.y.clamp(0.0, h));
            b = [b[0].min(px), b[1].min(py), b[2].max(px), b[3].max(py)];
        }
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
    fn cell(&self, v: f64, n: usize) -> usize {
        ((v / self.side).floor() as usize).min(n - 1)
    }
}
