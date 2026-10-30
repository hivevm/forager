// The drawing of a feature in a style follows MapServer's msDrawLineSymbol() and
// msDrawShadeSymbol(), src/maprendering.c, and the order in which a line layer draws its styles
// and their outlines, msDrawVectorLayer() and msOutlineRenderingPrepareStyle(), src/mapdraw.c and
// src/maprendering.c, of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
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

use std::borrow::Borrow;

use forager_ports::{Coord, Geometry};
use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point};
use forager_style_ir::{Bound, RenderMode, Rgba, Style, Symbol};

use crate::extent::View;
use crate::hatch::{self, Hatched, Hatching};
use crate::line_marker::{self, MarkerBudget};
use crate::offset::{self, Offset};
use crate::symbol;

/// The widest line, MapServer's default `MAXWIDTH` (`MS_MAXSYMBOLWIDTH`).
const MAX_SYMBOL_WIDTH: f64 = 32.0;

/// The narrowest line, MapServer's default `MINWIDTH` (`MS_MINSYMBOLWIDTH`).
const MIN_SYMBOL_WIDTH: f64 = 0.0;

/// The largest symbol, MapServer's default `MAXSIZE` of a style (`MS_MAXSYMBOLSIZE`).
const MAX_SYMBOL_SIZE: f64 = 500.0;

/// One drawing of a style on a feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// The style as written.
    Plain,
    /// The style as written, only when it has a `COLOR`.
    PlainIfColored,
    /// The outline of a line: the style widened by twice its `OUTLINEWIDTH`, its colours swapped.
    Outline,
    /// The outline, only when the style has an `OUTLINECOLOR`.
    OutlineIfOutlined,
}

/// The styles of a class with the passes each draws. A line layer draws them style by style,
/// every feature in one style before the next (MapServer's shape cache of a line layer): the
/// first style, as its outline when it has an `OUTLINEWIDTH`, then that first style itself, then
/// each further style with its outline before it, feature by feature. Any other layer draws
/// every style of a feature before the next feature. `bound_outline` tells, style by style, that
/// the class binds its `OUTLINEWIDTH` to a feature: msDrawVectorLayer's cache then draws the
/// passes after the first as for an outline, whatever width the feature binds.
pub fn passes<'s, S: Borrow<Style>>(
    mode: RenderMode,
    styles: &'s [S],
    bound_outline: &[bool],
) -> Vec<(&'s Style, Vec<Pass>)> {
    if mode != RenderMode::Line {
        return styles
            .iter()
            .map(|s| (s.borrow(), vec![Pass::Plain]))
            .collect();
    }
    let mut passes = Vec::new();
    for (i, style) in styles.iter().map(Borrow::borrow).enumerate() {
        let drawn_outlined = outlined(style);
        let outlined = drawn_outlined || bound_outline.get(i).copied().unwrap_or(false);
        if i == 0 {
            passes.push((
                style,
                vec![if drawn_outlined {
                    Pass::Outline
                } else {
                    Pass::Plain
                }],
            ));
            if outlined {
                passes.push((style, vec![Pass::PlainIfColored]));
            }
        } else if outlined {
            passes.push((style, vec![Pass::OutlineIfOutlined, Pass::PlainIfColored]));
        } else {
            passes.push((style, vec![Pass::PlainIfColored]));
        }
    }
    passes
}

/// Whether a layer of the render mode draws the geometry. As in MapServer, a line layer draws
/// polygons as their outlines and a point layer draws a marker at every vertex; a polygon layer
/// draws no lines or points, and a line layer no points.
pub fn can_draw(mode: RenderMode, geometry: &Geometry) -> bool {
    match (mode, geometry) {
        (_, Geometry::Collection(geometries)) => geometries.iter().all(|g| can_draw(mode, g)),
        (RenderMode::Point, _) => true,
        (RenderMode::Polygon, Geometry::Polygon(_) | Geometry::MultiPolygon(_)) => true,
        (RenderMode::Line, g) => !matches!(g, Geometry::Point(_) | Geometry::MultiPoint(_)),
        _ => false,
    }
}

/// What a layer draws its features on beside the view: the map's symbols, the image's width and
/// height in pixels, what is left to the layer of [`line_marker::MAX_MARKER_POINTS`], and the
/// hatching of the class drawn, which counts the hatches refused (Q-9).
pub struct Canvas<'a> {
    symbols: &'a [Symbol],
    image: (f64, f64),
    markers: MarkerBudget,
    hatching: Hatching,
}

impl<'a> Canvas<'a> {
    /// A canvas for one layer, its budget of markers whole.
    pub fn new(symbols: &'a [Symbol], width: u32, height: u32) -> Self {
        Self {
            symbols,
            image: (f64::from(width), f64::from(height)),
            markers: MarkerBudget::new(),
            hatching: Hatching::default(),
        }
    }

    /// Sets the class whose styles the next features are drawn in, for the clip of its hatches
    /// (see [`Hatching::for_class`]).
    pub fn class(&mut self, styles: &[Style]) {
        self.hatching = Hatching {
            refused: self.hatching.refused,
            ..Hatching::for_class(styles, self.symbols)
        };
    }

    /// Whether symbols along the layer's lines were left out, their points past the budget.
    pub fn markers_spent(&self) -> bool {
        self.markers.spent()
    }

    /// How many hatches of the layer were refused (Q-9).
    pub fn hatches_refused(&self) -> usize {
        self.hatching.refused
    }
}

/// The drawing operations of one pass of a style on a geometry [`can_draw`] accepts, a hatch
/// drawn as the canvas's hatching says, a hatch it refuses to draw counted there (Q-9).
pub fn draw_ops(
    mode: RenderMode,
    geometry: &Geometry,
    style: &Style,
    pass: Pass,
    view: &View,
    canvas: &mut Canvas,
) -> Vec<DrawOp> {
    let pixels = |cs: &[Coord]| cs.iter().map(|&c| view.to_pixel(c)).collect::<Vec<_>>();
    let mut ops = Vec::new();
    match (mode, geometry) {
        (_, Geometry::Collection(geometries)) => {
            for g in geometries {
                ops.extend(draw_ops(mode, g, style, pass, view, canvas));
            }
        }
        (RenderMode::Point, _) => {
            let mark = symbol::mark(style, canvas.symbols);
            // pointLayerDrawShape: a point outside the map's extent draws nothing.
            for c in vertices(geometry).into_iter().filter(|&c| view.contains(c)) {
                let at = view.point_to_pixel(c);
                match mark {
                    symbol::Mark::Square => point_ops(&mut ops, at, style),
                    symbol::Mark::Symbol(s) => symbol::symbol_ops(&mut ops, at, style, s),
                    symbol::Mark::Nothing => {}
                }
            }
        }
        (RenderMode::Polygon, Geometry::Polygon(_) | Geometry::MultiPolygon(_))
            if symbol::is_hatch(style, canvas.symbols) =>
        {
            let rings: Vec<Vec<Coord>> = match geometry {
                Geometry::Polygon(rings) => rings.clone(),
                Geometry::MultiPolygon(polygons) => polygons.concat(),
                _ => Vec::new(),
            };
            let rings = hatch_rings(&rings, view, &canvas.hatching);
            let drawn = shade(&mut ops, rings, style, view, canvas);
            canvas.hatching.refused += usize::from(!drawn);
        }
        (RenderMode::Polygon, Geometry::Polygon(rings)) => {
            let rings = rings.iter().map(|r| pixels(r)).collect();
            shade(&mut ops, rings, style, view, canvas);
        }
        (RenderMode::Polygon, Geometry::MultiPolygon(polygons)) => {
            for rings in polygons {
                let rings = rings.iter().map(|r| pixels(r)).collect();
                shade(&mut ops, rings, style, view, canvas);
            }
        }
        (RenderMode::Line, Geometry::LineString(points)) => {
            line_symbol(&mut ops, vec![pixels(points)], style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiLineString(lines) | Geometry::Polygon(lines)) => {
            let paths: Vec<_> = lines.iter().map(|l| pixels(l)).collect();
            let paths = match geometry {
                Geometry::Polygon(_) => open_rings(paths),
                _ => paths,
            };
            line_symbol(&mut ops, paths, style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiPolygon(polygons)) => {
            let rings = polygons.iter().flatten().map(|r| pixels(r)).collect();
            line_symbol(&mut ops, open_rings(rings), style, pass, canvas);
        }
        _ => {}
    }
    ops
}

/// The rings of a feature in pixels as msDrawShape() hands them to a hatch: all of its polygons
/// as one shape, and, where the shape reaches beyond the map's extent, clipped first to the
/// extent grown by the class's clip buffer (see [`Hatching::for_class`]) with
/// msClipPolygonRect(); in map units before the transformation to pixels, or, where a style of
/// the class has a `GEOMTRANSFORM`, to the image so grown after it. The hatch's box, and so where
/// its dashes fall, is the clipped shape's.
fn hatch_rings(rings: &[Vec<Coord>], view: &View, hatching: &Hatching) -> Vec<Vec<Point>> {
    let e = view.extent;
    let beyond = rings
        .iter()
        .flatten()
        .any(|c| c.x < e.min_x || c.y < e.min_y || c.x > e.max_x || c.y > e.max_y);
    let to_pixels = |rings: &[Vec<Point>]| -> Vec<Vec<Point>> {
        rings
            .iter()
            .map(|r| {
                r.iter()
                    .map(|p| view.to_pixel(Coord { x: p.x, y: p.y }))
                    .collect()
            })
            .collect()
    };
    let points: Vec<Vec<Point>> = rings
        .iter()
        .map(|r| r.iter().map(|c| Point { x: c.x, y: c.y }).collect())
        .collect();
    let buffer = hatching.clip_buffer;
    if !beyond {
        return to_pixels(&points);
    }
    if hatching.clip_in_pixels {
        let (width, height) = view.image_size();
        let min = Point {
            x: -buffer,
            y: -buffer,
        };
        let max = Point {
            x: width + buffer,
            y: height + buffer,
        };
        return hatch::clip_polygon_rect(&to_pixels(&points), min, max);
    }
    let grow = buffer * view.cellsize;
    let min = Point {
        x: e.min_x - grow,
        y: e.min_y - grow,
    };
    let max = Point {
        x: e.max_x + grow,
        y: e.max_y + grow,
    };
    to_pixels(&hatch::clip_polygon_rect(&points, min, max))
}

/// Every vertex of a geometry, in order.
pub(crate) fn vertices(geometry: &Geometry) -> Vec<Coord> {
    match geometry {
        Geometry::Point(c) => vec![*c],
        Geometry::MultiPoint(cs) | Geometry::LineString(cs) => cs.clone(),
        Geometry::MultiLineString(lines) | Geometry::Polygon(lines) => lines.concat(),
        Geometry::MultiPolygon(polygons) => polygons.iter().flatten().flatten().copied().collect(),
        Geometry::Collection(geometries) => geometries.iter().flat_map(vertices).collect(),
    }
}

pub fn kind(geometry: &Geometry) -> &'static str {
    match geometry {
        Geometry::Point(_) => "point",
        Geometry::MultiPoint(_) => "multipoint",
        Geometry::LineString(_) => "line",
        Geometry::MultiLineString(_) => "multiline",
        Geometry::Polygon(_) => "polygon",
        Geometry::MultiPolygon(_) => "multipolygon",
        Geometry::Collection(_) => "collection",
    }
}

/// A style property's value. The engine binds each style to the feature it draws before it draws
/// it, so a property bound to a property or an expression is a value here, or unset.
pub(crate) fn fixed<T: Copy>(property: &Option<Bound<T>>) -> Option<T> {
    property.as_ref().and_then(Bound::value)
}

/// Whether the style has an outline, an `OUTLINEWIDTH` above 0.
pub fn outlined(style: &Style) -> bool {
    outline_width(style) > 0.0
}

/// The style's `OUTLINEWIDTH` in pixels; 0 without one.
fn outline_width(style: &Style) -> f64 {
    fixed(&style.outline_width).unwrap_or(0.0)
}

/// The style's `WIDTH` bounded by its `MINWIDTH` and `MAXWIDTH`, as msDrawLineSymbol() bounds
/// it: the maximum first, then the minimum. With the width as written.
fn bounded_width(style: &Style) -> (f64, f64) {
    let written = fixed(&style.width).unwrap_or(1.0);
    let max = style.max_width.unwrap_or(MAX_SYMBOL_WIDTH);
    let min = style.min_width.unwrap_or(MIN_SYMBOL_WIDTH);
    (written.min(max).max(min), written)
}

/// The offset of the style, its numbers multiplied by `factor`; `None` when it moves nothing.
fn style_offset(style: &Style, factor: f64) -> Option<Offset> {
    if let Some(line) = &style.line_offset {
        let distance = line.distance.value()? * factor;
        return Some(if line.both_sides {
            Offset::BothSides(distance)
        } else {
            Offset::Parallel(distance)
        });
    }
    let [x, y] = style.offset.as_ref()?;
    let (x, y) = (x.value().unwrap_or(0.0), y.value().unwrap_or(0.0));
    (x != 0.0 || y != 0.0).then_some(Offset::Shift {
        x: x * factor,
        y: y * factor,
    })
}

fn moved(paths: Vec<Vec<Point>>, offset: Option<Offset>) -> Vec<Vec<Point>> {
    match offset {
        Some(offset) => offset::apply(&paths, offset),
        None => paths,
    }
}

/// The cap and the join agg2RenderLine() strokes a line of `width` with: those given for a line
/// wider than a pixel, and for any other bevel joins and AGG's default cap, butt. (It also bevels
/// the inner joins of such a line, which the display list does not carry; they lie within a
/// pixel of the path.)
fn line_ends(width: f64, cap: LineCap, join: LineJoin) -> (LineCap, LineJoin) {
    if width > 1.0 {
        (cap, join)
    } else {
        (LineCap::Butt, LineJoin::Bevel)
    }
}

/// Rings as MapServer's line_adaptor strokes them: open lines that end on their first point,
/// with caps there and no join.
fn open_rings(mut rings: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    for ring in &mut rings {
        if let (Some(&first), Some(&last)) = (ring.first(), ring.last())
            && first != last
        {
            ring.push(first);
        }
    }
    rings
}

/// A line drawn in a style, as msDrawLineSymbol() draws one of a simple symbol: in the `COLOR`,
/// or the `OUTLINECOLOR` without one; the width bounded; the offset, the dash pattern, and the
/// initial gap scaled by the factor the bounds applied to the width; and the style's cap and
/// join, by default `round` and `none`. An outline pass draws the style widened by twice its
/// outline width with its colours swapped, as msOutlineRenderingPrepareStyle() prepares it.
/// A style with a vector or ellipse symbol and a `GAP` draws the symbol along the line instead,
/// its markers' points taken from the canvas's budget; one whose symbol index is past the last
/// draws nothing.
fn line_symbol(
    ops: &mut Vec<DrawOp>,
    paths: Vec<Vec<Point>>,
    style: &Style,
    pass: Pass,
    canvas: &mut Canvas,
) {
    let prepared;
    let style = match pass {
        Pass::Plain | Pass::PlainIfColored => style,
        Pass::Outline | Pass::OutlineIfOutlined => {
            prepared = outline_style(style);
            &prepared
        }
    };
    let (first, second) = (fixed(&style.color), fixed(&style.outline_color));
    if matches!(pass, Pass::PlainIfColored | Pass::OutlineIfOutlined) && first.is_none() {
        return;
    }
    let Some(color) = first.or(second) else {
        return;
    };
    let (width, written) = bounded_width(style);
    let factor = if written != 0.0 { width / written } else { 1.0 };
    let initial_gap = style.initial_gap.unwrap_or(-1.0);
    // msDrawLineSymbol's `gap < 0` or `gap > 0`: a gap that is not a number brushes the line.
    let gapped = matches!(style.gap, Some(gap) if gap.abs() > 0.0);
    match symbol::mark(style, canvas.symbols) {
        symbol::Mark::Nothing => return,
        symbol::Mark::Symbol(symbol) if gapped => {
            let paths = moved(paths, style_offset(style, factor));
            let initial_gap = initial_gap * factor;
            let image = canvas.image;
            line_marker::draw(
                ops,
                &paths,
                style,
                symbol,
                initial_gap,
                image,
                &mut canvas.markers,
            );
            return;
        }
        symbol::Mark::Symbol(_) | symbol::Mark::Square => {}
    }
    let (cap, join) = line_ends(
        width,
        style.line_cap.unwrap_or(LineCap::Round),
        style.line_join.unwrap_or(LineJoin::None),
    );
    ops.push(DrawOp::Stroke {
        paths: moved(paths, style_offset(style, factor)),
        closed: false,
        color,
        width,
        cap,
        join,
        dashes: Dashes {
            pattern: style.pattern.iter().map(|p| p * factor).collect(),
            offset: if initial_gap <= 0.0 {
                0.0
            } else {
                initial_gap * factor
            },
        },
    });
}

/// The style of an outline pass as msOutlineRenderingPrepareStyle() prepares it, at a scale
/// factor of 1: its width and its bounds widened by twice its `OUTLINEWIDTH`, its size, MapServer's
/// -1 where it has none, grown by it, and its colours swapped.
fn outline_style(style: &Style) -> Style {
    let outline = outline_width(style);
    Style {
        color: style.outline_color.clone(),
        outline_color: style.color.clone(),
        width: Some(Bound::Value(
            fixed(&style.width).unwrap_or(1.0) + 2.0 * outline,
        )),
        min_width: Some(style.min_width.unwrap_or(MIN_SYMBOL_WIDTH) + 2.0 * outline),
        max_width: Some(style.max_width.unwrap_or(MAX_SYMBOL_WIDTH) + 2.0 * outline),
        size: Some(Bound::Value(fixed(&style.size).unwrap_or(-1.0) + outline)),
        ..style.clone()
    }
}

/// A polygon drawn in a style, as msDrawShadeSymbol() draws one: without a `COLOR`, its
/// outline as a line in the `OUTLINECOLOR`; with one, moved by the offset, and with a hatch
/// symbol hatched (see [`hatch::hatch`]), with any other filled, and outlined in the
/// `OUTLINECOLOR` at the colour's alpha, with round caps and joins. False where a hatch is
/// refused (Q-9).
fn shade(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style,
    view: &View,
    canvas: &mut Canvas,
) -> bool {
    let Some(color) = fixed(&style.color) else {
        line_symbol(ops, open_rings(rings), style, Pass::Plain, canvas);
        return true;
    };
    let rings = moved(rings, style_offset(style, 1.0));
    if symbol::is_hatch(style, canvas.symbols) {
        return match hatch::hatch(
            rings,
            style,
            color,
            view.image_size(),
            view.point_to_pixel(Coord { x: 0.0, y: 0.0 }),
        ) {
            Hatched::Drawn(hatch) => {
                ops.push(DrawOp::Hatch(hatch));
                true
            }
            Hatched::Nothing => true,
            Hatched::Refused => false,
        };
    }
    ops.push(DrawOp::Polygon {
        rings: rings.clone(),
        color,
    });
    if let Some(outline) = fixed(&style.outline_color) {
        let width = fixed(&style.width).unwrap_or(1.0);
        let width = if width == 0.0 { 1.0 } else { width };
        let max = style.max_width.unwrap_or(MAX_SYMBOL_WIDTH);
        let min = style.min_width.unwrap_or(MIN_SYMBOL_WIDTH);
        let width = width.min(max).max(min);
        let (cap, join) = line_ends(width, LineCap::Round, LineJoin::Round);
        ops.push(DrawOp::Stroke {
            paths: open_rings(rings),
            closed: false,
            color: Rgba {
                a: color.a,
                ..outline
            },
            width,
            cap,
            join,
            dashes: Dashes::default(),
        });
    }
    true
}

/// A point as a square of the style's size, centred on it, filled in the `COLOR` and outlined
/// in the `OUTLINECOLOR`.
fn point_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style) {
    let half = fixed(&style.size)
        .unwrap_or(1.0)
        .clamp(0.0, MAX_SYMBOL_SIZE)
        / 2.0;
    let corner = |dx: f64, dy: f64| Point {
        x: at.x + dx,
        y: at.y + dy,
    };
    let square = vec![
        corner(-half, -half),
        corner(half, -half),
        corner(half, half),
        corner(-half, half),
    ];
    if let Some(color) = fixed(&style.color) {
        ops.push(DrawOp::Fill {
            rings: vec![square.clone()],
            color,
        });
    }
    if let Some(color) = fixed(&style.outline_color) {
        let width = fixed(&style.width)
            .unwrap_or(1.0)
            .clamp(0.0, MAX_SYMBOL_WIDTH);
        let (cap, join) = line_ends(width, LineCap::Round, LineJoin::None);
        ops.push(DrawOp::Stroke {
            paths: vec![square],
            closed: true,
            color,
            width,
            cap,
            join,
            dashes: Dashes::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use forager_style_ir::{Extent, LineOffset, SymbolKind, SymbolRef};

    use super::*;

    const BLUE: Rgba = Rgba::opaque(30, 60, 200);
    const BLACK: Rgba = Rgba::opaque(0, 0, 0);

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// A canvas without symbols, of 100 × 100.
    fn plain() -> Canvas<'static> {
        Canvas::new(&[], 100, 100)
    }

    fn line() -> Vec<Vec<Point>> {
        vec![vec![p(0.0, 0.0), p(10.0, 0.0)]]
    }

    fn colored(color: Option<Rgba>, outline: Option<Rgba>) -> Style {
        Style {
            color: color.map(Bound::Value),
            outline_color: outline.map(Bound::Value),
            ..Style::default()
        }
    }

    /// The strokes of one pass of a style on [`line`]: color, width, dashes, and paths.
    fn strokes(style: &Style, pass: Pass) -> Vec<(Rgba, f64, Dashes, Vec<Vec<Point>>)> {
        let mut ops = Vec::new();
        line_symbol(&mut ops, line(), style, pass, &mut plain());
        ops.into_iter()
            .map(|op| match op {
                DrawOp::Stroke {
                    color,
                    width,
                    dashes,
                    paths,
                    ..
                } => (color, width, dashes, paths),
                DrawOp::Fill { .. }
                | DrawOp::Polygon { .. }
                | DrawOp::Hatch(_)
                | DrawOp::Glyphs { .. } => {
                    panic!("a line is not filled")
                }
            })
            .collect()
    }

    #[test]
    fn the_bound_on_the_width_scales_the_pattern_the_initial_gap_and_the_offset() {
        // WIDTH 10 bounded by MAXWIDTH 5: msDrawLineSymbol's finalscalefactor is 0.5.
        let style = Style {
            width: Some(Bound::Value(10.0)),
            max_width: Some(5.0),
            pattern: vec![20.0, 8.0],
            initial_gap: Some(6.0),
            line_offset: Some(LineOffset {
                distance: Bound::Value(4.0),
                both_sides: false,
            }),
            ..colored(Some(BLUE), None)
        };
        let drawn = strokes(&style, Pass::Plain);
        let (_, width, dashes, paths) = &drawn[0];
        assert_eq!(*width, 5.0);
        assert_eq!(dashes.pattern, vec![10.0, 4.0]);
        assert_eq!(dashes.offset, 3.0);
        assert_eq!(paths[0], vec![p(0.0, 2.0), p(10.0, 2.0)]);
        // MINWIDTH raises the width and scales alike.
        let raised = Style {
            width: Some(Bound::Value(2.0)),
            min_width: Some(4.0),
            pattern: vec![3.0, 1.0],
            ..colored(Some(BLUE), None)
        };
        assert_eq!(strokes(&raised, Pass::Plain)[0].2.pattern, vec![6.0, 2.0]);
    }

    #[test]
    fn an_outline_pass_of_symbols_grows_their_width_and_size_and_swaps_their_colors() {
        // msOutlineRenderingPrepareStyle at a scale factor of 1: the width and its bounds by
        // twice the outline width, the size by it, MapServer's -1 where none is given.
        let style = Style {
            width: Some(Bound::Value(2.0)),
            size: Some(Bound::Value(6.0)),
            outline_width: Some(Bound::Value(1.5)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let prepared = outline_style(&style);
        assert_eq!(fixed(&prepared.width), Some(5.0));
        assert_eq!(
            (prepared.min_width, prepared.max_width),
            (Some(3.0), Some(35.0))
        );
        assert_eq!(fixed(&prepared.size), Some(7.5));
        assert_eq!(
            (fixed(&prepared.color), fixed(&prepared.outline_color)),
            (Some(BLACK), Some(BLUE))
        );
        let without_size = Style {
            size: None,
            ..style
        };
        assert_eq!(fixed(&outline_style(&without_size).size), Some(0.5));
    }

    /// The points of drawing operations.
    fn points(ops: &[DrawOp]) -> usize {
        ops.iter()
            .map(|op| match op {
                DrawOp::Stroke { paths, .. } => paths.iter().map(Vec::len).sum(),
                DrawOp::Fill { rings, .. } | DrawOp::Polygon { rings, .. } => {
                    rings.iter().map(Vec::len).sum()
                }
                DrawOp::Hatch(_) | DrawOp::Glyphs { .. } => 0,
            })
            .sum()
    }

    #[test]
    fn the_symbols_along_the_lines_of_a_layer_share_one_budget() {
        // Verifies: Q-9
        // A filled triangle of 3 points every 0.01 pixels from 0.005 along lines 100 pixels long:
        // 10 000 markers, 30 000 points, each line far under the limit. A feature of 20 such lines
        // takes 600 000 points and one of 10 another 300 000; a third of 20 would pass the
        // million and draws nothing, nor does any feature after it.
        let view = View::for_image(
            Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            },
            101,
            101,
        )
        .unwrap();
        let triangle = Symbol {
            name: None,
            kind: SymbolKind::Vector,
            paths: vec![vec![[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]]],
            extent: Some([2.0, 2.0]),
            filled: true,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        };
        let symbols = [triangle];
        let style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            gap: Some(-0.01),
            ..colored(Some(BLUE), None)
        };
        let lines = |count: i32| {
            Geometry::MultiLineString(
                (0..count)
                    .map(|i| {
                        let y = f64::from(i) * 5.0;
                        vec![Coord { x: 0.0, y }, Coord { x: 100.0, y }]
                    })
                    .collect(),
            )
        };
        let mut canvas = Canvas::new(&symbols, 101, 101);
        let mut draw = |geometry: &Geometry| {
            let ops = draw_ops(
                RenderMode::Line,
                geometry,
                &style,
                Pass::Plain,
                &view,
                &mut canvas,
            );
            (points(&ops), canvas.markers_spent())
        };
        assert_eq!(draw(&lines(20)), (600_000, false));
        assert_eq!(draw(&lines(10)), (300_000, false));
        assert_eq!(draw(&lines(20)), (0, true));
        assert_eq!(
            draw(&lines(1)),
            (0, true),
            "nothing once the budget is spent"
        );
    }

    #[test]
    fn a_symbol_index_past_the_last_draws_no_line() {
        // msDrawLineSymbol returns at once for a symbol it does not have.
        let style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(9))),
            ..colored(Some(BLUE), None)
        };
        assert!(strokes(&style, Pass::Plain).is_empty());
    }

    #[test]
    fn a_line_without_a_color_is_drawn_in_its_outline_color() {
        let drawn = strokes(&colored(None, Some(BLACK)), Pass::Plain);
        assert_eq!(drawn[0].0, BLACK);
        assert!(strokes(&colored(None, None), Pass::Plain).is_empty());
    }

    #[test]
    fn an_outline_pass_widens_the_line_and_swaps_its_colors() {
        let style = Style {
            width: Some(Bound::Value(6.0)),
            outline_width: Some(Bound::Value(2.0)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let drawn = strokes(&style, Pass::Outline);
        assert_eq!((drawn[0].0, drawn[0].1), (BLACK, 10.0));
        // The first style's outline falls back to its colour, as the swapped style does.
        let uncolored_outline = Style {
            outline_width: Some(Bound::Value(2.0)),
            ..colored(Some(BLUE), None)
        };
        assert_eq!(strokes(&uncolored_outline, Pass::Outline)[0].0, BLUE);
        // A later style draws its outline only with an OUTLINECOLOR, itself only with a COLOR.
        assert!(strokes(&uncolored_outline, Pass::OutlineIfOutlined).is_empty());
        assert!(strokes(&colored(None, Some(BLACK)), Pass::PlainIfColored).is_empty());
    }

    #[test]
    fn a_line_layer_draws_outlines_in_passes_before_the_lines() {
        let outlined = Style {
            outline_width: Some(Bound::Value(1.0)),
            ..colored(Some(BLUE), Some(BLACK))
        };
        let plain = colored(Some(BLUE), None);
        let styles = [outlined.clone(), plain.clone(), outlined];
        let kinds: Vec<Vec<Pass>> = passes(RenderMode::Line, &styles, &[])
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        assert_eq!(
            kinds,
            vec![
                vec![Pass::Outline],
                vec![Pass::PlainIfColored],
                vec![Pass::PlainIfColored],
                vec![Pass::OutlineIfOutlined, Pass::PlainIfColored],
            ]
        );
        let plain_first = [plain.clone(), plain];
        let kinds: Vec<Vec<Pass>> = passes(RenderMode::Line, &plain_first, &[])
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        assert_eq!(kinds, vec![vec![Pass::Plain], vec![Pass::PlainIfColored]]);
    }

    #[test]
    fn a_line_no_wider_than_a_pixel_has_butt_caps_and_bevel_joins() {
        // agg2RenderLine applies the style's cap and join only to a line wider than a pixel.
        let style = |width| Style {
            width: Some(Bound::Value(width)),
            line_cap: Some(LineCap::Square),
            line_join: Some(LineJoin::Miter),
            ..colored(Some(BLUE), None)
        };
        let ends = |width| {
            let mut ops = Vec::new();
            line_symbol(&mut ops, line(), &style(width), Pass::Plain, &mut plain());
            match &ops[0] {
                DrawOp::Stroke { cap, join, .. } => (*cap, *join),
                DrawOp::Fill { .. }
                | DrawOp::Polygon { .. }
                | DrawOp::Hatch(_)
                | DrawOp::Glyphs { .. } => {
                    panic!("a line is not filled")
                }
            }
        };
        assert_eq!(ends(1.0), (LineCap::Butt, LineJoin::Bevel));
        assert_eq!(ends(1.5), (LineCap::Square, LineJoin::Miter));
    }

    #[test]
    fn a_filled_polygon_is_outlined_at_its_fill_s_alpha() {
        let fill = Rgba {
            a: 100,
            ..Rgba::opaque(200, 80, 80)
        };
        let mut ops = Vec::new();
        let style = Style {
            width: Some(Bound::Value(2.0)),
            ..colored(Some(fill), Some(BLACK))
        };
        let view = View::for_image(
            forager_style_ir::Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 10.0,
            },
            11,
            11,
        )
        .unwrap();
        assert!(shade(&mut ops, line(), &style, &view, &mut plain()));
        match &ops[1] {
            DrawOp::Stroke {
                color, cap, join, ..
            } => {
                assert_eq!(*color, Rgba { a: 100, ..BLACK });
                assert_eq!((*cap, *join), (LineCap::Round, LineJoin::Round));
            }
            DrawOp::Fill { .. }
            | DrawOp::Polygon { .. }
            | DrawOp::Hatch(_)
            | DrawOp::Glyphs { .. } => {
                panic!("the outline is a stroke")
            }
        }
    }

    fn hatch_symbol() -> Symbol {
        Symbol {
            name: None,
            kind: forager_style_ir::SymbolKind::Hatch,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }
    }

    /// The hatch ops of `geometry` in a hatch style of SIZE 10 and WIDTH 1 on a view of the map
    /// from 0 to 100 at a pixel a unit, y downwards from 100.
    fn hatches(geometry: &Geometry) -> Vec<forager_render::Hatch> {
        let view = View::for_image(
            forager_style_ir::Extent {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            },
            101,
            101,
        )
        .unwrap();
        let style = Style {
            symbol: Some(Bound::Value(forager_style_ir::SymbolRef::Index(1))),
            size: Some(Bound::Value(10.0)),
            ..colored(Some(BLUE), None)
        };
        let symbols = [hatch_symbol()];
        let mut canvas = Canvas::new(&symbols, 101, 101);
        canvas.class(std::slice::from_ref(&style));
        let ops = draw_ops(
            RenderMode::Polygon,
            geometry,
            &style,
            Pass::Plain,
            &view,
            &mut canvas,
        );
        ops.into_iter()
            .map(|op| match op {
                DrawOp::Hatch(hatch) => hatch,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Coord> {
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
            .map(|(x, y)| Coord { x, y })
            .to_vec()
    }

    #[test]
    fn a_hatch_s_box_is_that_of_the_polygon_clipped_beyond_the_extent() {
        // The class's clip buffer is max(1, max(10, 1)) + 1 + 2 = 13 pixels: the polygon from
        // x = -1000 is clipped at -13, and the box of the hatch, grown by 0.7072, starts there.
        let reaching = Geometry::Polygon(vec![square(-1000.0, 10.0, 50.0, 60.0)]);
        let hatch = &hatches(&reaching)[0];
        assert!(
            (hatch.lines[0][0].x - (-13.7072)).abs() < 1e-9,
            "{:?}",
            hatch.lines[0]
        );
        // Inside the extent the polygon is not clipped.
        let inside = Geometry::Polygon(vec![square(5.0, 10.0, 50.0, 60.0)]);
        let hatch = &hatches(&inside)[0];
        assert!(
            (hatch.lines[0][0].x - 4.2928).abs() < 1e-9,
            "{:?}",
            hatch.lines[0]
        );
    }

    #[test]
    fn a_multipolygon_is_hatched_as_one_shape() {
        // msDrawShadeSymbol hatches the whole shape: one box over both squares.
        let two = Geometry::MultiPolygon(vec![
            vec![square(5.0, 10.0, 20.0, 20.0)],
            vec![square(60.0, 70.0, 90.0, 90.0)],
        ]);
        let drawn = hatches(&two);
        assert_eq!(drawn.len(), 1);
        assert_eq!(drawn[0].rings.len(), 2);
        let first = drawn[0].lines[0];
        assert!((first[0].x - 4.2928).abs() < 1e-9 && (first[1].x - 91.2928).abs() < 1e-9);
    }
}
