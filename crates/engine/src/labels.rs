// The labels of point layers follow pointLayerDrawShape and msDrawLabelCache, src/mapdraw.c,
// msAddLabelGroup and msComputeTextPath, src/maplabel.c, and msDrawTextSymbol,
// src/maprendering.c, of MapServer at the release pinned in scripts/fetch-msautotest.sh, for a
// label at POSITION CC on a point, without the collisions of the label cache (ADR-0028).
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

use forager_label::{Font, Glyphs, LineError, TextPath, glyph_size, line, pixel_size};
use forager_ports::{Feature, Properties, ResourceLoader, Value};
use forager_render::{DisplayItem, DrawOp, Point, Renderer};
use forager_style_ir::{
    Bound, Diagnostic, Expr, Label, Layer, Map, Op, Position, Rgba, Span, Units,
};

use crate::draw::vertices;
use crate::extent::View;
use crate::fonts::Fonts;

/// A label's size where none is given, MapServer's `MS_MEDIUM`.
const DEFAULT_SIZE: f64 = 10.0;
/// The bounds of a label's size where none are given, `MS_MINFONTSIZE` and `MS_MAXFONTSIZE`.
const DEFAULT_MIN_SIZE: f64 = 4.0;
const DEFAULT_MAX_SIZE: f64 = 256.0;
/// A label's colour where none is given.
const DEFAULT_COLOR: Rgba = Rgba::opaque(0, 0, 0);

/// What of a label does not vary by feature, once checked to be drawable.
struct Fixed<'a> {
    text: &'a Expr,
    fonts: &'a str,
    glyph_size: f64,
    color: Rgba,
}

/// A label of the render by its layer's index, its class's, and its index in the class.
type LabelAt = (usize, usize, usize);

/// One label placed at one point, held until the labels are drawn: its text path, the glyphs it
/// is drawn from, and where and how.
struct Placed {
    path: usize,
    glyphs: usize,
    origin: Point,
    color: Rgba,
    feature_id: u64,
    label: LabelAt,
}

/// The labels of one render, collected over its layers and drawn after them, as MapServer's label
/// cache draws them, with each problem of a label reported once. A label is held as its glyphs'
/// indices and its point; the glyphs' outlines are drawn once per font and size, and a label's
/// path is built only when it is drawn.
pub(crate) struct Labels<'r, L> {
    map: &'r Map,
    view: &'r View,
    files: &'r L,
    fonts: Fonts,
    /// The glyphs of each font and pixel size, by the font's path and the size.
    glyphs: Vec<Glyphs>,
    caches: BTreeMap<(String, u16), usize>,
    paths: Vec<TextPath>,
    placed: Vec<Placed>,
    /// The labels already reported, with the diagnostic's code.
    reported: BTreeSet<(LabelAt, &'static str)>,
    /// The image the labels are drawn in, `[min x, min y, max x, max y]`.
    image: [f64; 4],
}

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
            placed: Vec::new(),
            reported: BTreeSet::new(),
            image: [0.0, 0.0, f64::from(size.0), f64::from(size.1)],
        }
    }

    /// Collects the labels of layer `index` for each of `drawn`, a feature and the class that
    /// draws it, in the order the layer draws them, as msDrawShape adds each feature's labels to
    /// the cache: on a point layer at each vertex inside the map's extent, as
    /// `pointLayerDrawShape` places them; a label of any other layer is not drawn yet.
    pub(crate) async fn collect(
        &mut self,
        index: usize,
        point_layer: bool,
        drawn: &[(&Feature, usize)],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let map = self.map;
        let layer = &map.layers[index];
        // What of each label of the classes drawn does not vary, or why it is not drawn.
        let mut fixed_labels = BTreeMap::new();
        for &(_, class_index) in drawn {
            let class = &layer.classes[class_index];
            for (n, label) in class.labels.iter().enumerate() {
                let at = (index, class_index, n);
                if fixed_labels.contains_key(&at) {
                    continue;
                }
                let Some(text) = &label.text else {
                    // No text and no style to draw in its place: MapServer skips the label.
                    fixed_labels.insert(at, None);
                    continue;
                };
                let fixed = if !point_layer {
                    Err("labels of line and polygon layers are not drawn yet".to_owned())
                } else if class.labels.len() > 1 {
                    Err(
                        "its class has several labels, which MapServer places as a group"
                            .to_owned(),
                    )
                } else {
                    fixed(map, layer, label, text)
                };
                match fixed {
                    Ok(fixed) => {
                        fixed_labels.insert(at, Some(fixed));
                    }
                    Err(why) => {
                        self.report(diagnostics, at, "label-not-drawn", why);
                        fixed_labels.insert(at, None);
                    }
                }
            }
        }
        for &(feature, class_index) in drawn {
            for n in 0..layer.classes[class_index].labels.len() {
                let at = (index, class_index, n);
                if let Some(Some(fixed)) = fixed_labels.get(&at) {
                    self.feature(at, fixed, feature, diagnostics).await;
                }
            }
        }
    }

    /// Lays out one label of one feature and places it at each of its points.
    async fn feature(
        &mut self,
        at: LabelAt,
        fixed: &Fixed<'_>,
        feature: &Feature,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let text = match text(fixed.text, &feature.properties) {
            Ok(text) => text,
            Err(why) => {
                self.report(diagnostics, at, "label-text-unsupported", why);
                return;
            }
        };
        let chars = match line(&text) {
            Ok(chars) => chars,
            Err(e) => {
                let why = match e {
                    LineError::SeveralLines => format!("the text `{text}` has several lines"),
                    LineError::NotLatin(c) => format!(
                        "the line `{text}` holds U+{:04X}, which needs a shaper",
                        u32::from(c)
                    ),
                };
                self.report(diagnostics, at, "label-line-unsupported", why);
                return;
            }
        };
        if chars.is_empty() {
            return;
        }
        let face = match self
            .fonts
            .face(self.map, fixed.fonts, &chars, self.files)
            .await
        {
            Ok(face) => face,
            Err(why) => {
                self.report(diagnostics, at, "label-font-unusable", why);
                return;
            }
        };
        let key = (face.path, pixel_size(fixed.glyph_size));
        let cache = *self.caches.entry(key).or_insert_with(|| {
            self.glyphs.push(Glyphs::new(fixed.glyph_size));
            self.glyphs.len() - 1
        });
        let path = Font::new(&face.bytes)
            .map_err(|_| 0)
            .and_then(|font| TextPath::new(&font, &mut self.glyphs[cache], &face.glyphs));
        let path = match path {
            Ok(path) => path,
            Err(glyph) => {
                let why = format!("its font cannot draw glyph {glyph} of `{text}`");
                self.report(diagnostics, at, "label-glyph-unreadable", why);
                return;
            }
        };
        let extent = self.view.extent;
        let mut placed = false;
        for c in vertices(&feature.geometry) {
            // pointLayerDrawShape skips a point outside the map's extent.
            if !(c.x >= extent.min_x && c.x <= extent.max_x)
                || !(c.y >= extent.min_y && c.y <= extent.max_y)
            {
                continue;
            }
            self.placed.push(Placed {
                path: self.paths.len(),
                glyphs: cache,
                origin: path.centred_on(self.view.to_pixel(c)),
                color: fixed.color,
                feature_id: feature.id,
                label: at,
            });
            placed = true;
        }
        if placed {
            self.paths.push(path);
        }
    }

    /// Draws the labels, last collected first, as `msDrawLabelCache` walks a slot of its cache,
    /// each label's path built just before it is drawn. A glyph left out for reaching too far
    /// from the image is reported once for its label.
    pub(crate) fn finish<R: Renderer>(
        mut self,
        renderer: &mut R,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let placed = std::mem::take(&mut self.placed);
        for label in placed.iter().rev() {
            let drawn =
                self.paths[label.path].draw(&self.glyphs[label.glyphs], label.origin, self.image);
            if drawn.out_of_reach > 0 {
                let why = format!(
                    "{} of its glyphs reach farther than {} pixels from the image",
                    drawn.out_of_reach,
                    forager_render::MAX_GLYPH_REACH
                );
                let outcome = "those glyphs are left out";
                self.report_with(
                    diagnostics,
                    label.label,
                    "label-glyph-out-of-reach",
                    why,
                    outcome,
                );
            }
            if !drawn.path.is_empty() {
                renderer.draw(&DisplayItem {
                    op: DrawOp::Glyphs {
                        path: drawn.path,
                        color: label.color,
                    },
                    feature_id: Some(label.feature_id),
                });
            }
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
        self.report_with(diagnostics, at, code, why, "the label is not drawn");
    }

    /// Reports `why` with what follows from it, once per label and code.
    fn report_with(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        at: LabelAt,
        code: &'static str,
        why: String,
        outcome: &str,
    ) {
        if self.reported.insert((at, code)) {
            let span = self.span(at);
            let message = format!("{why}; {outcome}");
            diagnostics.push(Diagnostic::warning(code, message, span));
        }
    }

    fn span(&self, (layer, class, label): LabelAt) -> Option<Span> {
        self.map.layers[layer].classes[class].labels[label]
            .span
            .clone()
    }
}

/// What of `label` does not vary by feature, or why the label is not drawn: a property it binds
/// to a feature's property or to an expression, or one whose meaning is not drawn yet, is not
/// guessed at.
fn fixed<'a>(
    map: &Map,
    layer: &Layer,
    label: &'a Label,
    text: &'a Expr,
) -> Result<Fixed<'a>, String> {
    let fonts = match &label.font {
        Some(Bound::Value(fonts)) => fonts.as_str(),
        Some(_) => return Err("its FONT varies by feature, which is not drawn yet".to_owned()),
        None => {
            return Err("it names no FONT; MapServer's built-in font is not bundled".to_owned());
        }
    };
    let size = match &label.size {
        None => DEFAULT_SIZE,
        Some(Bound::Value(size)) => *size,
        Some(_) => return Err("its SIZE varies by feature, which is not drawn yet".to_owned()),
    };
    let color = match &label.color {
        None => DEFAULT_COLOR,
        Some(Bound::Value(color)) => *color,
        Some(_) => return Err("its COLOR varies by feature, which is not drawn yet".to_owned()),
    };
    let unsupported = [
        (
            !matches!(label.position, None | Some(Bound::Value(Position::Cc))),
            "a POSITION other than CC",
        ),
        (
            !matches!(
                label.offset,
                None | Some([Bound::Value(0.0), Bound::Value(0.0)])
            ),
            "an OFFSET",
        ),
        (
            !matches!(label.angle, None | Some(Bound::Value(0.0))) || label.angle_mode.is_some(),
            "an ANGLE",
        ),
        (label.wrap.is_some(), "a WRAP"),
        (label.max_length.is_some_and(|m| m > 0.0), "a MAXLENGTH"),
        (
            !matches!(label.priority, None | Some(Bound::Value(1.0))),
            "a PRIORITY",
        ),
        (label.expression.is_some(), "an EXPRESSION"),
        (label.outline_color.is_some(), "an OUTLINECOLOR"),
        (label.shadow_color.is_some(), "a SHADOWCOLOR"),
        (!label.styles.is_empty(), "a STYLE"),
        (
            label.min_scale_denom.is_some() || label.max_scale_denom.is_some(),
            "a scale range",
        ),
        (
            layer.symbol_scale_denom.is_some(),
            "a layer's SYMBOLSCALEDENOM",
        ),
        (
            !matches!(layer.size_units, None | Some(Units::Pixels)),
            "a layer's SIZEUNITS other than pixels",
        ),
    ];
    if let Some((_, what)) = unsupported.iter().find(|(has, _)| *has) {
        return Err(format!("it has {what}, which is not drawn yet"));
    }
    // Without SYMBOLSCALEDENOM and SIZEUNITS, the scale factor and the resolution factor are both
    // the map's resolution over its DEFRESOLUTION.
    let factor = map.resolution.unwrap_or(72.0) / map.def_resolution.unwrap_or(72.0);
    let glyph_size = glyph_size(
        size,
        factor,
        label.min_size.unwrap_or(DEFAULT_MIN_SIZE),
        label.max_size.unwrap_or(DEFAULT_MAX_SIZE),
        factor,
    );
    Ok(Fixed {
        text,
        fonts,
        glyph_size,
        color,
    })
}

/// A label's text for a feature, for the forms that need no expression evaluator: a string, a
/// property as text, and their concatenation. A property the feature lacks, or whose value is
/// null, reads as empty, as OGR gives an unset field. Any other form is an error.
fn text(expr: &Expr, properties: &Properties) -> Result<String, String> {
    match expr {
        Expr::String(text) => Ok(text.clone()),
        Expr::Call {
            op: Op::ToString,
            args,
        } => match args.as_slice() {
            [Expr::Property { property }] => match properties.get(property) {
                None | Some(Value::Null) => Ok(String::new()),
                Some(Value::Number(digits)) => Ok(digits.to_owned()),
                Some(Value::String(text)) => Ok(text.into_owned()),
                Some(Value::Bool(_)) => Err(format!(
                    "the property `{property}` holds a boolean, whose text is not settled"
                )),
            },
            _ => Err("its text is an expression, which is not evaluated yet".to_owned()),
        },
        Expr::Call {
            op: Op::Concat,
            args,
        } => args.iter().map(|arg| text(arg, properties)).collect(),
        _ => Err("its text is an expression, which is not evaluated yet".to_owned()),
    }
}
