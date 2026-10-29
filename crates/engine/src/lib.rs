//! The engine: renders the map of a style IR from the sources bound to its layers. It is the
//! one core crate that awaits the ports (ADR-0009).

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod bind;
mod classes;
mod draw;
mod extent;
mod fonts;
mod offset;
mod symbol;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use forager_ports::{
    Feature, FeatureQuery, FeatureSource, ResourceLoader, ResourcePath, SourceError,
};
use forager_render::{DisplayItem, Renderer, RgbaImage};
use forager_style_ir::{Diagnostic, Layer, LayerStatus, Map, RenderMode, Rgba, Style, Symbol};

use forager_expr::Evaluator;

use crate::classes::{Reports, Selector};
use crate::draw::{Pass, can_draw, draw_ops, kind, outlined, passes};

pub use crate::bind::bind_sources;
pub use crate::extent::View;
pub use crate::fonts::MAX_FONT_BYTES;

use crate::fonts::Fonts;

/// The largest width or height of an image, MapServer's default `MAXSIZE` (Q-9).
pub const MAX_IMAGE_SIDE: u32 = 4096;

/// The most features one layer draws (Q-9); a layer with more is skipped with a diagnostic.
pub const MAX_FEATURES_PER_LAYER: usize = 1_000_000;

/// MapServer's default `IMAGECOLOR`.
const DEFAULT_BACKGROUND: Rgba = Rgba::opaque(255, 255, 255);

/// The result of a render: the image unless the map could not be drawn at all, and the
/// diagnostics, among them one for every layer skipped.
#[derive(Debug)]
pub struct Rendered {
    pub image: Option<RgbaImage>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Renders `map` with `renderer`, reading each layer's features from the source bound to the
/// layer's name in `sources`. A layer without a source, or whose source fails, is skipped with a
/// diagnostic (G-8). It reads no other file, so a label's font does not load; a host that has the
/// files the mapfile names renders with [`render_with_files`].
pub async fn render<S: FeatureSource, R: Renderer>(
    map: &Map,
    sources: &BTreeMap<String, S>,
    renderer: R,
) -> Rendered {
    render_with_files(map, sources, &NoFiles, renderer).await
}

/// A loader that holds no file.
struct NoFiles;

impl ResourceLoader for NoFiles {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Err(SourceError::new(format!(
            "{path} is not given to this render"
        )))
    }
}

/// Renders as [`render`] does, and reads the files the map names beyond its sources, the fonts
/// of its labels, through `files`, the host's resource loader rooted at the base context
/// (ADR-0028).
pub async fn render_with_files<S: FeatureSource, L: ResourceLoader, R: Renderer>(
    map: &Map,
    sources: &BTreeMap<String, S>,
    files: &L,
    mut renderer: R,
) -> Rendered {
    let mut fonts = Fonts::default();
    let mut diagnostics = Vec::new();
    let Some((width, height, view)) = view(map, &mut diagnostics) else {
        return Rendered {
            image: None,
            diagnostics,
        };
    };
    renderer.begin(width, height, map.image_color.unwrap_or(DEFAULT_BACKGROUND));
    let mut evaluator = Evaluator::new();
    let mut reports = Reports::default();
    for (layer_index, layer) in map.layers.iter().enumerate() {
        if !shown(layer) {
            continue;
        }
        let Some(mode) = drawable_mode(layer, &mut diagnostics) else {
            continue;
        };
        let Some(features) = features(layer, sources, &view, &mut diagnostics).await else {
            continue;
        };
        if layer.classes.is_empty() {
            continue;
        }
        if let Some(property) = classes::missing_property(layer, &features) {
            let message = format!(
                "layer {} is skipped: it reads the property `{property}`, which none of its \
                 features has",
                name(layer)
            );
            diagnostics.push(Diagnostic::warning(
                "property-missing",
                message,
                layer.span.clone(),
            ));
            continue;
        }
        let mut undrawn = BTreeMap::new();
        let mut drawable = Vec::with_capacity(features.len());
        for feature in &features {
            if can_draw(mode, &feature.geometry) {
                drawable.push(feature);
            } else {
                *undrawn.entry(kind(&feature.geometry)).or_insert(0usize) += 1;
            }
        }
        let mut selector = Selector {
            layer,
            layer_index,
            evaluator: &mut evaluator,
            reports: &mut reports,
            diagnostics: &mut diagnostics,
        };
        // Each feature drawn, with its class and its styles bound to it, in the source's order.
        let mut selected = Vec::with_capacity(drawable.len());
        for feature in drawable {
            let Some(index) = selector.class(feature) else {
                continue;
            };
            let class = &layer.classes[index];
            let styles: Vec<_> = class
                .styles
                .iter()
                .map(|style| selector.bind(style, feature, &map.symbols))
                .collect();
            selected.push((feature, index, styles));
        }
        // A label needs its fonts once a feature of its class is drawn; drawing them is still to
        // come.
        let classes_drawn: BTreeSet<usize> = selected.iter().map(|(_, index, _)| *index).collect();
        for index in classes_drawn {
            for label in &layer.classes[index].labels {
                fonts.for_label(map, label, files, &mut diagnostics).await;
            }
        }
        draw_selected(&mut renderer, mode, &selected, &view, &map.symbols);
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
    Rendered {
        image: Some(renderer.finish()),
        diagnostics,
    }
}

/// A feature drawn: it, the place of its class, and its styles bound to it.
type Selected<'a> = (&'a Feature, usize, Vec<Cow<'a, Style>>);

/// Draws a layer's selected features in the order msDrawVectorLayer draws them: a feature of a
/// line layer whose class has several styles, or an outline, draws its first pass at once and the
/// rest from its cache, style by style, after every feature; any other draws all its styles at
/// once.
fn draw_selected<R: Renderer>(
    renderer: &mut R,
    mode: RenderMode,
    selected: &[Selected<'_>],
    view: &View,
    symbols: &[Symbol],
) {
    let mut draw = |feature: &Feature, style: &Style, passes: &[Pass]| {
        for &pass in passes {
            for op in draw_ops(mode, &feature.geometry, style, pass, view, symbols) {
                renderer.draw(&DisplayItem {
                    op,
                    feature_id: Some(feature.id),
                });
            }
        }
    };
    let mut cached = Vec::new();
    let mut most_styles = 0;
    for (feature, _, styles) in selected {
        let passes = passes(mode, styles);
        let cache = mode == RenderMode::Line
            && (styles.len() > 1 || styles.first().is_some_and(|s| outlined(s)));
        if cache {
            let (style, first) = &passes[0];
            draw(feature, style, first);
            most_styles = most_styles.max(styles.len());
            // The style each later pass draws, by its place in the class.
            let later: Vec<_> = passes[1..]
                .iter()
                .map(|(style, passes)| {
                    let place = styles
                        .iter()
                        .position(|s| std::ptr::eq(&**s, *style))
                        .expect("a style of the class");
                    (place, *style, passes.clone())
                })
                .collect();
            cached.push((*feature, later));
        } else {
            for (style, passes) in &passes {
                draw(feature, style, passes);
            }
        }
    }
    for place in 0..most_styles {
        for (feature, later) in &cached {
            for (_, style, passes) in later.iter().filter(|(p, _, _)| *p == place) {
                draw(feature, style, passes);
            }
        }
    }
}

/// The image size and the view of the map, or `None` with an error.
fn view(map: &Map, diagnostics: &mut Vec<Diagnostic>) -> Option<(u32, u32, View)> {
    let span = map.span.clone();
    let Some([width, height]) = map.size else {
        diagnostics.push(Diagnostic::error(
            "size-missing",
            "the map has no SIZE",
            span,
        ));
        return None;
    };
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        let message =
            format!("the image of {width} × {height} exceeds {MAX_IMAGE_SIDE} pixels a side");
        diagnostics.push(Diagnostic::error("image-too-large", message, span));
        return None;
    }
    let Some(extent) = map.extent else {
        diagnostics.push(Diagnostic::error(
            "extent-missing",
            "the map has no EXTENT",
            span,
        ));
        return None;
    };
    let Some(view) = View::for_image(extent, width, height) else {
        let message = "the map's EXTENT and SIZE give no scale";
        diagnostics.push(Diagnostic::error("view-invalid", message, span));
        return None;
    };
    Some((width, height, view))
}

/// Whether [`render`] reads the features of `layer`: one shown, of a TYPE it draws. A host need
/// open no source for any other layer, whose source would cost memory and draw nothing.
pub fn reads_features(layer: &Layer) -> bool {
    shown(layer) && layer.render_mode.is_some_and(drawn)
}

/// As in MapServer, a layer without STATUS is OFF; ON and DEFAULT layers are drawn.
fn shown(layer: &Layer) -> bool {
    matches!(layer.status, Some(LayerStatus::On | LayerStatus::Default))
}

/// The render modes whose geometry the engine draws.
fn drawn(mode: RenderMode) -> bool {
    matches!(
        mode,
        RenderMode::Point | RenderMode::Line | RenderMode::Polygon
    )
}

fn drawable_mode(layer: &Layer, diagnostics: &mut Vec<Diagnostic>) -> Option<RenderMode> {
    match layer.render_mode {
        Some(mode) if drawn(mode) => Some(mode),
        other => {
            let what = other.map_or("no TYPE".to_owned(), |m| format!("{m:?}").to_lowercase());
            let message = format!("layer {} is not drawn: {what}", name(layer));
            diagnostics.push(Diagnostic::warning(
                "layer-not-drawn",
                message,
                layer.span.clone(),
            ));
            None
        }
    }
}

async fn features<S: FeatureSource>(
    layer: &Layer,
    sources: &BTreeMap<String, S>,
    view: &View,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Feature>> {
    let kind = layer.source.as_ref().map_or("none", |s| s.kind.as_str());
    let span = layer
        .source
        .as_ref()
        .and_then(|s| s.span.clone())
        .or_else(|| layer.span.clone());
    let skip = |reason: String| {
        let message = format!("layer {} (source {kind}) is skipped: {reason}", name(layer));
        Diagnostic::warning("layer-skipped", message, span.clone())
    };
    let Some(source) = layer.name.as_ref().and_then(|n| sources.get(n)) else {
        diagnostics.push(skip("no source is bound to it".to_owned()));
        return None;
    };
    let query = FeatureQuery {
        extent: view.extent,
    };
    let delivered = source.features(&query).await.map(|delivered| {
        // The records the source skipped, one warning for the layer however many (ADR-0025).
        let skipped = &delivered.skipped;
        if skipped.count > 0 {
            let first = skipped.first.as_deref().unwrap_or("no reason given");
            let records = if skipped.count == 1 {
                "record"
            } else {
                "records"
            };
            let message = format!(
                "layer {} (source {kind}): {} {records} of its source cannot be read and \
                 are skipped; the first: {first}",
                name(layer),
                skipped.count
            );
            diagnostics.push(Diagnostic::warning(
                "source-records-skipped",
                message,
                span.clone(),
            ));
        }
        delivered.features
    });
    match delivered {
        Ok(features) if features.len() > MAX_FEATURES_PER_LAYER => {
            let reason = format!(
                "{} features exceed the limit of {MAX_FEATURES_PER_LAYER}",
                features.len()
            );
            diagnostics.push(skip(reason));
            None
        }
        Ok(features) => Some(features),
        Err(e) => {
            diagnostics.push(skip(e.message));
            None
        }
    }
}

fn name(layer: &Layer) -> String {
    layer
        .name
        .as_deref()
        .map_or("without NAME".to_owned(), |n| format!("`{n}`"))
}
