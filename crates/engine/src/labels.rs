// The labels of point layers follow pointLayerDrawShape, msDrawLabelCache, its choice of a point
// label's position and offsets and its member's box, src/mapdraw.c, msAddLabelGroup and
// msComputeTextPath, src/maplabel.c, bindLabel, bindIntegerAttribute, and
// msShapeGetLabelAnnotation, src/maputil.c, msGetMarkerSize, src/mapsymbol.c, and
// msDrawTextSymbol, src/maprendering.c, of MapServer at the release pinned in
// scripts/fetch-msautotest.sh, with the label cache of the label crate (ADR-0031).
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
// Copyright (c) 1996-2013 Regents of the University of Minnesota.
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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use forager_label::{
    Align, Anchor, Font, Glyphs, Layout, LineError, MAX_LABELS, Member, Members, Offsets,
    Placement, PointLabel, Position, TextBox, TextPath, glyph_size, lines, member_box, pixel_size,
    placed_at, walk_label_cache,
};
use forager_ports::{Feature, ResourceLoader};
use forager_render::{Draw, DrawOp, GlyphOutline, Point};
use forager_style_ir::c::ms_nint;
use forager_style_ir::{Diagnostic, Expr, LabelTransform, Map, Op, RenderMode, Rgba, Span, Style};
use forager_symbolize::{DEG_TO_RAD, Pixmaps, View, marker_size_of};

use crate::classes::{self, Binder, c_bytes};
use crate::fonts::Fonts;
use crate::inline::feature_text;
use crate::label_binding::{BoundLabel, Fixed, bind, fixed};
use crate::label_styles::{self, LabelMarker, LabelStyles, Placed, Styled, StylesOf};
use crate::{CachedLabel, quoted};

/// Why a label's text is not laid out: the label names no font, no font of its list draws a line,
/// or its font cannot draw a glyph, nor its fallback.
enum Unlaid {
    NoFont,
    Font(String),
    Glyph(u32),
}

impl Unlaid {
    /// The code and the outcome a label is reported with. Its font is needed only for a text, so
    /// the label is still drawn for the features that give it none, and a glyph fails only the
    /// texts that hold it.
    fn code(&self) -> (&'static str, &'static str) {
        const WHERE_TEXT: &str = "the label is not drawn where a feature gives it a text";
        match self {
            Self::NoFont => ("label-not-drawn", WHERE_TEXT),
            Self::Font(_) => ("label-font-unusable", WHERE_TEXT),
            Self::Glyph(_) => (
                "label-glyph-unreadable",
                "the label is not drawn for a text that holds it",
            ),
        }
    }

    /// The reason a label of `text` is reported with.
    fn reason(self, text: &str) -> String {
        match self {
            Self::NoFont => "it names no FONT; MapServer's built-in font is not bundled".to_owned(),
            Self::Font(why) => why,
            Self::Glyph(glyph) => format!("its font cannot draw glyph {glyph} of {}", quoted(text)),
        }
    }
}

/// A label of the render by its layer's index, its class's, and its index in the class.
pub(crate) type LabelAt = (usize, usize, usize);

/// What of a label is the same for all its features, as the label cache reads it.
struct Def {
    at: LabelAt,
    /// The buffer in whole pixels.
    buffer: i32,
    force: bool,
    partials: bool,
    /// `MINDISTANCE` in pixels.
    min_distance: f64,
    /// What an outline adds to the offset from a marker: half its width, scaled.
    outline_grow: f64,
    /// The label's scale factor, its layer's, by which a labelpnt marker its styles bind to a
    /// feature is sized as the cache tests it.
    factor: f64,
    /// The width and height of the marker beneath each of its labels, where its class's first
    /// style varies by no feature and so sizes every feature's marker alike; held once here, not
    /// for each label (Q-9).
    marker: Option<(f64, f64)>,
}

/// A path that stands for a text of line breaks only, which has no box.
const NO_PATH: u32 = u32::MAX;

/// One label at one point, held in the label cache until the labels are drawn: its text path,
/// and where and how it is drawn.
#[derive(Clone, Copy)]
struct Cached {
    point: Point,
    rotation: f64,
    feature_id: u64,
    /// Into the paths, or [`NO_PATH`].
    path: u32,
    /// Into the label definitions.
    def: u32,
    /// Into the texts of the labels cached, one for each feature's label, whatever its points.
    text: u32,
    /// The label's `OFFSET`, scaled, in whole pixels.
    offset: [i32; 2],
    color: Option<Rgba>,
    /// The outline's colour and its width in whole pixels.
    outline: Option<(Rgba, i32)>,
    position: Position,
    priority: u8,
    /// The point is tested first: the label is neither forced nor offset.
    test_point: bool,
}

/// What a label's text is drawn with: its feature, its angle, its colour, and its outline's
/// colour and width in whole pixels.
#[derive(Clone, Copy)]
struct Ink {
    feature_id: u64,
    rotation: f64,
    color: Option<Rgba>,
    outline: Option<(Rgba, i32)>,
}

impl Ink {
    fn of(label: &Cached) -> Self {
        Self {
            feature_id: label.feature_id,
            rotation: label.rotation,
            color: label.color,
            outline: label.outline,
        }
    }
}

/// A feature's label laid out, its properties bound to the feature, to be cached at each of the
/// feature's points or drawn at once there.
pub(crate) struct Laid {
    pub(crate) at: LabelAt,
    text: String,
    /// The text's path; none for a text that is empty or of line breaks only.
    path: Option<TextPath>,
    ink: Ink,
    /// The label's `OFFSET`, scaled, in whole pixels.
    offset: [i32; 2],
    position: Position,
    priority: u8,
    /// The point is tested first: the label is neither forced nor offset.
    test_point: bool,
    /// Its styles bound to the feature where they vary by it, else none.
    styles: Box<[Style<LabelTransform>]>,
    /// The width and height of the marker the label cache holds beneath it, and its first
    /// labelpnt style's marker; neither for a label drawn at once.
    marker: Option<(f64, f64)>,
    label_marker: Option<LabelMarker>,
}

/// What of a layer's labels is settled before its features: what of each label of the classes
/// drawn is settled, or none where it is not drawn, by its class's index and its own.
pub(crate) struct LayerLabels<'r> {
    index: usize,
    fixed: BTreeMap<(usize, usize), Option<Fixed<'r>>>,
}

/// What of a point layer of `LABELCACHE OFF` is settled before its features: its labels, drawn at
/// once, and what they draw their styles with.
pub(crate) struct AtOnce<'r> {
    labels: LayerLabels<'r>,
    styled: Styled<'r>,
}

/// A label of a feature's class that the feature matches: where it is, what of it is settled,
/// and its styles bound to the feature where they vary by it, else none.
struct Matched<'a, 'r> {
    at: LabelAt,
    fixed: &'a Fixed<'r>,
    styles: Box<[Style<LabelTransform>]>,
}

/// A label's text for a feature, the text's lines, and the label's properties bound to the
/// feature.
struct Annotation {
    text: String,
    lines: Vec<Vec<char>>,
    bound: BoundLabel,
}

/// The labels of one render, collected over its layers and drawn after them by MapServer's label
/// cache, with each problem of a label reported once. A label is held as its glyphs' indices and
/// its point; the glyphs' outlines are drawn once per font and size, and a label's path and its
/// box are built only when it is placed and drawn.
pub(crate) struct Labels<'r, L> {
    map: &'r Map,
    view: &'r View,
    files: &'r L,
    fonts: Fonts,
    /// The glyphs of each font and pixel size, by the font's path and the size.
    glyphs: Vec<Glyphs>,
    caches: BTreeMap<(String, u16), usize>,
    paths: Vec<TextPath>,
    defs: Vec<Def>,
    def_index: BTreeMap<LabelAt, u32>,
    cached: Vec<Cached>,
    /// The text of each feature's label cached, held once whatever its points, which the label
    /// cache compares under `MINDISTANCE` and the render's report of its labels shares.
    label_texts: Vec<Box<str>>,
    /// The styles of the labels cached and their markers.
    styles: LabelStyles,
    /// The width and height of the marker beneath each label whose class's first style varies by
    /// feature, by the label's index, ascending; the marker of any other label is its
    /// definition's.
    markers: Vec<(u32, (f64, f64))>,
    /// Whether a label of the map has a `MINDISTANCE`, under which the cache compares texts.
    compares_texts: bool,
    /// The labels already reported, with the diagnostic's code.
    reported: BTreeSet<(LabelAt, &'static str)>,
    /// The labels left out past [`MAX_LABELS`], and the first of them.
    over_limit: (usize, Option<LabelAt>),
    /// The image the labels are drawn in, its width and height.
    size: (u32, u32),
    /// The hatches of labelpoly styles refused (Q-9), by the index of their layer.
    hatches_refused: BTreeMap<usize, usize>,
}

/// A feature the layer draws, the class that draws it, and the class's first style bound to it,
/// whose marker a label of a point moves away from.
pub(crate) type Labelled<'a> = (&'a Feature, usize, Option<&'a Style>);

impl<'r, L: ResourceLoader> Labels<'r, L> {
    pub(crate) fn new(map: &'r Map, view: &'r View, files: &'r L, size: (u32, u32)) -> Self {
        Self {
            map,
            view,
            files,
            fonts: Fonts::default(),
            glyphs: Vec::new(),
            caches: BTreeMap::new(),
            paths: Vec::new(),
            defs: Vec::new(),
            def_index: BTreeMap::new(),
            cached: Vec::new(),
            label_texts: Vec::new(),
            styles: LabelStyles::default(),
            markers: Vec::new(),
            compares_texts: map
                .layers
                .iter()
                .flat_map(|layer| &layer.classes)
                .flat_map(|class| &class.labels)
                .any(|label| label.min_distance.is_some_and(|d| d > 0.0)),
            reported: BTreeSet::new(),
            over_limit: (0, None),
            size,
            hatches_refused: BTreeMap::new(),
        }
    }

    /// Settles the labels of layer `index`, drawn in `mode` with its labels cached, for the
    /// classes of `drawn` before its features, each label not drawn reported once: a label of a
    /// layer other than a point layer is not drawn yet.
    pub(crate) fn layer(
        &mut self,
        index: usize,
        mode: RenderMode,
        drawn: &[Labelled<'_>],
        binder: &mut Binder<'_, '_>,
    ) -> LayerLabels<'r> {
        let several = "its class has several labels, which MapServer places as a group";
        self.settle_layer(index, mode, drawn, binder, several)
    }

    /// Settles the labels of point layer `index` of `LABELCACHE OFF` as [`Self::layer`] does,
    /// to be drawn at once, into the layer's image where it has one of its own.
    pub(crate) fn layer_at_once(
        &mut self,
        index: usize,
        drawn: &[Labelled<'_>],
        binder: &mut Binder<'_, '_>,
    ) -> AtOnce<'r> {
        let several = "its class has several labels, which MapServer draws each on its own under \
                       LABELCACHE OFF, and which are not drawn yet";
        let labels = self.settle_layer(index, RenderMode::Point, drawn, binder, several);
        let (width, height) = self.size;
        let mut styled = Styled::new(self.map, self.view, width, height);
        if crate::layer_opacity(&self.map.layers[index]).is_some() {
            styled.in_own_image();
        }
        AtOnce { labels, styled }
    }

    /// Settles the labels of layer `index` as [`Self::layer`] says, a label of a class with
    /// several of them not drawn for the reason `several`.
    fn settle_layer(
        &mut self,
        index: usize,
        mode: RenderMode,
        drawn: &[Labelled<'_>],
        binder: &mut Binder<'_, '_>,
        several: &str,
    ) -> LayerLabels<'r> {
        let map = self.map;
        let layer = &map.layers[index];
        let scale = classes::scale_denom(map, self.view, self.size.0);
        let factor = classes::layer_scale_factor(map, layer, self.view.cellsize, scale);
        let mut fixed_labels = BTreeMap::new();
        // The classes that draw a feature with a TEXT of its own.
        let with_text: BTreeSet<usize> = drawn
            .iter()
            .filter(|(feature, _, _)| feature_text(layer, feature.id).is_some())
            .map(|&(_, class_index, _)| class_index)
            .collect();
        for &(_, class_index, _) in drawn {
            let class = &layer.classes[class_index];
            for (n, label) in class.labels.iter().enumerate() {
                if fixed_labels.contains_key(&(class_index, n)) {
                    continue;
                }
                // msShapeGetLabelAnnotation takes the label's TEXT, else its class's, else the
                // LABELITEM's value, which the frontend has folded into the label's text, and
                // last the feature's own TEXT, which only an inline feature has. Without any, and
                // without a labelpnt style to draw in its place, MapServer skips the label
                // (maplabel.c:267-277).
                let text = label.text.as_ref();
                if text.is_none()
                    && !with_text.contains(&class_index)
                    && !has_label_point(&label.styles)
                {
                    fixed_labels.insert((class_index, n), None);
                    continue;
                }
                let fixed = if mode != RenderMode::Point {
                    Err("labels of line and polygon layers are not drawn yet".to_owned())
                } else if class.labels.len() > 1 {
                    Err(several.to_owned())
                } else {
                    fixed(map, label, text, factor)
                };
                let fixed = fixed
                    .map_err(|why| {
                        let at = (index, class_index, n);
                        self.report(binder.diagnostics, at, "label-not-drawn", why);
                    })
                    .ok();
                fixed_labels.insert((class_index, n), fixed);
            }
        }
        LayerLabels {
            index,
            fixed: fixed_labels,
        }
    }

    /// Caches the labels of `layer` for each of `drawn`, in the order the layer draws them, as
    /// msDrawShape adds each feature's labels to the cache: at each of its points inside the
    /// map's extent, as `pointLayerDrawShape` places them. A pixmap that a class's first style
    /// draws is sized by its image among `pixmaps`, loaded before the layer's labels are
    /// collected.
    pub(crate) async fn collect(
        &mut self,
        layer: &LayerLabels<'r>,
        drawn: &[Labelled<'_>],
        pixmaps: &Pixmaps,
        binder: &mut Binder<'_, '_>,
    ) {
        for &(feature, class, style) in drawn {
            let mut laid = Vec::new();
            for n in 0..self.map.layers[layer.index].classes[class].labels.len() {
                let Some(label) = self.matched(layer, feature, class, n, binder) else {
                    continue;
                };
                let fixed = label.fixed;
                let one = self.lay_out_cached(label, feature, style, pixmaps, binder);
                laid.extend(one.await.map(|one| (one, fixed)));
            }
            for (one, fixed) in laid {
                self.cache_label(feature, one, fixed);
            }
        }
    }

    /// The labels of `feature`, which class `class` of `layer` draws, laid out to be drawn at
    /// once, each as [`Self::matched`] gives it.
    pub(crate) async fn label_at_once(
        &mut self,
        layer: &AtOnce<'r>,
        feature: &Feature,
        class: usize,
        pixmaps: &Pixmaps,
        binder: &mut Binder<'_, '_>,
    ) -> Vec<Laid> {
        let mut laid = Vec::new();
        for n in 0..self.map.layers[layer.labels.index].classes[class]
            .labels
            .len()
        {
            let Some(label) = self.matched(&layer.labels, feature, class, n, binder) else {
                continue;
            };
            laid.extend(self.lay_out_at_once(label, feature, pixmaps, binder).await);
        }
        laid
    }

    /// Label `n` of class `class` of `layer` for `feature`, where it is settled and its
    /// `EXPRESSION` matches the feature, as `msGetLabelStatus` decides, its styles bound to the
    /// feature by `bindLabel`.
    fn matched<'a>(
        &self,
        layer: &'a LayerLabels<'r>,
        feature: &Feature,
        class: usize,
        n: usize,
        binder: &mut Binder<'_, '_>,
    ) -> Option<Matched<'a, 'r>> {
        let map = self.map;
        let Some(Some(fixed)) = layer.fixed.get(&(class, n)) else {
            return None;
        };
        if !binder.label_matches(feature, class, n) {
            return None;
        }
        // bindLabel binds the label's styles to the feature with its properties. Styles that vary
        // by no feature are the same for all, and are read from the map where the label is
        // drawn; only those of a label whose styles vary are held.
        let written = &map.layers[layer.index].classes[class].labels[n].styles;
        let styles: Box<[Style<LabelTransform>]> = if written.iter().any(classes::varies) {
            written
                .iter()
                .map(|s| {
                    binder
                        .bind_label_style(s, feature, &map.symbols, (class, n))
                        .into_owned()
                })
                .collect()
        } else {
            Box::default()
        };
        Some(Matched {
            at: (layer.index, class, n),
            fixed,
            styles,
        })
    }

    /// Lays out `label` of `feature` for the cache, with the first style of its class, `style`,
    /// sized among `pixmaps`, the marker the label moves away from. None where it is not drawn.
    async fn lay_out_cached(
        &mut self,
        label: Matched<'_, '_>,
        feature: &Feature,
        style: Option<&Style>,
        pixmaps: &Pixmaps,
        binder: &mut Binder<'_, '_>,
    ) -> Option<Laid> {
        let at = label.at;
        let (text, lines) = self.text_lines(at, label.fixed, feature, binder)?;
        // A text that is empty is none to msShapeGetLabelAnnotation (maputil.c:901-904, 929-932,
        // 973-976): msAddLabelGroup skips its label but for a labelpnt style (maplabel.c:267-277),
        // which the cache tests and draws alone. One of line breaks only and without styles is
        // cached, its class's marker with it, but takes no box.
        let written = &self.map.layers[at.0].classes[at.1].labels[at.2].styles;
        if text.is_empty() && !has_label_point(written) {
            return None;
        }
        // msDrawLabelCache draws a labelpoly or labelcenter style only with an annotext; without
        // one it fails the map with "Labels only support LABELPNT, LABELPOLY and LABELCENTER
        // GEOMTRANSFORMS" (mapdraw.c:4056-4133, 540, 593).
        if text.is_empty() && written.iter().any(boxed) {
            let why = "its text is empty, where msDrawLabelCache fails the map on a labelpoly or \
                       labelcenter style"
                .to_owned();
            self.report(binder.diagnostics, at, "label-not-drawn", why);
            return None;
        }
        let bound = bind(label.fixed, &feature.properties, |expr| {
            binder.label_number(expr, feature, at.1, at.2)
        });
        // msDrawLabelCache tests a labelpnt marker before the text and draws a member's labelpnt
        // styles whatever its text (mapdraw.c:3607-3646, 4056-4065), but a text of line breaks is
        // no NULL annotext: its path is laid out (3651, 3688-3693) on a box msLayoutTextSymbol
        // leaves at FLT_MAX and -FLT_MAX (textlayout.c:817-819), which get_metrics places
        // (maplabel.c:1019-1123) and msTestLabelCacheCollisions tests (mapdraw.c:3789, 3898)
        // before any style is drawn, and on which a labelpoly or labelcenter style is drawn.
        // Through the cache, that box is not ported.
        let blank = lines.iter().all(Vec::is_empty);
        if blank && !text.is_empty() && !written.is_empty() {
            let why = "its text is of line breaks only, whose box the label cache tests and \
                       draws its styles by is not ported"
                .to_owned();
            self.report(binder.diagnostics, at, "label-not-drawn", why);
            return None;
        }
        let annotation = Annotation { text, lines, bound };
        self.lay_out(label, annotation, style, pixmaps, feature, binder)
            .await
    }

    /// Lays out `label` of `feature` to be drawn at once, a text that is empty or of line breaks
    /// only drawing its styles. None where it is not drawn.
    async fn lay_out_at_once(
        &mut self,
        label: Matched<'_, '_>,
        feature: &Feature,
        pixmaps: &Pixmaps,
        binder: &mut Binder<'_, '_>,
    ) -> Option<Laid> {
        let at = label.at;
        let (text, lines) = self.text_lines(at, label.fixed, feature, binder)?;
        let bound = bind(label.fixed, &feature.properties, |expr| {
            binder.label_number(expr, feature, at.1, at.2)
        });
        let annotation = Annotation { text, lines, bound };
        // msDrawLabel moves a label off no marker: the class's style is not read for it.
        let laid = self.lay_out(label, annotation, None, pixmaps, feature, binder);
        let mut laid = laid.await?;
        let written = &self.map.layers[at.0].classes[at.1].labels[at.2].styles;
        if laid.path.is_none() && written.iter().any(boxed) {
            // msDrawLabel fails on a labelpoly or labelcenter style where the text is empty, and
            // draws it on a box of no size where it is of line breaks only.
            let why = "its text is empty or of line breaks only, where msDrawLabel fails on a \
                       labelpoly or labelcenter style or draws it on a box of no size"
                .to_owned();
            self.report(binder.diagnostics, at, "label-not-drawn", why);
            return None;
        }
        // A label drawn at once is tested against no marker: its labelpnt marker is computed
        // only for whether it is drawn, which msDrawLabel decides as the cache does.
        laid.label_marker = None;
        Some(laid)
    }

    /// Lays out `label` of `feature`, its text, lines, and bound properties `annotation`, moved
    /// away from the marker of `style`, sized among `pixmaps`, where it has one. None where it is
    /// not drawn.
    async fn lay_out(
        &mut self,
        label: Matched<'_, '_>,
        annotation: Annotation,
        style: Option<&Style>,
        pixmaps: &Pixmaps,
        feature: &Feature,
        binder: &mut Binder<'_, '_>,
    ) -> Option<Laid> {
        let Matched { at, fixed, styles } = label;
        let Annotation { text, lines, bound } = annotation;
        let written = &self.map.layers[at.0].classes[at.1].labels[at.2].styles;
        let blank = lines.iter().all(Vec::is_empty);
        let label_styles: &[Style<LabelTransform>] =
            if styles.is_empty() { written } else { &styles };
        let settled =
            settle(fixed, &bound, lines.len(), style, pixmaps, self.map).and_then(|settled| {
                let marker = label_styles::label_marker(label_styles, self.map, fixed.factor);
                Ok((settled, marker.map_err(|why| ("label-not-drawn", why))?))
            });
        let ((position, align, marker), label_marker) = match settled {
            Ok(settled) => settled,
            Err((code, why)) => {
                self.report(binder.diagnostics, at, code, why);
                return None;
            }
        };
        let glyph_size = glyph_size(
            bound.size,
            fixed.factor,
            fixed.min_size,
            fixed.max_size,
            fixed.resolution,
        );
        let path = if blank {
            None
        } else {
            match self.layout(fixed, lines, glyph_size, align).await {
                Ok(path) => Some(path),
                Err(unlaid) => {
                    let (code, outcome) = unlaid.code();
                    let why = || unlaid.reason(&text);
                    self.report_with(binder.diagnostics, at, code, why, outcome);
                    return None;
                }
            }
        };
        // msDrawTextSymbol's outline width, `MS_NINT(outlinewidth * (glyph_size / size))`, and
        // AGG's contour one pixel wider.
        let outline = bound.outline_color.map(|color| {
            let ow = ms_nint(fixed.outline_width * (glyph_size / bound.size));
            (color, ow.wrapping_add(1))
        });
        Some(Laid {
            at,
            text,
            path,
            ink: Ink {
                feature_id: feature.id,
                rotation: bound.angle * DEG_TO_RAD,
                color: bound.color,
                outline,
            },
            // The offset scaled into an int.
            offset: bound.offset.map(|o| (f64::from(o) * fixed.factor) as i32),
            position,
            priority: bound.priority,
            test_point: !fixed.force && bound.offset == [0, 0],
            styles,
            marker,
            label_marker,
        })
    }

    /// Draws `label`, laid out for a feature of `layer` to be drawn at once, at its point
    /// `point`, as `pointLayerDrawShape` draws the labels of a layer of `LABELCACHE OFF` with
    /// `msDrawLabel` (src/mapdraw.c:2086-2105, 2703-2897) after the markers of that point and
    /// before those of the next: by its position and its `OFFSET` alone, `get_metrics` given no
    /// marker's offset and no buffer, its styles before its text, and with no test against the
    /// other labels, the markers, or the image's edge. A `POSITION AUTO` stands as `get_metrics`
    /// places a position none of its cases names. The label is not cached, so the label cache
    /// never tests a label against it. Without a path, for a text that is empty or of line
    /// breaks only, its labelpnt styles are drawn at the point and nothing else
    /// (mapdraw.c:2714-2748). The count of glyphs left out for reaching too far from the image.
    pub(crate) fn draw_at_once<R: Draw>(
        &mut self,
        layer: &mut AtOnce<'r>,
        renderer: &mut R,
        label: &Laid,
        point: Point,
    ) -> usize {
        let styled = &mut layer.styled;
        // The text's box, which a label without one draws no style on.
        let text = label.path.as_ref().map_or(
            TextBox {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
                num_lines: 1,
            },
            TextPath::text_box,
        );
        let anchor = match label.position {
            Position::At(anchor) => anchor,
            Position::Auto => Anchor::Other,
        };
        let point_label = PointLabel {
            position: Position::At(anchor),
            text,
            offsets: Offsets {
                marker: [0.0, 0.0],
                label: label.offset,
            },
            rotation: label.ink.rotation,
            buffer: 0,
            force: true,
        };
        let (origin, bounds) = placed_at(point, &point_label, anchor);
        let member = Member {
            priority: label.priority,
            point,
            label: Some(point_label),
            force: true,
            partials: true,
            test_point: false,
            marker: None,
            label_marker: None,
            min_distance: -1.0,
        };
        let placement = Placement {
            origin,
            bounds,
            anchor,
        };
        let placed = Placed {
            at: label.at,
            member: &member,
            placement: &placement,
            styles: &StylesOf::Bound(&label.styles),
        };
        let out_of_reach = draw_label(
            renderer,
            styled,
            &placed,
            label.ink,
            label.path.as_ref(),
            &self.glyphs,
            self.image(),
        );
        self.hatches_refused(label.at, styled.take_hatches_refused());
        out_of_reach
    }

    /// The image the labels are drawn in, from its corner to its far one.
    fn image(&self) -> [f64; 4] {
        let (width, height) = self.size;
        [0.0, 0.0, f64::from(width), f64::from(height)]
    }

    /// Counts `count` hatches of labelpoly styles of `at` as refused (Q-9).
    fn hatches_refused(&mut self, (layer, _, _): LabelAt, count: usize) {
        if count > 0 {
            *self.hatches_refused.entry(layer).or_default() += count;
        }
    }

    /// Takes how many hatches of the labelpoly styles of layer `layer` were refused (Q-9), for
    /// the layer's report.
    pub(crate) fn take_hatches_refused(&mut self, layer: usize) -> usize {
        self.hatches_refused.remove(&layer).unwrap_or(0)
    }

    /// The text of label `at` for `feature` and its lines, or `None` where it has none or it is
    /// not drawn, which is reported.
    fn text_lines(
        &mut self,
        at: LabelAt,
        fixed: &Fixed<'_>,
        feature: &Feature,
        binder: &mut Binder<'_, '_>,
    ) -> Option<(String, Vec<Vec<char>>)> {
        let diagnostics = &mut *binder.diagnostics;
        let text = match fixed.text {
            Some(expr) if !is_text(expr) => {
                let why = "its text is an expression, which is not evaluated yet".to_owned();
                self.report(diagnostics, at, "label-text-unsupported", why);
                return None;
            }
            Some(expr) => {
                let (_, class, label) = at;
                let bytes = binder.label_text(expr, feature, class, label)?;
                String::from_utf8_lossy(&bytes).into_owned()
            }
            // A label of no text at all is kept for its labelpnt style.
            None => feature_text(&self.map.layers[at.0], feature.id)
                .unwrap_or_default()
                .to_owned(),
        };
        match lines(&text, fixed.wrap, fixed.max_length) {
            Ok(lines) => Some((text, lines)),
            Err(LineError::NotLatin { line, character }) => {
                let why = || {
                    format!(
                        "line {} of the text {} holds U+{:04X}, which needs a shaper",
                        line + 1,
                        quoted(&text),
                        u32::from(character)
                    )
                };
                let code = "label-line-unsupported";
                let outcome = "the label is not drawn";
                self.report_with(binder.diagnostics, at, code, why, outcome);
                None
            }
        }
    }

    /// Holds what a feature's label cached as `template` draws with: its text, its path, and its
    /// styles and their marker, those of a label whose styles vary by feature for the feature,
    /// else its definition's.
    fn hold(
        &mut self,
        template: &Cached,
        text: String,
        written: &[Style<LabelTransform>],
        styles: &[Style<LabelTransform>],
        label_marker: Option<LabelMarker>,
        path: Option<TextPath>,
    ) {
        push(&mut self.label_texts, text.into_boxed_str());
        self.styles
            .hold(template.def, template.text, written, styles, label_marker);
        if let Some(path) = path {
            push(&mut self.paths, path);
        }
    }

    /// Caches `laid`, a label of `feature` settled as `fixed`, at each of the feature's points
    /// inside the map's extent, and holds what it draws with where any was cached.
    fn cache_label(&mut self, feature: &Feature, laid: Laid, fixed: &Fixed<'_>) {
        let Laid {
            at,
            text,
            path,
            ink,
            offset,
            position,
            priority,
            test_point,
            styles,
            marker,
            label_marker,
        } = laid;
        let template = Cached {
            point: Point { x: 0.0, y: 0.0 },
            rotation: ink.rotation,
            feature_id: ink.feature_id,
            path: if path.is_some() {
                self.paths.len() as u32
            } else {
                NO_PATH
            },
            def: self.def(at, fixed),
            text: self.label_texts.len() as u32,
            offset,
            color: ink.color,
            outline: ink.outline,
            position,
            priority,
            test_point,
        };
        if self.cache(feature, &template, marker) {
            let written = &self.map.layers[at.0].classes[at.1].labels[at.2].styles;
            self.hold(&template, text, written, &styles, label_marker, path);
        }
    }

    /// Caches `template` at each point of `feature` inside the map's extent, as
    /// `pointLayerDrawShape` adds a label at each, with the marker of `marker`'s size beneath,
    /// held by the label's definition where the class's first style varies by no feature;
    /// whether any was cached.
    fn cache(&mut self, feature: &Feature, template: &Cached, marker: Option<(f64, f64)>) -> bool {
        let (layer, class, _) = self.defs[template.def as usize].at;
        let per_label = self.map.layers[layer].classes[class]
            .styles
            .first()
            .is_some_and(classes::varies);
        if !per_label {
            self.defs[template.def as usize].marker = marker;
        }
        let mut cached = false;
        for point in crate::points_in_view(self.view, &feature.geometry) {
            // The label cache holds at most MAX_LABELS labels a render (Q-9, ADR-0032).
            if self.cached.len() >= MAX_LABELS {
                let first = self
                    .over_limit
                    .1
                    .unwrap_or(self.defs[template.def as usize].at);
                self.over_limit = (self.over_limit.0 + 1, Some(first));
                continue;
            }
            let index = self.cached.len() as u32;
            if let Some(size) = marker.filter(|_| per_label) {
                push(&mut self.markers, (index, size));
            }
            push(&mut self.cached, Cached { point, ..*template });
            cached = true;
        }
        cached
    }

    /// The index of the definition of label `at`, added on its first feature.
    fn def(&mut self, at: LabelAt, fixed: &Fixed<'_>) -> u32 {
        let defs = &mut self.defs;
        *self.def_index.entry(at).or_insert_with(|| {
            defs.push(Def {
                at,
                // The buffer scaled into an int.
                buffer: (fixed.buffer * fixed.factor) as i32,
                force: fixed.force,
                partials: fixed.partials,
                min_distance: fixed.min_distance * fixed.resolution,
                outline_grow: fixed.outline_width / 2.0 * fixed.factor,
                factor: fixed.factor,
                marker: None,
            });
            (defs.len() - 1) as u32
        })
    }

    /// Lays out `lines` at `glyph_size`, each in the font of the label's list that draws it, or
    /// says why it cannot. Each line is let go once its glyphs are laid out, so that a long text
    /// is not held as characters and as glyphs at once.
    async fn layout(
        &mut self,
        fixed: &Fixed<'_>,
        lines: Vec<Vec<char>>,
        glyph_size: f64,
        align: Align,
    ) -> Result<TextPath, Unlaid> {
        let mut layout = Layout::new(glyph_size, lines.len());
        for line in lines {
            if line.is_empty() {
                layout.empty_line();
                continue;
            }
            // get_face_for_run chooses a font for each line.
            let fonts = fixed.fonts.ok_or(Unlaid::NoFont)?;
            let face = self
                .fonts
                .face(self.map, fonts, &line, self.files)
                .await
                .map_err(Unlaid::Font)?;
            let key = (face.path, pixel_size(glyph_size));
            let cache = *self.caches.entry(key).or_insert_with(|| {
                self.glyphs.push(Glyphs::new(glyph_size));
                self.glyphs.len() - 1
            });
            Font::new(&face.bytes)
                .map_err(|_| 0)
                .and_then(|font| {
                    layout.line(&font, &mut self.glyphs[cache], cache as u32, face.glyphs)
                })
                .map_err(Unlaid::Glyph)?;
        }
        Ok(layout.finish(align))
    }

    /// Draws the labels the label cache places, in the order it places them, each label's path
    /// built just before it is drawn. A glyph left out for reaching too far from the image is
    /// reported once for its label, and the labels left out past [`MAX_LABELS`] once. The cache is
    /// emptied; the fonts and glyphs stay for the labels of the layers of `POSTLABELCACHE TRUE`,
    /// drawn at once after it.
    pub(crate) fn finish<R: Draw>(
        &mut self,
        renderer: &mut R,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<CachedLabel> {
        if let (left_out, Some(first)) = self.over_limit {
            let message = format!(
                "the label cache of a render holds at most {MAX_LABELS} labels; {left_out} \
                 labels past it, this one the first, are left out untested"
            );
            let span = self.span(first);
            diagnostics.push(Diagnostic::warning("labels-over-limit", message, span));
        }
        let cached = std::mem::take(&mut self.cached);
        let markers = std::mem::take(&mut self.markers);
        let styles = std::mem::take(&mut self.styles);
        let texts = std::mem::take(&mut self.label_texts);
        let defs = std::mem::take(&mut self.defs);
        let paths = std::mem::take(&mut self.paths);
        let glyphs = std::mem::take(&mut self.glyphs);
        let cache = Cache {
            cached: &cached,
            paths: &paths,
            defs: &defs,
            markers: &markers,
            styles: &styles,
            texts: self.compares_texts.then_some(&texts[..]),
            map: self.map,
        };
        let (width, height) = self.size;
        let image = [0.0, 0.0, f64::from(width), f64::from(height)];
        let mut styled = Styled::new(self.map, self.view, width, height);
        // The report follows the cache's walk, every member it visits with whether it was drawn.
        let mut walked: Vec<(u32, Option<[f64; 4]>)> = Vec::with_capacity(cached.len());
        let gutter = self.map.label_cache_edge_buffer.unwrap_or(0);
        let left_out = walk_label_cache(&cache, self.size, gutter, |i, placement| {
            let label = &cached[i];
            let at = defs[label.def as usize].at;
            let Some(placement) = placement else {
                walked.push((i as u32, None));
                return;
            };
            let member = cache.member(i);
            // The member's box, of its text and its labelpoint marker (mapdraw.c:3976-4030).
            let drawn = member_box(&placement.bounds.bbox, member.label_marker.as_ref());
            walked.push((i as u32, Some(drawn)));
            let path = (label.path != NO_PATH).then(|| &paths[label.path as usize]);
            let placed = Placed {
                at,
                member: &member,
                placement,
                styles: &StylesOf::Held {
                    styles: &styles,
                    text: label.text,
                },
            };
            let out_of_reach = draw_label(
                renderer,
                &mut styled,
                &placed,
                Ink::of(label),
                path,
                &glyphs,
                image,
            );
            self.out_of_reach(diagnostics, at, out_of_reach);
            self.hatches_refused(at, styled.take_hatches_refused());
        });
        // The engine holds no more than MAX_LABELS while it caches, which bounds its memory (Q-9),
        // so the cache leaves none out of what it is handed.
        debug_assert_eq!(left_out, 0, "the engine holds at most MAX_LABELS labels");
        // The hatches refused of the labels the cache drew, reported for their layers as a
        // layer's own (Q-9).
        for (layer, refused) in std::mem::take(&mut self.hatches_refused) {
            diagnostics.extend(crate::refused_hatches(&self.map.layers[layer], refused));
        }
        // The labels of the report share their feature's text.
        let texts: Vec<Arc<str>> = texts.into_iter().map(Arc::from).collect();
        let report = walked
            .into_iter()
            .map(|(i, drawn)| {
                let label = &cached[i as usize];
                let (layer, class, _) = defs[label.def as usize].at;
                CachedLabel {
                    priority: label.priority,
                    layer,
                    class,
                    text: Arc::clone(&texts[label.text as usize]),
                    drawn,
                }
            })
            .collect();
        self.glyphs = glyphs;
        report
    }

    /// Reports the glyphs of `at` left out for reaching too far from the image, once per label.
    pub(crate) fn out_of_reach(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        at: LabelAt,
        count: usize,
    ) {
        if count > 0 {
            let why = format!(
                "{count} of its glyphs reach farther than {} pixels from the image",
                forager_render::clip::MAX_GLYPH_REACH
            );
            let outcome = "those glyphs are left out";
            self.report_with(diagnostics, at, "label-glyph-out-of-reach", || why, outcome);
        }
    }

    /// Reports why `at` is not drawn, once per label and code.
    fn report(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        at: LabelAt,
        code: &'static str,
        why: String,
    ) {
        self.report_with(diagnostics, at, code, || why, "the label is not drawn");
    }

    /// Reports `why` with what follows from it, once per label and code; `why` is built only
    /// where it is reported, so that a label refused for each of its features costs no message
    /// for each.
    fn report_with(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        at: LabelAt,
        code: &'static str,
        why: impl FnOnce() -> String,
        outcome: &str,
    ) {
        if self.reported.insert((at, code)) {
            let span = self.span(at);
            let message = format!("{}; {outcome}", why());
            diagnostics.push(Diagnostic::warning(code, message, span));
        }
    }

    fn span(&self, (layer, class, label): LabelAt) -> Option<Span> {
        self.map.layers[layer].classes[class].labels[label]
            .span
            .clone()
    }
}

/// Draws `placed` into `renderer`: its styles, then its text of `path` in the colour and outline
/// of `label`, as msDrawLabelCache and msDrawLabel draw them, each glyph that reaches into `image`
/// or near it; the count of glyphs left out for reaching too far from it.
fn draw_label<R: Draw>(
    renderer: &mut R,
    styled: &mut Styled<'_>,
    placed: &Placed<'_>,
    label: Ink,
    path: Option<&TextPath>,
    glyphs: &[Glyphs],
    image: [f64; 4],
) -> usize {
    // The label's styles, before its text.
    styled.draw(renderer, placed, label.feature_id);
    let placement = placed.placement;
    let Some(path) = path else {
        return 0;
    };
    if label.color.is_none() && label.outline.is_none() {
        return 0;
    }
    // An outline reaches past its glyphs, so a glyph that close outside the image is drawn for it.
    let width = label.outline.map_or(0.0, |(_, width)| f64::from(width));
    let drawn = path.draw_outlined(glyphs, placement.origin, label.rotation, image, width);
    if !drawn.path.is_empty() {
        let glyphs = DrawOp::Glyphs {
            path: drawn.path,
            color: label.color,
            outline: label.outline.map(|(color, width)| GlyphOutline {
                color,
                width: f64::from(width),
            }),
        };
        crate::draw_items(renderer, label.feature_id, [glyphs]);
    }
    drawn.out_of_reach
}

/// The labels of a render as the label cache reads them.
struct Cache<'c> {
    cached: &'c [Cached],
    paths: &'c [TextPath],
    defs: &'c [Def],
    markers: &'c [(u32, (f64, f64))],
    styles: &'c LabelStyles,
    /// The texts of the labels, where the cache compares them.
    texts: Option<&'c [Box<str>]>,
    map: &'c Map,
}

impl Members for Cache<'_> {
    fn count(&self) -> usize {
        self.cached.len()
    }

    fn priority(&self, index: usize) -> u8 {
        self.cached[index].priority
    }

    fn member(&self, index: usize) -> Member {
        let label = &self.cached[index];
        let def = &self.defs[label.def as usize];
        // msAddLabelGroup caches the marker of the class's first style: its box about the
        // label's point, whose far corner lies `w - 1` and `h - 1` from its near one.
        let marker = self
            .markers
            .binary_search_by_key(&(index as u32), |(i, _)| *i)
            .ok()
            .map(|i| self.markers[i].1)
            .or(def.marker)
            .map(|(w, h)| {
                let (min_x, min_y) = (label.point.x - 0.5 * w, label.point.y - 0.5 * h);
                [min_x, min_y, min_x + (w - 1.0), min_y + (h - 1.0)]
            });
        // computeMarkerBounds at the label's point.
        let (layer, class, n) = def.at;
        let written = &self.map.layers[layer].classes[class].labels[n].styles;
        let label_marker = self.styles.marker_bounds(
            self.map,
            label.def,
            label.text,
            written,
            label.point,
            def.factor,
        );
        // msDrawLabelCache's offset from the marker: half the size of its box, or of its
        // labelpoint marker's where that is larger, grown by half the label's outline where it
        // has one.
        let half =
            |[min_x, min_y, max_x, max_y]: [f64; 4]| [(max_x - min_x) / 2.0, (max_y - min_y) / 2.0];
        let mut from_marker = marker.map_or([0.0, 0.0], half);
        if let Some(m) = &label_marker {
            let [x, y] = half(m.bbox);
            // MS_MAX.
            from_marker = [
                if from_marker[0] > x {
                    from_marker[0]
                } else {
                    x
                },
                if from_marker[1] > y {
                    from_marker[1]
                } else {
                    y
                },
            ];
        }
        if from_marker[0] != 0.0 && label.outline.is_some() {
            from_marker = from_marker.map(|o| o + def.outline_grow);
        }
        Member {
            priority: label.priority,
            point: label.point,
            label: (label.path != NO_PATH).then(|| PointLabel {
                position: label.position,
                text: self.paths[label.path as usize].text_box(),
                offsets: Offsets {
                    marker: from_marker,
                    label: label.offset,
                },
                rotation: label.rotation,
                buffer: def.buffer,
                force: def.force,
            }),
            force: def.force,
            partials: def.partials,
            test_point: label.test_point,
            marker,
            label_marker,
            min_distance: def.min_distance,
        }
    }

    fn same_text(&self, a: usize, b: usize) -> bool {
        // strcmp compares the texts up to their first NUL.
        self.texts.is_some_and(|texts| {
            let text = |i: usize| c_bytes(&texts[self.cached[i].text as usize]);
            text(a) == text(b)
        })
    }
}

/// A label's position, its alignment, and the width and height of the marker it moves away from.
type Settled = (Position, Align, Option<(f64, f64)>);

/// Where a label stands for one feature: its position, its alignment over `num_lines` lines, and
/// the size of the marker it moves away from, that of `style`, a pixmap's by its image among
/// `pixmaps`; or the diagnostic's code and why it is not drawn. A pixmap whose image does not
/// load fails msAddLabelGroup and the map in MapServer (`src/maplabel.c:350-352`); its label is
/// left out here, and the rest of the map drawn.
fn settle(
    fixed: &Fixed<'_>,
    bound: &BoundLabel,
    num_lines: usize,
    style: Option<&Style>,
    pixmaps: &Pixmaps,
    map: &Map,
) -> Result<Settled, (&'static str, String)> {
    let Some(position) = bound.position else {
        let why = "its bound POSITION names no position, where MapServer keeps the one an \
                   earlier feature or the mapfile left, which is not drawn yet";
        return Err(("label-binding-unsettled", why.to_owned()));
    };
    let align = match bound.align {
        Some(align) => align,
        // One line stands alike at any alignment.
        None if num_lines == 1 => Align::Left,
        None => {
            let why = "its bound ALIGN names no alignment, where MapServer keeps the one an \
                       earlier feature or the mapfile left, which is not drawn yet";
            return Err(("label-binding-unsettled", why.to_owned()));
        }
    };
    // The marker the label cache holds beneath the label, which a label away from its point
    // moves away from.
    let marker = match style {
        None => None,
        Some(style) => match marker_size_of(style, &map.symbols, pixmaps, fixed.factor) {
            Ok(size) => Some(size),
            Err(what) => {
                let why =
                    format!("its class's first style draws {what}, which the label cache holds");
                return Err(("label-not-drawn", why));
            }
        },
    };
    Ok((position, align, marker))
}

/// Whether `styles` hold a `GEOMTRANSFORM labelpnt` style, for which msAddLabelGroup keeps a
/// label without text (maplabel.c:267-277).
fn has_label_point(styles: &[Style<LabelTransform>]) -> bool {
    styles
        .iter()
        .any(|s| s.geom_transform == LabelTransform::Point)
}

/// Whether `style` is drawn on the box of a label's text: `GEOMTRANSFORM labelpoly` or
/// `labelcenter`.
fn boxed(style: &Style<LabelTransform>) -> bool {
    matches!(
        style.geom_transform,
        LabelTransform::Poly | LabelTransform::Center
    )
}

/// Whether a label's text is of the forms Forager evaluates for a label so far: a string, a
/// property as text, and their concatenation. Any other is left out as not evaluated yet; this
/// decides the form alone, and the binder's evaluator builds the text.
fn is_text(expr: &Expr) -> bool {
    match expr {
        Expr::String(_) => true,
        Expr::Call {
            op: Op::ToString,
            args,
        } => matches!(args.as_slice(), [Expr::Property { .. }]),
        Expr::Call {
            op: Op::Concat,
            args,
        } => args.iter().all(is_text),
        _ => false,
    }
}

/// Makes room in `list`, a list of the label cache, for `more` entries as they are held, growing
/// it by a quarter of its capacity, or by what they need where that is more. A list that
/// doubled as it grew would hold up to three times what it fills while it moved, past what Q-9
/// bounds per label; one sized ahead for the features of a layer would hold room for labels a
/// feature's `EXPRESSION`, its empty text, or its layout leaves out. Grown by half, the lists
/// hold 400 bytes per label without style just past a step, at 15,586 labels; by a quarter, 363
/// at 5,262, the most measured, and 392 at 1,109, where the render's fixed cost counts more
/// per label. Each entry moves about four times as the list grows.
pub(crate) fn make_room<T>(list: &mut Vec<T>, more: usize) {
    if list.capacity() - list.len() < more {
        list.reserve_exact(more.max(list.capacity() / 4).max(MIN_ROOM));
    }
}

/// Pushes `entry` onto `list`, a list of the label cache, grown as [`make_room`] grows it.
pub(crate) fn push<T>(list: &mut Vec<T>, entry: T) {
    make_room(list, 1);
    list.push(entry);
}

/// The fewest entries a list of the label cache grows by, so that a small one is not moved at
/// each entry.
const MIN_ROOM: usize = 8;

#[cfg(test)]
mod tests {
    use forager_ports::{Coord, Properties, ResourcePath, SourceError, Stored};
    use forager_style_ir::{Bound, Class, Extent};

    use super::*;
    use forager_style_ir::Layer;

    struct NoFiles;

    impl ResourceLoader for NoFiles {
        async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
            Err(SourceError::new("no files"))
        }
    }

    /// Whether `list` holds no more room than a quarter of what it fills, and the fewest entries
    /// a list grows by, as [`make_room`] grows it.
    fn grown_by_a_quarter<T>(list: &[T], capacity: usize) -> bool {
        capacity <= list.len() + list.len() / 4 + MIN_ROOM
    }

    #[test]
    fn the_texts_and_markers_of_the_labels_cached_grow_by_a_quarter() {
        // Verifies: Q-9
        // A list that doubled would hold up to three times what it fills while it moved.
        let map = Map {
            layers: vec![Layer {
                classes: vec![Class {
                    styles: vec![Style {
                        size: Some(Bound::Binding {
                            binding: "size".to_owned(),
                        }),
                        ..Style::default()
                    }],
                    ..Class::default()
                }],
                ..Layer::default()
            }],
            ..Map::default()
        };
        let extent = Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        };
        let view = View::for_image(extent, 100, 100).unwrap();
        let mut labels = Labels::new(&map, &view, &NoFiles, (100, 100));
        labels.defs.push(Def {
            at: (0, 0, 0),
            buffer: 0,
            force: false,
            partials: false,
            min_distance: 0.0,
            outline_grow: 0.0,
            factor: 1.0,
            marker: None,
        });
        let template = Cached {
            point: Point { x: 0.0, y: 0.0 },
            rotation: 0.0,
            feature_id: 0,
            path: NO_PATH,
            def: 0,
            text: 0,
            offset: [0, 0],
            color: None,
            outline: None,
            position: Position::Auto,
            priority: 1,
            test_point: true,
        };
        let feature = Feature {
            id: 0,
            geometry: forager_ports::Geometry::Point(Coord { x: 50.0, y: 50.0 }),
            properties: Properties::list(vec![("size".into(), Stored::Number("4".into()))]),
        };
        for text in 0..5_000u32 {
            let template = Cached { text, ..template };
            assert!(labels.cache(&feature, &template, Some((4.0, 4.0))));
            labels.hold(&template, format!("{text}"), &[], &[], None, None);
            let (texts, markers) = (&labels.label_texts, &labels.markers);
            assert_eq!(
                (texts.len(), markers.len()),
                (text as usize + 1, text as usize + 1)
            );
            assert!(
                grown_by_a_quarter(texts, texts.capacity()),
                "{}",
                texts.capacity()
            );
            assert!(
                grown_by_a_quarter(markers, markers.capacity()),
                "{}",
                markers.capacity()
            );
        }
    }
}
