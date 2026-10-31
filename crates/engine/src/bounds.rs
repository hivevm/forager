// The bounds of a style follow MapServer's MS_MIN and MS_MAX and its default bounds of a style,
// src/mapserver.h, and the bounding of a style's width and size in msDrawLineSymbol(),
// computeSymbolStyle(), and msDrawShadeSymbol(), src/maprendering.c, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

//! The bounds of a style's width and size as msDrawLineSymbol(), computeSymbolStyle(), and
//! msDrawShadeSymbol() apply them, one function per site, and the constants they bound by. Two
//! sites bound a size outside this module, with these constants: msGetMarkerSize()'s marker size
//! (`symbol::marker_size`) and msDrawShape()'s clip buffer (`hatch::Hatching::for_class`). MapServer bounds in two orders: the maximum first where it
//! draws a line or a polygon, the minimum first where it draws a symbol. With `MS_MIN` and
//! `MS_MAX`, which keep their second operand where they cannot compare, the order decides what a
//! width that is no number, as an expression may bind, comes out as: the style's `MINWIDTH` in the
//! one order, its `MAXWIDTH` in the other.

use forager_style_ir::Style;

/// `MS_DEG_TO_RAD`, `src/mapserver.h:251`.
pub(crate) const DEG_TO_RAD: f64 = 0.017_453_292_519_943_295;

/// A style's default `MINSIZE`, `MS_MINSYMBOLSIZE`, `src/mapserver.h:277`.
pub(crate) const MIN_SIZE: f64 = 0.0;

/// A style's default `MAXSIZE`, `MS_MAXSYMBOLSIZE`, `src/mapserver.h:278`.
pub(crate) const MAX_SIZE: f64 = 500.0;

/// A style's default `MINWIDTH`, `MS_MINSYMBOLWIDTH`, `src/mapserver.h:280`.
pub(crate) const MIN_WIDTH: f64 = 0.0;

/// A style's default `MAXWIDTH`, `MS_MAXSYMBOLWIDTH`, `src/mapserver.h:281`.
pub(crate) const MAX_WIDTH: f64 = 32.0;

/// `MS_MAX(a, b)`, `src/mapserver.h:81`, which keeps `b` where it cannot compare.
pub(crate) fn ms_max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// `MS_MIN(a, b)`, `src/mapserver.h:78`, which keeps `b` where it cannot compare.
pub(crate) fn ms_min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// The style's `MINWIDTH`, its default without one.
fn min_width(style: &Style) -> f64 {
    style.min_width.unwrap_or(MIN_WIDTH)
}

/// The style's `MAXWIDTH`, its default without one.
fn max_width(style: &Style) -> f64 {
    style.max_width.unwrap_or(MAX_WIDTH)
}

/// The style's `MINSIZE`, its default without one.
fn min_size(style: &Style) -> f64 {
    style.min_size.unwrap_or(MIN_SIZE)
}

/// The style's `MAXSIZE`, its default without one.
fn max_size(style: &Style) -> f64 {
    style.max_size.unwrap_or(MAX_SIZE)
}

/// msDrawLineSymbol(), `src/maprendering.c:622-624`: a line's `width`, scaled, bounded by the
/// style's `MAXWIDTH` and then its `MINWIDTH`, both times the `resolution` factor.
pub(crate) fn line_width(width: f64, style: &Style, resolution: f64) -> f64 {
    let width = ms_min(width, max_width(style) * resolution);
    ms_max(width, min_width(style) * resolution)
}

/// computeSymbolStyle(), `src/maprendering.c:62-64`: a symbol's `size`, scaled, bounded by the
/// style's `MINSIZE` and then its `MAXSIZE`, both times the `resolution` factor.
pub(crate) fn symbol_size(size: f64, style: &Style, resolution: f64) -> f64 {
    let size = ms_max(size, min_size(style) * resolution);
    ms_min(size, max_size(style) * resolution)
}

/// computeSymbolStyle(), `src/maprendering.c:69-71`: a symbol's outline `width`, scaled, bounded
/// by the style's `MINWIDTH` and then its `MAXWIDTH`, both times the `resolution` factor.
pub(crate) fn symbol_outline_width(width: f64, style: &Style, resolution: f64) -> f64 {
    let width = ms_max(width, min_width(style) * resolution);
    ms_min(width, max_width(style) * resolution)
}

/// msDrawShadeSymbol(), `src/maprendering.c:796-799`: a plain polygon's outline `width`, scaled,
/// bounded by the style's `MAXWIDTH` and then its `MINWIDTH`, neither scaled.
pub(crate) fn shade_outline_width(width: f64, style: &Style) -> f64 {
    let width = ms_min(width, max_width(style));
    ms_max(width, min_width(style))
}

/// msDrawShadeSymbol(), `src/maprendering.c:808-810`: a hatch's line `width`, scaled, bounded by
/// the style's `MAXWIDTH` and then its `MINWIDTH`, both times the `resolution` factor.
pub(crate) fn hatch_width(width: f64, style: &Style, resolution: f64) -> f64 {
    let width = ms_min(width, max_width(style) * resolution);
    ms_max(width, min_width(style) * resolution)
}

/// msDrawShadeSymbol(), `src/maprendering.c:811-813`: a hatch's `spacing`, scaled, bounded by the
/// style's `MAXSIZE` and then its `MINSIZE`, both times the `resolution` factor.
pub(crate) fn hatch_spacing(spacing: f64, style: &Style, resolution: f64) -> f64 {
    let spacing = ms_min(spacing, max_size(style) * resolution);
    ms_max(spacing, min_size(style) * resolution)
}

#[cfg(test)]
mod tests;
