// The reports of a layer's and of a class style's GEOMTRANSFORM that draws nothing follow
// msDrawVectorLayer, msDrawTransformedShape, and msDrawMap, src/mapdraw.c, and
// msGeomTransformShape, src/mapgeomtransform.c, of MapServer at the release pinned in
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

//! The reports of what a layer drawn left undrawn, which the crate's root and the labels turn into
//! the layer's diagnostics: after the layer is drawn, what its canvas, the renderer, and its
//! render mode left undrawn, and what its styles and their transform trees drew nothing of; before
//! it is drawn, its own transform tree's.

use std::collections::{BTreeMap, BTreeSet};

use forager_render::Refused;
use forager_style_ir::{Diagnostic, GeomTransform, Layer, NotDrawn, Symbol, TransformTree};
use forager_symbolize::{MAX_MARKER_POINTS, Refusals, Resolved, Undrawn, resolve};

use crate::layer::name;

/// Reports what `layer` left undrawn of `refusals`, its features' and those of its labels drawn
/// at once: symbols past the budget of markers where they were left out, then its hatches and
/// symbols (see [`refusal_reports`]); then what the renderer refused of it in `refused` (see
/// [`renderer_refusals`]), then the geometries of `undrawn`, by kind, its render mode cannot draw.
pub(crate) fn layer_reports(
    layer: &Layer,
    refusals: Refusals,
    refused: Refused,
    undrawn: BTreeMap<&'static str, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if refusals.markers_spent {
        let message = format!(
            "layer {}: its symbols at its features, along its lines, or placed by a \
             GEOMTRANSFORM exceed {MAX_MARKER_POINTS} points; those past the limit are not drawn",
            name(layer)
        );
        diagnostics.push(Diagnostic::warning(
            "line-markers-over-limit",
            message,
            layer.span.clone(),
        ));
    }
    diagnostics.extend(refusal_reports(layer, refusals));
    diagnostics.extend(renderer_refusals(layer, refused));
    for (kind, count) in undrawn {
        let message = format!(
            "layer {} cannot draw {count} {kind} geometr{} in its render mode",
            name(layer),
            if count == 1 { "y" } else { "ies" }
        );
        diagnostics.push(Diagnostic::warning(
            "geometry-not-drawn",
            message,
            layer.span.clone(),
        ));
    }
}

/// The reports that `layer` left the hatches and symbols of `refusals` undrawn: the warning for
/// the hatches (see [`refused_hatches`]), then one for each pixmap symbol, then one for each
/// vector, ellipse, or TrueType symbol whose fill tile is refused or TrueType symbol whose font
/// cannot be used, then one for each TrueType symbol whose font lacks its character, each named,
/// and why. Its markers left out and its trees' bound are the layer's own, which
/// [`layer_reports`] and [`style_reports`] report.
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

/// The reports on the styles of a layer, drawn by its paths where `paths` holds: the symbols no
/// symbol is named; where it is drawn by its paths, the styles of a GEOMTRANSFORM not drawn, which
/// a point layer draws at its points, and a circle layer ignores as it draws its circle whatever
/// the transform (`src/mapdraw.c:2390-2396`); and what the class styles' transform trees left
/// undrawn, their failures and the bound they passed (see [`style_tree_reports`]).
pub(crate) fn style_reports(
    layer: &Layer,
    paths: bool,
    symbols: &[Symbol],
    (failures, over_limit): (StyleFailures, Option<String>),
    diagnostics: &mut Vec<Diagnostic>,
) {
    unknown_symbols(layer, symbols, diagnostics);
    if paths {
        styles_not_drawn(layer, diagnostics);
    }
    style_tree_reports(layer, failures, over_limit.as_deref(), diagnostics);
}

/// The first failure of each class style's transform tree in a layer's drawing, by the places of
/// its class in the layer and of the style in its class.
pub(crate) type StyleFailures = BTreeMap<(usize, usize), String>;

/// Warns once for each name that a style of `layer` gives its symbol and that none of `symbols`
/// carries: such a style draws nothing, as a style document has no image file to read the name
/// as. A mapfile's names never reach here, as its frontend resolves each to an index.
fn unknown_symbols(layer: &Layer, symbols: &[Symbol], diagnostics: &mut Vec<Diagnostic>) {
    let mut reported = BTreeSet::new();
    for style in layer.classes.iter().flat_map(|class| &class.styles) {
        let Resolved::Unnamed(symbol) = resolve(&style.symbol, symbols) else {
            continue;
        };
        if reported.insert(symbol) {
            let message = format!(
                "layer {}: no symbol is named `{symbol}`, so the styles that name it draw nothing",
                name(layer)
            );
            diagnostics.push(Diagnostic::warning(
                "symbol-unknown",
                message,
                style.span.clone().or_else(|| layer.span.clone()),
            ));
        }
    }
}

/// Warns of each class style of a line or a polygon layer whose `GEOMTRANSFORM` the engine does
/// not draw on a style: a transform tree `not_drawn`, naming its reason
/// (ADR-0048, *Diagnostics*), which MapServer evaluates
/// by msDrawTransformedShape, through GEOS or a JavaScript engine, or fails; and labelcenter, which
/// fails the layer's draw there (src/mapgeomtransform.c:245), msDrawLabelCache alone drawing it on
/// a label's style. The style draws nothing; the class's other styles draw in their passes as
/// before. A point layer draws such a style at its points, as pointLayerDrawShape draws every
/// style whatever its transform, and is not warned. The warning holds for a style document as for
/// a mapfile, whose frontend does not warn of it again.
fn styles_not_drawn(layer: &Layer, diagnostics: &mut Vec<Diagnostic>) {
    for style in layer.classes.iter().flat_map(|class| &class.styles) {
        let message = match &style.geom_transform {
            Some(GeomTransform::Expression(TransformTree::NotDrawn(reason))) => format!(
                "layer {}: the GEOMTRANSFORM expression of a class's style holds {} \
                 (not_drawn: {}), which the engine does not draw: the style draws nothing",
                name(layer),
                described(*reason),
                reason.name()
            ),
            Some(GeomTransform::LabelCenter) => format!(
                "layer {}: a class's style of GEOMTRANSFORM labelcenter is not drawn: only a \
                 label's style draws labelcenter, and MapServer fails the layer's draw on it; the \
                 style draws nothing",
                name(layer),
            ),
            _ => continue,
        };
        diagnostics.push(Diagnostic::warning(
            "geomtransform-not-drawn",
            message,
            style.span.clone().or_else(|| layer.span.clone()),
        ));
    }
}

/// Warns of the class styles' transform trees of `layer` that drew nothing
/// (ADR-0048, *A class's style*): of each style in
/// `failures`, its first failure, once, as `geomtransform-not-drawn`; and, where they passed
/// `over_limit`, a bound of Q-9 they share with the layer's `GEOMTRANSFORM`, once for the layer as
/// `geomtransform-over-limit`. MapServer makes no map where msDrawTransformedShape fails
/// (src/mapdraw.c:1402-1405, 508-519); Forager draws nothing of the style on that shape, and the
/// rest.
fn style_tree_reports(
    layer: &Layer,
    failures: StyleFailures,
    over_limit: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for ((c, s), why) in failures {
        let class = &layer.classes[c];
        let class_name = class
            .name
            .as_deref()
            .map_or(String::new(), |n| format!(" `{n}`"));
        let message = format!(
            "layer {}: style {} of its class {}{class_name} draws nothing on a shape: its \
             GEOMTRANSFORM expression {why}, where MapServer makes no map",
            name(layer),
            s + 1,
            c + 1,
        );
        let span = class.styles[s]
            .span
            .clone()
            .or_else(|| class.span.clone())
            .or_else(|| layer.span.clone());
        diagnostics.push(Diagnostic::warning(
            "geomtransform-not-drawn",
            message,
            span,
        ));
    }
    if let Some(why) = over_limit {
        let message = format!(
            "layer {}: a GEOMTRANSFORM expression of its styles, charged with the layer's \
             GEOMTRANSFORM and its other styles', {why}: its styles' expressions draw nothing on \
             the shape past the limit and on every later one",
            name(layer),
        );
        diagnostics.push(Diagnostic::warning(
            "geomtransform-over-limit",
            message,
            layer.span.clone(),
        ));
    }
}

/// What a transform tree's reason `not_drawn` stands for, as a warning names it.
fn described(reason: NotDrawn) -> String {
    match reason {
        NotDrawn::Binding => "a property's value where a number or a text is due".to_owned(),
        NotDrawn::Text => "a computed smoothsia preprocessing".to_owned(),
        NotDrawn::Fromtext => "a shape fromtext reads from a text".to_owned(),
        NotDrawn::Literal => "an operation of literals that folds to no finite number".to_owned(),
        NotDrawn::Syntax => "an expression that does not parse as a shape".to_owned(),
        function => format!("the function {}", function.name()),
    }
}

/// The warning that `layer` draws nothing, its `GEOMTRANSFORM` a transform tree of the reason
/// `reason` (ADR-0048). MapServer evaluates a function the tree has no node
/// for through GEOS, or a JavaScript engine, and fails on a syntax error or a binding where it
/// makes no number; Forager draws none of the layer, and the rest of the map.
pub(crate) fn transform_not_drawn(layer: &Layer, reason: NotDrawn) -> Diagnostic {
    let message = format!(
        "layer {} draws nothing: its GEOMTRANSFORM holds {} (not_drawn: {}), which the engine \
         does not draw",
        name(layer),
        described(reason),
        reason.name()
    );
    Diagnostic::warning("geomtransform-not-drawn", message, layer.span.clone())
}

/// The warning that `layer` draws nothing, its `GEOMTRANSFORM` failed on a shape or past a bound
/// of Q-9. Where MapServer's parser fails the transform on a shape (densify on a point, a division
/// by zero, outer on a line), msGeomTransformShape fails, msDrawVectorLayer returns MS_FAILURE
/// (src/mapdraw.c:1427-1434), and msDrawMap frees the image and returns none
/// (src/mapdraw.c:508-519): MapServer makes no map. Forager draws nothing of the layer, with the
/// warning, and the rest of the map.
pub(crate) fn transform_failed(layer: &Layer, undrawn: Undrawn) -> Diagnostic {
    let (code, why) = match undrawn {
        Undrawn::Failed(why) => ("geomtransform-not-drawn", why),
        Undrawn::Limit(why) => ("geomtransform-over-limit", why),
    };
    let message = format!(
        "layer {} draws nothing: its GEOMTRANSFORM {why}",
        name(layer)
    );
    Diagnostic::warning(code, message, layer.span.clone())
}
