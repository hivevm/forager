// The labels of point layers follow pointLayerDrawShape, msDrawLabelCache, and its choice of a
// point label's position and offsets, src/mapdraw.c, msAddLabelGroup and msComputeTextPath,
// src/maplabel.c, bindLabel and bindIntegerAttribute, src/maputil.c, msGetMarkerSize,
// src/mapsymbol.c, and msDrawTextSymbol, src/maprendering.c, of MapServer at the release pinned
// in scripts/fetch-msautotest.sh, for labels without the collisions of the label cache
// (ADR-0028).
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

use forager_expr::atof;
use forager_label::{
    Align, Anchor, Font, Glyphs, Layout, LineError, Offsets, PointLabel, Position, TextPath,
    glyph_size, lines, pixel_size, place,
};
use forager_ports::{Feature, Properties, ResourceLoader, Value};
use forager_render::{DisplayItem, DrawOp, GlyphOutline, Point, Renderer};
use forager_style_ir::{
    Align as IrAlign, Bound, Diagnostic, Expr, Force, Label, Layer, Map, Op,
    Position as IrPosition, Rgba, Span, Style, Units,
};

use crate::classes::{FeatureProperties, c_bytes, color};
use crate::draw::vertices;
use crate::extent::View;
use crate::fonts::Fonts;
use crate::symbol::marker_size_of;

/// A label's size where none is given, MapServer's `MS_MEDIUM`.
const DEFAULT_SIZE: f64 = 10.0;
/// The bounds of a label's size where none are given, `MS_MINFONTSIZE` and `MS_MAXFONTSIZE`.
const DEFAULT_MIN_SIZE: f64 = 4.0;
const DEFAULT_MAX_SIZE: f64 = 256.0;
/// A label's colour where none is given.
const DEFAULT_COLOR: Rgba = Rgba::opaque(0, 0, 0);
/// A label's `OUTLINEWIDTH` where none is given.
const DEFAULT_OUTLINE_WIDTH: f64 = 1.0;
/// MapServer's `MS_DEG_TO_RAD`, `src/mapserver.h`.
const DEG_TO_RAD: f64 = 0.017_453_292_519_943_295;

/// A property of a label as it is drawn: a value, or the feature's property it is bound to.
#[derive(Clone, Copy)]
enum Prop<'a, T> {
    Value(T),
    Binding(&'a str),
}

/// What of a label is settled before its features, once checked to be drawable.
struct Fixed<'a> {
    text: &'a Expr,
    fonts: &'a str,
    wrap: Option<char>,
    size: Prop<'a, f64>,
    min_size: f64,
    max_size: f64,
    color: Prop<'a, Option<Rgba>>,
    outline_color: Prop<'a, Option<Rgba>>,
    outline_width: f64,
    offset: [Prop<'a, f64>; 2],
    angle: Prop<'a, f64>,
    position: Prop<'a, Position>,
    align: Prop<'a, Align>,
    /// The layer's scale factor and the image's resolution factor, which are one here.
    factor: f64,
    buffer: f64,
    force: bool,
}

/// A label's properties for one feature, as `bindLabel` leaves them.
struct BoundLabel {
    size: f64,
    color: Option<Rgba>,
    outline_color: Option<Rgba>,
    offset: [i32; 2],
    angle: f64,
    /// `None` where a bound value names no position or alignment.
    position: Option<Position>,
    align: Option<Align>,
}

/// Why a label's text is not laid out: no font of its list draws a line, or its font cannot draw
/// a glyph, nor its fallback.
enum Unlaid {
    Font(String),
    Glyph(u32),
}

/// A label of the render by its layer's index, its class's, and its index in the class.
type LabelAt = (usize, usize, usize);

/// [`LabelAt`] as a placed label holds it.
type HeldAt = [u32; 3];

/// One label placed at one point, held until the labels are drawn: its text path, and where and
/// how it is drawn.
struct Placed {
    origin: Point,
    rotation: f64,
    feature_id: u64,
    path: u32,
    label: HeldAt,
    color: Option<Rgba>,
    /// The outline's colour and its width in whole pixels.
    outline: Option<(Rgba, i32)>,
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
            placed: Vec::new(),
            reported: BTreeSet::new(),
            image: [0.0, 0.0, f64::from(size.0), f64::from(size.1)],
        }
    }

    /// Collects the labels of layer `index` for each of `drawn`, in the order the layer draws
    /// them, as msDrawShape adds each feature's labels to the cache: on a point layer at each
    /// vertex inside the map's extent, as `pointLayerDrawShape` places them; a label of any other
    /// layer is not drawn yet.
    pub(crate) async fn collect(
        &mut self,
        index: usize,
        point_layer: bool,
        drawn: &[Labelled<'_>],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let map = self.map;
        let layer = &map.layers[index];
        // What of each label of the classes drawn is settled, or why it is not drawn.
        let mut fixed_labels = BTreeMap::new();
        for &(_, class_index, _) in drawn {
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
        for &(feature, class_index, style) in drawn {
            for n in 0..layer.classes[class_index].labels.len() {
                let at = (index, class_index, n);
                if let Some(Some(fixed)) = fixed_labels.get(&at) {
                    self.feature(at, fixed, feature, style, diagnostics).await;
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
        style: Option<&Style>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let text = match text(fixed.text, &feature.properties) {
            Ok(text) => text,
            Err(why) => {
                self.report(diagnostics, at, "label-text-unsupported", why);
                return;
            }
        };
        let lines = match lines(&text, fixed.wrap) {
            Ok(lines) => lines,
            Err(LineError::NotLatin(c)) => {
                let why = format!(
                    "the text `{text}` holds U+{:04X}, which needs a shaper",
                    u32::from(c)
                );
                self.report(diagnostics, at, "label-line-unsupported", why);
                return;
            }
        };
        if lines.iter().all(Vec::is_empty) {
            return;
        }
        let bound = bind(fixed, &feature.properties);
        let (position, align, marker) = match settle(fixed, &bound, lines.len(), style, self.map) {
            Ok(settled) => settled,
            Err((code, why)) => {
                self.report(diagnostics, at, code, why);
                return;
            }
        };
        let glyph_size = glyph_size(
            bound.size,
            fixed.factor,
            fixed.min_size,
            fixed.max_size,
            fixed.factor,
        );
        let path = match self.layout(fixed, &lines, glyph_size, align).await {
            Ok(path) => path,
            Err(Unlaid::Font(why)) => {
                self.report(diagnostics, at, "label-font-unusable", why);
                return;
            }
            Err(Unlaid::Glyph(glyph)) => {
                let why = format!("its font cannot draw glyph {glyph} of `{text}`");
                self.report(diagnostics, at, "label-glyph-unreadable", why);
                return;
            }
        };
        // msDrawTextSymbol's outline width, `MS_NINT(outlinewidth * (glyph_size / size))`, and
        // AGG's contour one pixel wider.
        let outline = bound.outline_color.map(|color| {
            let ow = lrint_int(fixed.outline_width * (glyph_size / bound.size));
            (color, ow.wrapping_add(1))
        });
        let text_box = path.text_box();
        let rotation = bound.angle * DEG_TO_RAD;
        // The offset and the buffer scaled into an int.
        let label_offset = bound.offset.map(|o| (f64::from(o) * fixed.factor) as i32);
        let buffer = (fixed.buffer * fixed.factor) as i32;
        let extent = self.view.extent;
        let mut placed = false;
        for c in vertices(&feature.geometry) {
            // pointLayerDrawShape skips a point outside the map's extent.
            if !(c.x >= extent.min_x && c.x <= extent.max_x)
                || !(c.y >= extent.min_y && c.y <= extent.max_y)
            {
                continue;
            }
            let point = self.view.to_pixel(c);
            let label = PointLabel {
                position,
                text: text_box,
                offsets: Offsets {
                    marker: marker.map_or([0.0, 0.0], |(w, h)| {
                        marker_offset(point, w, h, fixed, &bound)
                    }),
                    label: label_offset,
                },
                rotation,
                buffer,
                force: fixed.force,
            };
            // Without the label cache, every box fits.
            let Some(placement) = place(point, &label, |_| true) else {
                continue;
            };
            self.placed.push(Placed {
                origin: placement.origin,
                rotation,
                feature_id: feature.id,
                path: self.paths.len() as u32,
                label: [at.0, at.1, at.2].map(|i| i as u32),
                color: bound.color,
                outline,
            });
            placed = true;
        }
        if placed {
            self.paths.push(path);
        }
    }

    /// Lays out `lines` at `glyph_size`, each in the font of the label's list that draws it, or
    /// says why it cannot.
    async fn layout(
        &mut self,
        fixed: &Fixed<'_>,
        lines: &[Vec<char>],
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
            let face = self
                .fonts
                .face(self.map, fixed.fonts, line, self.files)
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
                    layout.line(&font, &mut self.glyphs[cache], cache as u32, &face.glyphs)
                })
                .map_err(Unlaid::Glyph)?;
        }
        Ok(layout.finish(align))
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
            if label.color.is_none() && label.outline.is_none() {
                continue;
            }
            let at = label.label.map(|i| i as usize);
            let at = (at[0], at[1], at[2]);
            let drawn = self.paths[label.path as usize].draw(
                &self.glyphs,
                label.origin,
                label.rotation,
                self.image,
            );
            if drawn.out_of_reach > 0 {
                let why = format!(
                    "{} of its glyphs reach farther than {} pixels from the image",
                    drawn.out_of_reach,
                    forager_render::MAX_GLYPH_REACH
                );
                let outcome = "those glyphs are left out";
                self.report_with(diagnostics, at, "label-glyph-out-of-reach", why, outcome);
            }
            if !drawn.path.is_empty() {
                renderer.draw(&DisplayItem {
                    op: DrawOp::Glyphs {
                        path: drawn.path,
                        color: label.color,
                        outline: label.outline.map(|(color, width)| GlyphOutline {
                            color,
                            width: f64::from(width),
                        }),
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

/// A label's position, its alignment, and the width and height of the marker it moves away from.
type Settled = (Position, Align, Option<(f64, f64)>);

/// Where a label stands for one feature: its position, its alignment over `num_lines` lines, and
/// the size of the marker it moves away from; or the diagnostic's code and why it is not drawn.
fn settle(
    fixed: &Fixed<'_>,
    bound: &BoundLabel,
    num_lines: usize,
    style: Option<&Style>,
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
    // The marker a label away from its point moves away from.
    let marker = match (position, style) {
        (Position::At(Anchor::Cc), _) | (_, None) => None,
        (_, Some(style)) => match marker_size_of(style, &map.symbols, fixed.factor) {
            Ok(size) => Some(size),
            Err(what) => {
                let why =
                    format!("its class's first style draws {what}, and the label stands beside it");
                return Err(("label-not-drawn", why));
            }
        },
    };
    Ok((position, align, marker))
}

/// msDrawLabelCache's offset of a label from the marker at `point`, `w` by `h` pixels: half the
/// size of its box in the marker cache, whose far corner lies `w - 1` and `h - 1` from its near
/// one, grown by half the label's outline where it has one.
fn marker_offset(point: Point, w: f64, h: f64, fixed: &Fixed<'_>, bound: &BoundLabel) -> [f64; 2] {
    let (min_x, min_y) = (point.x - 0.5 * w, point.y - 0.5 * h);
    let (max_x, max_y) = (min_x + (w - 1.0), min_y + (h - 1.0));
    let mut offset = [(max_x - min_x) / 2.0, (max_y - min_y) / 2.0];
    if offset[0] != 0.0 && bound.outline_color.is_some() {
        let grow = fixed.outline_width / 2.0 * fixed.factor;
        offset = [offset[0] + grow, offset[1] + grow];
    }
    offset
}

/// `MS_NINT` into an `int`, as the x86-64 build at the pin computes it: `lrint`, half to even,
/// gives a `long`, `LONG_MIN` for a value past one or no number, and the `long` narrowed to an
/// `int` keeps its low 32 bits.
fn lrint_int(x: f64) -> i32 {
    let r = x.round_ties_even();
    if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&r) {
        (r as i64) as i32
    } else {
        0
    }
}

/// `bindIntegerAttribute` into `unset`: `MS_NINT(atof(value))`, or `unset` where the value is
/// empty.
fn bound_integer(text: &[u8], unset: i32) -> i32 {
    if text.is_empty() {
        unset
    } else {
        lrint_int(atof(text))
    }
}

/// A label's properties for a feature with `properties`, as `bindLabel` binds them.
fn bind(fixed: &Fixed<'_>, properties: &Properties) -> BoundLabel {
    let lookup = FeatureProperties(properties);
    let read = |binding: &str| -> Vec<u8> {
        use forager_expr::Lookup;
        lookup
            .text(binding)
            .map(|t| c_bytes(&t).to_vec())
            .unwrap_or_default()
    };
    let number = |prop: Prop<'_, f64>, unset: i32| match prop {
        Prop::Value(v) => v,
        Prop::Binding(b) => f64::from(bound_integer(&read(b), unset)),
    };
    let colour = |prop: Prop<'_, Option<Rgba>>| match prop {
        Prop::Value(c) => c,
        Prop::Binding(b) => color(&read(b)),
    };
    let position = match fixed.position {
        Prop::Value(p) => Some(p),
        Prop::Binding(b) => bound_position(&read(b)),
    };
    let align = match fixed.align {
        Prop::Value(a) => Some(a),
        Prop::Binding(b) => bound_align(&read(b)),
    };
    BoundLabel {
        size: number(fixed.size, 1),
        color: colour(fixed.color),
        outline_color: colour(fixed.outline_color),
        offset: fixed.offset.map(|o| number(o, 0) as i32),
        angle: match fixed.angle {
            Prop::Value(a) => a,
            // bindDoubleAttribute after the angle is set to 0.
            Prop::Binding(b) => {
                let text = read(b);
                if text.is_empty() { 0.0 } else { atof(&text) }
            }
        },
        position,
        align,
    }
}

/// A bound `POSITION` as `bindLabel` reads it: a number rounded to an integer, which where it is
/// not 0 is MapServer's position constant, unchecked; else two characters naming a position,
/// without regard to case. `None` where it names none.
fn bound_position(text: &[u8]) -> Option<Position> {
    let anchors = [
        Anchor::Ul,
        Anchor::Lr,
        Anchor::Ur,
        Anchor::Ll,
        Anchor::Cr,
        Anchor::Cl,
        Anchor::Uc,
        Anchor::Lc,
        Anchor::Cc,
    ];
    match bound_integer(text, 0) {
        0 => {
            let names = [
                b"ul", b"lr", b"ur", b"ll", b"cr", b"cl", b"uc", b"lc", b"cc",
            ];
            if text.len() != 2 {
                return None;
            }
            names
                .iter()
                .position(|name| text.eq_ignore_ascii_case(*name))
                .map(|i| Position::At(anchors[i]))
        }
        // MS_UL (101) to MS_CC (109), and MS_AUTO.
        n @ 101..=109 => Some(Position::At(anchors[(n - 101) as usize])),
        110 => Some(Position::Auto),
        _ => Some(Position::At(Anchor::Other)),
    }
}

/// A bound `ALIGN` as `bindLabel` reads it: a number rounded to an integer, which where it is not
/// 0 is MapServer's alignment constant, `MS_ALIGN_CENTER` 2 and `MS_ALIGN_RIGHT` 3 and any other
/// the left of a line read left to right; else a value of at least four characters that begins
/// with `cente`, `left`, or `right`, without regard to case. `None` where it names none.
fn bound_align(text: &[u8]) -> Option<Align> {
    match bound_integer(text, 0) {
        0 => {
            let starts = |prefix: &[u8]| {
                text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix)
            };
            if text.len() < 4 {
                None
            } else if starts(b"cente") {
                Some(Align::Center)
            } else if starts(b"left") {
                Some(Align::Left)
            } else if starts(b"right") {
                Some(Align::Right)
            } else {
                None
            }
        }
        2 => Some(Align::Center),
        3 => Some(Align::Right),
        _ => Some(Align::Left),
    }
}

/// A property that is a value or bound to a feature's property, or why it is not drawn: one
/// computed by an expression is not evaluated yet.
fn prop<'a, T, U>(
    property: Option<&'a Bound<T>>,
    what: &str,
    default: U,
    value: impl Fn(&T) -> U,
) -> Result<Prop<'a, U>, String> {
    match property {
        None => Ok(Prop::Value(default)),
        Some(Bound::Value(v)) => Ok(Prop::Value(value(v))),
        Some(Bound::Binding { binding }) => Ok(Prop::Binding(binding)),
        Some(Bound::Expr(_)) => Err(format!(
            "its {what} is computed by an expression, which is not drawn yet"
        )),
    }
}

/// What of `label` is settled before its features, or why the label is not drawn: a property
/// computed by an expression, or one whose meaning is not drawn yet, is not guessed at.
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
    let unsupported = [
        (
            label.angle_mode.is_some(),
            "an ANGLE AUTO, AUTO2, or FOLLOW",
        ),
        (label.max_length.is_some_and(|m| m > 0.0), "a MAXLENGTH"),
        (
            !matches!(label.priority, None | Some(Bound::Value(1.0))),
            "a PRIORITY",
        ),
        (label.expression.is_some(), "an EXPRESSION"),
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
    let offset = match &label.offset {
        None => [Prop::Value(0.0), Prop::Value(0.0)],
        Some([x, y]) => [
            prop(Some(x), "OFFSET", 0.0, |v| *v)?,
            prop(Some(y), "OFFSET", 0.0, |v| *v)?,
        ],
    };
    // Without SYMBOLSCALEDENOM and SIZEUNITS, the scale factor and the resolution factor are both
    // the map's resolution over its DEFRESOLUTION.
    let factor = map.resolution.unwrap_or(72.0) / map.def_resolution.unwrap_or(72.0);
    Ok(Fixed {
        text,
        fonts,
        // WRAP is the first character of its string.
        wrap: label.wrap.as_deref().and_then(|w| w.chars().next()),
        size: prop(label.size.as_ref(), "SIZE", DEFAULT_SIZE, |v| *v)?,
        min_size: label.min_size.unwrap_or(DEFAULT_MIN_SIZE),
        max_size: label.max_size.unwrap_or(DEFAULT_MAX_SIZE),
        color: prop(label.color.as_ref(), "COLOR", Some(DEFAULT_COLOR), |c| {
            Some(*c)
        })?,
        outline_color: prop(label.outline_color.as_ref(), "OUTLINECOLOR", None, |c| {
            Some(*c)
        })?,
        outline_width: label.outline_width.unwrap_or(DEFAULT_OUTLINE_WIDTH),
        offset,
        angle: prop(label.angle.as_ref(), "ANGLE", 0.0, |v| *v)?,
        position: prop(
            label.position.as_ref(),
            "POSITION",
            Position::At(Anchor::Cc),
            |p| position(*p),
        )?,
        align: prop(label.align.as_ref(), "ALIGN", Align::Left, |a| match a {
            IrAlign::Left => Align::Left,
            IrAlign::Center => Align::Center,
            IrAlign::Right => Align::Right,
        })?,
        factor,
        buffer: label.buffer.unwrap_or(0.0),
        force: label.force == Some(Force::On),
    })
}

/// A literal `POSITION` as the label crate places it.
fn position(p: IrPosition) -> Position {
    let anchor = match p {
        IrPosition::Ul => Anchor::Ul,
        IrPosition::Uc => Anchor::Uc,
        IrPosition::Ur => Anchor::Ur,
        IrPosition::Cl => Anchor::Cl,
        IrPosition::Cc => Anchor::Cc,
        IrPosition::Cr => Anchor::Cr,
        IrPosition::Ll => Anchor::Ll,
        IrPosition::Lc => Anchor::Lc,
        IrPosition::Lr => Anchor::Lr,
        IrPosition::Auto => return Position::Auto,
    };
    Position::At(anchor)
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
