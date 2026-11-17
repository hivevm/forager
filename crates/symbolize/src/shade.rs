// The drawing of a polygon in a style follows MapServer's msDrawShadeSymbol(),
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

use forager_render::{DrawOp, LineCap, LineJoin, Pen, Point};
use forager_style_ir::c::{ms_compute_bounds, rect_overlap};
use forager_style_ir::{Rgba, Style, Symbol, SymbolKind};

use crate::bounds;
use crate::canvas::Canvas;
use crate::extent::View;
use crate::hatch::{self, Hatched};
use crate::line::{Pass, line_ends, line_symbol, moved, open_rings, style_offset};
use crate::mark;
use crate::style::{GAMMA, fixed};
use crate::symbol;
use crate::tile::TileFor;

/// A polygon drawn in a style, as msDrawShadeSymbol() draws one at the canvas's factor: nothing
/// for a symbol past the last or a name no symbol carries; with a pixmap, whatever its colours,
/// moved by the offset scaled and filled with the image tiled (see [`tiled`]); without a `COLOR`,
/// its outline as msDrawLineSymbol draws a line in the style (see [`line_symbol`]); with one, moved
/// by the offset scaled, and with a hatch symbol hatched (see [`hatch::hatch`]), with a vector or
/// an ellipse symbol filled with the symbol tiled, seamless for a vector without a `GAP`, with a
/// TrueType symbol filled with its glyph tiled (see [`symbol_tiled`]), with any other filled, and
/// outlined in the `OUTLINECOLOR` at the colour's alpha, with round caps and joins, its width
/// scaled, the factor itself for a width of 0, and bounded by `MAXWIDTH` and `MINWIDTH` unscaled,
/// as MapServer bounds it there. False where a hatch is refused (Q-9).
pub fn shade<T: Clone>(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style<T>,
    view: &View,
    canvas: &mut Canvas,
) -> bool {
    let mark = symbol::mark(style, canvas.symbols);
    match mark {
        // No such symbol (src/maprendering.c:742-743).
        symbol::Mark::Nothing => return true,
        // A pixmap needs no colour (753), and draws neither its COLOR nor an outline.
        symbol::Mark::Pixmap(index, symbol) => {
            tiled(ops, rings, style, index, symbol, canvas);
            return true;
        }
        _ => {}
    }
    let Some(color) = fixed(&style.color) else {
        line_symbol(ops, open_rings(rings), style, Pass::Plain, canvas);
        return true;
    };
    let factor = canvas.factors.scale;
    let rings = moved(rings, style_offset(style, factor));
    match mark {
        symbol::Mark::Hatch => {
            return match hatch::hatch(
                rings,
                style,
                color,
                canvas.size,
                hatch::reference(canvas.own_image, view),
                canvas.factors,
            ) {
                Hatched::Drawn(hatch) => {
                    ops.push(DrawOp::Hatch(hatch));
                    true
                }
                Hatched::Nothing => true,
                Hatched::Refused => false,
            };
        }
        symbol::Mark::Symbol(..) | symbol::Mark::Glyph(..) => {
            symbol_tiled(ops, rings, style, mark, canvas);
            return true;
        }
        symbol::Mark::Square | symbol::Mark::Pixmap(..) | symbol::Mark::Nothing => {}
    }
    ops.push(DrawOp::Fill {
        rings: rings.clone(),
        color,
        gamma: Some(GAMMA),
    });
    if let Some(outline) = fixed(&style.outline_color) {
        let width = fixed(&style.width).unwrap_or(1.0);
        let width = if width == 0.0 { factor } else { width * factor };
        let width = bounds::shade_outline_width(width, style);
        let (cap, join, inner_join) = line_ends(width, LineCap::Round, LineJoin::Round);
        ops.push(DrawOp::Stroke {
            paths: open_rings(rings),
            closed: false,
            color: Rgba {
                a: color.a,
                ..outline
            },
            pen: Pen::new(width, cap, join, inner_join),
        });
    }
    true
}

/// The pixmap symbol `symbol`, at `index` from 1, over a polygon, as msDrawShadeSymbol tiles it:
/// computeSymbolStyle's scale, rotation, and gap at the canvas's factor give its tile from the
/// render's cache (see [`mark::pixmap_tile`]), and the polygon, moved by the offset scaled, is
/// filled with it. Where the image has not loaded or its tile is refused (Q-9), nothing is drawn,
/// and the canvas records why. A polygon moved wholly off the image builds no tile, which would
/// draw nothing and take bytes from the tiles of the polygons on it.
fn tiled<T>(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style<T>,
    index: usize,
    symbol: &Symbol,
    canvas: &mut Canvas,
) {
    let Some(image) = canvas.pixmap(index, symbol) else {
        return;
    };
    let rings = moved(rings, style_offset(style, canvas.factors.scale));
    if !meets_image(&rings, canvas.image()) {
        return;
    }
    if let Some(tile) = mark::pixmap_tile(style, index, image, TileFor::Polygon, canvas) {
        ops.push(DrawOp::TiledFill { rings, tile });
    }
}

/// The vector, ellipse, or TrueType symbol of `mark` over `rings`, already moved by the offset, as
/// msDrawShadeSymbol tiles it: computeSymbolStyle's style at the canvas's factors, the tile of its
/// size, scale, and gap (see [`mark::tile`]), and the polygon filled with it. The tile is seamless
/// for a vector symbol whose `GAP` is 0, as msDrawShadeSymbol sets it under AGG
/// (`src/maprendering.c:876-883`), and never for an ellipse, which getTile draws once at the
/// tile's centre, or a TrueType symbol. With the `COLOR` checked, computeSymbolStyle gives a fill
/// or an outline colour, so the check of msDrawShadeSymbol for neither cannot fail here. Where
/// the tile is refused (Q-9), nothing is drawn, and the canvas records why. A polygon wholly off
/// the image builds no tile.
fn symbol_tiled<T>(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style<T>,
    mark: symbol::Mark,
    canvas: &mut Canvas,
) {
    if !meets_image(&rings, canvas.image()) {
        return;
    }
    let vector = matches!(mark, symbol::Mark::Symbol(_, s) if s.kind == SymbolKind::Vector);
    let seamless = vector && style.gap.unwrap_or(0.0) == 0.0;
    if let Some(tile) = mark::tile(mark, style, (seamless, TileFor::Polygon), canvas) {
        ops.push(DrawOp::TiledFill { rings, tile });
    }
}

/// Whether the box of `rings`, as `msComputeBounds` computes it, meets the image of
/// `(width, height)` pixels, as `msRectOverlap` decides: a bound of no number meets it, so that
/// only a polygon the comparisons put off the image builds no tile.
fn meets_image(rings: &[Vec<Point>], (width, height): (f64, f64)) -> bool {
    ms_compute_bounds(rings.iter().flatten().map(|p| [p.x, p.y]))
        .is_some_and(|bounds| rect_overlap(bounds, [0.0, 0.0, width, height]))
}

#[cfg(test)]
mod tests;
