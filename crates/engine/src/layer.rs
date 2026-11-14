//! What the crate's root and its modules share of a layer drawn, below them all: the layer's
//! name, and a text, as a diagnostic quotes them, the reports of what the layer left undrawn, its
//! opacity, the points of a feature in view, a feature drawn in its class's styles, and the
//! display items of a feature's drawing operations.

use std::borrow::Cow;

use forager_ports::{Feature, Geometry};
use forager_render::{DisplayItem, Draw, DrawOp, Point};
use forager_style_ir::c::int_cast;
use forager_style_ir::{Diagnostic, Layer, Style};
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

/// The reports that `layer` left the hatches and pixmaps of `refusals` undrawn: the warning for
/// the hatches (see [`refused_hatches`]), then one for each pixmap symbol, named, and why.
pub(crate) fn refusal_reports(
    layer: &Layer,
    refusals: Refusals,
) -> impl Iterator<Item = Diagnostic> {
    let pixmaps = refusals.pixmaps.into_values().map(|(symbol, why)| {
        let message = format!("layer {}: {symbol} is not drawn: {why}", name(layer));
        Diagnostic::warning("pixmap-not-drawn", message, layer.span.clone())
    });
    refused_hatches(layer, refusals.hatches)
        .into_iter()
        .chain(pixmaps)
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
