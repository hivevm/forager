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

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point};
use forager_style_ir::{Rgba, Style, Symbol};

use crate::bounds;
use crate::canvas::Canvas;
use crate::extent::View;
use crate::hatch::{self, Hatched};
use crate::line::{Pass, line_ends, line_symbol, moved, open_rings, style_offset};
use crate::style::{GAMMA, fixed};
use crate::symbol;
use crate::tile;

/// A polygon drawn in a style, as msDrawShadeSymbol() draws one at the canvas's factor: nothing
/// for a symbol past the last or a name no symbol carries; with a pixmap, whatever its colours,
/// moved by the offset scaled and filled with the image tiled (see [`tiled`]); without a `COLOR`,
/// its outline as a line in the `OUTLINECOLOR`; with one, moved by the offset scaled, and with a
/// hatch symbol hatched (see [`hatch::hatch`]), with any other filled, and outlined in the
/// `OUTLINECOLOR` at the colour's alpha, with round caps and joins, its width scaled, the factor
/// itself for a width of 0, and bounded by `MAXWIDTH` and `MINWIDTH` unscaled, as MapServer
/// bounds it there. False where a hatch is refused (Q-9).
pub fn shade<T: Clone>(
    ops: &mut Vec<DrawOp>,
    rings: Vec<Vec<Point>>,
    style: &Style<T>,
    view: &View,
    canvas: &mut Canvas,
) -> bool {
    match symbol::mark(style, canvas.symbols) {
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
    let factor = canvas.factor;
    let rings = moved(rings, style_offset(style, factor));
    if symbol::is_hatch(style, canvas.symbols) {
        return match hatch::hatch(
            rings,
            style,
            color,
            canvas.size,
            hatch::reference(canvas.own_image, view),
            factor,
            canvas.resolution,
        ) {
            Hatched::Drawn(hatch) => {
                ops.push(DrawOp::Hatch(hatch));
                true
            }
            Hatched::Nothing => true,
            Hatched::Refused => false,
        };
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

/// The pixmap symbol `symbol`, at `index` from 1, over a polygon, as msDrawShadeSymbol tiles it:
/// computeSymbolStyle's scale, rotation, and gap at the canvas's factor give the tile of the
/// canvas's (see [`tile::tile`]), and the polygon, moved by the offset scaled, is filled with it.
/// Where the image has not loaded or its tile is refused (Q-9), nothing is drawn, and the canvas
/// records why. A polygon moved wholly off the image builds no tile, which would draw nothing
/// and take bytes from the tiles of the polygons on it.
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
    let factor = canvas.factor;
    let rings = moved(rings, style_offset(style, factor));
    if !meets_image(&rings, canvas.image()) {
        return;
    }
    let drawn = symbol::pixmap_style(style, image.height(), factor, canvas.resolution);
    // The engine's canvas always keeps tiles; one made without them, as a test's is, refuses the
    // fill rather than build a tile that nothing would hold or count (Q-9).
    let tile = match canvas.tiles.as_deref_mut() {
        Some(tiles) => tile::tile(tiles, index, image, drawn),
        None => Err("no tiles are kept where it is drawn".to_owned()),
    };
    match tile {
        Ok(tile) => ops.push(DrawOp::TiledFill { rings, tile }),
        Err(why) => {
            canvas.refused_pixmaps.entry(index).or_insert(why);
        }
    }
}

/// Whether the box of `rings` meets the image of `(width, height)` pixels.
fn meets_image(rings: &[Vec<Point>], (width, height): (f64, f64)) -> bool {
    let mut points = rings.iter().flatten();
    let Some(first) = points.next() else {
        return false;
    };
    let (mut min, mut max) = (*first, *first);
    for p in points {
        (min.x, min.y) = (min.x.min(p.x), min.y.min(p.y));
        (max.x, max.y) = (max.x.max(p.x), max.y.max(p.y));
    }
    min.x <= width && max.x >= 0.0 && min.y <= height && max.y >= 0.0
}

#[cfg(test)]
mod tests;
