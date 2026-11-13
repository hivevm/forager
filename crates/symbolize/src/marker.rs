// The drawing of a marker at a point follows MapServer's msDrawMarkerSymbol() and
// msAdjustMarkerPos(), src/maprendering.c, of the release pinned in scripts/fetch-msautotest.sh.
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

use std::rc::Rc;

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point, PremultipliedImage};
use forager_style_ir::{Bound, Style, Symbol, SymbolRef};

use crate::bounds::{self, MAX_SIZE};
use crate::canvas::Canvas;
use crate::line::line_ends;
use crate::line_marker::MarkerBudget;
use crate::style::fixed;
use crate::symbol::{self, Factors};

/// What msDrawMarkerSymbol draws of `style` at the pixel `at` at the canvas's factor: its
/// symbol, or the square that stands for a symbol not drawn yet, moved as msAdjustMarkerPos
/// moves it. The square is Forager's, not MapServer's, which draws nothing for symbol 0; it is
/// drawn at a factor of 1. A pixmap whose image the canvas lacks draws nothing, and the canvas
/// records why.
pub(crate) fn marker_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style, canvas: &mut Canvas) {
    match symbol::mark(style, canvas.symbols) {
        symbol::Mark::Square => point_ops(ops, symbol::shifted(at, style, 1.0), style),
        symbol::Mark::Symbol(s) => {
            symbol::symbol_ops(ops, at, style, s, canvas.factors);
        }
        symbol::Mark::Pixmap(index, s) => {
            if let Some(image) = canvas.pixmap(index, s) {
                pixmap_ops(ops, at, style, s, image, canvas.factors);
            }
        }
        symbol::Mark::Nothing => {}
    }
}

/// Whether `style` names no symbol, or symbol 0, MapServer's default, which msDrawMarkerSymbol
/// does not draw.
pub(crate) fn of_symbol_zero(style: &Style) -> bool {
    matches!(style.symbol, None | Some(Bound::Value(SymbolRef::Index(0))))
}

/// The price in points of the symbol `style` draws at a point (see [`MarkerBudget::price`]),
/// the same at every point: what [`marker_ops`] draws of it at the origin.
pub(crate) fn marker_price(style: &Style, canvas: &mut Canvas) -> Option<usize> {
    let mut probe = Vec::new();
    marker_ops(&mut probe, Point { x: 0.0, y: 0.0 }, style, canvas);
    MarkerBudget::price(&probe)
}

/// A pixmap's `image` at the pixel `at` in `style` at `factors`, as msDrawMarkerSymbol draws it:
/// nothing at a scale of 0; moved by the style's offsets and by the symbol's anchor point, of
/// the size msGetMarkerSize gives the image (msAdjustMarkerPos); then drawn as
/// agg2RenderPixmapSymbol draws it (see [`symbol::render_pixmap`]). A style's colour and opacity
/// do not reach the image (computeSymbolStyle).
fn pixmap_ops(
    ops: &mut Vec<DrawOp>,
    at: Point,
    style: &Style,
    symbol: &Symbol,
    image: Rc<PremultipliedImage>,
    factors: Factors,
) {
    let (scale, rotation, _) = symbol::pixmap_style(style, image.height(), factors);
    let factor = factors.scale;
    if scale == 0.0 {
        return;
    }
    let at = symbol::anchored(symbol::shifted(at, style, factor), symbol, rotation, || {
        symbol::pixmap_marker_size(
            style,
            fixed(&style.size),
            image.width(),
            image.height(),
            factor,
        )
    });
    ops.push(symbol::render_pixmap(at, image, scale, rotation));
}

/// A point as a square of the style's size, centred on it, filled in the `COLOR` and outlined
/// in the `OUTLINECOLOR` at the style's width, bounded as computeSymbolStyle() bounds the outline
/// of the symbol the square stands for, at a factor of 1 (see [`bounds::symbol_outline_width`]).
fn point_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style) {
    let half = fixed(&style.size).unwrap_or(1.0).clamp(0.0, MAX_SIZE) / 2.0;
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
            gamma: None,
        });
    }
    if let Some(color) = fixed(&style.outline_color) {
        let width = bounds::symbol_outline_width(fixed(&style.width).unwrap_or(1.0), style, 1.0);
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
mod tests;
