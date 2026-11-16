// The circle of a circle layer's shape follows MapServer's circleLayerDrawShape(), src/mapdraw.c,
// msCircleDrawShadeSymbol(), src/maprendering.c, and msRasterizeArc(), src/mapgeomutil.cpp, of the
// release pinned in scripts/fetch-msautotest.sh, and the arc of the AGG it bundles,
// src/renderers/agg/src/agg_arc.cpp.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota (src/maprendering.c,
// src/mapgeomutil.cpp).
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
//
// Anti-Grain Geometry - Version 2.4
// Copyright (C) 2002-2005 Maxim Shemanarev (http://www.antigrain.com)
// Permission to copy, use, modify, sell and distribute this software is granted provided this
// copyright notice appears in all copies. This software is provided "as is" without express or
// implied warranty, and with no claim as to its suitability for any purpose.

//! The circle a layer of the render mode circle draws of a shape. Its trigonometry is libm's, the
//! same code on every target, so that the native and the browser build draw the same bytes (Q-2).

use std::f64::consts::PI;

use forager_render::{DrawOp, Point};
use forager_style_ir::c::{DEG_TO_RAD, lrint};
use forager_style_ir::{Coord, Geometry, Style};

use crate::canvas::Canvas;
use crate::extent::View;
use crate::shade::shade;
use crate::symbol::MAX_ELLIPSE_STEPS;

/// A circle of a circle layer, in the coordinates of its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    pub centre: Coord,
    pub radius: f64,
}

/// The circle circleLayerDrawShape takes from a shape (`src/mapdraw.c:1988-2006`): one of exactly
/// one part of exactly two points, centred on their midpoint, its radius half their distance in
/// x, or in y where that is 0. None for any other shape and for a radius of 0, which MapServer
/// skips without failing, and none for a centre or a radius of no finite number, which it would
/// draw as nothing.
pub fn circle(geometry: &Geometry) -> Option<Circle> {
    let part = match geometry {
        Geometry::LineString(points) => points.as_slice(),
        Geometry::MultiPoint(parts)
        | Geometry::MultiLineString(parts)
        | Geometry::Polygon(parts) => match parts.as_slice() {
            [part] => part.as_slice(),
            _ => return None,
        },
        Geometry::MultiPolygon(polygons) => match polygons.as_slice() {
            [rings] => match rings.as_slice() {
                [ring] => ring.as_slice(),
                _ => return None,
            },
            _ => return None,
        },
        Geometry::Point(_) | Geometry::Collection(_) => return None,
    };
    let [p0, p1] = part else {
        return None;
    };
    let centre = Coord {
        x: (p0.x + p1.x) / 2.0,
        y: (p0.y + p1.y) / 2.0,
    };
    let mut radius = (centre.x - p0.x).abs();
    if radius == 0.0 {
        radius = (centre.y - p0.y).abs();
    }
    let finite = centre.x.is_finite() && centre.y.is_finite() && radius.is_finite();
    (radius != 0.0 && finite).then_some(Circle { centre, radius })
}

/// The drawing operations of `style` on `circle`, its centre in the map's projection, as
/// circleLayerDrawShape hands it to msCircleDrawShadeSymbol (`src/mapdraw.c:2008-2029`): the
/// centre turned into the nearest pixel by `MS_MAP2IMAGE_X` and `MS_MAP2IMAGE_Y`, the radius
/// divided by the cellsize, and the polygon of [`rasterize_arc`] shaded as [`shade`] shades one
/// (`src/maprendering.c:1183-1196`). A hatch refused is counted (Q-9).
pub fn circle_ops(circle: Circle, style: &Style, view: &View, canvas: &mut Canvas) -> Vec<DrawOp> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "MS_NINT's long, stored in the double of a pointObj, as C converts it"
    )]
    let centre = Point {
        x: lrint((circle.centre.x - view.extent.min_x) / view.cellsize) as f64,
        y: lrint((view.extent.max_y - circle.centre.y) / view.cellsize) as f64,
    };
    let radius = circle.radius / view.cellsize;
    let mut ops = Vec::new();
    // A radius past any double draws nothing, where AGG's arc would place its points nowhere.
    if !radius.is_finite() {
        return ops;
    }
    let ring = rasterize_arc(centre, radius);
    let drawn = shade(&mut ops, vec![ring], style, view, canvas);
    canvas.hatching.refused += usize::from(!drawn);
    ops
}

/// msRasterizeArc's ring of a full circle about `centre` of `radius`, from 0° to 360°: the
/// vertices of AGG's `arc` at an approximation scale of 1, closed by its first where its last
/// differs. Its step is AGG's but where that would take more than [`MAX_ELLIPSE_STEPS`] steps,
/// at a radius of some 425 000 pixels, far past any image, where it takes that many (Q-9).
fn rasterize_arc(centre: Point, radius: f64) -> Vec<Point> {
    let (start, end) = (0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD);
    // arc::normalize, counter-clockwise, its end past its start.
    let ra = (radius.abs() + radius.abs()) / 2.0;
    let mut da = libm::acos(ra / (ra + 0.125)) * 2.0;
    if 2.0 * PI / da > f64::from(MAX_ELLIPSE_STEPS) {
        da = 2.0 * PI / f64::from(MAX_ELLIPSE_STEPS);
    }
    let at = |angle: f64| Point {
        x: centre.x + libm::cos(angle) * radius,
        y: centre.y + libm::sin(angle) * radius,
    };
    // arc::vertex: each step's vertex until the angle reaches the end less a quarter step, then
    // the end's.
    let mut ring = Vec::new();
    let mut angle = start;
    while angle < end - da / 4.0 {
        ring.push(at(angle));
        angle += da;
    }
    ring.push(at(end));
    if ring[ring.len() - 1] != ring[0] {
        ring.push(ring[0]);
    }
    ring
}

#[cfg(test)]
mod tests;
