// The marks along a line follow the branch of msDrawLineSymbol() that hands them to
// msImagePolylineMarkers(), and the tiles the branches of msDrawLineSymbol() and
// msDrawShadeSymbol() that ask getTile(), src/maprendering.c of the release pinned in
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

//! A style's [`Mark`] where a line or a polygon places it, one dispatch over the kinds for each
//! placement: along a line, each marker drawn by a [`Styled`] mark at its point and angle and
//! priced by it against the canvas's budget (see [`along_line`]); as a brush or a fill tile (see
//! [`tile`]). The point's own dispatch is `marker::marker_ops`.

use std::collections::BTreeMap;
use std::rc::Rc;

use forager_render::{DrawOp, Point, PremultipliedImage, Tile};
use forager_style_ir::{Style, Symbol};

use crate::canvas::Canvas;
use crate::glyph::Glyph;
use crate::glyph_marker;
use crate::line_marker::{MarkerBudget, Placement, clamped, taken};
use crate::symbol::{self, Mark, SymbolStyle};
use crate::tile::{self, TileFor};

/// A mark of a vector, an ellipse, a pixmap, or a TrueType symbol as computeSymbolStyle styles it
/// at the canvas's factors, with what it draws: its symbol, its pixmap's image, or its glyph.
enum Styled<'a> {
    /// A vector or ellipse symbol in its style.
    Symbol(&'a Symbol, SymbolStyle),
    /// A pixmap symbol, its image, and the image's scale, rotation, and gap.
    Pixmap(&'a Symbol, Rc<PremultipliedImage>, (f64, f64, f64)),
    /// A TrueType symbol, its glyph at the style's size, and its style.
    Glyph(&'a Symbol, Rc<Glyph>, SymbolStyle),
}

impl<'a> Styled<'a> {
    /// The mark `mark` of `style` styled at the canvas's factors, or `None` where it draws
    /// nothing: a square, no symbol, a pixmap whose image is refused or whose scale is 0, and a
    /// TrueType symbol of neither colour, of a scale of 0, or whose glyph is refused. The pixmap's
    /// image is read, and the glyph asked, here, so that what they lack is recorded whether or
    /// not markers are left.
    fn of<T>(mark: Mark<'a>, style: &Style<T>, canvas: &mut Canvas) -> Option<Self> {
        match mark {
            Mark::Symbol(_, symbol) => Some(Self::Symbol(
                symbol,
                symbol::symbol_style(style, symbol, canvas.factors),
            )),
            Mark::Pixmap(index, symbol) => {
                // computeSymbolStyle sets no colour for a pixmap (src/maprendering.c:47-48), and
                // msDrawLineSymbol asks for none on this branch (666-688).
                let image = canvas.pixmap(index, symbol)?;
                let drawn = symbol::pixmap_style(style, image.height(), canvas.factors);
                (drawn.0 != 0.0).then_some(Self::Pixmap(symbol, image, drawn))
            }
            Mark::Glyph(index, symbol) => {
                let s = symbol::symbol_style(style, symbol, canvas.factors);
                if (s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0 {
                    return None;
                }
                let glyph = glyph_marker::styled_glyph(symbol, index, &s, canvas)?;
                Some(Self::Glyph(symbol, glyph, s))
            }
            Mark::Square | Mark::Hatch | Mark::Nothing => None,
        }
    }

    /// The symbol the mark draws, whose anchor point places it.
    fn symbol(&self) -> &Symbol {
        match self {
            Self::Symbol(symbol, _) | Self::Pixmap(symbol, ..) | Self::Glyph(symbol, ..) => symbol,
        }
    }

    /// computeSymbolStyle's gap and rotation of the mark.
    fn gap_and_rotation(&self) -> (f64, f64) {
        match self {
            Self::Symbol(_, s) | Self::Glyph(_, _, s) => (s.gap, s.rotation),
            &Self::Pixmap(_, _, (_, rotation, gap)) => (gap, rotation),
        }
    }

    /// The marker's size msImagePolylineMarkers is handed (see [`Placement::of`]): a vector's,
    /// an ellipse's, and a pixmap's width and height scaled, at least 1 each way; a TrueType
    /// symbol's its glyph's control box as it is.
    fn extent(&self) -> (f64, f64) {
        match self {
            Self::Symbol(symbol, s) => {
                let [size_x, size_y] = symbol.extent.unwrap_or([0.0, 0.0]);
                clamped((size_x * s.scale, size_y * s.scale))
            }
            &Self::Pixmap(_, ref image, (scale, ..)) => clamped((
                f64::from(image.width()) * scale,
                f64::from(image.height()) * scale,
            )),
            Self::Glyph(_, glyph, _) => glyph_marker::glyph_extent(glyph),
        }
    }

    /// The drawing operations of the mark centred on `at`, turned by `rotation`, in radians
    /// counter-clockwise.
    fn draw(&mut self, ops: &mut Vec<DrawOp>, at: Point, rotation: f64) {
        match self {
            Self::Symbol(symbol, s) => {
                s.rotation = rotation;
                symbol::render(ops, at, symbol, s);
            }
            Self::Pixmap(_, image, (scale, ..)) => {
                ops.push(symbol::render_pixmap(
                    at,
                    Rc::clone(image),
                    *scale,
                    rotation,
                ));
            }
            Self::Glyph(_, glyph, s) => {
                s.rotation = rotation;
                ops.push(glyph_marker::glyph_op(glyph, at, s));
            }
        }
    }

    /// The price of one marker against the budget, what it draws at one place at its own
    /// rotation (see [`MarkerBudget::price`]): a pixmap and a glyph one point, as each is one
    /// operation of one point; `None` where it draws nothing.
    fn price(&mut self) -> Option<usize> {
        let (_, rotation) = self.gap_and_rotation();
        let mut probe = Vec::new();
        self.draw(&mut probe, Point { x: 0.0, y: 0.0 }, rotation);
        MarkerBudget::price(&probe)
    }
}

/// The marks of `mark`, of `style`, along `paths`, in pixels already offset, as msDrawLineSymbol
/// hands a vector, an ellipse, a pixmap, or a TrueType symbol with a `GAP` to
/// msImagePolylineMarkers: placed by [`Placement::of`] at the mark's gap, rotation, size, and
/// anchor point, `initial_gap` the `INITIALGAP` scaled as the width is, each marker's points taken
/// from the canvas's budget (see [`taken`]) and drawn by the styled mark at its point and angle.
/// Nothing where the mark draws nothing (see [`Styled::of`]) or the budget is spent.
pub(crate) fn along_line<T>(
    paths: &[Vec<Point>],
    style: &Style<T>,
    mark: Mark,
    initial_gap: f64,
    canvas: &mut Canvas,
) -> Vec<DrawOp> {
    let Some(mut styled) = Styled::of(mark, style, canvas) else {
        return Vec::new();
    };
    if canvas.markers.spent() {
        return Vec::new();
    }
    let Some(price) = styled.price() else {
        return Vec::new();
    };
    let placement = Placement::of(
        style,
        styled.gap_and_rotation(),
        styled.extent(),
        styled.symbol().anchor(),
        initial_gap,
        canvas.image(),
    );
    let mut ops = Vec::new();
    for marker in taken(paths, &placement, price, &mut canvas.markers) {
        styled.draw(&mut ops, marker.at, marker.rotation);
    }
    ops
}

/// The tile of `mark`, of `style` at the canvas's factors, that msDrawShadeSymbol, or
/// msDrawLineSymbol where `asked` is a line's, asks getTile for, from the render's cache of tiles
/// (see [`Canvas::tiles`]): a pixmap's of its image (see [`pixmap_tile`]); a vector's or an
/// ellipse's, seamless where `seamless` (see [`tile::vector_tile`]); and a TrueType symbol's of its
/// glyph, asked only for a tile the cache does not hold and drawn at the tile's centre by
/// [`glyph_marker::glyph_op`], a font that lacks the character noted with the tile, so that each
/// layer that draws it records that (see [`tile::glyph_tile`]). None for a vector, an ellipse, or
/// a TrueType symbol of neither colour, for a square or no symbol, and where the image, the glyph,
/// or the tile is refused (Q-9): the canvas records why, a pixmap's among the pixmaps, the others
/// among the symbols.
pub(crate) fn tile<T>(
    mark: Mark,
    style: &Style<T>,
    (seamless, asked): (bool, TileFor),
    canvas: &mut Canvas,
) -> Option<Rc<Tile>> {
    match mark {
        Mark::Pixmap(index, symbol) => {
            let image = canvas.pixmap(index, symbol)?;
            pixmap_tile(style, index, image, asked, canvas)
        }
        Mark::Symbol(index, symbol) => {
            let s = coloured(style, symbol, canvas)?;
            let tile = canvas
                .tiles()
                .and_then(|tiles| tile::vector_tile(tiles, index, symbol, &s, (seamless, asked)));
            recorded(&mut canvas.refused_symbols, index, tile)
        }
        Mark::Glyph(index, symbol) => {
            let s = coloured(style, symbol, canvas)?;
            let (glyphs, size) = (canvas.glyphs, glyph_marker::glyph_size(s.scale));
            let mut refused = None;
            let mark = |at| match glyph_marker::ask(glyphs, symbol, size) {
                Ok((glyph, missing)) => Some((glyph_marker::glyph_op(&glyph, at, &s), missing)),
                Err(why) => {
                    refused = Some(why);
                    None
                }
            };
            let tile = canvas
                .tiles()
                .and_then(|tiles| tile::glyph_tile(tiles, index, &s, asked, mark));
            match tile {
                Ok(Some((tile, missing))) => {
                    if let Some(missing) = missing {
                        canvas.missing_glyphs.entry(index).or_insert(missing);
                    }
                    Some(tile)
                }
                Ok(None) => recorded(&mut canvas.refused_symbols, index, Err(refused?)),
                Err(why) => recorded(&mut canvas.refused_symbols, index, Err(why)),
            }
        }
        Mark::Square | Mark::Hatch | Mark::Nothing => None,
    }
}

/// The tile of the pixmap symbol at `index` from 1 with the image `image`, read already, of
/// `style` at the canvas's factors, asked for `asked`: computeSymbolStyle's scale, rotation, and
/// gap give the tile of the render's cache (see [`tile::tile`]). Where it is refused (Q-9), there
/// is none, and the canvas records why among the pixmaps. A shaded pixmap asks it with the image
/// it read before it tested whether the polygon meets the image.
pub(crate) fn pixmap_tile<T>(
    style: &Style<T>,
    index: usize,
    image: Rc<PremultipliedImage>,
    asked: TileFor,
    canvas: &mut Canvas,
) -> Option<Rc<Tile>> {
    let drawn = symbol::pixmap_style(style, image.height(), canvas.factors);
    let tile = canvas
        .tiles()
        .and_then(|tiles| tile::tile(tiles, index, image, drawn, asked));
    recorded(&mut canvas.refused_pixmaps, index, tile)
}

/// The tile `tile`, or `None` with why it is refused recorded in `refused` for the symbol at
/// `index`, the first reason a layer meets kept.
fn recorded(
    refused: &mut BTreeMap<usize, String>,
    index: usize,
    tile: Result<Rc<Tile>, String>,
) -> Option<Rc<Tile>> {
    tile.map_err(|why| {
        refused.entry(index).or_insert(why);
    })
    .ok()
}

/// computeSymbolStyle's style of `symbol` in `style` at the canvas's factors, or `None` where it
/// has neither a fill nor an outline colour, and msDrawLineSymbol and msDrawShadeSymbol draw
/// nothing.
fn coloured<T>(style: &Style<T>, symbol: &Symbol, canvas: &Canvas) -> Option<SymbolStyle> {
    let s = symbol::symbol_style(style, symbol, canvas.factors);
    (s.fill.is_some() || s.outline.is_some()).then_some(s)
}
