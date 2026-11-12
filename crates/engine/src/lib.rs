// The loading of a pixmap for the marker a label holds follows msGetMarkerSize, src/mapsymbol.c,
// of MapServer at the release pinned in scripts/fetch-msautotest.sh.
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

//! The engine: renders the map of a style IR from the sources bound to its layers. It is the
//! one core crate that awaits the ports (ADR-0008).

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod bind;
mod bindable;
mod classes;
mod fonts;
mod images;
mod inline;
mod label_binding;
mod label_styles;
mod labels;
mod plan;
mod resource;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use forager_ports::{
    Feature, FeatureQuery, FeatureSource, Geometry, ResourceLoader, ResourcePath, SourceError,
};
use forager_render::{DisplayItem, Draw, DrawOp, Point, Renderer, RgbaImage};
use forager_style_ir::c::ms_nint;
use forager_style_ir::{Diagnostic, Layer, LayerStatus, Map, RenderMode, Rgba, Style, Symbol};
use forager_symbolize::{
    Canvas, Pass, Pixmaps, Resolved, Tiles, can_draw, clip_pixmaps, draw_ops, marked_pixmap,
    point_markers, resolve, vertices,
};

use forager_expr::Evaluator;

use crate::classes::{Binder, Reports, Selector};
use crate::images::Images;
use crate::plan::Plan;

pub use crate::bind::bind_sources;
pub use crate::inline::is_inline;
pub use crate::resource::MAX_FONT_OR_IMAGE_FILE_BYTES;
pub use forager_symbolize::{MAX_MARKER_POINTS, View};

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

impl Rendered {
    /// The result of a render whose map could not be drawn at all.
    fn no_map(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            image: None,
            diagnostics,
            labels: Vec::new(),
        }
    }
}

/// A label of a render's label cache.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedLabel {
    /// From 1 to `MAX_PRIORITY` of `forager-label`.
    pub priority: u8,
    /// The index of the layer in the map, and of the class in the layer.
    pub layer: usize,
    pub class: usize,
    /// The label's text, held once for a feature's label and shared by its labels at every point.
    pub text: Arc<str>,
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
/// of its labels and the images of its pixmap symbols, through `files`, the host's resource
/// loader rooted at the base context (ADR-0031). The images are decoded once each and held until
/// the render ends (ADR-0029).
pub async fn render_with_files<S: FeatureSource, L: ResourceLoader, R: Renderer>(
    map: &Map,
    sources: &BTreeMap<String, S>,
    files: &L,
    mut renderer: R,
) -> Rendered {
    let mut diagnostics = Vec::new();
    let Some((width, height, view)) = view(map, &mut diagnostics) else {
        return Rendered::no_map(diagnostics);
    };
    renderer.begin(width, height, map.image_color.unwrap_or(DEFAULT_BACKGROUND));
    let mut labels = Labels::new(map, &view, files, (width, height));
    // The images of the render's pixmap symbols, dropped with the render (ADR-0029).
    let mut images = Images::default();
    let mut evaluator = Evaluator::new();
    let mut reports = Reports::default();
    let scale = classes::scale_denom(map, &view, width);
    let (order, cache_at) = drawing_order(map);
    let mut cached_labels = None;
    for (position, &(layer_index, layer)) in order.iter().enumerate() {
        if position == cache_at {
            cached_labels = Some(labels.finish(&mut renderer, &mut diagnostics));
        }
        let Some(mode) = layer_mode(layer, scale, &mut diagnostics) else {
            continue;
        };
        let Some(features) = features(layer, sources, &view, &mut diagnostics).await else {
            continue;
        };
        if layer.classes.is_empty() {
            continue;
        }
        let (drawable, undrawn) = drawable(mode, &features);
        let mut selector = Selector {
            binder: Binder {
                layer,
                layer_index,
                evaluator: &mut evaluator,
                reports: &mut reports,
                diagnostics: &mut diagnostics,
            },
            scale,
        };
        let selected = selector.select(drawable, &map.symbols);
        let drawing = Drawing {
            mode,
            layer,
            scale,
            view: &view,
        };
        // A point layer of LABELCACHE OFF draws each feature's labels into the layer's image as
        // pointLayerDrawShape draws them: at each point, after its markers, before the next's.
        // Any other layer's labels go through the label cache.
        // A layer of POSTLABELCACHE TRUE draws its labels at once whatever its label_cache says,
        // as msDrawLayer turns the cache off for it as it draws it (src/mapdraw.c:850-856).
        let at_once = mode == RenderMode::Point && (!layer.label_cache || layer.post_label_cache);
        drawing
            .load_pixmaps(&selected, &map.symbols, !at_once, &mut images, files)
            .await;
        let (pixmaps, tiles) = images.drawing();
        let mut canvas = drawing.canvas(map, (width, height), pixmaps, tiles);
        // Each feature is labelled by the class that draws it, in the order it is drawn.
        let drawn: Vec<labels::Labelled<'_>> = selected
            .iter()
            .map(|(feature, class, styles)| (*feature, *class, styles.first().map(|s| &**s)))
            .collect();
        let mut target = drawing.target(&mut renderer, &mut canvas);
        if at_once {
            let binder = &mut selector.binder;
            let mut settled = labels.layer_at_once(layer_index, &drawn, binder);
            for (one, &(feature, class, _)) in selected.iter().zip(&drawn) {
                let laid = labels
                    .label_at_once(&settled, feature, class, pixmaps, binder)
                    .await;
                let mut out_of_reach = vec![0; laid.len()];
                drawing.draw_points(&mut target, &mut canvas, one, |target, point| {
                    for (label, count) in laid.iter().zip(&mut out_of_reach) {
                        *count += labels.draw_at_once(&mut settled, target, label, point);
                    }
                });
                for (label, count) in laid.iter().zip(out_of_reach) {
                    labels.out_of_reach(binder.diagnostics, label.at, count);
                }
            }
        } else {
            drawing.draw(&mut target, &mut canvas, &selected);
        }
        // A layer of its own is laid over the map here, before the layer's labels are cached.
        drop(target);
        let label_hatches = labels.take_hatches_refused(layer_index);
        layer_reports(
            layer,
            &canvas,
            label_hatches,
            undrawn,
            selector.binder.diagnostics,
        );
        unknown_symbols(layer, &map.symbols, selector.binder.diagnostics);
        // Only a line, polygon, or annotation layer of POSTLABELCACHE TRUE comes here after the
        // cache is drawn: its labels are not drawn yet, each reported, so none enters the cache.
        if !at_once {
            let binder = &mut selector.binder;
            let settled = labels.layer(layer_index, mode, &drawn, binder);
            labels.collect(&settled, &drawn, pixmaps, binder).await;
        }
    }
    // The labels of the cache are drawn after every layer but those of POSTLABELCACHE TRUE, as
    // MapServer's label cache draws them.
    let labels = cached_labels.unwrap_or_else(|| labels.finish(&mut renderer, &mut diagnostics));
    Rendered {
        image: Some(renderer.finish()),
        labels,
        diagnostics,
    }
}

/// The map's layers with their indexes in the order msDrawMap draws them, and the position at which
/// it draws the label cache: the layers in their order but those of `POSTLABELCACHE TRUE`, then
/// the label cache, then those in their order (src/mapdraw.c:456, 593, 604-611).
fn drawing_order(map: &Map) -> (Vec<(usize, &Layer)>, usize) {
    let (mut order, post): (Vec<_>, Vec<_>) = map
        .layers
        .iter()
        .enumerate()
        .partition(|(_, layer)| !layer.post_label_cache);
    let cache_at = order.len();
    order.extend(post);
    (order, cache_at)
}

/// The points of `geometry` inside the map's extent, in pixels, in order: where
/// pointLayerDrawShape draws a point layer's markers and labels, which skips a point outside
/// (msPointInRect) and transforms the others by the inverse of the cellsize (msTransformPoint).
fn points_in_view(view: &View, geometry: &Geometry) -> impl Iterator<Item = Point> {
    vertices(geometry)
        .into_iter()
        .filter(|&c| view.contains(c))
        .map(|c| view.point_to_pixel(c))
}

/// The render mode `layer` is drawn in at the map's scale `scale`; none where it is hidden, out
/// of scale, or of no opacity, where msDrawLayer skips it and its labels, or of a mode not drawn.
fn layer_mode(layer: &Layer, scale: f64, diagnostics: &mut Vec<Diagnostic>) -> Option<RenderMode> {
    if !shown(layer) || !classes::layer_in_scale(layer, scale) {
        return None;
    }
    // msDrawLayer skips a layer of no opacity, its labels too.
    if checked_opacity(layer, diagnostics) == Some(0) {
        return None;
    }
    drawable_mode(layer, diagnostics)
}

/// The features a layer of `mode` draws, and how many of each kind of geometry it cannot.
fn drawable(
    mode: RenderMode,
    features: &[Feature],
) -> (Vec<&Feature>, BTreeMap<&'static str, usize>) {
    let mut undrawn = BTreeMap::new();
    let mut drawable = Vec::with_capacity(features.len());
    for feature in features {
        if can_draw(mode, &feature.geometry) {
            drawable.push(feature);
        } else {
            *undrawn.entry(kind(&feature.geometry)).or_insert(0usize) += 1;
        }
    }
    (drawable, undrawn)
}

/// Reports what `layer` left undrawn on `canvas`: symbols past the budget of markers, hatches
/// refused, those of the labelpoly styles of its labels drawn at once, `label_hatches`, among them,
/// and the geometries of `undrawn`, by kind, its render mode cannot draw.
fn layer_reports(
    layer: &Layer,
    canvas: &Canvas,
    label_hatches: usize,
    undrawn: BTreeMap<&'static str, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if canvas.markers_spent() {
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
    diagnostics.extend(refused_hatches(
        layer,
        canvas.hatches_refused() + label_hatches,
    ));
    for (index, symbol, why) in canvas.pixmaps_refused() {
        let symbol = symbol
            .name
            .as_deref()
            .map_or(format!("symbol {index}"), |n| format!("symbol `{n}`"));
        let message = format!("layer {}: {symbol} is not drawn: {why}", name(layer));
        diagnostics.push(Diagnostic::warning(
            "pixmap-not-drawn",
            message,
            layer.span.clone(),
        ));
    }
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

/// Where a layer is drawn: straight onto the map, or, where it has an opacity, into a layer of
/// the renderer, laid over the map when the target is dropped.
enum Target<'r, R: Renderer + 'r> {
    Map(&'r mut R),
    Layer(R::Layer<'r>),
}

impl<R: Renderer> Draw for Target<'_, R> {
    fn draw(&mut self, item: &DisplayItem) {
        match self {
            Target::Map(renderer) => renderer.draw(item),
            Target::Layer(layer) => layer.draw(item),
        }
    }
}

/// What a layer's features are drawn with.
struct Drawing<'a> {
    mode: RenderMode,
    layer: &'a Layer,
    /// The map's scale, which a style's scale range is compared to.
    scale: f64,
    view: &'a View,
}

impl Drawing<'_> {
    /// The canvas the layer is drawn on, in an image `width` by `height`, with the images of
    /// `pixmaps` and the fill tiles of `tiles`: the layer's scale factor sizes its styles, the
    /// image's resolution factor bounds them.
    fn canvas<'c>(
        &self,
        map: &'c Map,
        (width, height): (u32, u32),
        pixmaps: &'c Pixmaps,
        tiles: &'c mut Tiles,
    ) -> Canvas<'c> {
        let factor = classes::layer_scale_factor(map, self.layer, self.view.cellsize, self.scale);
        Canvas::new(&map.symbols, width, height, factor)
            .with_resolution(classes::resolution_factor(map))
            .with_pixmaps(pixmaps)
            .with_tiles(tiles)
    }

    /// Where the layer is drawn: into an image of its own, laid over the map, where the layer has
    /// an opacity, as msDrawLayer draws a layer with a `COMPOSITE` block, and straight onto the
    /// map otherwise.
    fn target<'r, R: Renderer>(&self, renderer: &'r mut R, canvas: &mut Canvas) -> Target<'r, R> {
        let Some(opacity) = layer_opacity(self.layer) else {
            return Target::Map(renderer);
        };
        canvas.in_own_image();
        // aggCompositeRasterBufferNoPixman lays the layer over at MS_NINT(opacity × 2.55).
        let alpha = ms_nint(f64::from(opacity) * 2.55);
        Target::Layer(renderer.layer(alpha.clamp(0, 255) as u8))
    }

    /// Loads into `images`, through `files`, the image of each pixmap symbol the styles of the
    /// `selected` features draw as a mark, those outside their scale range left out, so that the
    /// layer is drawn from them; one the store refuses leaves its style undrawn, as the canvas
    /// records. On a point layer of the label cache, the first style of a class with labels is
    /// loaded at any scale, as msAddLabelGroup and msAddLabel (`src/maplabel.c:344-352, 604-608`)
    /// size the marker the cache holds by msGetMarkerSize, which loads it
    /// (`src/mapsymbol.c:736-738`) whatever the style's scale range. Those the clip buffer of a
    /// hatched polygon's class counts are loaded whatever theirs too, as msDrawShape loads them
    /// (see [`clip_pixmaps`]). `cached` tells whether the layer's labels go through the label
    /// cache.
    async fn load_pixmaps<L: ResourceLoader>(
        &self,
        selected: &[Selected<'_>],
        symbols: &[forager_style_ir::Symbol],
        cached: bool,
        images: &mut Images,
        files: &L,
    ) {
        let holds_markers = self.mode == RenderMode::Point && cached;
        for (_, class, styles) in selected {
            let marker = holds_markers && !self.layer.classes[*class].labels.is_empty();
            for (place, style) in styles.iter().enumerate() {
                if !(place == 0 && marker)
                    && !classes::in_scale(self.scale, style.min_scale_denom, style.max_scale_denom)
                {
                    continue;
                }
                if let Some(path) = marked_pixmap(self.mode, style, symbols)
                    && !images.decoded().contains_key(path)
                {
                    // The image, or why it has none, is kept by the store for the canvas.
                    let _ = images.image(path, files).await;
                }
            }
            if self.mode == RenderMode::Polygon {
                for path in clip_pixmaps(&self.layer.classes[*class].styles, symbols) {
                    if !images.decoded().contains_key(path) {
                        let _ = images.image(path, files).await;
                    }
                }
            }
        }
    }

    /// Draws the markers of a point layer's feature `selected` at each of its points inside the
    /// map's extent, every style of its class in its scale range, each point's followed by what
    /// `label` draws at that point, as pointLayerDrawShape draws a point layer
    /// (src/mapdraw.c:2062-2105): the markers of a point, then its labels, then the next point's.
    /// It decides the order of a point layer's markers whatever its `LABELCACHE`; a layer of the
    /// label cache draws no label here.
    fn draw_points<R: Draw>(
        &self,
        renderer: &mut R,
        canvas: &mut Canvas,
        (feature, class, styles): &Selected<'_>,
        mut label: impl FnMut(&mut R, Point),
    ) {
        canvas.class(&self.layer.classes[*class].styles);
        let mut markers: Vec<Vec<Vec<DrawOp>>> = styles
            .iter()
            .filter(|s| classes::in_scale(self.scale, s.min_scale_denom, s.max_scale_denom))
            .map(|style| point_markers(&feature.geometry, style, self.view, canvas))
            .collect();
        for (k, point) in points_in_view(self.view, &feature.geometry).enumerate() {
            for style in &mut markers {
                for op in std::mem::take(&mut style[k]) {
                    renderer.draw(&DisplayItem {
                        op,
                        feature_id: Some(feature.id),
                    });
                }
            }
            label(renderer, point);
        }
    }

    /// Draws a layer's selected features in the order msDrawVectorLayer draws them, at once or
    /// from the cache as the [`Plan`] of each class says. A style outside its scale range draws
    /// nothing, where msDrawShape and the cache's pass skip it. A point layer's features are
    /// drawn point by point by [`Self::draw_points`], without their labels.
    fn draw<R: Draw>(&self, renderer: &mut R, canvas: &mut Canvas, selected: &[Selected<'_>]) {
        let mode = self.mode;
        if mode == RenderMode::Point {
            for one in selected {
                self.draw_points(renderer, canvas, one, |_, _| {});
            }
            return;
        }
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
/// ([`is_inline`], ADR-0018), whatever source is bound to it, which is not read and is reported;
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
        // The records the source skipped, one warning for the layer however many (ADR-0022).
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

/// The kind of a geometry, as a diagnostic names it.
fn kind(geometry: &Geometry) -> &'static str {
    match geometry {
        Geometry::Point(_) => "point",
        Geometry::MultiPoint(_) => "multipoint",
        Geometry::LineString(_) => "line",
        Geometry::MultiLineString(_) => "multiline",
        Geometry::Polygon(_) => "polygon",
        Geometry::MultiPolygon(_) => "multipolygon",
        Geometry::Collection(_) => "collection",
    }
}

fn name(layer: &Layer) -> String {
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
fn quoted(text: &str) -> String {
    let count = text.chars().count();
    if count <= QUOTED_CHARACTERS {
        format!("`{text}`")
    } else {
        let head: String = text.chars().take(QUOTED_CHARACTERS).collect();
        format!("`{head}\u{2026}` ({count} characters)")
    }
}
