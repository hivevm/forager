// The selection of a feature's class and the binding of its style's properties follow
// msShapeGetClass, msEvalExpression, bindStyle, and bindColorAttribute of src/maputil.c, the layer's
// filter as msLayerNextShape applies it and the attributes as msLayerWhichItems finds them in
// src/maplayer.c, and the scale factor as msPrepareImage computes it in src/mapdraw.c, of the
// release pinned in scripts/fetch-msautotest.sh.
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

//! Which class draws a feature, and the values its style's bound and computed properties take for
//! it (ADR-0031).

use std::borrow::Cow;
use std::collections::BTreeSet;

use forager_expr::{Evaluator, Failure, Lookup, atof, atoi};
use forager_ports::{Feature, Properties, Value};
use forager_style_ir::{
    Bound, Diagnostic, Expr, Layer, LineOffset, Map, Rgba, Span, Style, Symbol, SymbolRef, Units,
};

use crate::extent::View;

/// A feature's properties as the evaluator reads them. A name is found without regard to ASCII
/// case, as MapServer finds an attribute in a shapefile's `.dbf` or an OGR source: by the
/// source's own lookup first, then the first property whose name equals it in ASCII case.
pub(crate) struct FeatureProperties<'a>(pub &'a Properties);

/// A property's value as text: a number's as its source gives it, a boolean as OGR gives one, 1
/// or 0, and none for null.
fn value_text(value: Value<'_>) -> Option<Cow<'_, str>> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(Cow::Borrowed(if b { "1" } else { "0" })),
        Value::Number(text) => Some(Cow::Borrowed(text)),
        Value::String(text) => Some(text),
    }
}

impl Lookup for FeatureProperties<'_> {
    fn text(&self, name: &str) -> Option<Cow<'_, str>> {
        if let Some(value) = self.0.get(name) {
            return value_text(value);
        }
        let mut found: Option<Option<String>> = None;
        self.0.each(|key, value| {
            if found.is_none() && key.eq_ignore_ascii_case(name) {
                found = Some(value_text(value).map(Cow::into_owned));
            }
        });
        found.flatten().map(Cow::Owned)
    }
}

/// What carries an expression, for a report.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Carrier {
    Filter,
    Class(usize),
    /// A style of a class, by their places.
    Style(usize, usize),
    /// A label of a class, by their places.
    Label(usize, usize),
}

/// The patterns a render cannot match as MapServer does, and the texts past the limit, each
/// reported once per layer, carrier, and kind.
#[derive(Default)]
pub(crate) struct Reports {
    reported: BTreeSet<(usize, Carrier, &'static str)>,
}

impl Reports {
    fn report(
        &mut self,
        failure: &Failure,
        layer_index: usize,
        layer: &Layer,
        carrier: Carrier,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let code = match failure {
            Failure::Pattern { .. } => "pattern-unsupported",
            Failure::TooLong => "text-too-long",
            Failure::Undefined => return,
        };
        if !self.reported.insert((layer_index, carrier, code)) {
            return;
        }
        let (what, span): (String, Option<Span>) = match carrier {
            Carrier::Filter => ("its filter".to_owned(), layer.span.clone()),
            Carrier::Class(i) | Carrier::Style(i, _) | Carrier::Label(i, _) => {
                let class = &layer.classes[i];
                let name = class
                    .name
                    .as_deref()
                    .map_or(String::new(), |n| format!(" `{n}`"));
                let class_span = class.span.clone().or_else(|| layer.span.clone());
                match carrier {
                    Carrier::Style(_, s) => (
                        format!("style {} of its class {}{name}", s + 1, i + 1),
                        class.styles[s].span.clone().or(class_span),
                    ),
                    Carrier::Label(_, l) => (
                        format!("label {} of its class {}{name}", l + 1, i + 1),
                        class.labels[l].span.clone().or(class_span),
                    ),
                    _ => (format!("its class {}{name}", i + 1), class_span),
                }
            }
        };
        let layer_name = crate::name(layer);
        let message = match failure {
            Failure::Pattern { pattern, construct } => format!(
                "layer {layer_name}: the pattern `{pattern}` of {what} uses {construct}, which \
                 Forager cannot match as MapServer does; the expression that holds it never \
                 matches"
            ),
            _ => format!(
                "layer {layer_name}: an expression of {what} reads or computes a text past {} \
                 bytes; the expression fails where it does",
                forager_expr::MAX_TEXT_BYTES
            ),
        };
        diagnostics.push(Diagnostic::warning(code, message, span));
    }
}

/// The selection of one layer's classes for its features.
pub(crate) struct Selector<'a, 'm> {
    pub layer: &'a Layer,
    pub layer_index: usize,
    /// The map's scale, as msCalculateScale computes it.
    pub scale: f64,
    pub evaluator: &'m mut Evaluator,
    pub reports: &'m mut Reports,
    pub diagnostics: &'m mut Vec<Diagnostic>,
}

impl<'a> Selector<'a, '_> {
    fn matches(&mut self, expr: &Expr, feature: &Feature, carrier: Carrier) -> bool {
        match self
            .evaluator
            .matches(expr, &FeatureProperties(&feature.properties))
        {
            Ok(matched) => matched,
            Err(failure) => {
                self.reports.report(
                    &failure,
                    self.layer_index,
                    self.layer,
                    carrier,
                    self.diagnostics,
                );
                false
            }
        }
    }

    /// The class that draws `feature`: none where it fails the layer's filter, else the first
    /// in the map's scale whose expression matches it, a class without one matching every
    /// feature; none where no class matches.
    pub fn class(&mut self, feature: &Feature) -> Option<usize> {
        if let Some(filter) = &self.layer.filter
            && !self.matches(filter, feature, Carrier::Filter)
        {
            return None;
        }
        (0..self.layer.classes.len()).find(|&i| {
            let class = &self.layer.classes[i];
            in_scale(self.scale, class.min_scale_denom, class.max_scale_denom)
                && match &class.expression {
                    None => true,
                    Some(expression) => self.matches(expression, feature, Carrier::Class(i)),
                }
        })
    }

    /// Whether label `label` of class `class` is drawn for `feature`, as `msGetLabelStatus`
    /// evaluates its `EXPRESSION`: one that fails does not match.
    pub fn label_matches(&mut self, feature: &Feature, class: usize, label: usize) -> bool {
        match &self.layer.classes[class].labels[label].expression {
            None => true,
            Some(expression) => self.matches(expression, feature, Carrier::Label(class, label)),
        }
    }

    /// The number the expression `expr` of label `label` of class `class` computes for `feature`,
    /// 0 where it fails, as msEvalDoubleExpression gives it; the frontend has wrapped the
    /// expression in the `%g` text and the `atof` that msEvalDoubleExpression reads it with.
    pub fn label_number(
        &mut self,
        expr: &Expr,
        feature: &Feature,
        class: usize,
        label: usize,
    ) -> f64 {
        match self
            .evaluator
            .number(expr, &FeatureProperties(&feature.properties))
        {
            Ok(number) => number,
            Err(failure) => {
                self.reports.report(
                    &failure,
                    self.layer_index,
                    self.layer,
                    Carrier::Label(class, label),
                    self.diagnostics,
                );
                0.0
            }
        }
    }

    /// Each of `features` the layer draws, with the place of its class and its styles bound to
    /// it, in the source's order.
    pub fn select<'f>(
        &mut self,
        features: Vec<&'f Feature>,
        symbols: &[Symbol],
    ) -> Vec<crate::Selected<'f>>
    where
        'a: 'f,
    {
        let mut selected = Vec::with_capacity(features.len());
        for feature in features {
            let Some(index) = self.class(feature) else {
                continue;
            };
            let layer: &'a Layer = self.layer;
            let styles = layer.classes[index]
                .styles
                .iter()
                .enumerate()
                .map(|(s, style)| self.bind(style, feature, symbols, Carrier::Style(index, s)))
                .collect();
            selected.push((feature, index, styles));
        }
        selected
    }

    /// `style` of label `label` of class `class`, bound to `feature` as bindLabel binds a
    /// label's styles, through bindStyle as a class's.
    pub fn bind_label_style<'s>(
        &mut self,
        style: &'s Style,
        feature: &Feature,
        symbols: &[Symbol],
        (class, label): (usize, usize),
    ) -> Cow<'s, Style> {
        self.bind(style, feature, symbols, Carrier::Label(class, label))
    }

    /// `style`, which `carrier` carries, with each property bound to a property or computed by
    /// an expression given its value for `feature`, as bindStyle binds it; the style itself
    /// where none is.
    fn bind<'s>(
        &mut self,
        style: &'s Style,
        feature: &Feature,
        symbols: &[Symbol],
        carrier: Carrier,
    ) -> Cow<'s, Style> {
        if !varies(style) {
            return with_opacity(Cow::Borrowed(style));
        }
        let properties = FeatureProperties(&feature.properties);
        let mut bind = Binder {
            properties: &properties,
            evaluator: self.evaluator,
            failures: Vec::new(),
        };
        let mut bound = style.clone();
        bound.color = bind.color(&style.color);
        bound.outline_color = bind.color(&style.outline_color);
        // bindStyle sets an OPACITY bound to an empty value to 100.
        bound.opacity = bind.number(&style.opacity, 100.0);
        bound.angle = bind.number(&style.angle, 360.0);
        bound.size = bind.number(&style.size, 1.0);
        bound.width = bind.number(&style.width, 1.0);
        // bindStyle computes no OUTLINEWIDTH from an expression: the style keeps its 0.
        bound.outline_width = match &style.outline_width {
            Some(Bound::Expr(_)) => Some(Bound::Value(0.0)),
            other => bind.number(other, 1.0),
        };
        bound.offset = style.offset.as_ref().map(|[x, y]| {
            [
                bind.number(&Some(x.clone()), 0.0).expect("a value"),
                bind.number(&Some(y.clone()), 0.0).expect("a value"),
            ]
        });
        bound.polar_offset = style.polar_offset.as_ref().map(|[d, a]| {
            [
                bind.number(&Some(d.clone()), 0.0).expect("a value"),
                bind.number(&Some(a.clone()), 0.0).expect("a value"),
            ]
        });
        bound.line_offset = style.line_offset.as_ref().map(|line| LineOffset {
            distance: bind
                .number(&Some(line.distance.clone()), 0.0)
                .expect("a value"),
            both_sides: line.both_sides,
        });
        if let Some(Bound::Binding { binding }) = &style.symbol {
            let name = properties.text(binding).unwrap_or_default();
            bound.symbol = Some(Bound::Value(symbol_index(&name, symbols)));
        }
        for failure in std::mem::take(&mut bind.failures) {
            self.reports.report(
                &failure,
                self.layer_index,
                self.layer,
                carrier,
                self.diagnostics,
            );
        }
        with_opacity(Cow::Owned(bound))
    }
}

/// `style` with its opacity applied to its colours, as loadStyle at its `END` (src/mapfile.c)
/// and bindStyle (src/maputil.c) apply it: an opacity below 100 replaces the alpha of its colour
/// and its outline colour with `opacity × 2.55` rounded as `MS_NINT` rounds, kept in a byte as
/// AGG keeps it. The opacity is rounded first, as bindIntegerAttribute rounds a bound one, and is
/// not bounded to 0 to 100.
fn with_opacity(style: Cow<'_, Style>) -> Cow<'_, Style> {
    let Some(opacity) = style.opacity.as_ref().and_then(Bound::value) else {
        return style;
    };
    let opacity = ms_nint(opacity);
    if opacity >= 100 {
        return style;
    }
    let alpha = ms_nint(f64::from(opacity) * 2.55).rem_euclid(256) as u8;
    let mut style = style.into_owned();
    for color in [&mut style.color, &mut style.outline_color] {
        if let Some(Bound::Value(c)) = color {
            c.a = alpha;
        }
    }
    Cow::Owned(style)
}

/// `MS_NINT`, which is `lrint` in the build at the pin: the nearest integer, half to even.
pub(crate) fn ms_nint(x: f64) -> i32 {
    x.round_ties_even() as i32
}

/// `style`, which [`varies`] by no feature, as bindStyle leaves it for any: its opacity applied
/// to its colours.
pub(crate) fn unbound(style: &Style) -> Cow<'_, Style> {
    with_opacity(Cow::Borrowed(style))
}

/// Whether a property the engine draws varies by feature.
pub(crate) fn varies(style: &Style) -> bool {
    fn bound<T>(property: &Option<Bound<T>>) -> bool {
        property
            .as_ref()
            .is_some_and(|p| !matches!(p, Bound::Value(_)))
    }
    bound(&style.color)
        || bound(&style.outline_color)
        || bound(&style.opacity)
        || bound(&style.angle)
        || bound(&style.size)
        || bound(&style.width)
        || bound(&style.outline_width)
        || style
            .offset
            .as_ref()
            .is_some_and(|pair| pair.iter().any(|p| !matches!(p, Bound::Value(_))))
        || style
            .polar_offset
            .as_ref()
            .is_some_and(|pair| pair.iter().any(|p| !matches!(p, Bound::Value(_))))
        || style
            .line_offset
            .as_ref()
            .is_some_and(|line| !matches!(line.distance, Bound::Value(_)))
        || matches!(style.symbol, Some(Bound::Binding { .. }))
}

/// msScaleInBounds: whether `scale` lies in `[min, max)`; any scale where it is not above 0.
pub(crate) fn in_scale(scale: f64, min: Option<f64>, max: Option<f64>) -> bool {
    !(scale > 0.0 && (max.is_some_and(|max| scale >= max) || min.is_some_and(|min| scale < min)))
}

/// msLayerIsVisible's scale checks: a layer is drawn at a scale at most its `MAXSCALEDENOM`
/// where that is above 0 and above its `MINSCALEDENOM`, and only where a class might draw: one
/// at a scale at most its own `MAXSCALEDENOM` and above its own `MINSCALEDENOM`, each where above
/// 0. Bounds that differ from msScaleInBounds' at their ends, as MapServer's do. Any scale where
/// it is not above 0.
pub(crate) fn layer_in_scale(layer: &Layer, scale: f64) -> bool {
    if scale <= 0.0 {
        return true;
    }
    let above = |max: Option<f64>| max.is_some_and(|max| max > 0.0 && scale > max);
    // An unset MINSCALEDENOM is -1, which no scale above 0 reaches.
    if above(layer.max_scale_denom) || layer.min_scale_denom.is_some_and(|min| scale <= min) {
        return false;
    }
    layer.classes.is_empty()
        || layer.classes.iter().any(|class| {
            !above(class.max_scale_denom)
                && !class
                    .min_scale_denom
                    .is_some_and(|min| min > 0.0 && scale <= min)
        })
}

/// msCalculateScale: the scale denominator of the view at the map's resolution, by the inches
/// of its unit, as src/mapscale.c computes it without `ENABLE_VARIABLE_INCHES_PER_DEGREE`, which
/// the build at the pin leaves off; -1, which bounds nothing, for pixels.
pub(crate) fn scale_denom(map: &Map, view: &View, width: u32) -> f64 {
    let inches_per_unit = match map.units.unwrap_or(Units::Meters) {
        Units::Inches => 1.0,
        Units::Feet => 12.0,
        Units::Miles => 63360.0,
        Units::Meters => 39.3701,
        Units::Kilometers => 39370.1,
        Units::Degrees => 4_374_754.0,
        Units::NauticalMiles => 72913.3858,
        Units::Pixels => return -1.0,
    };
    let resolution = map.resolution.unwrap_or(72.0);
    let md = f64::from(width - 1) / (resolution * inches_per_unit);
    (view.extent.max_x - view.extent.min_x) / md
}

/// msPrepareImage's scale factor of a layer and of its classes, styles, and labels, and its
/// resolution factor of the image: the map's `RESOLUTION` over its `DEFRESOLUTION`, both 72
/// unless written. The two are one number for sizes in pixels without a `SYMBOLSCALEDENOM`,
/// msPrepareImage's last branch, which is the one the engine draws; a label of a layer with
/// either is not drawn yet, and a style draws as if it had neither.
pub(crate) fn scale_factor(map: &Map) -> f64 {
    map.resolution.unwrap_or(72.0) / map.def_resolution.unwrap_or(72.0)
}

/// A symbol by its name, as msGetSymbolIndex finds it, without regard to case; 0, the default
/// symbol, for a name the symbol set does not hold, which MapServer would next read as an image
/// file, not drawn yet.
fn symbol_index(name: &str, symbols: &[Symbol]) -> SymbolRef {
    let position = symbols.iter().position(|s| {
        s.name
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
    });
    SymbolRef::Index(position.map_or(0, |p| p as u32 + 1))
}

struct Binder<'a> {
    properties: &'a FeatureProperties<'a>,
    evaluator: &'a mut Evaluator,
    /// The failures of the style's expressions, to be reported.
    failures: Vec<Failure>,
}

/// A C string's bytes: up to the first NUL.
pub(crate) fn c_bytes(text: &str) -> &[u8] {
    let bytes = text.as_bytes();
    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
}

impl Binder<'_> {
    /// A number: a binding as bindDoubleAttribute reads it, `unset` where the value is empty; an
    /// expression as msEvalDoubleExpression gives it, 0 where it fails.
    fn number(&mut self, property: &Option<Bound<f64>>, unset: f64) -> Option<Bound<f64>> {
        let value = match property.as_ref()? {
            Bound::Value(v) => *v,
            Bound::Binding { binding } => {
                let text = self.properties.text(binding).unwrap_or_default();
                let bytes = c_bytes(&text);
                if bytes.is_empty() { unset } else { atof(bytes) }
            }
            Bound::Expr(expr) => match self.evaluator.number(expr, self.properties) {
                Ok(value) => value,
                Err(failure) => {
                    self.failures.push(failure);
                    0.0
                }
            },
        };
        Some(Bound::Value(value))
    }

    /// A colour: a binding's value or an expression's text as bindColorAttribute reads it; none
    /// where it does not read.
    fn color(&mut self, property: &Option<Bound<Rgba>>) -> Option<Bound<Rgba>> {
        let text = match property.as_ref()? {
            Bound::Value(v) => return Some(Bound::Value(*v)),
            Bound::Binding { binding } => self
                .properties
                .text(binding)
                .map(|t| c_bytes(&t).to_vec())
                .unwrap_or_default(),
            Bound::Expr(expr) => match self.evaluator.text(expr, self.properties) {
                Ok(text) => text,
                Err(failure) => {
                    self.failures.push(failure);
                    return None;
                }
            },
        };
        color(&text).map(Bound::Value)
    }
}

/// msHexToInt of two characters, as C's signed `char` gives them.
fn hex_to_int(pair: [u8; 2]) -> i32 {
    let digit = |c: u8| {
        let c = i32::from(c as i8);
        if c >= i32::from(b'A') {
            ((c & 0xdf) - i32::from(b'A')) + 10
        } else {
            c - i32::from(b'0')
        }
    };
    digit(pair[0]) * 16 + digit(pair[1])
}

/// msStringSplit at spaces: several in a row part once, a leading or a trailing one leaves an
/// empty token.
fn split_spaces(text: &[u8]) -> Vec<&[u8]> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut last = 0u8;
    for (i, &b) in text.iter().enumerate() {
        if b == b' ' {
            if last != b' ' {
                tokens.push(&text[start..i]);
            }
            start = i + 1;
        }
        last = b;
    }
    tokens.push(&text[start..]);
    tokens
}

/// bindColorAttribute: `#rrggbb` or `#rrggbbaa` by msHexToInt, or three integers parted by
/// spaces by `atoi`; none for anything else, or for a channel of -1, which MS_VALID_COLOR refuses.
/// Each channel is kept in a byte, as AGG keeps it.
pub(crate) fn color(text: &[u8]) -> Option<Rgba> {
    let channels = if text.first() == Some(&b'#') && (text.len() == 7 || text.len() == 9) {
        let pair = |i: usize| hex_to_int([text[i], text[i + 1]]);
        let alpha = if text.len() == 9 { pair(7) } else { 255 };
        [pair(1), pair(3), pair(5), alpha]
    } else {
        let tokens = split_spaces(text);
        if text.is_empty() || tokens.len() != 3 {
            return None;
        }
        [atoi(tokens[0]), atoi(tokens[1]), atoi(tokens[2]), 255]
    };
    if channels[..3].contains(&-1) {
        return None;
    }
    let [r, g, b, a] = channels.map(|c| c.rem_euclid(256) as u8);
    Some(Rgba { r, g, b, a })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_the_one_ms_calculate_scale_gives() {
        use forager_style_ir::Extent;
        // 100 units over 100 pixel steps at 72 dpi: the scale is 72 times the inches of a unit.
        let extent = Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        };
        let view = View::for_image(extent, 101, 101).unwrap();
        let scale = |units| {
            let map = Map {
                units,
                ..Map::default()
            };
            scale_denom(&map, &view, 101)
        };
        assert!((scale(None) - 72.0 * 39.3701).abs() < 1e-9);
        assert!((scale(Some(Units::Meters)) - 72.0 * 39.3701).abs() < 1e-9);
        assert!((scale(Some(Units::Degrees)) - 72.0 * 4_374_754.0).abs() < 1e-6);
        assert!((scale(Some(Units::Feet)) - 72.0 * 12.0).abs() < 1e-9);
        assert_eq!(scale(Some(Units::Pixels)), -1.0);
        // At a resolution of 96 dpi, a third more.
        let map = Map {
            resolution: Some(96.0),
            ..Map::default()
        };
        assert!((scale_denom(&map, &view, 101) - 96.0 * 39.3701).abs() < 1e-9);
    }

    #[test]
    fn no_scale_bounds_a_map_in_pixels() {
        assert!(in_scale(-1.0, Some(10.0), Some(20.0)));
        let layer = Layer {
            max_scale_denom: Some(1.0),
            ..Layer::default()
        };
        assert!(layer_in_scale(&layer, -1.0));
        assert!(!layer_in_scale(&layer, 2.0));
    }

    #[test]
    fn colours_read_as_bind_color_attribute_reads_them() {
        assert_eq!(color(b"#ff0080"), Some(Rgba::opaque(255, 0, 128)));
        assert_eq!(
            color(b"#FF008040"),
            Some(Rgba {
                a: 64,
                ..Rgba::opaque(255, 0, 128)
            })
        );
        assert_eq!(color(b"10 20 30"), Some(Rgba::opaque(10, 20, 30)));
        assert_eq!(color(b"10   20 30"), Some(Rgba::opaque(10, 20, 30)));
        assert_eq!(color(b" 1 2"), Some(Rgba::opaque(0, 1, 2)));
        assert_eq!(color(b" 1 2 3"), None);
        assert_eq!(color(b"1\t2 3"), None);
        assert_eq!(color(b"-1 0 0"), None);
        assert_eq!(color(b"256 0 0"), Some(Rgba::opaque(0, 0, 0)));
        assert_eq!(color(b""), None);
        assert_eq!(color(b"red"), None);
    }

    #[test]
    fn names_are_found_without_regard_to_case() {
        use forager_ports::Stored;
        let properties = Properties::list(vec![
            ("Name".into(), Stored::String("a".into())),
            ("flag".into(), Stored::Bool(true)),
            ("none".into(), Stored::Null),
        ]);
        let lookup = FeatureProperties(&properties);
        assert_eq!(lookup.text("NAME").as_deref(), Some("a"));
        assert_eq!(lookup.text("flag").as_deref(), Some("1"));
        assert_eq!(lookup.text("none"), None);
        assert_eq!(lookup.text("other"), None);
    }
}
