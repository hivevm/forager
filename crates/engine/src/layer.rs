// The turn of a style's or a label's angle by the map's follows pointLayerDrawShape,
// src/mapdraw.c, of MapServer at the release pinned in scripts/fetch-msautotest.sh.
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

//! What the crate's root and its modules share of a layer drawn and of the map's rotation, below
//! them all: the layer's name, and a text, as a diagnostic quotes them, the reports of what the
//! layer left undrawn, its opacity, the points of a feature in view, a feature drawn in its
//! class's styles, the display items of a feature's drawing operations, and the map's rotation,
//! the frame its layers are drawn in, which turns a style's or a label's angle by the map's.

use std::borrow::Cow;

use forager_ports::{Feature, Geometry};
use forager_proj::Geotransform;
use forager_render::{DisplayItem, Draw, DrawOp, Point, Refused};
use forager_style_ir::c::int_cast;
use forager_style_ir::{Bound, Diagnostic, Extent, Layer, Map, RenderMode, Style};
use forager_symbolize::{Refusals, View, vertices};

/// The points of `geometry` inside the map's extent, in pixels, in order: where
/// pointLayerDrawShape draws a point layer's markers and labels, which skips a point outside
/// (msPointInRect) and transforms the others by the inverse of the cellsize (msTransformPoint).
pub(crate) fn points_in_view(view: &View, geometry: &Geometry) -> impl Iterator<Item = Point> {
    vertices(geometry)
        .into_iter()
        .filter(|&c| view.contains(c))
        .map(|c| view.point_to_pixel(c))
}

/// The reports that `layer` left the hatches and symbols of `refusals` undrawn: the warning for
/// the hatches (see [`refused_hatches`]), then one for each pixmap symbol, then one for each
/// vector, ellipse, or TrueType symbol whose fill tile is refused or TrueType symbol whose font
/// cannot be used, then one for each TrueType symbol whose font lacks its character, each named, and why.
pub(crate) fn refusal_reports(
    layer: &Layer,
    refusals: Refusals,
) -> impl Iterator<Item = Diagnostic> {
    let pixmaps = refusals.pixmaps.into_values().map(|(symbol, why)| {
        let message = format!("layer {}: {symbol} is not drawn: {why}", name(layer));
        Diagnostic::warning("pixmap-not-drawn", message, layer.span.clone())
    });
    let symbols = refusals.symbols.into_values().map(|(symbol, why)| {
        let message = format!("layer {}: {symbol} is not drawn: {why}", name(layer));
        Diagnostic::warning("symbol-not-drawn", message, layer.span.clone())
    });
    let glyphs = refusals.glyphs.into_values().map(|(symbol, what)| {
        let message = format!("layer {}: {symbol}: {what}", name(layer));
        Diagnostic::warning("symbol-glyph-missing", message, layer.span.clone())
    });
    refused_hatches(layer, refusals.hatches)
        .into_iter()
        .chain(pixmaps)
        .chain(symbols)
        .chain(glyphs)
}

/// The warnings that `layer` left undrawn what the renderer counted in `refused` (ADR-0049): one
/// for the pieces of its brushed lines of pixmap symbols, then one for those of the other symbols,
/// then one for its TrueType markers' glyphs out of reach, then one for its paths the clip cut,
/// each where there are any.
pub(crate) fn renderer_refusals(
    layer: &Layer,
    refused: Refused,
) -> impl Iterator<Item = Diagnostic> {
    let pieces = |code, kind, count: usize| {
        let message = format!(
            "layer {}: {count} piece{} of its lines brushed with {kind} {} not drawn (Q-3): \
             the rasterizer would compute past its integers on {}",
            name(layer),
            plural(count),
            if count == 1 { "is" } else { "are" },
            if count == 1 { "it" } else { "them" },
        );
        (code, count, message)
    };
    let glyphs = refused.glyphs;
    let paths = refused.paths;
    let kinds = [
        pieces("pixmap-not-drawn", "a pixmap symbol", refused.pixmaps),
        pieces(
            "symbol-not-drawn",
            "a vector, ellipse, or TrueType symbol",
            refused.symbols,
        ),
        (
            "symbol-not-drawn",
            glyphs,
            format!(
                "layer {}: {glyphs} glyph{} of its TrueType symbols {} not drawn (Q-3): \
                 {} past the image by more than {} pixels, past the rasterizer's integers",
                name(layer),
                plural(glyphs),
                if glyphs == 1 { "is" } else { "are" },
                if glyphs == 1 {
                    "it reaches"
                } else {
                    "they reach"
                },
                forager_render::clip::MAX_GLYPH_REACH,
            ),
        ),
        (
            "path-not-drawn",
            paths,
            format!(
                "layer {}: {paths} path{} {} drawn without a point, a segment, or a \
                 crossing that is no finite number in pixels (Q-3)",
                name(layer),
                plural(paths),
                if paths == 1 { "is" } else { "are" },
            ),
        ),
    ];
    kinds
        .into_iter()
        .filter(|&(_, count, _)| count > 0)
        .map(|(code, _, message)| Diagnostic::warning(code, message, layer.span.clone()))
}

/// The plural's `s` of a noun counted `count` times.
fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// A layer's opacity as MapServer holds it, an integer percent from 0 to 100: the IR's number
/// truncated, as `loadLayerCompositer` truncates it, and bounded, as it checks it; none for a
/// number that is not finite.
pub(crate) fn layer_opacity(layer: &Layer) -> Option<i32> {
    let opacity = layer.opacity.filter(|o| o.is_finite())?;
    Some(int_cast(opacity).clamp(0, 100))
}

/// Draws each of `ops` into `renderer` as a display item of feature `feature_id`, in order.
pub(crate) fn draw_items<R: Draw>(
    renderer: &mut R,
    feature_id: u64,
    ops: impl IntoIterator<Item = DrawOp>,
) {
    for op in ops {
        renderer.draw(&DisplayItem {
            op,
            feature_id: Some(feature_id),
        });
    }
}

/// A feature drawn: it, the place of its class, and its styles bound to it.
pub(crate) type Selected<'a> = (&'a Feature, usize, Vec<Cow<'a, Style>>);

/// The warning for the hatch fills of a layer refused to be drawn (Q-9), if any.
pub(crate) fn refused_hatches(layer: &Layer, refused: usize) -> Option<Diagnostic> {
    (refused > 0).then(|| {
        let message = format!(
            "layer {}: {refused} hatch fill{} not drawn (Q-9): its spacing or its dash pattern \
             would never end the hatch, or it would lay more lines or dashes over the image, or \
             cut them from a polygon of more vertices or crossings, than a hatch may",
            name(layer),
            if refused == 1 { " is" } else { "s are" }
        );
        Diagnostic::warning("hatch-not-drawn", message, layer.span.clone())
    })
}

pub(crate) fn name(layer: &Layer) -> String {
    layer
        .name
        .as_deref()
        .map_or("without NAME".to_owned(), |n| format!("`{n}`"))
}

/// The most characters of a text a diagnostic quotes, a feature's text or a pattern computed from
/// one, so that a render keeps no copy of a long text for each label or expression it reports
/// (Q-9).
const QUOTED_CHARACTERS: usize = 64;

/// `text` in backticks as a diagnostic quotes it: whole up to [`QUOTED_CHARACTERS`] characters,
/// else cut there, followed by an ellipsis and the text's length in characters.
pub(crate) fn quoted(text: &str) -> String {
    let count = text.chars().count();
    if count <= QUOTED_CHARACTERS {
        format!("`{text}`")
    } else {
        let head: String = text.chars().take(QUOTED_CHARACTERS).collect();
        format!("`{head}\u{2026}` ({count} characters)")
    }
}

/// The rotation of a map by its `angle`: its frame, geotransform, and geo cellsize, as ADR-0045
/// decides them, and the turn of a style's or a label's angle by the map's, which ADR-0045 leaves
/// out of its scope and pointLayerDrawShape, lineLayerDrawShape, and polygonLayerDrawShape give.
pub(crate) struct Rotation {
    /// The view everything after the reprojection is drawn in: on a rotated map, the frame of its
    /// image, `0, 0, width, height` at a cellsize of 1, as msMapSetFakedExtent fakes it.
    pub(crate) frame: View,
    /// The map's geotransform, held on the map's side of every reprojection; none unrotated.
    pub(crate) geotransform: Option<Geotransform>,
    /// The cellsize the scale factor of a size in units other than pixels divides by.
    pub(crate) geo_cellsize: f64,
    /// The map's `ANGLE` in degrees, which turns the angles of styles and labels; none unrotated.
    angle: Option<f64>,
}

impl Rotation {
    /// The rotation of `map` in `view`, its image of `size`: none for an angle of 0 or none, as
    /// msMapSetRotation sets none; `None`, with an error, where the angle gives no geotransform.
    pub(crate) fn of(
        map: &Map,
        view: &View,
        size: (u32, u32),
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        let Some(angle) = map.angle.filter(|&angle| angle != 0.0) else {
            return Some(Self::unrotated(*view));
        };
        let (width, height) = size;
        let Some(geotransform) = Geotransform::rotated_map(&view.extent, width, height, angle)
        else {
            let message = format!("the map's EXTENT, SIZE, and ANGLE {angle} give no geotransform");
            diagnostics.push(Diagnostic::error("view-invalid", message, map.span.clone()));
            return None;
        };
        Some(Self {
            frame: View {
                extent: Extent {
                    min_x: 0.0,
                    min_y: 0.0,
                    max_x: f64::from(width),
                    max_y: f64::from(height),
                },
                cellsize: 1.0,
            },
            geotransform: Some(geotransform),
            geo_cellsize: geo_cellsize(&view.extent, width, height),
            angle: Some(angle),
        })
    }

    /// No rotation, everything drawn in `view`.
    pub(crate) fn unrotated(view: View) -> Self {
        Self {
            frame: view,
            geotransform: None,
            geo_cellsize: view.cellsize,
            angle: None,
        }
    }

    /// `angle`, a style's or a label's in degrees, turned by the map's, as pointLayerDrawShape
    /// turns a point layer's (`src/mapdraw.c:2048-2060`): an angle that is not 0 less the map's,
    /// and 0 as it is, so that a symbol of angle 0 stays upright on the image. Any angle where
    /// the map is not rotated.
    fn turned(&self, angle: f64) -> f64 {
        match self.angle {
            Some(map_angle) if angle != 0.0 => angle - map_angle,
            _ => angle,
        }
    }

    /// `style`, a point layer's, with its angle turned by the map's (see [`Rotation::turned`]).
    pub(crate) fn turned_style<'s>(&self, style: &'s Style) -> Cow<'s, Style> {
        match style.angle {
            Some(Bound::Value(angle)) if self.angle.is_some() && angle != 0.0 => {
                let mut style = style.clone();
                style.angle = Some(Bound::Value(self.turned(angle)));
                Cow::Owned(style)
            }
            _ => Cow::Borrowed(style),
        }
    }

    /// The angle `angle` of a label of a layer drawn in `mode` is drawn and tested at, in
    /// degrees: turned by the map's (see [`Rotation::turned`]) in a point, line, or polygon
    /// layer, as pointLayerDrawShape, lineLayerDrawShape, and polygonLayerDrawShape turn it
    /// (`src/mapdraw.c:2050-2054, 2226-2227, 2324-2326`). The last two never turn it back, so
    /// that MapServer turns a label once more with each shape; each label here is turned once,
    /// as a point layer's is.
    pub(crate) fn turned_label(&self, mode: Option<RenderMode>, angle: f64) -> f64 {
        match mode {
            Some(RenderMode::Point | RenderMode::Line | RenderMode::Polygon) => self.turned(angle),
            _ => angle,
        }
    }
}

/// msGetGeoCellSize (`src/mapdraw.c:43`) of a rotated map, its `extent` the real one grown to an
/// image of `width` × `height` pixels: the diagonal of the extent's cell over width and height,
/// over the square root of 2. An unrotated map's is its cellsize.
fn geo_cellsize(extent: &Extent, width: u32, height: u32) -> f64 {
    let cellsize_x = (extent.max_x - extent.min_x) / f64::from(width);
    let cellsize_y = (extent.max_y - extent.min_y) / f64::from(height);
    (cellsize_x * cellsize_x + cellsize_y * cellsize_y).sqrt() / 2.0_f64.sqrt()
}
