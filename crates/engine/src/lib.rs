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
//!
//! Its modules import one another in one direction only: `labels` imports `classes`, `fonts`,
//! `inline`, `label_binding`, `label_cache`, `label_styles`, `layer`, and `marker_glyphs`;
//! `label_styles` imports `bindable`, `classes`, `label_cache`, and `layer`; `label_binding`
//! imports `classes`; `classes` imports `bindable` and `layer`; `fonts` imports `resource`, and
//! `images` imports `marker_glyphs` and `resource`; `bind`, `bindable`, `inline`, `label_cache`,
//! `layer`, `marker_glyphs`, `plan`, `reproject`, and `resource` import none, nor does a
//! module take an item of this root. The test `engine_modules.rs` of `crates/arch-test` holds that
//! direction, its tests aside, by a table in this order.

// The crate's own clippy.toml adds bans to Q-2's; its header says which.
#![warn(clippy::disallowed_types, clippy::disallowed_methods)]
// C's `(int)` cast is `forager_style_ir::c::int_cast`, never `as` (ADR-0017).
#![deny(clippy::cast_possible_truncation)]

mod bind;
mod bindable;
mod classes;
mod fonts;
mod images;
mod inline;
mod label_binding;
mod label_cache;
mod label_styles;
mod labels;
mod layer;
mod marker_glyphs;
mod plan;
mod reproject;
mod resource;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use forager_ports::{
    DeclaredProjection, Feature, FeatureQuery, FeatureSource, Geometry, ResourceLoader,
    ResourcePath, SourceError,
};
use forager_render::{DisplayItem, Draw, DrawOp, Point, Renderer, RgbaImage};
use forager_style_ir::c::opacity_alpha;
use forager_style_ir::{
    Bound, Diagnostic, Extent, GeomTransform, Layer, LayerProjection, LayerStatus, Map, NotDrawn,
    RenderMode, Rgba, Style, Symbol, SymbolKind, TransformTree,
};
use forager_symbolize::{
    Budget, Canvas, Factors, Pass, Pixmaps, Refusals, Resolved, StyleTrees, Undrawn, can_draw,
    circle, circle_ops, clip_pixmaps, draw_ops, label_pixmap, marked_pixmap, point_markers,
    resolve,
};

use forager_expr::Evaluator;
use forager_proj::Geotransform;

use crate::classes::{Binder, Reports, Selector};
use crate::images::Images;
use crate::plan::Plan;

pub use crate::bind::bind_sources;
pub use crate::inline::is_inline;
pub use crate::resource::MAX_FONT_OR_IMAGE_FILE_BYTES;
pub use forager_symbolize::{MAX_MARKER_POINTS, View};

use crate::labels::Labels;
use crate::layer::{
    Selected, draw_items, layer_opacity, name, points_in_view, refusal_reports, turned_by_map,
};

pub use crate::labels::CachedLabel;
pub use crate::reproject::MAX_HORIZON_POINTS;

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
    let Some((width, height, scale, factors, rotation)) = prepare(map, &mut diagnostics) else {
        return Rendered::no_map(diagnostics);
    };
    let view = &rotation.frame;
    renderer.begin(width, height, map.image_color.unwrap_or(DEFAULT_BACKGROUND));
    let mut labels = Labels::new(map, view, &factors, files, (width, height));
    // The images of the render's pixmap symbols, dropped with the render (ADR-0029).
    let mut images = Images::default();
    let mut evaluator = Evaluator::new();
    let mut reports = Reports::default();
    let (order, cache_at) = drawing_order(map);
    let mut cached_labels = None;
    for (position, &(layer_index, layer)) in order.iter().enumerate() {
        if position == cache_at {
            cached_labels = Some(labels.finish(&mut renderer, images.decoded(), &mut diagnostics));
        }
        let Some(mode) = layer_mode(layer, scale, &mut diagnostics) else {
            continue;
        };
        let Some((mut features, centres)) =
            in_view(map, layer, mode, sources, &rotation, &mut diagnostics).await
        else {
            continue;
        };
        if layer.classes.is_empty() {
            continue;
        }
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
        let cellsize = classes::layer_cellsize(map, view.cellsize);
        let Some(budget) = transformed(&mut selector, mode, &mut features, cellsize) else {
            continue;
        };
        let (drawable, undrawn) = drawable(mode, &features);
        let selected = selector.select(drawable, &map.symbols);
        let drawing = Drawing::new(mode, layer, scale, view, centres.as_ref(), map.angle);
        // A point layer of LABELCACHE OFF draws each feature's labels into the layer's image as
        // pointLayerDrawShape draws them: at each point, after its markers, before the next's.
        // Any other layer's labels go through the label cache.
        // A layer of POSTLABELCACHE TRUE draws its labels at once whatever its label_cache says,
        // as msDrawLayer turns the cache off for it as it draws it (src/mapdraw.c:850-856).
        let at_once = mode == RenderMode::Point && (!layer.label_cache || layer.post_label_cache);
        drawing
            .load_resources(&selected, &map.symbols, !at_once, &mut images, &mut labels)
            .await;
        let (pixmaps, tiles, glyphs) = images.drawing();
        let mut canvas = layer_canvas(map, layer, factors[layer_index], (width, height), pixmaps)
            .with_tiles(tiles)
            .with_glyphs(glyphs)
            .with_style_trees(StyleTrees::new(budget, cellsize));
        let drawn = labelled(&selected);
        let mut target = drawing.target(&mut renderer, &mut canvas);
        let style_failures = if at_once {
            let binder = &mut selector.binder;
            let mut settled = labels.layer_at_once(layer_index, &drawn, pixmaps, binder);
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
            StyleFailures::new()
        } else {
            drawing.draw(&mut target, &mut canvas, &selected)
        };
        // A layer of its own is laid over the map here, before the layer's labels are cached.
        drop(target);
        let refusals = labels.layer_refusals(layer_index, canvas.take_refusals());
        let (spent, diagnostics) = (canvas.markers_spent(), &mut *selector.binder.diagnostics);
        layer_reports(layer, spent, refusals, undrawn, diagnostics);
        let trees = (style_failures, canvas.style_trees_over_limit());
        style_reports(layer, mode, &map.symbols, trees, diagnostics);
        // Only a line, polygon, or annotation layer of POSTLABELCACHE TRUE comes here after the
        // cache is drawn: its labels are not drawn yet, each reported, so none enters the cache.
        if !at_once {
            let binder = &mut selector.binder;
            let settled = labels.layer(layer_index, mode, &drawn, binder);
            labels
                .collect(&settled, &drawn, pixmaps, glyphs, binder)
                .await;
        }
    }
    // The labels of the cache are drawn after every layer but those of POSTLABELCACHE TRUE, as
    // MapServer's label cache draws them.
    let labels = cached_labels
        .unwrap_or_else(|| labels.finish(&mut renderer, images.decoded(), &mut diagnostics));
    Rendered {
        image: Some(renderer.finish()),
        labels,
        diagnostics,
    }
}

/// The reports on the styles of a layer drawn in `mode`: the symbols no symbol is named; but in a
/// point layer, which draws them at its points, and a circle layer, which draws its circle
/// whatever the transform (`src/mapdraw.c:2390-2396`), the styles of a GEOMTRANSFORM not drawn;
/// and what the class styles' transform trees left undrawn, their failures and the bound they
/// passed (see [`style_tree_reports`]).
fn style_reports(
    layer: &Layer,
    mode: RenderMode,
    symbols: &[Symbol],
    (failures, over_limit): (StyleFailures, Option<&str>),
    diagnostics: &mut Vec<Diagnostic>,
) {
    unknown_symbols(layer, symbols, diagnostics);
    if !matches!(mode, RenderMode::Point | RenderMode::Circle) {
        styles_not_drawn(layer, diagnostics);
    }
    style_tree_reports(layer, failures, over_limit, diagnostics);
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

/// The features a layer of `mode` draws, and how many of each kind of geometry it cannot. A
/// circle layer takes every feature, as msDrawVectorLayer counts each against its `MAXFEATURES`,
/// and counts those that are no circle (see [`circle`]), which it draws nothing of.
fn drawable(
    mode: RenderMode,
    features: &[Feature],
) -> (Vec<&Feature>, BTreeMap<&'static str, usize>) {
    let mut undrawn = BTreeMap::new();
    let mut drawable = Vec::with_capacity(features.len());
    for feature in features {
        let drawn = can_draw(mode, &feature.geometry);
        if drawn {
            drawable.push(feature);
        }
        if !drawn || (mode == RenderMode::Circle && circle(&feature.geometry).is_none()) {
            *undrawn.entry(kind(&feature.geometry)).or_insert(0usize) += 1;
        }
    }
    (drawable, undrawn)
}

/// Reports what `layer` left undrawn: symbols past the budget of markers where `markers_spent`,
/// the hatches and pixmaps of `refusals`, its features' and those of its labels drawn at once, and
/// the geometries of `undrawn`, by kind, its render mode cannot draw.
fn layer_reports(
    layer: &Layer,
    markers_spent: bool,
    refusals: Refusals,
    undrawn: BTreeMap<&'static str, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if markers_spent {
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

/// The canvas `layer` of `map` is drawn on, in an image `width` by `height`, with the images of
/// `pixmaps`, at `factors`, the layer's scale factor, which sizes its styles, and the image's
/// resolution factor, which bounds them; its lines unclipped where the layer says so.
fn layer_canvas<'c>(
    map: &'c Map,
    layer: &Layer,
    factors: Factors,
    (width, height): (u32, u32),
    pixmaps: &'c Pixmaps,
) -> Canvas<'c> {
    let canvas = Canvas::new(&map.symbols, width, height, factors).with_pixmaps(pixmaps);
    if layer.unclipped_lines {
        canvas.with_lines_unclipped()
    } else {
        canvas
    }
}

/// The fonts the TrueType symbols of the `selected` features' styles name, each once, which their
/// markers draw their glyphs from (ADR-0044); those of
/// styles outside their scale range at the map's `scale` are left out, as their pixmaps are, but
/// for the first style of a class for which `holds_marker` is true: the label cache sizes that
/// marker by msGetMarkerSize whatever its scale range (`src/maplabel.c:344-352, 604-608`).
fn truetype_fonts<'s>(
    selected: &'s [Selected<'_>],
    symbols: &'s [Symbol],
    scale: f64,
    holds_marker: impl Fn(usize) -> bool,
) -> Vec<&'s str> {
    let mut fonts: Vec<&str> = Vec::new();
    for (_, class, styles) in selected {
        let marker = holds_marker(*class);
        for (place, style) in styles.iter().enumerate() {
            if ((place == 0 && marker)
                || classes::in_scale(scale, style.min_scale_denom, style.max_scale_denom))
                && let Resolved::Found(_, symbol) = resolve(&style.symbol, symbols)
                && symbol.kind == SymbolKind::Truetype
                && let Some(font) = symbol.font.as_deref()
                && !fonts.contains(&font)
            {
                fonts.push(font);
            }
        }
    }
    fonts
}

/// The first failure of each class style's transform tree in a layer's drawing, by the places of
/// its class in the layer and of the style in its class.
type StyleFailures = BTreeMap<(usize, usize), String>;

/// What a layer's features are drawn with.
struct Drawing<'a> {
    mode: RenderMode,
    layer: &'a Layer,
    /// The map's scale, which a style's scale range is compared to.
    scale: f64,
    view: &'a View,
    /// The reprojection of a circle layer's centres, where the layer is reprojected.
    centres: Option<&'a reproject::LayerReprojection>,
    /// The map's `ANGLE`, which turns the angles of a point layer's styles.
    map_angle: Option<f64>,
}

impl<'a> Drawing<'a> {
    fn new(
        mode: RenderMode,
        layer: &'a Layer,
        scale: f64,
        view: &'a View,
        centres: Option<&'a reproject::LayerReprojection>,
        map_angle: Option<f64>,
    ) -> Self {
        Self {
            mode,
            layer,
            scale,
            view,
            centres,
            map_angle,
        }
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
        Target::Layer(renderer.layer(opacity_alpha(opacity)))
    }

    /// Loads what the layer is drawn from, through the resource loader of `labels`: the fonts of
    /// the TrueType symbols the `selected` features' styles name, kept in `images` from the store
    /// of the labels' fonts (see [`truetype_fonts`]), by the scale rule of the pixmaps below; and
    /// into `images` the image of each pixmap symbol the styles of the
    /// `selected` features draw as a mark, those outside their scale range left out, so that the
    /// layer is drawn from them; one the store refuses leaves its style undrawn, as the canvas
    /// records. On a point layer of the label cache, the first style of a class with labels is
    /// loaded at any scale, as msAddLabelGroup and msAddLabel (`src/maplabel.c:344-352, 604-608`)
    /// size the marker the cache holds by msGetMarkerSize, which loads it
    /// (`src/mapsymbol.c:736-738`) whatever the style's scale range. Those the clip buffer of a
    /// hatched polygon's class counts are loaded whatever theirs too, as msDrawShape loads them
    /// (see [`clip_pixmaps`]), and those of the labelpoint and labelcenter styles of the classes'
    /// labels (see [`label_pixmap`]). `cached` tells whether the layer's labels go through the
    /// label cache.
    async fn load_resources<L: ResourceLoader>(
        &self,
        selected: &[Selected<'_>],
        symbols: &[forager_style_ir::Symbol],
        cached: bool,
        images: &mut Images,
        labels: &mut Labels<'_, L>,
    ) {
        let holds_markers = self.mode == RenderMode::Point && cached;
        let holds_marker =
            |class: usize| holds_markers && !self.layer.classes[class].labels.is_empty();
        let fonts = truetype_fonts(selected, symbols, self.scale, holds_marker);
        labels.load_marker_fonts(images.glyphs_mut(), &fonts).await;
        let files = labels.files();
        for (_, class, styles) in selected {
            let marker = holds_marker(*class);
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
            // The pixmaps of its labels' labelpoint and labelcenter styles, which msDrawMarkerSymbol
            // and msGetMarkerSize load as they draw and size them.
            for label in &self.layer.classes[*class].labels {
                for style in &label.styles {
                    if let Some(path) = label_pixmap(style, symbols)
                        && !images.decoded().contains_key(path)
                    {
                        let _ = images.image(path, files).await;
                    }
                }
            }
            for path in clip_pixmaps(self.mode, &self.layer.classes[*class].styles, symbols) {
                if !images.decoded().contains_key(path) {
                    let _ = images.image(path, files).await;
                }
            }
        }
    }

    /// Draws the markers of a point layer's feature `selected` at each of its points inside the
    /// map's extent, every style of its class in its scale range, each point's followed by what
    /// `label` draws at that point, as pointLayerDrawShape draws a point layer
    /// (src/mapdraw.c:2062-2105): the markers of a point, then its labels, then the next point's.
    /// It decides the order of a point layer's markers whatever its `LABELCACHE`; a layer of the
    /// label cache draws no label here. On a rotated map, each style's angle is turned by the
    /// map's (see [`turned_by_map`]), as pointLayerDrawShape turns it for the shape and back.
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
            .map(|style| {
                let style = self.turned(style);
                point_markers(&feature.geometry, &style, self.view, canvas)
            })
            .collect();
        for (k, point) in points_in_view(self.view, &feature.geometry).enumerate() {
            for style in &mut markers {
                draw_items(renderer, feature.id, std::mem::take(&mut style[k]));
            }
            label(renderer, point);
        }
    }

    /// `style` with its angle turned by the map's (see [`turned_by_map`]).
    fn turned<'s>(&self, style: &'s Style) -> Cow<'s, Style> {
        match style.angle {
            Some(Bound::Value(angle)) if self.map_angle.is_some() && angle != 0.0 => {
                let mut style = style.clone();
                style.angle = Some(Bound::Value(turned_by_map(angle, self.map_angle)));
                Cow::Owned(style)
            }
            _ => Cow::Borrowed(style),
        }
    }

    /// Draws a circle layer's `selected` features in order, each as its circle (see [`circle`]),
    /// its centre reprojected where the layer is, and left where it does not reproject, as
    /// msProjectPoint leaves it: every style of its class in its scale range, as
    /// circleLayerDrawShape draws them (`src/mapdraw.c:2008-2029`). A feature without a circle
    /// draws nothing.
    fn draw_circles<R: Draw>(
        &self,
        renderer: &mut R,
        canvas: &mut Canvas,
        selected: &[Selected<'_>],
    ) {
        for (feature, _, styles) in selected {
            let Some(mut circle) = circle(&feature.geometry) else {
                continue;
            };
            if let Some(centre) = self.centres.and_then(|c| c.point(circle.centre)) {
                circle.centre = centre;
            }
            for style in styles {
                if classes::in_scale(self.scale, style.min_scale_denom, style.max_scale_denom) {
                    let ops = circle_ops(circle, style, self.view, canvas);
                    draw_items(renderer, feature.id, ops);
                }
            }
        }
    }

    /// Draws a layer's selected features in the order msDrawVectorLayer draws them, at once or
    /// from the cache as the [`Plan`] of each class says. A style outside its scale range draws
    /// nothing, where msDrawShape and the cache's pass skip it. A point layer's features are
    /// drawn point by point by [`Self::draw_points`], without their labels. The first failure of
    /// each class style's transform tree, in the order the shapes are drawn, by the places of its
    /// class in the layer and of the style in its class.
    fn draw<R: Draw>(
        &self,
        renderer: &mut R,
        canvas: &mut Canvas,
        selected: &[Selected<'_>],
    ) -> StyleFailures {
        let mut failures = StyleFailures::new();
        let mode = self.mode;
        if mode == RenderMode::Point {
            for one in selected {
                self.draw_points(renderer, canvas, one, |_, _| {});
            }
            return failures;
        }
        if mode == RenderMode::Circle {
            self.draw_circles(renderer, canvas, selected);
            return failures;
        }
        let mut draw = |canvas: &mut Canvas,
                        feature: &Feature,
                        (class, place): (usize, usize),
                        style: &Style,
                        passes: &[Pass]| {
            if !classes::in_scale(self.scale, style.min_scale_denom, style.max_scale_denom) {
                return;
            }
            for &pass in passes {
                let ops = draw_ops(mode, &feature.geometry, style, pass, self.view, canvas);
                draw_items(renderer, feature.id, ops);
                if let Some(why) = canvas.take_style_tree_failure() {
                    failures.entry((class, place)).or_insert(why);
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
            for (place, style, passes) in &passes.at_once {
                draw(canvas, feature, (*class, *place), style, passes);
            }
            if plan.cached() {
                most_styles = most_styles.max(styles.len());
                cached.push((*feature, *class, passes.cached));
            }
        }
        for place in 0..most_styles {
            for (feature, class, later) in &cached {
                // The shape cached is the one its own class's buffer clipped.
                canvas.class(&self.layer.classes[*class].styles);
                for (_, style, passes) in later.iter().filter(|(p, _, _)| *p == place) {
                    draw(canvas, feature, (*class, place), style, passes);
                }
            }
        }
        failures
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

/// What msPrepareImage computes of `map` before its layers are drawn, or `None` with an error:
/// the image's width and height, the scale from the real extent, the factors of each layer by
/// its index from msGetGeoCellSize's cellsize (see [`classes::scales`]), and the rotation.
fn prepare(
    map: &Map,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(u32, u32, f64, Vec<Factors>, Rotation)> {
    let (width, height, view) = view(map, diagnostics)?;
    let rotation = Rotation::of(map, &view, (width, height), diagnostics)?;
    let (scale, factors) = classes::scales(map, &view, rotation.geo_cellsize, width);
    Some((width, height, scale, factors, rotation))
}

/// The rotation of a map by its `angle` (ADR-draft-map-rotation-in-the-reprojection).
struct Rotation {
    /// The view everything after the reprojection is drawn in: on a rotated map, the frame of its
    /// image, `0, 0, width, height` at a cellsize of 1, as msMapSetFakedExtent fakes it.
    frame: View,
    /// The map's geotransform, held on the map's side of every reprojection; none unrotated.
    geotransform: Option<Geotransform>,
    /// The cellsize the scale factor of a size in units other than pixels divides by.
    geo_cellsize: f64,
}

impl Rotation {
    /// The rotation of `map` in `view`, its image of `size`: none for an angle of 0 or none, as
    /// msMapSetRotation sets none; `None`, with an error, where the angle gives no geotransform.
    fn of(
        map: &Map,
        view: &View,
        size: (u32, u32),
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        let unrotated = Self {
            frame: *view,
            geotransform: None,
            geo_cellsize: view.cellsize,
        };
        let Some(angle) = map.angle.filter(|&angle| angle != 0.0) else {
            return Some(unrotated);
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
            geo_cellsize: classes::rotated_geo_cellsize(&view.extent, width, height),
        })
    }
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
        RenderMode::Point | RenderMode::Line | RenderMode::Polygon | RenderMode::Circle
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

/// The features of `layer` in view, drawn in `mode`: those its source delivers for the area
/// [`asked`], in the map's projection, reprojected where the layer is, and on a rotated map in the
/// frame of its image (see [`Rotation`]). A circle layer's are left
/// in the layer's projection, and returned with the reprojection of their centres, where the
/// layer is reprojected: circleLayerDrawShape reprojects only a circle's centre
/// (`src/mapdraw.c:2008-2011`).
async fn in_view<S: FeatureSource>(
    map: &Map,
    layer: &Layer,
    mode: RenderMode,
    sources: &BTreeMap<String, S>,
    rotation: &Rotation,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Vec<Feature>, Option<reproject::LayerReprojection>)> {
    let declared = declared_projection(layer, sources, diagnostics);
    let (reprojection, asked) = asked(map, layer, &declared, rotation, diagnostics)?;
    let mut features = source_features(layer, sources, asked, diagnostics).await?;
    if mode == RenderMode::Circle {
        if let Some(reprojection) = &reprojection {
            centres_not_reprojected(reprojection, layer, &features, diagnostics);
        }
        return Some((features, reprojection));
    }
    if let Some(reprojection) = &reprojection {
        reproject_features(reprojection, layer, &mut features, diagnostics);
    }
    Some((features, None))
}

/// Reports the circles of a circle layer's `features` whose centre does not reproject by
/// `reprojection` (see [`circle`]). The centres are reprojected here to count them and again as
/// they are drawn, which keeps the warning with the other warnings of the layer's reprojection.
fn centres_not_reprojected(
    reprojection: &reproject::LayerReprojection,
    layer: &Layer,
    features: &[Feature],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let failed = features
        .iter()
        .filter_map(|feature| circle(&feature.geometry))
        .filter(|one| reprojection.point(one.centre).is_none())
        .count();
    if failed > 0 {
        diagnostics.push(points_not_reprojected(layer, failed));
    }
}

/// How `layer` is reprojected into the map's projection, where it is, and the area its source is
/// asked for: the map's extent, reprojected into the layer's projection where the layer is
/// reprojected. None, with a warning, where a projection does not read or no point of the map's
/// extent reprojects into the layer's. The map's extent is `rotation`'s frame; on a rotated map,
/// a layer that is not reprojected is asked for the frame itself, as msDrawVectorLayer asks it,
/// and drawn unrotated, with a warning.
fn asked(
    map: &Map,
    layer: &Layer,
    declared: &DeclaredProjection,
    rotation: &Rotation,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Option<reproject::LayerReprojection>, Extent)> {
    let (view, geotransform) = (&rotation.frame, rotation.geotransform.as_ref());
    let reprojection = match reproject::for_layer(map, layer, declared, geotransform) {
        Ok(reprojection) => reprojection,
        Err(error) => {
            let message = format!(
                "layer {} is not drawn: its projection or the map's cannot be reprojected: {error}",
                name(layer)
            );
            diagnostics.push(Diagnostic::warning(
                "projection-unsupported",
                message,
                layer.span.clone(),
            ));
            return None;
        }
    };
    let Some(reprojection) = reprojection else {
        if geotransform.is_some() {
            diagnostics.push(rotation_without_projection(layer));
        }
        return Some((None, view.extent));
    };
    let Some(extent) = reprojection.asked(&view.extent) else {
        let message = format!(
            "layer {} is not drawn: no point of the map's extent reprojects into its projection",
            name(layer)
        );
        diagnostics.push(Diagnostic::warning(
            "extent-not-reprojected",
            message,
            layer.span.clone(),
        ));
        return None;
    };
    Some((Some(reprojection), extent))
}

/// Reprojects `layer`'s `features` into the map's projection, with a warning for lines MapServer
/// would cut with GEOS, for points that do not reproject, and for horizon points past
/// [`MAX_HORIZON_POINTS`] (Q-9), at which the layer stops.
fn reproject_features(
    reprojection: &reproject::LayerReprojection,
    layer: &Layer,
    features: &mut Vec<Feature>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    reproject_features_within(
        reprojection,
        layer,
        features,
        diagnostics,
        MAX_HORIZON_POINTS,
    );
}

/// [`reproject_features`], the horizon points held to `bound`, which a test sets low.
fn reproject_features_within(
    reprojection: &reproject::LayerReprojection,
    layer: &Layer,
    features: &mut Vec<Feature>,
    diagnostics: &mut Vec<Diagnostic>,
    bound: usize,
) {
    if reprojection.cuts_lines_with_geos() && features.iter().any(|f| has_lines(&f.geometry)) {
        let message = format!(
            "layer {}: MapServer cuts its lines with GEOS where they cross the dateline from \
         a polar projection into longitude and latitude, which Forager does not; they are \
         drawn uncut",
            name(layer)
        );
        diagnostics.push(Diagnostic::warning(
            "lines-not-cut",
            message,
            layer.span.clone(),
        ));
    }
    let done = reprojection.features_within(features, bound);
    if done.failed > 0 {
        diagnostics.push(points_not_reprojected(layer, done.failed));
    }
    if done.stopped {
        diagnostics.push(horizon_points_limit(layer));
    }
}

/// Whether `geometry` holds a line, which a line shape of MapServer holds.
fn has_lines(geometry: &Geometry) -> bool {
    match geometry {
        Geometry::LineString(_) | Geometry::MultiLineString(_) => true,
        Geometry::Collection(members) => members.iter().any(has_lines),
        _ => false,
    }
}

/// The warning that the horizon search of `layer`'s reprojection passes
/// [`reproject::MAX_HORIZON_POINTS`] and the layer stops at the feature that passes it (Q-9).
fn horizon_points_limit(layer: &Layer) -> Diagnostic {
    let message = format!(
        "layer {}: the horizon search of its reprojection adds more than {} points to its shapes \
         (Q-9), and it stops at the feature that passes it",
        name(layer),
        reproject::MAX_HORIZON_POINTS
    );
    Diagnostic::warning("horizon-points-limit", message, layer.span.clone())
}

/// The projection the source bound to a layer of `PROJECTION AUTO` declares
/// (ADR-0039), none for any other layer or one read from its inline
/// features; one that does not read is none, with a warning naming the layer and the file, where
/// MapServer writes a debug message.
fn declared_projection<S: FeatureSource>(
    layer: &Layer,
    sources: &BTreeMap<String, S>,
    diagnostics: &mut Vec<Diagnostic>,
) -> DeclaredProjection {
    if layer.projection != Some(LayerProjection::FromSource) || is_inline(layer) {
        return DeclaredProjection::None;
    }
    let Some(source) = layer.name.as_ref().and_then(|n| sources.get(n)) else {
        return DeclaredProjection::None;
    };
    let declared = source.declared_projection();
    if let DeclaredProjection::Unreadable { file, why } = &declared {
        let message = format!(
            "layer {}: the projection of {file} does not read ({why}); the layer takes its \
             coordinates as longitude and latitude in the datum of the map's projection",
            name(layer)
        );
        diagnostics.push(Diagnostic::warning(
            "projection-unreadable",
            message,
            layer.span.clone(),
        ));
    }
    declared
}

/// The warning that `layer`, on a rotated map, is not reprojected, because it or the map has no
/// projection, and lands in the frame of the image unrotated, where MapServer draws it. MapServer
/// is silent; the warning tells the user why (ADR-draft-map-rotation-in-the-reprojection).
fn rotation_without_projection(layer: &Layer) -> Diagnostic {
    let message = format!(
        "layer {} is not rotated by the map's ANGLE: it or the map has no PROJECTION, so its \
         coordinates are drawn as the image's pixels, as MapServer draws them",
        name(layer)
    );
    Diagnostic::warning("rotation-without-projection", message, layer.span.clone())
}

/// The warning that `count` points of `layer`'s features do not reproject into the map's
/// projection (Q-3): cut at the horizon, left out with their point shape, or, into longitude and
/// latitude from a projected layer, left where they were, as MapServer reprojects them.
fn points_not_reprojected(layer: &Layer, count: usize) -> Diagnostic {
    let message = format!(
        "layer {}: {count} point{} of its features do{} not reproject into the map's projection",
        name(layer),
        if count == 1 { "" } else { "s" },
        if count == 1 { "es" } else { "" },
    );
    Diagnostic::warning("points-not-reprojected", message, layer.span.clone())
}

/// The features of a layer within `extent`, in its source's coordinates: those of its inline
/// features where it is read from them ([`is_inline`], ADR-0018), whatever source is bound to it,
/// which is not read and is reported; otherwise those of the source bound to its name.
async fn source_features<S: FeatureSource>(
    layer: &Layer,
    sources: &BTreeMap<String, S>,
    extent: Extent,
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
        return query(layer, "inline", &source, extent, diagnostics).await;
    }
    let kind = layer.source.as_ref().map_or("none", |s| s.kind.as_str());
    let Some(source) = bound else {
        diagnostics.push(skipped(layer, kind, "no source is bound to it".to_owned()));
        return None;
    };
    query(layer, kind, source, extent, diagnostics).await
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

/// The features `source`, of `kind`, delivers for `extent`, bounded in number (Q-9).
async fn query<S: FeatureSource>(
    layer: &Layer,
    kind: &str,
    source: &S,
    extent: Extent,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<Feature>> {
    let span = layer
        .source
        .as_ref()
        .and_then(|s| s.span.clone())
        .or_else(|| layer.span.clone());
    let skip = |reason: String| skipped(layer, kind, reason);
    let query = FeatureQuery { extent };
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

/// Warns of each class style of a line or a polygon layer whose `GEOMTRANSFORM` the engine does
/// not draw on a style: a transform tree `not_drawn`, naming its reason
/// (ADR-0043, *Diagnostics*), which MapServer evaluates
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
/// (ADR-0043, *A class's style*): of each style in
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

/// Each feature of `selected` with the class that draws it, by which it is labelled, and its
/// first style, in the order it is drawn.
fn labelled<'a>(selected: &'a [Selected<'a>]) -> Vec<labels::Labelled<'a>> {
    selected
        .iter()
        .map(|(feature, class, styles)| (*feature, *class, styles.first().map(|s| &**s)))
        .collect()
}

/// The budget of Q-9 the layer of `selector` leaves to its class styles' transform trees, where
/// it is drawn after its `GEOMTRANSFORM`: its `features` transformed, `[map_cellsize]` at
/// `cellsize`, where it has one, whole where it has none; `None`, none drawn, with a warning,
/// where its transform tree is `not_drawn` or fails (ADR-0043).
fn transformed(
    selector: &mut Selector<'_, '_>,
    mode: RenderMode,
    features: &mut [Feature],
    cellsize: f64,
) -> Option<Budget> {
    let layer = selector.binder.layer;
    let mut budget = Budget::default();
    let warning = match &layer.geom_transform {
        None => return Some(budget),
        Some(TransformTree::NotDrawn(reason)) => transform_not_drawn(layer, *reason),
        Some(TransformTree::Drawn(shape)) => {
            match selector.transform(mode, features, shape, cellsize, &mut budget) {
                Ok(()) => return Some(budget),
                Err(undrawn) => transform_failed(layer, undrawn),
            }
        }
    };
    selector.binder.diagnostics.push(warning);
    None
}

/// The warning that `layer` draws nothing, its `GEOMTRANSFORM` a transform tree of the reason
/// `reason` (ADR-0043). MapServer evaluates a function the tree has no node
/// for through GEOS, or a JavaScript engine, and fails on a syntax error or a binding where it
/// makes no number; Forager draws none of the layer, and the rest of the map.
fn transform_not_drawn(layer: &Layer, reason: NotDrawn) -> Diagnostic {
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
fn transform_failed(layer: &Layer, undrawn: Undrawn) -> Diagnostic {
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
