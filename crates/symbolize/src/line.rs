// The drawing of a line in a style follows MapServer's msDrawLineSymbol() and
// msOutlineRenderingPrepareStyle(), src/maprendering.c, of the release pinned in
// scripts/fetch-msautotest.sh.
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

use std::borrow::Cow;

use forager_render::{Dashes, DrawOp, LineCap, LineJoin, Point};
use forager_style_ir::{Bound, Style};

use crate::bounds;
use crate::canvas::Canvas;
use crate::line_marker;
use crate::offset::{self, Offset};
use crate::style::fixed;
use crate::symbol;

/// One drawing of a style on a feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// The style as written.
    Plain,
    /// The style as written, only when it has a `COLOR`.
    PlainIfColored,
    /// The style as written, only when it has a `COLOR` or its symbol is a pixmap, as the shape
    /// cache draws a style after the first (`src/mapdraw.c:1487-1494`).
    PlainIfColoredOrPixmap,
    /// The outline of a line: the style widened by twice its `OUTLINEWIDTH`, its colours swapped.
    Outline,
    /// The outline, only when the style has an `OUTLINECOLOR`.
    OutlineIfOutlined,
}

impl Pass {
    /// The style this pass draws of `style` on `canvas`: an outline pass's as
    /// msOutlineRenderingPrepareStyle() prepares it (see [`outline_style`]), any other's as
    /// written.
    pub(crate) fn style<'s, T: Clone>(
        self,
        style: &'s Style<T>,
        canvas: &Canvas,
    ) -> Cow<'s, Style<T>> {
        match self {
            Self::Plain | Self::PlainIfColored | Self::PlainIfColoredOrPixmap => {
                Cow::Borrowed(style)
            }
            Self::Outline | Self::OutlineIfOutlined => {
                Cow::Owned(outline_style(style, canvas.factor, canvas.resolution))
            }
        }
    }
}

/// Whether msDrawLineSymbol draws the style's symbol along the line, `gap < 0` or `gap > 0`
/// (`src/maprendering.c:679-688`), rather than as a brush: a gap that is not a number brushes it.
pub(crate) fn gapped<T>(style: &Style<T>) -> bool {
    matches!(style.gap, Some(gap) if gap.abs() > 0.0)
}

/// Whether the style has an outline, an `OUTLINEWIDTH` above 0.
pub fn outlined(style: &Style) -> bool {
    outline_width(style) > 0.0
}

/// The style's `OUTLINEWIDTH` in pixels; 0 without one.
fn outline_width<T>(style: &Style<T>) -> f64 {
    fixed(&style.outline_width).unwrap_or(0.0)
}

/// The style's `WIDTH` scaled by the scale factor `factor` and bounded by its `MINWIDTH` and
/// `MAXWIDTH` scaled by the resolution factor `resolution`, as msDrawLineSymbol() bounds it (see
/// [`bounds::line_width`]). With the width as written.
fn bounded_width<T>(style: &Style<T>, factor: f64, resolution: f64) -> (f64, f64) {
    let written = fixed(&style.width).unwrap_or(1.0);
    (
        bounds::line_width(written * factor, style, resolution),
        written,
    )
}

/// The offset of the style, its numbers multiplied by `factor`; `None` when it moves nothing.
pub(crate) fn style_offset<T>(style: &Style<T>, factor: f64) -> Option<Offset> {
    if let Some(line) = &style.line_offset {
        let distance = line.distance.value()? * factor;
        return Some(if line.both_sides {
            Offset::BothSides(distance)
        } else {
            Offset::Parallel(distance)
        });
    }
    let [x, y] = style.offset.as_ref()?;
    let (x, y) = (x.value().unwrap_or(0.0), y.value().unwrap_or(0.0));
    (x != 0.0 || y != 0.0).then_some(Offset::Shift {
        x: x * factor,
        y: y * factor,
    })
}

pub(crate) fn moved(paths: Vec<Vec<Point>>, offset: Option<Offset>) -> Vec<Vec<Point>> {
    match offset {
        Some(offset) => offset::apply(&paths, offset),
        None => paths,
    }
}

/// The cap and the join agg2RenderLine() strokes a line of `width` with: those given for a line
/// wider than a pixel, and for any other bevel joins and AGG's default cap, butt. (It also bevels
/// the inner joins of such a line, which the display list does not carry; they lie within a
/// pixel of the path.)
pub(crate) fn line_ends(width: f64, cap: LineCap, join: LineJoin) -> (LineCap, LineJoin) {
    if width > 1.0 {
        (cap, join)
    } else {
        (LineCap::Butt, LineJoin::Bevel)
    }
}

/// Rings as MapServer's line_adaptor strokes them: open lines that end on their first point,
/// with caps there and no join.
pub(crate) fn open_rings(mut rings: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    for ring in &mut rings {
        if let (Some(&first), Some(&last)) = (ring.first(), ring.last())
            && first != last
        {
            ring.push(first);
        }
    }
    rings
}

/// A line drawn in a style, as msDrawLineSymbol() draws one of a simple symbol: in the `COLOR`, or
/// the `OUTLINECOLOR` without one; the width scaled by the canvas's factor and bounded; the offset,
/// the dash pattern, and the initial gap scaled by the factor the scaling and the bounds applied to
/// the width; and the style's cap and join, by default `round` and `none`. An outline pass draws
/// the style widened by twice its outline width with its colours swapped, as
/// msOutlineRenderingPrepareStyle() prepares it. A style with a vector, an ellipse, or a pixmap
/// symbol and a `GAP` draws the symbol along the line instead, its markers' points taken from the
/// canvas's budget, a pixmap without a colour and not at all where its image is refused; one whose
/// symbol index is past the last draws nothing.
pub(crate) fn line_symbol<T: Clone>(
    ops: &mut Vec<DrawOp>,
    paths: Vec<Vec<Point>>,
    style: &Style<T>,
    pass: Pass,
    canvas: &mut Canvas,
) {
    let style = &*pass.style(style, canvas);
    let (first, second) = (fixed(&style.color), fixed(&style.outline_color));
    let mark = symbol::mark(style, canvas.symbols);
    let gapped = gapped(style);
    if !gapped {
        canvas.untiled_pixmap(&mark);
    }
    let skipped = match pass {
        Pass::Plain | Pass::Outline => false,
        Pass::PlainIfColored | Pass::OutlineIfOutlined => first.is_none(),
        // A pixmap with a GAP of 0, which MapServer tiles along the line (src/maprendering.c:
        // 687-707) and Forager does not, is skipped without a COLOR as any other symbol.
        Pass::PlainIfColoredOrPixmap => {
            first.is_none() && !(gapped && matches!(mark, symbol::Mark::Pixmap(..)))
        }
    };
    if skipped {
        return;
    }
    let (width, written) = bounded_width(style, canvas.factor, canvas.resolution);
    let factor = if written != 0.0 { width / written } else { 1.0 };
    let initial_gap = style.initial_gap.unwrap_or(-1.0);
    match mark {
        symbol::Mark::Nothing => return,
        symbol::Mark::Pixmap(index, symbol) if gapped => {
            // computeSymbolStyle sets no colour for a pixmap (src/maprendering.c:47-48), and
            // msDrawLineSymbol asks for none on this branch (666-688).
            if let Some(pixmap) = canvas.pixmap(index, symbol) {
                let paths = moved(paths, style_offset(style, factor));
                ops.extend(line_marker::draw_pixmap(
                    &paths,
                    style,
                    symbol,
                    &pixmap,
                    initial_gap * factor,
                    canvas.image(),
                    (canvas.factor, canvas.resolution),
                    &mut canvas.markers,
                ));
            }
            return;
        }
        symbol::Mark::Symbol(symbol) if gapped => {
            let paths = moved(paths, style_offset(style, factor));
            let initial_gap = initial_gap * factor;
            ops.extend(line_marker::draw(
                &paths,
                style,
                symbol,
                initial_gap,
                canvas.image(),
                (canvas.factor, canvas.resolution),
                &mut canvas.markers,
            ));
            return;
        }
        symbol::Mark::Symbol(_) | symbol::Mark::Pixmap(..) | symbol::Mark::Square => {}
    }
    let Some(color) = first.or(second) else {
        return;
    };
    let (cap, join) = line_ends(
        width,
        style.line_cap.unwrap_or(LineCap::Round),
        style.line_join.unwrap_or(LineJoin::None),
    );
    ops.push(DrawOp::Stroke {
        paths: moved(paths, style_offset(style, factor)),
        closed: false,
        color,
        width,
        cap,
        join,
        dashes: Dashes {
            pattern: style.pattern.iter().map(|p| p * factor).collect(),
            offset: if initial_gap <= 0.0 {
                0.0
            } else {
                initial_gap * factor
            },
        },
    });
}

/// The style of an outline pass as msOutlineRenderingPrepareStyle() prepares it: its width and
/// its bounds widened by twice its `OUTLINEWIDTH`, its size, MapServer's -1 where it has none,
/// grown by it, and its colours swapped. MapServer divides the width's growth by the layer's
/// scale factor `factor` over the image's resolution factor `resolution`, and divides the size's
/// by the scale factor and multiplies it by the resolution factor, so that, scaled as the drawing
/// scales them after, both grow by the outline width at the resolution; the bounds grow by it as
/// written.
fn outline_style<T: Clone>(style: &Style<T>, factor: f64, resolution: f64) -> Style<T> {
    let outline = outline_width(style);
    Style {
        color: style.outline_color.clone(),
        outline_color: style.color.clone(),
        width: Some(Bound::Value(
            fixed(&style.width).unwrap_or(1.0) + outline / (factor / resolution) * 2.0,
        )),
        min_width: Some(bounds::min_width(style) + 2.0 * outline),
        max_width: Some(bounds::max_width(style) + 2.0 * outline),
        size: Some(Bound::Value(
            fixed(&style.size).unwrap_or(-1.0) + outline / factor * resolution,
        )),
        ..style.clone()
    }
}

#[cfg(test)]
mod tests;
