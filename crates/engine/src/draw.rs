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
/// every style of a feature before the next feature.
pub fn passes<S: Borrow<Style>>(mode: RenderMode, styles: &[S]) -> Vec<(&Style, Vec<Pass>)> {
    if mode != RenderMode::Line {
        return styles
            .iter()
            .map(|s| (s.borrow(), vec![Pass::Plain]))
            .collect();
    }
    let mut passes = Vec::new();
    for (i, style) in styles.iter().map(Borrow::borrow).enumerate() {
        let outlined = outlined(style);
        if i == 0 {
            passes.push((
                style,
                vec![if outlined { Pass::Outline } else { Pass::Plain }],
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

/// The drawing operations of one pass of a style on a geometry [`can_draw`] accepts.
pub fn draw_ops(
    mode: RenderMode,
    geometry: &Geometry,
    style: &Style,
    pass: Pass,
    view: &View,
    symbols: &[Symbol],
) -> Vec<DrawOp> {
    let pixels = |cs: &[Coord]| cs.iter().map(|&c| view.to_pixel(c)).collect::<Vec<_>>();
    let mut ops = Vec::new();
    match (mode, geometry) {
        (_, Geometry::Collection(geometries)) => {
            for g in geometries {
                ops.extend(draw_ops(mode, g, style, pass, view, symbols));
            }
        }
        (RenderMode::Point, _) => {
            let mark = symbol::mark(style, symbols);
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
        (RenderMode::Polygon, Geometry::Polygon(rings)) => {
            shade(&mut ops, rings.iter().map(|r| pixels(r)).collect(), style);
        }
        (RenderMode::Polygon, Geometry::MultiPolygon(polygons)) => {
            for rings in polygons {
                shade(&mut ops, rings.iter().map(|r| pixels(r)).collect(), style);
            }
        }
        (RenderMode::Line, Geometry::LineString(line)) => {
            line_symbol(&mut ops, vec![pixels(line)], style, pass);
        }
        (RenderMode::Line, Geometry::MultiLineString(lines) | Geometry::Polygon(lines)) => {
            let paths: Vec<_> = lines.iter().map(|l| pixels(l)).collect();
            let paths = match geometry {
                Geometry::Polygon(_) => open_rings(paths),
                _ => paths,
            };
            line_symbol(&mut ops, paths, style, pass);
        }
        (RenderMode::Line, Geometry::MultiPolygon(polygons)) => {
            let rings = polygons.iter().flatten().map(|r| pixels(r)).collect();
            line_symbol(&mut ops, open_rings(rings), style, pass);
        }
        _ => {}
    }
    ops
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

/// The style's `WIDTH` bounded by its `MINWIDTH` and `MAXWIDTH`, each grown by `grow`, as
/// msDrawLineSymbol() bounds it: the maximum first, then the minimum. With the width as written,
/// grown alike.
fn bounded_width(style: &Style, grow: f64) -> (f64, f64) {
    let written = fixed(&style.width).unwrap_or(1.0) + grow;
    let max = style.max_width.unwrap_or(MAX_SYMBOL_WIDTH) + grow;
    let min = style.min_width.unwrap_or(MIN_SYMBOL_WIDTH) + grow;
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
fn line_symbol(ops: &mut Vec<DrawOp>, paths: Vec<Vec<Point>>, style: &Style, pass: Pass) {
    let (outline, first, second) = match pass {
        Pass::Plain | Pass::PlainIfColored => (0.0, &style.color, &style.outline_color),
        Pass::Outline | Pass::OutlineIfOutlined => {
            (outline_width(style), &style.outline_color, &style.color)
        }
    };
    let (first, second) = (fixed(first), fixed(second));
    if matches!(pass, Pass::PlainIfColored | Pass::OutlineIfOutlined) && first.is_none() {
        return;
    }
    let Some(color) = first.or(second) else {
        return;
    };
    let (width, written) = bounded_width(style, 2.0 * outline);
    let factor = if written != 0.0 { width / written } else { 1.0 };
    let initial_gap = style.initial_gap.unwrap_or(-1.0);
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

/// A polygon drawn in a style, as msDrawShadeSymbol() draws one of a simple symbol: without a
/// `COLOR`, its outline as a line in the `OUTLINECOLOR`; with one, moved by the offset, filled,
/// and outlined in the `OUTLINECOLOR` at the colour's alpha, with round caps and joins.
fn shade(ops: &mut Vec<DrawOp>, rings: Vec<Vec<Point>>, style: &Style) {
    let Some(color) = fixed(&style.color) else {
        line_symbol(ops, open_rings(rings), style, Pass::Plain);
        return;
    };
    let rings = moved(rings, style_offset(style, 1.0));
    ops.push(DrawOp::Fill {
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
    use forager_style_ir::LineOffset;

    use super::*;

    const BLUE: Rgba = Rgba::opaque(30, 60, 200);
    const BLACK: Rgba = Rgba::opaque(0, 0, 0);

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
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
        line_symbol(&mut ops, line(), style, pass);
        ops.into_iter()
            .map(|op| match op {
                DrawOp::Stroke {
                    color,
                    width,
                    dashes,
                    paths,
                    ..
                } => (color, width, dashes, paths),
                DrawOp::Fill { .. } | DrawOp::Glyphs { .. } => panic!("a line is not filled"),
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
        let kinds: Vec<Vec<Pass>> = passes(RenderMode::Line, &styles)
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
        let kinds: Vec<Vec<Pass>> = passes(RenderMode::Line, &plain_first)
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
            line_symbol(&mut ops, line(), &style(width), Pass::Plain);
            match &ops[0] {
                DrawOp::Stroke { cap, join, .. } => (*cap, *join),
                DrawOp::Fill { .. } | DrawOp::Glyphs { .. } => panic!("a line is not filled"),
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
        shade(&mut ops, line(), &style);
        match &ops[1] {
            DrawOp::Stroke {
                color, cap, join, ..
            } => {
                assert_eq!(*color, Rgba { a: 100, ..BLACK });
                assert_eq!((*cap, *join), (LineCap::Round, LineJoin::Round));
            }
            DrawOp::Fill { .. } | DrawOp::Glyphs { .. } => panic!("the outline is a stroke"),
        }
    }
}
