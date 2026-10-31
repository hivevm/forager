// The placement follows get_metrics, src/maplabel.c, and the choice of a point label's position in
// msDrawLabelCache, its point, its box, and its member's box, with computeMarkerBounds and
// fastComputeBounds, src/mapdraw.c, of MapServer at the release pinned in
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

//! Where a label stands: the point its text is drawn from and the box it takes, from its text's
//! box alone, as MapServer places a label at a point. The trigonometry is libm's, the same code
//! on every target (Q-2).

use forager_render::Point;

/// MapServer's `MARKER_SLOP`: the pixels a label at an upper, lower, left, or right centre keeps
/// from its point beyond its offset.
const MARKER_SLOP: f64 = 2.0;

/// A position of a label about its point, as `get_metrics` takes it; `Other` stands for a bound
/// integer that names none of the nine, which `get_metrics` places as it places no case of its
/// switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Ul,
    Uc,
    Ur,
    Cl,
    Cc,
    Cr,
    Ll,
    Lc,
    Lr,
    Other,
}

/// The positions `POSITION AUTO` tries for a label of a point, in `msDrawLabelCache`'s order.
pub const AUTO_POSITIONS: [Anchor; 8] = [
    Anchor::Ul,
    Anchor::Lr,
    Anchor::Ur,
    Anchor::Ll,
    Anchor::Cr,
    Anchor::Cl,
    Anchor::Uc,
    Anchor::Lc,
];

/// A label's `POSITION`: one of the nine, or the first of [`AUTO_POSITIONS`] that fits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    At(Anchor),
    Auto,
}

/// The box of a label's text from its origin, as its `textPathObj` holds it: x to the right,
/// y down, and its count of lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextBox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub num_lines: usize,
}

/// The space a placed label takes, `label_bounds`: its turned corners where it is turned, and
/// their box `[min x, min y, max x, max y]`, or the box alone where it is not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Lower left, upper left, upper right, lower right, and the lower left again.
    pub poly: Option<[Point; 5]>,
    pub bbox: [f64; 4],
}

/// A label placed: the point its text is drawn from, the box it takes, and the position it
/// stands at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub origin: Point,
    pub bounds: Bounds,
    pub anchor: Anchor,
}

/// `get_metrics`: the point a text of box `text` is drawn from to stand at `anchor` of `p`, `ox`
/// and `oy` pixels away, turned by `rotation` in radians counter-clockwise, and the box it takes
/// grown by `buffer` pixels.
pub fn metrics(
    p: Point,
    anchor: Anchor,
    text: &TextBox,
    ox: i32,
    oy: i32,
    rotation: f64,
    buffer: i32,
) -> (Point, Bounds) {
    let w = text.max_x - text.min_x;
    let h = text.max_y - text.min_y;
    let (ox, oy) = (f64::from(ox), f64::from(oy));
    let buffer = f64::from(buffer);
    let one_line_below = oy > 0.0 && text.num_lines == 1;
    let (x1, y1) = match anchor {
        Anchor::Ul => (-w - ox, -oy),
        Anchor::Uc => (-(w / 2.0), -oy - MARKER_SLOP),
        Anchor::Ur => (ox, -oy),
        Anchor::Cl => (
            -w - ox - MARKER_SLOP,
            if one_line_below { oy } else { h / 2.0 },
        ),
        Anchor::Cc => (-(w / 2.0) + ox, (h / 2.0) + oy),
        Anchor::Cr => (ox + MARKER_SLOP, if one_line_below { oy } else { h / 2.0 }),
        Anchor::Ll => (-w - ox, h + oy),
        Anchor::Lc => (-(w / 2.0), h + oy + MARKER_SLOP),
        Anchor::Lr => (ox, h + oy),
        Anchor::Other => (0.0, 0.0),
    };
    if rotation != 0.0 {
        let (sin_a, cos_a) = (libm::sin(rotation), libm::cos(rotation));
        let x = x1 - text.min_x;
        let y = text.max_y - y1;
        let origin = Point {
            x: p.x + (x * cos_a - y * sin_a),
            y: p.y - (x * sin_a + y * cos_a),
        };
        let corner = |x2: f64, y2: f64| Point {
            x: p.x + (x2 * cos_a - (-y2) * sin_a),
            y: p.y - (x2 * sin_a + (-y2) * cos_a),
        };
        let ll = corner(x1 - buffer, y1 + buffer);
        let poly = [
            ll,
            corner(x1 - buffer, y1 - h - buffer),
            corner(x1 + w + buffer, y1 - h - buffer),
            corner(x1 + w + buffer, y1 + buffer),
            ll,
        ];
        (
            origin,
            Bounds {
                poly: Some(poly),
                bbox: fast_bounds(&poly),
            },
        )
    } else {
        let origin = Point {
            x: p.x + x1 - text.min_x,
            y: p.y + y1 - text.max_y,
        };
        let max_y = origin.y + buffer + text.max_y;
        let bbox = [
            origin.x - buffer,
            max_y - h - buffer * 2.0,
            origin.x + w + buffer,
            max_y,
        ];
        (origin, Bounds { poly: None, bbox })
    }
}

/// `fastComputeBounds`: the box of the points, by MapServer's `MS_MIN` and `MS_MAX`.
fn fast_bounds(points: &[Point; 5]) -> [f64; 4] {
    let first = points[0];
    let mut b = [first.x, first.y, first.x, first.y];
    for p in points {
        b[0] = if b[0] < p.x { b[0] } else { p.x };
        b[2] = if b[2] > p.x { b[2] } else { p.x };
        b[1] = if b[1] < p.y { b[1] } else { p.y };
        b[3] = if b[3] > p.y { b[3] } else { p.y };
    }
    b
}

/// What moves a label off its point: half the size of the marker drawn there, and the label's
/// `OFFSET` scaled, in whole pixels, as `msDrawLabelCache` holds them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offsets {
    pub marker: [f64; 2],
    pub label: [i32; 2],
}

/// A label of a point as it is placed: its position, its text's box, its offsets, its rotation
/// in radians counter-clockwise, its buffer in whole pixels, and whether it is forced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLabel {
    pub position: Position,
    pub text: TextBox,
    pub offsets: Offsets,
    pub rotation: f64,
    pub buffer: i32,
    pub force: bool,
}

/// Where `label` stands at `anchor` of `at`, as `msDrawLabelCache` calls `get_metrics`: at a fixed
/// `CC` by its offset alone, at any other position, and at every position `AUTO` tries, by its
/// offset and the marker's.
pub fn placed_at(at: Point, label: &PointLabel, anchor: Anchor) -> (Point, Bounds) {
    placed_with_buffer(at, label, anchor, label.buffer)
}

/// The polygon of a label's `GEOMTRANSFORM labelpoly` styles at `anchor` of `at`, as
/// `msDrawLabelCache` computes it once the label is placed: `get_metrics` at the label's position
/// and offsets with a buffer of 1 pixel, whatever the label's own `BUFFER`. The label cache never
/// tests it: a label's collisions are its text's box and its `labelpoint` marker's.
pub fn label_poly_at(at: Point, label: &PointLabel, anchor: Anchor) -> Bounds {
    placed_with_buffer(at, label, anchor, 1).1
}

/// A marker of a label's `GEOMTRANSFORM labelpoint` style at `at`, as `computeMarkerBounds`
/// computes its bounds: `size` the width and height `msGetMarkerSize` gives it, `anchor_point`
/// its symbol's, `angle` the style's in degrees, and `offset` the style's `OFFSET`, scaled. A
/// marker turned by any angle but 0 has its turned corners, and their box; one not turned, its
/// box alone.
pub fn marker_bounds(
    at: Point,
    (sx, sy): (f64, f64),
    [anchor_x, anchor_y]: [f64; 2],
    angle: f64,
    [offset_x, offset_y]: [f64; 2],
) -> Bounds {
    if angle != 0.0 {
        let (hx, hy) = (sx / 2.0, sy / 2.0);
        let p = |x, y| Point { x, y };
        let mut points = [p(hx, hy), p(hx, -hy), p(-hx, -hy), p(-hx, hy), p(hx, hy)];
        if anchor_x != 0.5 || anchor_y != 0.5 {
            let (aox, aoy) = ((0.5 - anchor_x) * sx, (0.5 - anchor_y) * sy);
            for point in &mut points {
                point.x += aox;
                point.y += aoy;
            }
        }
        // MS_DEG_TO_RAD, MS_PI / 180.
        let rot = -angle * (std::f64::consts::PI / 180.0);
        let (sin, cos) = (libm::sin(rot), libm::cos(rot));
        for point in &mut points {
            let x = point.x;
            point.x = point.x * cos - point.y * sin;
            point.y = x * sin + point.y * cos;
        }
        let (aox, aoy) = (at.x + offset_x, at.y + offset_y);
        for point in &mut points {
            point.x += aox;
            point.y += aoy;
        }
        Bounds {
            poly: Some(points),
            bbox: fast_bounds(&points),
        }
    } else {
        let aox = (0.5 - anchor_x) * sx + at.x + offset_x;
        let aoy = (0.5 - anchor_y) * sy + at.y + offset_y;
        Bounds {
            poly: None,
            bbox: [
                -sx / 2.0 + aox,
                -sy / 2.0 + aoy,
                sx / 2.0 + aox,
                sy / 2.0 + aoy,
            ],
        }
    }
}

/// The box of a label drawn, as `msDrawLabelCache` forms the member's `bbox`: its text's box
/// `text`, grown by `MS_MIN` and `MS_MAX` to the bounds of its `labelpoint` marker where it has
/// one. The label cache tests a box against it first, and a label's minimum distance is measured
/// from it.
pub fn member_box(text: &[f64; 4], marker: Option<&Bounds>) -> [f64; 4] {
    let mut b = *text;
    if let Some(m) = marker {
        let m = &m.bbox;
        b[0] = if b[0] < m[0] { b[0] } else { m[0] };
        b[1] = if b[1] < m[1] { b[1] } else { m[1] };
        b[2] = if b[2] > m[2] { b[2] } else { m[2] };
        b[3] = if b[3] > m[3] { b[3] } else { m[3] };
    }
    b
}

/// [`placed_at`] with a box grown by `buffer` pixels.
fn placed_with_buffer(
    at: Point,
    label: &PointLabel,
    anchor: Anchor,
    buffer: i32,
) -> (Point, Bounds) {
    let [lx, ly] = label.offsets.label;
    let (ox, oy) = if label.position == Position::At(Anchor::Cc) {
        (lx, ly)
    } else {
        // The sum is a double passed as get_metrics' int: the fraction is cut.
        (
            (label.offsets.marker[0] + f64::from(lx)) as i32,
            (label.offsets.marker[1] + f64::from(ly)) as i32,
        )
    };
    metrics(at, anchor, &label.text, ox, oy, label.rotation, buffer)
}

/// Places `label` at `at`, as `msDrawLabelCache` places a label without a leader: at `AUTO` at
/// the first of [`AUTO_POSITIONS`] whose box `fits`, or at the last where it is forced; at a fixed
/// position where its box fits, or where it is forced. `None` names a label that is not placed.
/// `fits` is the label cache's test of a box.
pub fn place(
    at: Point,
    label: &PointLabel,
    mut fits: impl FnMut(&Bounds) -> bool,
) -> Option<Placement> {
    let placement = |anchor| {
        let (origin, bounds) = placed_at(at, label, anchor);
        Placement {
            origin,
            bounds,
            anchor,
        }
    };
    match label.position {
        Position::Auto => {
            let last = AUTO_POSITIONS.len() - 1;
            AUTO_POSITIONS
                .into_iter()
                .enumerate()
                .map(|(i, anchor)| (i, placement(anchor)))
                .find(|(i, p)| fits(&p.bounds) || (*i == last && label.force))
                .map(|(_, p)| p)
        }
        Position::At(anchor) => {
            let p = placement(anchor);
            (label.force || fits(&p.bounds)).then_some(p)
        }
    }
}
