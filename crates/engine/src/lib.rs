//! The engine: renders the map of a style IR from the sources bound to its layers. It is the
//! one core crate that awaits the ports (ADR-0009).

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod bind;
mod bounds;
mod classes;
mod draw;
mod extent;
mod fonts;
mod geom_transform;
mod hatch;
mod inline;
mod label_styles;
mod labels;
mod line_marker;
mod offset;
mod symbol;

use std::borrow::Cow;
use std::collections::BTreeMap;

use forager_ports::{
    Feature, FeatureQuery, FeatureSource, ResourceLoader, ResourcePath, SourceError,
};
use forager_render::{DisplayItem, Renderer, RgbaImage};
use forager_style_ir::{Diagnostic, Layer, LayerStatus, Map, RenderMode, Rgba, Style};

use forager_expr::Evaluator;

use crate::classes::{Reports, Selector};
use crate::draw::{Canvas, Pass, Plan, can_draw, draw_ops, kind};

pub use crate::bind::bind_sources;
pub use crate::extent::View;
pub use crate::fonts::MAX_FONT_BYTES;
pub use crate::inline::is_inline;
pub use crate::line_marker::MAX_MARKER_POINTS;

use crate::labels::Labels;

/// The largest width or height of an image, MapServer's default `MAXSIZE` (Q-9).
pub const MAX_IMAGE_SIDE: u32 = 4096;

/// The most features one layer draws (Q-9); a layer with more is skipped with a diagnostic.
pub const MAX_FEATURES_PER_LAYER: usize = 1_000_000;

/// MapServer's default `IMAGECOLOR`.
const DEFAULT_BACKGROUND: Rgba = Rgba::opaque(255, 255, 255);

/// The result of a render: the image unless the map could not be drawn at all, the diagnostics,
/// among them one for every layer skipped, and the labels of its label cache.
#[derive(Debug)]
pub struct Rendered {
    pub image: Option<RgbaImage>,
    pub diagnostics: Vec<Diagnostic>,
    /// Every label the label cache held, drawn or discarded, in the order `msDrawLabelCache`
    /// walks them: from the highest priority down, and within a priority from the last added to
    /// the first. The labels of a layer of `LABELCACHE OFF`, drawn at once, are not among them.
    pub labels: Vec<CachedLabel>,
}

/// A label of a render's label cache.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedLabel {
    /// From 1 to `MAX_PRIORITY` of `forager-label`.
    pub priority: u8,
    /// The index of the layer in the map, and of the class in the layer.
    pub layer: usize,
    pub class: usize,
    pub text: String,
    /// The box the label was drawn in, `[min x, min y, max x, max y]` in pixels, its buffer
    /// included; `None` for a label the cache discarded.
    pub drawn: Option<[f64; 4]>,
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
    let mut diagnostics = Vec::new();
    let Some((width, height, view)) = view(map, &mut diagnostics) else {
        return Rendered {
            image: None,
            diagnostics,
            labels: Vec::new(),
        };
    };
    renderer.begin(width, height, map.image_color.unwrap_or(DEFAULT_BACKGROUND));
    let mut labels = Labels::new(map, &view, files, (width, height));
    let mut evaluator = Evaluator::new();
    let mut reports = Reports::default();
    let scale = classes::scale_denom(map, &view, width);
    // Each layer's scale factor, the same for every layer the engine draws.
    let factor = classes::scale_factor(map);
    for (layer_index, layer) in map.layers.iter().enumerate() {
        if !shown(layer) || !classes::layer_in_scale(layer, scale) {
            continue;
        }
        // msDrawLayer skips a layer of no opacity, its labels too.
        if checked_opacity(layer, &mut diagnostics) == Some(0) {
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
            scale,
            evaluator: &mut evaluator,
            reports: &mut reports,
            diagnostics: &mut diagnostics,
        };
        let selected = selector.select(drawable, &map.symbols);
        let drawing = Drawing {
            mode,
            layer,
            scale,
            view: &view,
        };
        let mut canvas = Canvas::new(&map.symbols, width, height, factor);
        // Each feature is labelled by the class that draws it, in the order it is drawn.
        let point_layer = mode == RenderMode::Point;
        let drawn: Vec<labels::Labelled<'_>> = selected
            .iter()
            .map(|(feature, class, styles)| (*feature, *class, styles.first().map(|s| &**s)))
            .collect();
        // A point layer of LABELCACHE OFF draws each feature's labels after the feature, before
        // the next feature, into the layer's image, as pointLayerDrawShape does (labels.rs).
        let at_once = point_layer && !layer.label_cache;
        let own_image = drawing.begin_layer(&mut renderer, &mut canvas);
        if at_once {
            for (one, labelled) in selected.chunks(1).zip(drawn.chunks(1)) {
                drawing.draw(&mut renderer, &mut canvas, one);
                labels
                    .collect(
                        layer_index,
                        true,
                        labelled,
                        &mut selector,
                        Some(&mut renderer),
                    )
                    .await;
            }
        } else {
            drawing.draw(&mut renderer, &mut canvas, &selected);
        }
        Drawing::end_layer(&mut renderer, own_image);
        layer_reports(layer, &canvas, undrawn, selector.diagnostics);
        if !at_once {
            labels
                .collect(
                    layer_index,
                    point_layer,
                    &drawn,
                    &mut selector,
                    None::<&mut R>,
                )
                .await;
        }
    }
    // The labels of the cache are drawn after every layer, as MapServer's label cache draws
    // them.
    let labels = labels.finish(&mut renderer, &mut diagnostics);
    Rendered {
        image: Some(renderer.finish()),
        labels,
        diagnostics,
    }
}

/// Reports what `layer` left undrawn on `canvas`: symbols past the budget of markers, hatches
/// refused, and the geometries of `undrawn`, by kind, its render mode cannot draw.
fn layer_reports(
    layer: &Layer,
    canvas: &Canvas,
    undrawn: BTreeMap<&'static str, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if canvas.markers_spent() {
        let message = format!(
            "layer {}: its symbols along lines or placed by a GEOMTRANSFORM exceed \
             {MAX_MARKER_POINTS} points; those past the limit are not drawn",
            name(layer)
        );
        diagnostics.push(Diagnostic::warning(
            "line-markers-over-limit",
            message,
            layer.span.clone(),
        ));
    }
    diagnostics.extend(refused_hatches(layer, canvas.hatches_refused()));
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

/// A layer's opacity as MapServer holds it, an integer percent from 0 to 100: the IR's number
/// truncated, as `loadLayerCompositer` truncates it, and bounded, as it checks it; none for a
/// number that is not finite.
fn layer_opacity(layer: &Layer) -> Option<i32> {
    let opacity = layer.opacity.filter(|o| o.is_finite())?;
    Some((opacity as i32).clamp(0, 100))
}

/// [`layer_opacity`], with a warning for an opacity outside 0 to 100, which a mapfile cannot give
/// (`loadLayerCompositer` refuses it) but a style document can.
fn checked_opacity(layer: &Layer, diagnostics: &mut Vec<Diagnostic>) -> Option<i32> {
    let opacity = layer_opacity(layer);
    if let Some(written) = layer.opacity
        && !(0.0..=100.0).contains(&written)
    {
        let message = match opacity {
            Some(drawn) => format!(
                "layer {}: its opacity {written} lies outside 0 to 100; it is drawn at {drawn}",
                name(layer)
            ),
            None => format!(
                "layer {}: its opacity {written} is no number; it is drawn without one",
                name(layer)
            ),
        };
        diagnostics.push(Diagnostic::warning(
            "opacity-out-of-range",
            message,
            layer.span.clone(),
        ));
    }
    opacity
}

/// A feature drawn: it, the place of its class, and its styles bound to it.
pub(crate) type Selected<'a> = (&'a Feature, usize, Vec<Cow<'a, Style>>);

/// What a layer's features are drawn with.
struct Drawing<'a> {
    mode: RenderMode,
    layer: &'a Layer,
    /// The map's scale, which a style's scale range is compared to.
    scale: f64,
    view: &'a View,
}

impl Drawing<'_> {
    /// Begins the layer's drawing into an image of its own, which is laid over the map where the
    /// layer has an opacity, as msDrawLayer draws a layer with a `COMPOSITE` block; the alpha it
    /// is laid over at, or `None` where the layer is drawn straight onto the map.
    fn begin_layer<R: Renderer>(&self, renderer: &mut R, canvas: &mut Canvas) -> Option<u8> {
        let opacity = layer_opacity(self.layer)?;
        canvas.in_own_image();
        renderer.begin_layer();
        // aggCompositeRasterBufferNoPixman lays the layer over at MS_NINT(opacity × 2.55).
        let alpha = classes::ms_nint(f64::from(opacity) * 2.55);
        Some(alpha.clamp(0, 255) as u8)
    }

    /// Lays the layer's own image over the map at `alpha`, where [`Self::begin_layer`] began one.
    fn end_layer<R: Renderer>(renderer: &mut R, alpha: Option<u8>) {
        if let Some(alpha) = alpha {
            renderer.end_layer(alpha);
        }
    }

    /// Draws a layer's selected features in the order msDrawVectorLayer draws them, at once or
    /// from the cache as the [`Plan`] of each class says. A style outside its scale range draws
    /// nothing, where msDrawShape and the cache's pass skip it.
    fn draw<R: Renderer>(&self, renderer: &mut R, canvas: &mut Canvas, selected: &[Selected<'_>]) {
        let mode = self.mode;
        let mut draw = |canvas: &mut Canvas, feature: &Feature, style: &Style, passes: &[Pass]| {
            if !classes::in_scale(self.scale, style.min_scale_denom, style.max_scale_denom) {
                return;
            }
            for &pass in passes {
                for op in draw_ops(mode, &feature.geometry, style, pass, self.view, canvas) {
                    renderer.draw(&DisplayItem {
                        op,
                        feature_id: Some(feature.id),
                    });
                }
            }
        };
        let plans: Vec<Plan> = self
            .layer
            .classes
            .iter()
            .map(|class| Plan::new(mode, &class.styles))
            .collect();
        let mut cached = Vec::new();
        let mut most_styles = 0;
        for (feature, class, styles) in selected {
            canvas.class(&self.layer.classes[*class].styles);
            let plan = &plans[*class];
            let passes = plan.passes(styles);
            for (style, passes) in &passes.at_once {
                draw(canvas, feature, style, passes);
            }
            if plan.cached() {
                most_styles = most_styles.max(styles.len());
                cached.push((*feature, passes.cached));
            }
        }
        for place in 0..most_styles {
            for (feature, later) in &cached {
                for (_, style, passes) in later.iter().filter(|(p, _, _)| *p == place) {
                    draw(canvas, feature, style, passes);
                }
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

/// Whether [`render`] reads the features of `layer`: one shown, of a TYPE it draws, from its
/// inline features or from the source bound to it.
pub fn reads_features(layer: &Layer) -> bool {
    shown(layer) && layer.render_mode.is_some_and(drawn)
}

/// Whether [`render`] reads the source a host binds to `layer`: one whose features it reads
/// ([`reads_features`]) and that has no inline features it reads instead ([`is_inline`]). A host
/// need open no source for any other layer, whose source would cost memory and draw nothing.
pub fn reads_bound_source(layer: &Layer) -> bool {
    reads_features(layer) && !is_inline(layer)
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

/// The features of a layer in view: those of its inline features where it is read from them
/// ([`is_inline`], ADR-0033), whatever source is bound to it, which is not read and is reported;
/// otherwise those of the source bound to its name.
async fn features<S: FeatureSource>(
    layer: &Layer,
    sources: &BTreeMap<String, S>,
    view: &View,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Feature>> {
    let bound = layer.name.as_ref().and_then(|n| sources.get(n));
    if is_inline(layer) {
        if bound.is_some() {
            let message = format!(
                "layer {} has inline features, which it draws; the source bound to it is not read",
                name(layer)
            );
            diagnostics.push(Diagnostic::warning(
                "source-ignored",
                message,
                layer.span.clone(),
            ));
        }
        let source = inline::InlineSource::new(&layer.features);
        return query(layer, "inline", &source, view, diagnostics).await;
    }
    let kind = layer.source.as_ref().map_or("none", |s| s.kind.as_str());
    let Some(source) = bound else {
        diagnostics.push(skipped(layer, kind, "no source is bound to it".to_owned()));
        return None;
    };
    query(layer, kind, source, view, diagnostics).await
}

/// The warning that a layer whose source is of `kind` is skipped for `reason`.
fn skipped(layer: &Layer, kind: &str, reason: String) -> Diagnostic {
    let span = layer
        .source
        .as_ref()
        .and_then(|s| s.span.clone())
        .or_else(|| layer.span.clone());
    let message = format!("layer {} (source {kind}) is skipped: {reason}", name(layer));
    Diagnostic::warning("layer-skipped", message, span)
}

/// The features `source`, of `kind`, delivers for the view, bounded in number (Q-9).
async fn query<S: FeatureSource>(
    layer: &Layer,
    kind: &str,
    source: &S,
    view: &View,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Feature>> {
    let span = layer
        .source
        .as_ref()
        .and_then(|s| s.span.clone())
        .or_else(|| layer.span.clone());
    let skip = |reason: String| skipped(layer, kind, reason);
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

/// The warning for the hatch fills of a layer refused to be drawn (Q-9), if any.
fn refused_hatches(layer: &Layer, refused: usize) -> Option<Diagnostic> {
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

fn name(layer: &Layer) -> String {
    layer
        .name
        .as_deref()
        .map_or("without NAME".to_owned(), |n| format!("`{n}`"))
}
