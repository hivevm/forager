// The drawing of a feature in a style dispatches by the layer's type as MapServer's msDrawShape()
// and msDrawTransformedShape(), src/mapdraw.c, of the release pinned in
// scripts/fetch-msautotest.sh, hand a shape to msDrawLineSymbol(), msDrawShadeSymbol(), and
// msDrawMarkerSymbol(), src/maprendering.c.
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

use forager_render::{DrawOp, Point};
use forager_style_ir::{
    Bound, Coord, GeomTransform, Geometry, RenderMode, Style, Symbol, TransformShape, TransformTree,
};

use crate::canvas::Canvas;
use crate::extent::View;
use crate::geom_transform;
use crate::hatch;
use crate::layer_transform::Kind;
use crate::line::{Pass, gapped, line_symbol, open_rings};
use crate::marker::{marker_ops, marker_price, of_symbol_zero};
use crate::shade::shade;
use crate::symbol;

/// Whether a layer of the render mode draws the geometry. As in MapServer, a line layer draws
/// polygons as their outlines and a point layer draws a marker at every vertex; a polygon layer
/// draws no lines or points, and a line layer no points. A circle layer takes every geometry, as
/// msDrawShape hands every shape to circleLayerDrawShape, which draws only a circle (see
/// [`crate::circle()`]).
pub fn can_draw(mode: RenderMode, geometry: &Geometry) -> bool {
    match (mode, geometry) {
        (RenderMode::Circle, _) => true,
        (_, Geometry::Collection(geometries)) => geometries.iter().all(|g| can_draw(mode, g)),
        (RenderMode::Point, _) => true,
        (RenderMode::Polygon, Geometry::Polygon(_) | Geometry::MultiPolygon(_)) => true,
        (RenderMode::Line, g) => !matches!(g, Geometry::Point(_) | Geometry::MultiPoint(_)),
        _ => false,
    }
}

/// The image path of the pixmap symbol that [`draw_ops`] draws as the mark of `style` in a layer
/// of `mode`: at each point of a point layer, at each point a `GEOMTRANSFORM` places on a line or
/// a polygon, along each line of a line layer every `GAP` pixels, and tiled over each polygon of
/// a polygon or a circle layer. The engine loads it before the layer is drawn, since this crate
/// does no I/O.
pub fn marked_pixmap<'a>(
    mode: RenderMode,
    style: &Style,
    symbols: &'a [Symbol],
) -> Option<&'a str> {
    let marked = matches!(
        mode,
        RenderMode::Point | RenderMode::Polygon | RenderMode::Circle
    ) || style.geom_transform.is_some()
        || (mode == RenderMode::Line && gapped(style));
    match symbol::mark(style, symbols) {
        symbol::Mark::Pixmap(_, symbol) if marked => symbol.image.as_deref(),
        _ => None,
    }
}

/// The image paths of the pixmap symbols that msDrawShape counts in the clip buffer of a class's
/// `styles` at their images' heights, having loaded them whatever their scale range
/// (`src/mapdraw.c:2466-2490`), in a layer drawn in `mode`: those of any class of a line layer,
/// whose lines the buffer clips, and those of a polygon layer's class with a hatch, the one
/// polygon style the clip buffer reaches here (see [`hatch::Hatching::for_class`]).
pub fn clip_pixmaps<'a>(mode: RenderMode, styles: &[Style], symbols: &'a [Symbol]) -> Vec<&'a str> {
    let clipped = match mode {
        RenderMode::Line => true,
        RenderMode::Polygon => styles.iter().any(|style| symbol::is_hatch(style, symbols)),
        _ => false,
    };
    if !clipped {
        return Vec::new();
    }
    styles
        .iter()
        .filter_map(|style| match symbol::mark(style, symbols) {
            symbol::Mark::Pixmap(_, symbol) => symbol.image.as_deref(),
            _ => None,
        })
        .collect()
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
    // The shape in pixels as msTransformShape gives it in the AGG renderer's mode, which
    // msImageStartLayer keeps for a layer without APPROXIMATION_SCALE (`src/mapdraw.c:4201-4224`):
    // msTransformShapeSimplify, its lines or its rings as the shape is a line or a polygon,
    // whatever the layer, a part it leaves without points dropped; a line clipped first (see
    // [`clipped_lines`]).
    let rings = |parts: &[Vec<Coord>]| -> Vec<Vec<Point>> {
        parts
            .iter()
            .map(|r| view.ring_to_pixels(r))
            .filter(|r| !r.is_empty())
            .collect()
    };
    let mut ops = Vec::new();
    match (mode, geometry) {
        // The whole shape, a collection's parts together, as msDrawTransformedShape takes it.
        (RenderMode::Line | RenderMode::Polygon, _) if style.geom_transform.is_some() => {
            transformed(&mut ops, geometry, style, pass, view, canvas);
        }
        (_, Geometry::Collection(geometries)) => {
            for g in geometries {
                ops.extend(draw_ops(mode, g, style, pass, view, canvas));
            }
        }
        (RenderMode::Point, _) => {
            ops.extend(
                point_markers(geometry, style, view, canvas)
                    .into_iter()
                    .flatten(),
            );
        }
        (RenderMode::Polygon, Geometry::Polygon(_) | Geometry::MultiPolygon(_))
            if symbol::is_hatch(style, canvas.symbols) =>
        {
            let rings: Vec<Vec<Coord>> = match geometry {
                Geometry::Polygon(rings) => rings.clone(),
                Geometry::MultiPolygon(polygons) => polygons.concat(),
                _ => Vec::new(),
            };
            let rings = hatch::hatch_rings(&rings, view, canvas.image(), &canvas.hatching);
            let drawn = shade(&mut ops, rings, style, view, canvas);
            canvas.hatching.refused += usize::from(!drawn);
        }
        (RenderMode::Polygon, Geometry::Polygon(parts)) => {
            shade(&mut ops, rings(parts), style, view, canvas);
        }
        (RenderMode::Polygon, Geometry::MultiPolygon(polygons)) => {
            for parts in polygons {
                shade(&mut ops, rings(parts), style, view, canvas);
            }
        }
        (RenderMode::Line, Geometry::LineString(points)) => {
            let paths = clipped_lines(std::slice::from_ref(points), view, canvas);
            line_symbol(&mut ops, paths, style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiLineString(parts)) => {
            let paths = clipped_lines(parts, view, canvas);
            line_symbol(&mut ops, paths, style, pass, canvas);
        }
        (RenderMode::Line, Geometry::Polygon(parts)) => {
            line_symbol(&mut ops, open_rings(rings(parts)), style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiPolygon(polygons)) => {
            let paths = rings(&polygons.concat());
            line_symbol(&mut ops, open_rings(paths), style, pass, canvas);
        }
        _ => {}
    }
    ops
}

/// A line shape's `parts` in pixels, clipped first as msDrawShape clips a line before it draws it
/// (see [`hatch::line_paths`]), unless the canvas's layer leaves its lines unclipped.
fn clipped_lines(parts: &[Vec<Coord>], view: &View, canvas: &Canvas) -> Vec<Vec<Point>> {
    hatch::line_paths(
        parts,
        view,
        canvas.image(),
        &canvas.hatching,
        canvas.clip_lines,
    )
}

/// The markers a point layer draws of `style` at each vertex of `geometry` inside the map's
/// extent, in order, one list for each such vertex: pointLayerDrawShape draws every style at
/// every point, whatever its GEOMTRANSFORM, and nothing at a point outside the map's extent. The
/// markers' points are taken from the canvas's budget (Q-9): where they would exceed what is
/// left, none of the shape's is drawn, nor any after them, its lists left empty; a collection's
/// parts are each a shape. The square Forager draws for symbol 0, which MapServer does not draw,
/// takes nothing.
pub fn point_markers(
    geometry: &Geometry,
    style: &Style,
    view: &View,
    canvas: &mut Canvas,
) -> Vec<Vec<DrawOp>> {
    if let Geometry::Collection(geometries) = geometry {
        return geometries
            .iter()
            .flat_map(|g| point_markers(g, style, view, canvas))
            .collect();
    }
    let points: Vec<Coord> = vertices(geometry)
        .into_iter()
        .filter(|&c| view.contains(c))
        .collect();
    let drawn = !points.is_empty()
        && (of_symbol_zero(style)
            || marker_price(style, canvas)
                .is_some_and(|price| canvas.markers.take(points.len(), price)));
    points
        .into_iter()
        .map(|c| {
            let mut ops = Vec::new();
            if drawn {
                marker_ops(&mut ops, view.point_to_pixel(c), style, canvas);
            }
            ops
        })
        .collect()
}

/// Every vertex of a geometry, in order.
pub fn vertices(geometry: &Geometry) -> Vec<Coord> {
    match geometry {
        Geometry::Point(c) => vec![*c],
        Geometry::MultiPoint(cs) | Geometry::LineString(cs) => cs.clone(),
        Geometry::MultiLineString(lines) | Geometry::Polygon(lines) => lines.concat(),
        Geometry::MultiPolygon(polygons) => polygons.iter().flatten().flatten().copied().collect(),
        Geometry::Collection(geometries) => geometries.iter().flat_map(vertices).collect(),
    }
}

/// A style with a `GEOMTRANSFORM` on a shape of a line or a polygon layer, as
/// msDrawTransformedShape draws it: with an expression, the shape its tree computes (see
/// [`expression`]); with `bbox`, a polygon (see [`bbox`]); otherwise its symbol at
/// each point the transform places, turned there where the style's angle is `AUTO` and the
/// transform gives one; nothing for a style without a symbol or with symbol 0, as
/// msDrawMarkerSymbol draws none. The outline pass of a line layer's
/// first style draws the style its outline prepares, as msDrawVectorLayer prepares the style
/// before msDrawShape draws the transform. The symbols' points are taken from the canvas's
/// budget of markers (Q-9): where they would exceed what is left, none of the shape's is drawn,
/// nor any after them.
fn transformed(
    ops: &mut Vec<DrawOp>,
    geometry: &Geometry,
    style: &Style,
    pass: Pass,
    view: &View,
    canvas: &mut Canvas,
) {
    let Some(transform) = &style.geom_transform else {
        return;
    };
    if let GeomTransform::Expression(tree) = transform {
        // A tree the engine does not draw draws nothing; the engine reports it.
        if let TransformTree::Drawn(shape) = tree {
            expression(ops, geometry, shape, style, pass, view, canvas);
        }
        return;
    }
    if *transform == GeomTransform::Bbox {
        bbox(ops, geometry, &pass.style(style, canvas), view, canvas);
        return;
    }
    if of_symbol_zero(style) {
        return;
    }
    // A pass's condition, a COLOR or an OUTLINECOLOR, does not apply here, and the engine's
    // `Plan::passes` never hands this function `PlainIfColored`, `PlainIfColoredOrPixmap`, or
    // `OutlineIfOutlined`: a later transformed style of a cached class takes `Plain`, the first
    // `Plain` or `Outline`, as lineLayerDrawShape (src/mapdraw.c:2140-2149) hands every style with
    // a GEOMTRANSFORM in its scale range to msDrawTransformedShape without testing a colour,
    // whatever MS_DRAWMODE_SINGLESTYLE asks, once: in a cached layer in its first pass, where only
    // the first style is prepared for its outline (1365-1378), and in an uncached one as written.
    // The cache's later passes, which test MS_VALID_COLOR, skip such a style (1443-1444). Which
    // colours a marker draws, msDrawMarkerSymbolInternal decides (`marker_ops`).
    let style = &*pass.style(style, canvas);
    let placed = geom_transform::placed(transform, geometry, view, canvas.image());
    if placed.is_empty() {
        return;
    }
    // What one symbol costs, the same at every point and angle.
    let Some(price) = marker_price(style, canvas) else {
        return;
    };
    if !canvas.markers.take(placed.len(), price) {
        return;
    }
    let mut turned = style.auto_angle.then(|| style.clone());
    for (at, angle) in placed {
        match (&mut turned, angle) {
            (Some(turned), Some(angle)) => {
                turned.angle = Some(Bound::Value(angle));
                marker_ops(ops, at, turned, canvas);
            }
            _ => marker_ops(ops, at, style, canvas),
        }
    }
}

/// A style of the transform tree `shape` on `geometry`, a shape of a line or a polygon layer, as
/// msDrawTransformedShape draws it (`src/mapgeomtransform.c:198-240`,
/// ADR-0043): the tree evaluated on the unclipped shape
/// in pixels (see [`in_pixels`]), as msDrawShape hands it on; a line result drawn as
/// msDrawLineSymbol draws it, a point or a polygon result as msDrawShadeSymbol draws it, a hatch
/// refused counted (Q-9), each with the style the pass prepares. A shape left without a point in
/// pixels draws nothing and is not evaluated, as msDrawShape returns before it draws a style of a
/// shape msTransformShapeSimplify empties (`src/mapdraw.c:2506-2507`, `2577-2580`). The
/// evaluation charges the layer's budget of the canvas's style trees, which keeps its failure for
/// the engine to report; one past a bound of Q-9, or failed, draws nothing, and so does a canvas
/// without style trees.
fn expression(
    ops: &mut Vec<DrawOp>,
    geometry: &Geometry,
    shape: &TransformShape,
    style: &Style,
    pass: Pass,
    view: &View,
    canvas: &mut Canvas,
) {
    let in_pixels = in_pixels(geometry, view);
    if vertices(&in_pixels).is_empty() {
        return;
    }
    let Some(style_trees) = canvas.style_trees.as_mut() else {
        return;
    };
    let Some((kind, parts)) = style_trees.evaluated(shape, &in_pixels) else {
        return;
    };
    let points = |part: Vec<Coord>| -> Vec<Point> {
        part.into_iter().map(|c| Point { x: c.x, y: c.y }).collect()
    };
    match kind {
        Kind::Line => {
            // A part of fewer than 2 points strokes nothing in the AGG renderer.
            let paths = parts
                .into_iter()
                .filter(|part| part.len() >= 2)
                .map(points)
                .collect();
            line_symbol(ops, paths, style, pass, canvas);
        }
        Kind::Point | Kind::Polygon => {
            // msDrawShadeSymbol draws nothing of a shape without parts.
            let rings: Vec<Vec<Point>> = parts
                .into_iter()
                .filter(|part| !part.is_empty())
                .map(points)
                .collect();
            if rings.is_empty() {
                return;
            }
            let drawn = shade(ops, rings, &pass.style(style, canvas), view, canvas);
            canvas.hatching.refused += usize::from(!drawn);
        }
    }
}

/// `geometry` in pixels, its coordinates pixels, as msDrawShape transforms a shape of a line or a
/// polygon layer before it hands it to msDrawTransformedShape: each line by
/// [`View::line_to_pixels`], each ring by [`View::ring_to_pixels`], a multipolygon's rings listed
/// flat as MapServer's polygon holds them. A part the simplification leaves without points is
/// kept empty, as msTransformShapeSimplify keeps it with no points
/// (`src/mapprimitive.cpp:968-969`, `996-997`). A collection, no shape of MapServer's, is kept a
/// collection, which the evaluation refuses.
fn in_pixels(geometry: &Geometry, view: &View) -> Geometry {
    let coord = |p: Point| Coord { x: p.x, y: p.y };
    let coords = |points: Vec<Point>| -> Vec<Coord> { points.into_iter().map(coord).collect() };
    let lines = |parts: &[Vec<Coord>]| -> Vec<Vec<Coord>> {
        parts
            .iter()
            .map(|l| coords(view.line_to_pixels(l)))
            .collect()
    };
    let rings = |parts: &[Vec<Coord>]| -> Vec<Vec<Coord>> {
        parts
            .iter()
            .map(|r| coords(view.ring_to_pixels(r)))
            .collect()
    };
    match geometry {
        Geometry::Point(c) => Geometry::Point(coord(view.point_to_pixel(*c))),
        Geometry::MultiPoint(cs) => {
            Geometry::MultiPoint(cs.iter().map(|&c| coord(view.point_to_pixel(c))).collect())
        }
        Geometry::LineString(line) => Geometry::MultiLineString(lines(std::slice::from_ref(line))),
        Geometry::MultiLineString(parts) => Geometry::MultiLineString(lines(parts)),
        Geometry::Polygon(parts) => Geometry::Polygon(rings(parts)),
        Geometry::MultiPolygon(polygons) => Geometry::Polygon(rings(&polygons.concat())),
        Geometry::Collection(geometries) => {
            Geometry::Collection(geometries.iter().map(|g| in_pixels(g, view)).collect())
        }
    }
}

/// A style transformed by `bbox` on a shape of a line or a polygon layer, as
/// msDrawTransformedShape hands its bounding box to msDrawShadeSymbol
/// (`src/mapgeomtransform.c:161-188`): the ring of [`geom_transform::bbox`] shaded as a polygon,
/// filled, outlined, hatched, or tiled with a pixmap as [`shade`] draws one, symbol 0 among them;
/// the padding takes the style's width, 1 where it has none, and its size, -1 where it has none,
/// as initStyle sets them. A hatch refused is counted (Q-9).
fn bbox(
    ops: &mut Vec<DrawOp>,
    geometry: &Geometry,
    style: &Style,
    view: &View,
    canvas: &mut Canvas,
) {
    let width = style.width.as_ref().and_then(Bound::value).unwrap_or(1.0);
    let size = style.size.as_ref().and_then(Bound::value).unwrap_or(-1.0);
    let Some(ring) = geom_transform::bbox(geometry, view, canvas.size, width, size) else {
        return;
    };
    let drawn = shade(ops, vec![ring], style, view, canvas);
    canvas.hatching.refused += usize::from(!drawn);
}

#[cfg(test)]
mod tests;
