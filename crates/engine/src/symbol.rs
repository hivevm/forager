// Vector and ellipse symbols at a point follow MapServer's msDrawMarkerSymbol, msAdjustMarkerPos,
// and computeSymbolStyle, src/maprendering.c, msSymbolGetDefaultSize and msGetMarkerSize,
// src/mapsymbol.c, and agg2RenderVectorSymbol and agg2RenderEllipseSymbol, src/mapagg.cpp, of the
// release pinned in scripts/fetch-msautotest.sh, and the ellipse and affine transformation of the
// AGG it bundles, src/renderers/agg/include/agg_ellipse.h and src/agg_trans_affine.cpp.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota (src/mapagg.cpp).
// Copyright (c) 1996-2011 Regents of the University of Minnesota (src/maprendering.c).
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

//! The marks a point style draws: a vector or an ellipse symbol, sized, rotated, and anchored as
//! MapServer's AGG renderer draws it at a point. Its trigonometry is libm's, the same code on
//! every target, so that the native and the browser build draw the same bytes (Q-2).

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point};
use forager_style_ir::{Bound, Rgba, Style, Symbol, SymbolKind, SymbolRef};

use crate::draw::fixed;

/// MapServer's `MS_DEG_TO_RAD`, `src/mapserver.h`.
const DEG_TO_RAD: f64 = 0.017_453_292_519_943_295;

/// MapServer's default `MINSIZE` and `MAXSIZE` of a style, `MS_MINSYMBOLSIZE` and
/// `MS_MAXSYMBOLSIZE`, and its `MINWIDTH` and `MAXWIDTH`, `MS_MINSYMBOLWIDTH` and
/// `MS_MAXSYMBOLWIDTH`.
const MIN_SIZE: f64 = 0.0;
const MAX_SIZE: f64 = 500.0;
const MIN_WIDTH: f64 = 0.0;
const MAX_WIDTH: f64 = 32.0;

/// The most vertices of one ellipse. AGG takes as many as the radius asks for, about 2π√r: 16
/// for a radius of 6, some 2000 for 100 000 pixels; this many are reached at a radius of some
/// 425 000 pixels, far past any image, and bound what a hostile symbol costs (Q-9).
const MAX_ELLIPSE_STEPS: u32 = 4096;

/// What a point style draws at each point.
#[derive(Debug, PartialEq)]
pub(crate) enum Mark<'a> {
    /// A square of the style's size: a style without a symbol, or with one not drawn yet.
    Square,
    /// A vector or ellipse symbol.
    Symbol(&'a Symbol),
    /// Nothing, for an index past the last symbol, as msDrawMarkerSymbol draws nothing.
    Nothing,
}

/// The mark of `style` among `symbols`, MapServer's symbols from index 1.
pub(crate) fn mark<'a>(style: &Style, symbols: &'a [Symbol]) -> Mark<'a> {
    let Some(Bound::Value(SymbolRef::Index(index))) = &style.symbol else {
        return Mark::Square;
    };
    let Some(position) = (*index as usize).checked_sub(1) else {
        return Mark::Square;
    };
    match symbols.get(position) {
        None => Mark::Nothing,
        Some(symbol) if matches!(symbol.kind, SymbolKind::Vector | SymbolKind::Ellipse) => {
            Mark::Symbol(symbol)
        }
        Some(_) => Mark::Square,
    }
}

/// Whether the style's symbol among `symbols` is a hatch.
pub(crate) fn is_hatch(style: &Style, symbols: &[Symbol]) -> bool {
    let Some(Bound::Value(SymbolRef::Index(index))) = &style.symbol else {
        return false;
    };
    (*index as usize)
        .checked_sub(1)
        .and_then(|position| symbols.get(position))
        .is_some_and(|symbol| symbol.kind == SymbolKind::Hatch)
}

/// How a symbol is drawn, computeSymbolStyle's `symbolStyleObj` at a scale factor of 1.
#[derive(Debug, PartialEq)]
struct SymbolStyle {
    fill: Option<Rgba>,
    outline: Option<Rgba>,
    outline_width: f64,
    /// Of the symbol's extent to pixels.
    scale: f64,
    /// Radians, counter-clockwise.
    rotation: f64,
}

/// msSymbolGetDefaultSize for a vector or ellipse symbol: its height, or its width where it has
/// none, and 1 where neither is positive.
fn default_size(symbol: &Symbol) -> f64 {
    let [size_x, size_y] = symbol.extent.unwrap_or([0.0, 0.0]);
    let size = if size_y <= 0.0 { size_x } else { size_y };
    if size > 0.0 { size } else { 1.0 }
}

/// msSymbolGetDefaultSize of the style's symbol: of a vector or an ellipse its height, or its
/// width; 1 for MapServer's default symbol, for any other kind, and where none is found.
pub(crate) fn default_size_of(style: &Style, symbols: &[Symbol]) -> f64 {
    let Some(Bound::Value(SymbolRef::Index(index))) = &style.symbol else {
        return 1.0;
    };
    match (*index as usize)
        .checked_sub(1)
        .and_then(|i| symbols.get(i))
    {
        Some(symbol) if matches!(symbol.kind, SymbolKind::Vector | SymbolKind::Ellipse) => {
            default_size(symbol)
        }
        _ => 1.0,
    }
}

/// MapServer's `MS_MAX(a, b)` and `MS_MIN(a, b)`, which keep `b` where they cannot compare.
pub(crate) fn ms_max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

pub(crate) fn ms_min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// computeSymbolStyle for a vector or ellipse symbol. A filled symbol is filled with the style's
/// colour and outlined with its outline colour; one not filled is outlined with its colour, or
/// its outline colour where it has none. The outline is the style's width.
fn symbol_style(style: &Style, symbol: &Symbol) -> SymbolStyle {
    let default_size = default_size(symbol);
    let style_size = fixed(&style.size).unwrap_or(default_size);
    let color = fixed(&style.color);
    let outline_color = fixed(&style.outline_color);
    let (fill, outline) = if symbol.filled {
        (color, outline_color)
    } else {
        (None, color.or(outline_color))
    };
    let target = ms_max(style_size, style.min_size.unwrap_or(MIN_SIZE));
    let target = ms_min(target, style.max_size.unwrap_or(MAX_SIZE));
    let outline_width = if outline.is_some() {
        let width = fixed(&style.width).unwrap_or(1.0);
        let width = ms_max(width, style.min_width.unwrap_or(MIN_WIDTH));
        ms_min(width, style.max_width.unwrap_or(MAX_WIDTH))
    } else {
        0.0
    };
    SymbolStyle {
        fill,
        outline,
        outline_width,
        scale: target / default_size,
        rotation: fixed(&style.angle).unwrap_or(0.0) * DEG_TO_RAD,
    }
}

/// msGetMarkerSize for a vector or ellipse symbol: the width and height it is drawn at, from a
/// size cut to whole pixels, or its own extent for a style without a size.
fn marker_size(style: &Style, symbol: &Symbol) -> (f64, f64) {
    let [size_x, size_y] = symbol.extent.unwrap_or([0.0, 0.0]);
    let size = fixed(&style.size);
    // `size` is an int there: each assignment cuts the fraction.
    let pixels = size.unwrap_or_else(|| default_size(symbol)).trunc();
    let pixels = ms_max(pixels, style.min_size.unwrap_or(MIN_SIZE)).trunc();
    let pixels = ms_min(pixels, style.max_size.unwrap_or(MAX_SIZE)).trunc();
    match size {
        Some(size) if size > 0.0 => (pixels / size_y * size_x, pixels),
        _ => (size_x, size_y),
    }
}

/// msAdjustMarkerPos without the offsets: the point moved so that the symbol's anchor point,
/// turned with the symbol, lies on `at`.
fn anchored(at: Point, style: &Style, symbol: &Symbol, rotation: f64) -> Point {
    let [anchor_x, anchor_y] = symbol.anchor_point.unwrap_or([0.5, 0.5]);
    if anchor_x == 0.5 && anchor_y == 0.5 {
        return at;
    }
    let (width, height) = marker_size(style, symbol);
    let ox = (0.5 - anchor_x) * width;
    let oy = (0.5 - anchor_y) * height;
    if rotation == 0.0 {
        return Point {
            x: at.x + ox,
            y: at.y + oy,
        };
    }
    let (sin, cos) = (libm::sin(-rotation), libm::cos(-rotation));
    Point {
        x: at.x + (ox * cos - oy * sin),
        y: at.y + (ox * sin + oy * cos),
    }
}

/// The drawing operations of `symbol` at the pixel `at` in `style`, as msDrawMarkerSymbol and
/// the AGG renderer draw a vector or an ellipse symbol: filled first, then outlined.
pub(crate) fn symbol_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style, symbol: &Symbol) {
    let s = symbol_style(style, symbol);
    if (s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0 {
        return;
    }
    let at = anchored(at, style, symbol, s.rotation);
    let (paths, closed) = match symbol.kind {
        SymbolKind::Vector => (vector(symbol, at, &s), false),
        SymbolKind::Ellipse => (vec![ellipse(symbol, at, &s)], true),
        _ => return,
    };
    if let Some(color) = s.fill {
        ops.push(DrawOp::Fill {
            rings: paths.clone(),
            color,
        });
    }
    // agg2RenderEllipseSymbol outlines only at a width other than 0; a vector symbol's outline
    // of width 0 covers nothing.
    if let Some(color) = s.outline
        && s.outline_width != 0.0
    {
        // AGG's defaults, which agg2RenderVectorSymbol and agg2RenderEllipseSymbol leave as they
        // are: butt caps and miter joins. A vector symbol's path is open, an ellipse closed.
        ops.push(DrawOp::Stroke {
            paths,
            closed,
            color,
            width: s.outline_width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dashes: Dashes::default(),
        });
    }
}

/// agg2RenderVectorSymbol's paths: the symbol's centre on the origin, scaled, turned, and moved
/// to `at`.
fn vector(symbol: &Symbol, at: Point, s: &SymbolStyle) -> Vec<Vec<Point>> {
    let [size_x, size_y] = symbol.extent.unwrap_or([0.0, 0.0]);
    let matrix = Affine::IDENTITY
        .then(Affine::translation(-size_x * 0.5, -size_y * 0.5))
        .then(Affine::scaling(s.scale))
        .then(Affine::rotation(-s.rotation))
        .then(Affine::translation(at.x, at.y));
    symbol
        .paths
        .iter()
        .map(|path| path.iter().map(|&[x, y]| matrix.apply(x, y)).collect())
        .collect()
}

/// agg2RenderEllipseSymbol's ellipse: AGG's polygon of its width and height scaled, around `at`,
/// turned about it.
fn ellipse(symbol: &Symbol, at: Point, s: &SymbolStyle) -> Vec<Point> {
    let [size_x, size_y] = symbol.extent.unwrap_or([0.0, 0.0]);
    let points = agg_ellipse(at.x, at.y, size_x * s.scale / 2.0, size_y * s.scale / 2.0);
    if s.rotation == 0.0 {
        return points;
    }
    let matrix = Affine::IDENTITY
        .then(Affine::translation(-at.x, -at.y))
        .then(Affine::rotation(-s.rotation))
        .then(Affine::translation(at.x, at.y));
    points.iter().map(|p| matrix.apply(p.x, p.y)).collect()
}

/// The vertices of AGG's `ellipse` with its step count calculated at an approximation scale of
/// 1, counter-clockwise in AGG's sense from the angle 0, at most [`MAX_ELLIPSE_STEPS`].
fn agg_ellipse(x: f64, y: f64, rx: f64, ry: f64) -> Vec<Point> {
    use std::f64::consts::PI;
    let ra = (rx.abs() + ry.abs()) / 2.0;
    let da = libm::acos(ra / (ra + 0.125)) * 2.0;
    // AGG's uround: `unsigned(v + 0.5)`; Rust's cast saturates where C's would not be defined.
    let num = ((2.0 * PI / da + 0.5) as u32).min(MAX_ELLIPSE_STEPS);
    (0..num)
        .map(|step| {
            let angle = f64::from(step) / f64::from(num) * 2.0 * PI;
            Point {
                x: x + libm::cos(angle) * rx,
                y: y + libm::sin(angle) * ry,
            }
        })
        .collect()
}

/// AGG's `trans_affine`: `x' = sx·x + shx·y + tx`, `y' = shy·x + sy·y + ty`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Affine {
    sx: f64,
    shy: f64,
    shx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
}

impl Affine {
    const IDENTITY: Self = Self {
        sx: 1.0,
        shy: 0.0,
        shx: 0.0,
        sy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    fn translation(x: f64, y: f64) -> Self {
        Self {
            tx: x,
            ty: y,
            ..Self::IDENTITY
        }
    }

    fn scaling(s: f64) -> Self {
        Self {
            sx: s,
            sy: s,
            ..Self::IDENTITY
        }
    }

    fn rotation(a: f64) -> Self {
        Self {
            sx: libm::cos(a),
            shy: libm::sin(a),
            shx: -libm::sin(a),
            sy: libm::cos(a),
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// This transformation followed by `m`, AGG's `multiply` (`*=`), in its order of operations.
    fn then(self, m: Self) -> Self {
        Self {
            sx: self.sx * m.sx + self.shy * m.shx,
            shx: self.shx * m.sx + self.sy * m.shx,
            tx: self.tx * m.sx + self.ty * m.shx + m.tx,
            shy: self.sx * m.shy + self.shy * m.sy,
            sy: self.shx * m.shy + self.sy * m.sy,
            ty: self.tx * m.shy + self.ty * m.sy + m.ty,
        }
    }

    fn apply(&self, x: f64, y: f64) -> Point {
        Point {
            x: x * self.sx + y * self.shx + self.tx,
            y: x * self.shy + y * self.sy + self.ty,
        }
    }
}

#[cfg(test)]
mod tests;
