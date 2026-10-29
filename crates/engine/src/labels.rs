// The labels of point layers follow pointLayerDrawShape and msDrawLabelCache, src/mapdraw.c,
// msAddLabelGroup and msComputeTextPath, src/maplabel.c, and msDrawTextSymbol,
// src/maprendering.c, of MapServer at the release pinned in scripts/fetch-msautotest.sh, for a
// label at POSITION CC on a point, without the collisions of the label cache (ADR-0028).
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
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

use std::collections::BTreeSet;

use forager_label::{Font, LineError, TextPath, glyph_size, line};
use forager_ports::{Feature, Properties, ResourceLoader, Value};
use forager_render::{DisplayItem, DrawOp};
use forager_style_ir::{Bound, Diagnostic, Expr, Label, Layer, Map, Op, Position, Rgba};

use crate::extent::View;
use crate::fonts::Fonts;
use crate::vertices;

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

/// The labels of one render, collected over its layers and drawn after them, as MapServer's label
/// cache draws them, with each problem of a label reported once.
pub(crate) struct Labels {
    fonts: Fonts,
    /// Each label's drawing operations with its feature, in the order they were collected.
    drawn: Vec<DisplayItem>,
    /// The labels already reported, by layer and label index, with the diagnostic's code.
    reported: BTreeSet<(usize, usize, &'static str)>,
    /// The image the labels are drawn in, `[min x, min y, max x, max y]`.
    image: [f64; 4],
}

impl Labels {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            fonts: Fonts::default(),
            drawn: Vec::new(),
            reported: BTreeSet::new(),
            image: [0.0, 0.0, f64::from(width), f64::from(height)],
        }
    }

    /// Collects the labels of `labels`, the first class's of layer `index`, for each of
    /// `features`: on a point layer at each vertex inside the map's extent, as
    /// `pointLayerDrawShape` places them; a label of any other layer is not drawn yet.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn collect<L: ResourceLoader>(
        &mut self,
        map: &Map,
        index: usize,
        layer: &Layer,
        labels: &[Label],
        point_layer: bool,
        features: &[Feature],
        view: &View,
        files: &L,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (n, label) in labels.iter().enumerate() {
            let Some(text) = &label.text else {
                // No text and no style to draw in its place: MapServer skips the label.
                continue;
            };
            if features.is_empty() {
                continue;
            }
            let fixed = if point_layer {
                fixed(map, layer, label, text)
            } else {
                Err("labels of line and polygon layers are not drawn yet".to_owned())
            };
            let fixed = match fixed {
                Ok(fixed) => fixed,
                Err(why) => {
                    self.report(diagnostics, (index, n), "label-not-drawn", label, why);
                    continue;
                }
            };
            for feature in features {
                self.feature(
                    map,
                    (index, n),
                    label,
                    &fixed,
                    feature,
                    view,
                    files,
                    diagnostics,
                )
                .await;
            }
        }
    }

    /// Draws one label of one feature at each of its points.
    #[allow(clippy::too_many_arguments)]
    async fn feature<L: ResourceLoader>(
        &mut self,
        map: &Map,
        at: (usize, usize),
        label: &Label,
        fixed: &Fixed<'_>,
        feature: &Feature,
        view: &View,
        files: &L,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let text = match text(fixed.text, &feature.properties) {
            Ok(text) => text,
            Err(why) => {
                self.report(diagnostics, at, "label-text-unsupported", label, why);
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
                self.report(diagnostics, at, "label-line-unsupported", label, why);
                return;
            }
        };
        if chars.is_empty() {
            return;
        }
        let face = match self.fonts.face(map, fixed.fonts, &chars, files).await {
            Ok(face) => face,
            Err(why) => {
                self.report(diagnostics, at, "label-font-unusable", label, why);
                return;
            }
        };
        let path = Font::new(&face.bytes)
            .map_err(|_| 0)
            .and_then(|font| TextPath::new(&font, &face.glyphs, fixed.glyph_size));
        let path = match path {
            Ok(path) => path,
            Err(glyph) => {
                let why = format!("its font cannot draw glyph {glyph} of `{text}`");
                self.report(diagnostics, at, "label-glyph-unreadable", label, why);
                return;
            }
        };
        let extent = view.extent;
        for c in vertices(&feature.geometry) {
            // pointLayerDrawShape skips a point outside the map's extent.
            if !(c.x >= extent.min_x && c.x <= extent.max_x)
                || !(c.y >= extent.min_y && c.y <= extent.max_y)
            {
                continue;
            }
            let origin = path.centred_on(view.to_pixel(c));
            let commands = path.draw(origin, self.image);
            if !commands.is_empty() {
                self.drawn.push(DisplayItem {
                    op: DrawOp::Glyphs {
                        path: commands,
                        color: fixed.color,
                    },
                    feature_id: Some(feature.id),
                });
            }
        }
    }

    /// The labels' drawing operations, last collected first, as `msDrawLabelCache` walks a slot
    /// of its cache.
    pub(crate) fn finish(self) -> impl Iterator<Item = DisplayItem> {
        self.drawn.into_iter().rev()
    }

    fn report(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        (layer, label_index): (usize, usize),
        code: &'static str,
        label: &Label,
        why: String,
    ) {
        if self.reported.insert((layer, label_index, code)) {
            let message = format!("{why}; the label is not drawn");
            diagnostics.push(Diagnostic::warning(code, message, label.span.clone()));
        }
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
        None => return Err("it names no FONT; MapServer's built-in font is not bundled".to_owned()),
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
    if !matches!(label.position, None | Some(Bound::Value(Position::Cc))) {
        return Err("its POSITION is not CC, which is not drawn yet".to_owned());
    }
    let unsupported = [
        (label.expression.is_some(), "an EXPRESSION"),
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
    ];
    if let Some((_, what)) = unsupported.iter().find(|(has, _)| *has) {
        return Err(format!("it has {what}, which is not drawn yet"));
    }
    // Without SYMBOLSCALEDENOM, the scale factor and the resolution factor are both the map's
    // resolution over its DEFRESOLUTION.
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
