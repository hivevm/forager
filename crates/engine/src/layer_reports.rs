//! The reports of what a layer drawn left undrawn, which the crate's root and the labels turn into
//! the layer's diagnostics.

use forager_render::Refused;
use forager_style_ir::{Diagnostic, Layer};
use forager_symbolize::Refusals;

use crate::layer::name;

/// The reports that `layer` left the hatches and symbols of `refusals` undrawn: the warning for
/// the hatches (see [`refused_hatches`]), then one for each pixmap symbol, then one for each
/// vector, ellipse, or TrueType symbol whose fill tile is refused or TrueType symbol whose font
/// cannot be used, then one for each TrueType symbol whose font lacks its character, each named,
/// and why. Its markers left out and its trees' bound are the layer's own, which the crate's root
/// reports where a layer's reports put them.
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
