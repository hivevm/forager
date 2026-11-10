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
use forager_style_ir::{Bound, Coord, Geometry, RenderMode, Style, Symbol};

use crate::canvas::Canvas;
use crate::extent::View;
use crate::geom_transform;
use crate::hatch;
use crate::line::{Pass, gapped, line_symbol, open_rings};
use crate::marker::{marker_ops, marker_price, of_symbol_zero};
use crate::shade::shade;
use crate::symbol;

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

/// The image path of the pixmap symbol that [`draw_ops`] draws as the mark of `style` in a layer
/// of `mode`: at each point of a point layer, at each point a `GEOMTRANSFORM` places on a line or
/// a polygon, along each line of a line layer every `GAP` pixels, and tiled over each polygon of
/// a polygon layer. The engine loads it before the layer is drawn, since this crate does no I/O.
pub fn marked_pixmap<'a>(
    mode: RenderMode,
    style: &Style,
    symbols: &'a [Symbol],
) -> Option<&'a str> {
    let marked = matches!(mode, RenderMode::Point | RenderMode::Polygon)
        || style.geom_transform.is_some()
        || (mode == RenderMode::Line && gapped(style));
    match symbol::mark(style, symbols) {
        symbol::Mark::Pixmap(_, symbol) if marked => symbol.image.as_deref(),
        _ => None,
    }
}

/// The image paths of the pixmap symbols that msDrawShape counts in the clip buffer of a class's
/// `styles` at their images' heights, having loaded them whatever their scale range
/// (`src/mapdraw.c:2466-2490`): those of a class with a hatch, the one style the clip buffer
/// reaches here (see [`hatch::Hatching::for_class`]).
pub fn clip_pixmaps<'a>(styles: &[Style], symbols: &'a [Symbol]) -> Vec<&'a str> {
    if !styles.iter().any(|style| symbol::is_hatch(style, symbols)) {
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
    // whatever the layer, a part it leaves without points dropped.
    let lines = |parts: &[Vec<Coord>]| -> Vec<Vec<Point>> {
        parts
            .iter()
            .map(|l| view.line_to_pixels(l))
            .filter(|l| !l.is_empty())
            .collect()
    };
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
            let paths = lines(std::slice::from_ref(points));
            line_symbol(&mut ops, paths, style, pass, canvas);
        }
        (RenderMode::Line, Geometry::MultiLineString(parts)) => {
            line_symbol(&mut ops, lines(parts), style, pass, canvas);
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
/// msDrawTransformedShape draws it: its symbol at each point the transform places, turned there
/// where the style's angle is `AUTO` and the transform gives one; nothing for a style without a
/// symbol or with symbol 0, as msDrawMarkerSymbol draws none. The outline pass of a line layer's
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
    let Some(transform) = style.geom_transform else {
        return;
    };
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

#[cfg(test)]
mod tests;
