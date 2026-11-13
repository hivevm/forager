// The styles of a label are drawn as msDrawLabelCache draws its labelpoly, labelpoint, and
// labelcenter styles, src/mapdraw.c, with msDrawLabelBounds and msDrawMarkerSymbol,
// src/maprendering.c, of MapServer at the release pinned in scripts/fetch-msautotest.sh.
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

use forager_render::{DrawOp, Point};
use forager_style_ir::{LabelTransform, Style, Symbol};

use crate::canvas::Canvas;
use crate::extent::View;
use crate::marker::{marker_ops, of_symbol_zero};
use crate::shade::shade;
use crate::symbol::{self, Mark, Resolved};

/// Where a label's styles are drawn: its point, and the box around its text, which a label
/// without text lacks.
pub struct LabelGeometry<'a> {
    /// The label's point, in pixels.
    pub point: Point,
    /// The box around the label's text, its labelpoly: the box's bounds, `[min_x, min_y, max_x,
    /// max_y]`, and the turned polygon where the label is turned. None for a label without text.
    pub poly: Option<([f64; 4], Option<&'a [Point]>)>,
}

/// Why a label's `style` is not drawn, or `None` where it is: a labelpoly of a symbol other than
/// MapServer's default or a hatch, whose tile is not drawn yet, or a labelpoint or labelcenter of
/// a symbol that is not drawn as a marker: a vector, an ellipse, or a pixmap, or that the symbol
/// set does not hold. MapServer's default symbol draws no marker but is no refusal. `ANGLE AUTO`
/// refuses none of them: `loadStyle` sets only the style's `autoangle` for it
/// (mapfile.c:2773-2775), which neither msDrawLabelCache nor computeMarkerBounds
/// (mapdraw.c:3284-3353, 4056-4135), msDrawMarkerSymbol through computeSymbolStyle
/// (maprendering.c:76), nor msDrawShadeSymbol (maprendering.c:821) reads; they turn by its
/// `ANGLE` alone.
pub fn label_style_refused(
    style: &Style<LabelTransform>,
    symbols: &[Symbol],
) -> Option<&'static str> {
    match style.geom_transform {
        LabelTransform::Poly => match symbol::resolve(&style.symbol, symbols) {
            Resolved::Default => None,
            _ if symbol::is_hatch(style, symbols) => None,
            _ => Some(
                "a STYLE of GEOMTRANSFORM labelpoly whose SYMBOL is no hatch of the symbol set",
            ),
        },
        LabelTransform::Point | LabelTransform::Center if marker_drawn(style, symbols) => None,
        LabelTransform::Point => Some(
            "a STYLE of GEOMTRANSFORM labelpnt whose SYMBOL is no vector, ellipse, or pixmap of \
             the symbol set",
        ),
        LabelTransform::Center => Some(
            "a STYLE of GEOMTRANSFORM labelcenter whose SYMBOL is no vector, ellipse, or pixmap \
             of the symbol set",
        ),
    }
}

/// Whether a label's marker `style` is drawn by msDrawMarkerSymbol: of a vector, an ellipse, or a
/// pixmap, or of MapServer's default symbol, of which it draws nothing.
fn marker_drawn(style: &Style<LabelTransform>, symbols: &[Symbol]) -> bool {
    match symbol::mark(style, symbols) {
        Mark::Symbol(_) | Mark::Pixmap(..) => true,
        Mark::Square => of_symbol_zero(style),
        Mark::Nothing => false,
    }
}

/// The image path of the pixmap symbol a label's `style` draws as its marker, a labelpoint or a
/// labelcenter, which the engine loads before the label is laid out and drawn.
pub fn label_pixmap<'a>(style: &Style<LabelTransform>, symbols: &'a [Symbol]) -> Option<&'a str> {
    match (style.geom_transform, symbol::mark(style, symbols)) {
        (LabelTransform::Point | LabelTransform::Center, Mark::Pixmap(_, symbol)) => {
            symbol.image.as_deref()
        }
        _ => None,
    }
}

/// The drawing operations of a label's `style` that [`label_style_refused`] does not refuse, on
/// `at`, at the canvas's factors, as msDrawLabelCache draws it before the text
/// (`src/mapdraw.c:4056-4127`): a labelpoint style's marker at the label's point, a labelpoly
/// style on its labelpoly through msDrawLabelBounds and msDrawShadeSymbol (see [`shade`]), the
/// turned polygon where the label is turned, else the ring of its box from its lower left corner
/// up, and a labelcenter style's marker at the centre of the labelpoly's box. A marker is drawn
/// as msDrawMarkerSymbol draws it (see [`marker_ops`]), nothing for MapServer's default symbol. A
/// labelpoly or labelcenter style of a label without text draws nothing. False where a hatch is
/// refused (Q-9).
pub fn label_style_ops(
    ops: &mut Vec<DrawOp>,
    style: &Style<LabelTransform>,
    at: &LabelGeometry<'_>,
    view: &View,
    canvas: &mut Canvas,
) -> bool {
    match (style.geom_transform, &at.poly) {
        (LabelTransform::Point, _) => label_marker(ops, at.point, style, canvas),
        (LabelTransform::Center, Some(([min_x, min_y, max_x, max_y], _))) => {
            let center = Point {
                x: (max_x + min_x) / 2.0,
                y: (max_y + min_y) / 2.0,
            };
            label_marker(ops, center, style, canvas);
        }
        (LabelTransform::Poly, Some((bbox, poly))) => {
            let ring = match poly {
                Some(points) => points.to_vec(),
                None => {
                    let [min_x, min_y, max_x, max_y] = *bbox;
                    let p = |x, y| Point { x, y };
                    vec![
                        p(min_x, min_y),
                        p(min_x, max_y),
                        p(max_x, max_y),
                        p(max_x, min_y),
                        p(min_x, min_y),
                    ]
                }
            };
            return shade(ops, vec![ring], style, view, canvas);
        }
        // The label cache refuses a label without text that has such a style before it is
        // cached; should one reach here, it draws nothing rather than fail the map (Q-3).
        (LabelTransform::Center | LabelTransform::Poly, None) => {}
    }
    true
}

/// A label's marker `style` at the pixel `at`, as msDrawMarkerSymbol draws it: nothing for
/// MapServer's default symbol, where a point layer's [`marker_ops`] draws Forager's square.
fn label_marker(
    ops: &mut Vec<DrawOp>,
    at: Point,
    style: &Style<LabelTransform>,
    canvas: &mut Canvas,
) {
    if !of_symbol_zero(style) {
        marker_ops(ops, at, style, canvas);
    }
}
