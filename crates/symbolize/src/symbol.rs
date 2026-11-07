// Vector and ellipse symbols at a point follow MapServer's msDrawMarkerSymbol, msAdjustMarkerPos,
// and computeSymbolStyle, src/maprendering.c, msSymbolGetDefaultSize and msGetMarkerSize,
// src/mapsymbol.c, and agg2RenderVectorSymbol, agg2RenderEllipseSymbol, and the square and
// transformation of agg2RenderPixmapSymbol, src/mapagg.cpp, of the
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
use forager_style_ir::c::lrint;
use forager_style_ir::{Bound, Rgba, Style, Symbol, SymbolKind, SymbolRef};

use crate::bounds;
use crate::style::{DEG_TO_RAD, fixed};

/// The most vertices of one ellipse. AGG takes as many as the radius asks for, about 2π√r: 16
/// for a radius of 6, some 2000 for 100 000 pixels; this many are reached at a radius of some
/// 425 000 pixels, far past any image, and bound what a hostile symbol costs (Q-9).
const MAX_ELLIPSE_STEPS: u32 = 4096;

/// What a point style draws at each point.
#[derive(Debug, PartialEq)]
pub enum Mark<'a> {
    /// A square of the style's size: a style without a symbol, or with one not drawn yet.
    Square,
    /// A vector or ellipse symbol.
    Symbol(&'a Symbol),
    /// A pixmap symbol, and its index among MapServer's symbols, from 1.
    Pixmap(usize, &'a Symbol),
    /// Nothing, for an index past the last symbol, as msDrawMarkerSymbol draws nothing.
    Nothing,
}

/// What a style's symbol is among MapServer's symbols, from index 1.
enum Resolved<'a> {
    /// MapServer's default symbol: no symbol, or index 0.
    Default,
    /// A name the frontend did not resolve to an index, or a binding not read yet.
    Unresolved,
    /// An index past the last symbol.
    Missing(usize),
    /// The symbol at the index, and the index, from 1.
    Found(usize, &'a Symbol),
}

/// The symbol `symbol` names among `symbols`.
fn resolve<'a>(symbol: &Option<Bound<SymbolRef>>, symbols: &'a [Symbol]) -> Resolved<'a> {
    let index = match symbol {
        None => return Resolved::Default,
        Some(Bound::Value(SymbolRef::Index(index))) => *index as usize,
        Some(_) => return Resolved::Unresolved,
    };
    let Some(position) = index.checked_sub(1) else {
        return Resolved::Default;
    };
    symbols
        .get(position)
        .map_or(Resolved::Missing(index), |symbol| {
            Resolved::Found(index, symbol)
        })
}

/// Whether `symbol` is drawn as a vector or an ellipse.
fn is_vector_or_ellipse(symbol: &Symbol) -> bool {
    matches!(symbol.kind, SymbolKind::Vector | SymbolKind::Ellipse)
}

/// The mark of `style` among `symbols`, MapServer's symbols from index 1.
pub fn mark<'a>(style: &Style, symbols: &'a [Symbol]) -> Mark<'a> {
    match resolve(&style.symbol, symbols) {
        Resolved::Missing(_) => Mark::Nothing,
        Resolved::Found(_, symbol) if is_vector_or_ellipse(symbol) => Mark::Symbol(symbol),
        Resolved::Found(index, symbol) if symbol.kind == SymbolKind::Pixmap => {
            Mark::Pixmap(index, symbol)
        }
        Resolved::Default | Resolved::Unresolved | Resolved::Found(..) => Mark::Square,
    }
}

/// Whether the style's symbol among `symbols` is a hatch.
pub(crate) fn is_hatch(style: &Style, symbols: &[Symbol]) -> bool {
    matches!(resolve(&style.symbol, symbols), Resolved::Found(_, symbol) if symbol.kind == SymbolKind::Hatch)
}

/// How a symbol is drawn, computeSymbolStyle's `symbolStyleObj`.
#[derive(Debug, PartialEq)]
pub(crate) struct SymbolStyle {
    fill: Option<Rgba>,
    outline: Option<Rgba>,
    outline_width: f64,
    /// Of the symbol's extent to pixels.
    pub(crate) scale: f64,
    /// Radians, counter-clockwise.
    pub(crate) rotation: f64,
    /// The style's `GAP` scaled as its size is, in pixels; negative as the `GAP` is.
    pub(crate) gap: f64,
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
    match resolve(&style.symbol, symbols) {
        Resolved::Found(_, symbol) if is_vector_or_ellipse(symbol) => default_size(symbol),
        _ => 1.0,
    }
}

/// computeSymbolStyle for a vector or ellipse symbol at `factor`, the style's scale factor and
/// the image's resolution factor, which are one here (see the engine's `classes::scale_factor`). A
/// filled symbol is filled with the style's colour and outlined with its outline colour; one not
/// filled is outlined with its colour, or its outline colour where it has none. The size is
/// scaled by the scale factor and bounded by `MINSIZE` and `MAXSIZE` scaled by the resolution
/// factor (see [`bounds::symbol_size`]); the outline is the style's width scaled and bounded alike
/// by `MINWIDTH` and `MAXWIDTH` (see [`bounds::symbol_outline_width`]); the `GAP` is scaled as the
/// size is.
pub(crate) fn symbol_style(style: &Style, symbol: &Symbol, factor: f64) -> SymbolStyle {
    let default_size = default_size(symbol);
    let style_size = fixed(&style.size).unwrap_or(default_size);
    let color = fixed(&style.color);
    let outline_color = fixed(&style.outline_color);
    let (fill, outline) = if symbol.filled {
        (color, outline_color)
    } else {
        (None, color.or(outline_color))
    };
    let target = bounds::symbol_size(style_size * factor, style, factor);
    let outline_width = if outline.is_some() {
        bounds::symbol_outline_width(fixed(&style.width).unwrap_or(1.0) * factor, style, factor)
    } else {
        0.0
    };
    SymbolStyle {
        fill,
        outline,
        outline_width,
        scale: target / default_size,
        rotation: fixed(&style.angle).unwrap_or(0.0) * DEG_TO_RAD,
        gap: style.gap.unwrap_or(0.0) * target / style_size,
    }
}

/// computeSymbolStyle for a pixmap symbol whose image is `height` pixels high, at `factor` (see
/// [`symbol_style`]): the scale of the image, the style's size, its default the image's height
/// (msSymbolGetDefaultSize), scaled and bounded by `MINSIZE` and `MAXSIZE`, over that height;
/// and its rotation in radians, counter-clockwise.
pub(crate) fn pixmap_style(style: &Style, height: u32, factor: f64) -> (f64, f64) {
    let default_size = if height > 0 { f64::from(height) } else { 1.0 };
    let style_size = fixed(&style.size).unwrap_or(default_size);
    let target = bounds::symbol_size(style_size * factor, style, factor);
    (
        target / default_size,
        fixed(&style.angle).unwrap_or(0.0) * DEG_TO_RAD,
    )
}

/// What agg2RenderPixmapSymbol, `src/mapagg.cpp:656-683`, resamples an image of `width` by
/// `height` pixels at the pixel `at` through, turned by `rotation` and at `scale`: the half-side
/// of the square it fills about the point, `MS_NINT(max(width, height) × scale × 1.415) / 2 + 1`
/// in a `long` narrowed to an `int`, and the inverse of the image's transformation, a translation
/// by minus half the image, a rotation by `-rotation`, the scaling, and a translation to the
/// point, in AGG's order of operations.
pub(crate) fn pixmap_square(
    width: u32,
    height: u32,
    at: Point,
    scale: f64,
    rotation: f64,
) -> (i32, forager_render::Affine) {
    let side = f64::from(width.max(height)) * scale * 1.415;
    let half = (lrint(side) / 2 + 1) as i32;
    let m = Affine::translation(-(f64::from(width) / 2.0), -(f64::from(height) / 2.0))
        .then(Affine::rotation(-rotation))
        .then(Affine::scaling(scale))
        .then(Affine::translation(at.x, at.y))
        .inverse();
    let inverse = forager_render::Affine {
        sx: m.sx,
        shy: m.shy,
        shx: m.shx,
        sy: m.sy,
        tx: m.tx,
        ty: m.ty,
    };
    (half, inverse)
}

/// msGetMarkerSize for a vector or ellipse symbol at `scale_factor`: the width and height it is
/// drawn at, from a size cut to whole pixels, or its own extent for a style without a size.
fn marker_size(style: &Style, symbol: &Symbol, scale_factor: f64) -> (f64, f64) {
    marker_size_at(style, fixed(&style.size), symbol, scale_factor)
}

/// [`marker_size`] of `style` at the size `size` in place of its own.
fn marker_size_at(
    style: &Style,
    size: Option<f64>,
    symbol: &Symbol,
    scale_factor: f64,
) -> (f64, f64) {
    // initSymbol's 1 by 1 for a symbol without points.
    let [size_x, size_y] = symbol.extent.unwrap_or([1.0, 1.0]);
    // `size` is an int there: each assignment cuts the fraction.
    let pixels = (size.unwrap_or_else(|| default_size(symbol)) * scale_factor).trunc();
    let pixels = bounds::marker_pixels(pixels, style);
    match size {
        Some(size) if size > 0.0 => (pixels / size_y * size_x, pixels),
        _ => (size_x, size_y),
    }
}

/// msGetMarkerSize of `style` among `symbols` at `scale_factor`, the size of the marker a label
/// of a point moves away from: 1 by 1 for MapServer's default symbol, a vector's, an ellipse's,
/// or any other symbol's MapServer sizes by its extent as it sizes them; the error names a symbol
/// whose size needs its image or its font, which is not read for it yet, or one the symbol set
/// does not hold, for which MapServer fails the map.
pub fn marker_size_of(
    style: &Style,
    symbols: &[Symbol],
    scale_factor: f64,
) -> Result<(f64, f64), String> {
    marker_size_with(
        style,
        fixed(&style.size),
        &style.symbol,
        symbols,
        scale_factor,
    )
}

/// [`marker_size_of`] of `style` at the size `size` and of the symbol `symbol` in place of its
/// own, as a feature binds them.
pub fn marker_size_with(
    style: &Style,
    size: Option<f64>,
    symbol: &Option<Bound<SymbolRef>>,
    symbols: &[Symbol],
    scale_factor: f64,
) -> Result<(f64, f64), String> {
    match resolve(symbol, symbols) {
        Resolved::Default => Ok((1.0, 1.0)),
        Resolved::Unresolved => Err("a symbol by a name that is not resolved".to_owned()),
        Resolved::Missing(index) => Err(format!(
            "symbol {index}, which the symbol set does not hold"
        )),
        Resolved::Found(_, symbol) => match symbol.kind {
            SymbolKind::Pixmap | SymbolKind::Svg | SymbolKind::Truetype => Err(format!(
                "a {:?} symbol, whose size is not computed yet",
                symbol.kind
            )
            .to_lowercase()),
            SymbolKind::Vector | SymbolKind::Ellipse | SymbolKind::Simple | SymbolKind::Hatch => {
                Ok(marker_size_at(style, size, symbol, scale_factor))
            }
        },
    }
}

/// The shift of a marker of `size`, width and height, that puts the symbol's anchor point
/// `anchor` on the point it is drawn at, turned with the symbol by `rotation`, as both
/// msAdjustMarkerPos and msImagePolylineMarkers shift it; nothing for an anchor at the centre.
pub(crate) fn anchor_shift(anchor: [f64; 2], size: (f64, f64), rotation: f64) -> (f64, f64) {
    let [anchor_x, anchor_y] = anchor;
    if anchor_x == 0.5 && anchor_y == 0.5 {
        return (0.0, 0.0);
    }
    let ox = (0.5 - anchor_x) * size.0;
    let oy = (0.5 - anchor_y) * size.1;
    if rotation == 0.0 {
        return (ox, oy);
    }
    let (sin, cos) = (libm::sin(-rotation), libm::cos(-rotation));
    (ox * cos - oy * sin, ox * sin + oy * cos)
}

/// The rest of msAdjustMarkerPos after [`shifted`]: the point moved so that the symbol's anchor
/// point, turned with the symbol, lies on `at`, of the size msGetMarkerSize gives at `factor`.
fn anchored(at: Point, style: &Style, symbol: &Symbol, rotation: f64, factor: f64) -> Point {
    let anchor = symbol.anchor_point.unwrap_or([0.5, 0.5]);
    if anchor == [0.5, 0.5] {
        return at;
    }
    let (ox, oy) = anchor_shift(anchor, marker_size(style, symbol, factor), rotation);
    Point {
        x: at.x + ox,
        y: at.y + oy,
    }
}

/// msAdjustMarkerPos before the anchor point, at the scale factor `factor`: `at` moved by the
/// style's `POLAROFFSET`, where its distance or its angle is not 0, then by its `OFFSET`, each
/// scaled. A line offset is the `OFFSET` it was written as, its `y` MapServer's −99 or −999.
pub(crate) fn shifted(at: Point, style: &Style, factor: f64) -> Point {
    let mut at = at;
    if let Some([distance, angle]) = &style.polar_offset {
        let (distance, angle) = (
            distance.value().unwrap_or(0.0),
            angle.value().unwrap_or(0.0),
        );
        if distance != 0.0 || angle != 0.0 {
            let angle = angle * DEG_TO_RAD;
            at.x += distance * libm::cos(-angle) * factor;
            at.y += distance * libm::sin(-angle) * factor;
        }
    }
    let (x, y) = match (&style.offset, &style.line_offset) {
        (_, Some(line)) => (
            line.distance.value().unwrap_or(0.0),
            if line.both_sides { -999.0 } else { -99.0 },
        ),
        (Some([x, y]), None) => (x.value().unwrap_or(0.0), y.value().unwrap_or(0.0)),
        (None, None) => (0.0, 0.0),
    };
    at.x += x * factor;
    at.y += y * factor;
    at
}

/// The drawing operations of `symbol` at the pixel `at` in `style` at `factor`, as
/// msDrawMarkerSymbol and the AGG renderer draw a vector or an ellipse symbol: moved by
/// msAdjustMarkerPos, filled first, then outlined.
pub fn symbol_ops(ops: &mut Vec<DrawOp>, at: Point, style: &Style, symbol: &Symbol, factor: f64) {
    let s = symbol_style(style, symbol, factor);
    let at = anchored(
        shifted(at, style, factor),
        style,
        symbol,
        s.rotation,
        factor,
    );
    render(ops, at, symbol, &s);
}

/// The drawing operations of `symbol` centred on the pixel `at` in `s`, as the AGG renderer
/// draws a vector or an ellipse symbol: filled first, then outlined.
pub(crate) fn render(ops: &mut Vec<DrawOp>, at: Point, symbol: &Symbol, s: &SymbolStyle) {
    if (s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0 {
        return;
    }
    let (paths, closed) = match symbol.kind {
        SymbolKind::Vector => (vector(symbol, at, s), false),
        SymbolKind::Ellipse => (vec![ellipse(symbol, at, s)], true),
        _ => return,
    };
    if let Some(color) = s.fill {
        ops.push(DrawOp::Fill {
            rings: paths.clone(),
            color,
            gamma: None,
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

    /// The inverse of this transformation, AGG's `invert`, in its order of operations.
    fn inverse(self) -> Self {
        let d = 1.0 / (self.sx * self.sy - self.shy * self.shx);
        let t0 = self.sy * d;
        let sy = self.sx * d;
        let shy = -self.shy * d;
        let shx = -self.shx * d;
        Self {
            sx: t0,
            shy,
            shx,
            sy,
            tx: -self.tx * t0 - self.ty * shx,
            ty: -self.tx * shy - self.ty * sy,
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
