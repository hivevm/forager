// The selection of a feature's class and the binding of its style's properties follow
// msShapeGetClass, msEvalExpression, bindStyle, and bindColorAttribute of src/maputil.c, the layer's
// filter as msLayerNextShape applies it and the attributes as msLayerWhichItems finds them in
// src/maplayer.c, of the release pinned in scripts/fetch-msautotest.sh.
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
    Bound, Diagnostic, Expr, Layer, LineOffset, Rgba, Span, Style, Symbol, SymbolRef,
};

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

/// Whether `feature` holds a property of the name, null or not, without regard to case.
fn holds(properties: &Properties, name: &str) -> bool {
    if properties.get(name).is_some() {
        return true;
    }
    let mut found = false;
    properties.each(|key, _| found |= key.eq_ignore_ascii_case(name));
    found
}

/// The properties of a tree.
fn expr_names(expr: &Expr, names: &mut BTreeSet<String>) {
    match expr {
        Expr::Property { property } => {
            names.insert(property.clone());
        }
        Expr::Call { args: items, .. } | Expr::List(items) => {
            for item in items {
                expr_names(item, names);
            }
        }
        _ => {}
    }
}

fn bound_names<T>(bound: &Bound<T>, names: &mut BTreeSet<String>) {
    match bound {
        Bound::Binding { binding } => {
            names.insert(binding.clone());
        }
        Bound::Expr(expr) => expr_names(expr, names),
        Bound::Value(_) => {}
    }
}

/// The properties the engine reads of a layer's features: its filter's, its classes'
/// expressions', and its styles' bound and computed properties'. msLayerWhichItems asks the
/// source for each, labels' too, which are read with the labels.
fn layer_names(layer: &Layer) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    if let Some(filter) = &layer.filter {
        expr_names(filter, &mut names);
    }
    for class in &layer.classes {
        if let Some(expression) = &class.expression {
            expr_names(expression, &mut names);
        }
        for style in &class.styles {
            for color in [&style.color, &style.outline_color].into_iter().flatten() {
                bound_names(color, &mut names);
            }
            let numbers = [
                &style.width,
                &style.size,
                &style.angle,
                &style.opacity,
                &style.outline_width,
            ];
            for number in numbers.into_iter().flatten() {
                bound_names(number, &mut names);
            }
            for pair in [&style.offset, &style.polar_offset].into_iter().flatten() {
                pair.iter().for_each(|b| bound_names(b, &mut names));
            }
            if let Some(line) = &style.line_offset {
                bound_names(&line.distance, &mut names);
            }
            if let Some(symbol) = &style.symbol {
                bound_names(symbol, &mut names);
            }
        }
    }
    names
}

/// The first property the layer reads that none of its features holds. MapServer asks the source
/// for each attribute it reads before it draws, and a source without one fails the layer.
pub(crate) fn missing_property(layer: &Layer, features: &[Feature]) -> Option<String> {
    if features.is_empty() {
        return None;
    }
    layer_names(layer)
        .into_iter()
        .find(|name| !features.iter().any(|f| holds(&f.properties, name)))
}

/// What carries an expression, for a report.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Carrier {
    Filter,
    Class(usize),
}

/// The patterns a render cannot match as MapServer does, each reported once per layer and
/// carrier.
#[derive(Default)]
pub(crate) struct Reports {
    reported: BTreeSet<(usize, Carrier)>,
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
        let Failure::Pattern { pattern, construct } = failure else {
            return;
        };
        if !self.reported.insert((layer_index, carrier)) {
            return;
        }
        let (what, span): (String, Option<Span>) = match carrier {
            Carrier::Filter => ("its filter".to_owned(), layer.span.clone()),
            Carrier::Class(i) => {
                let class = &layer.classes[i];
                let name = class
                    .name
                    .as_deref()
                    .map_or(String::new(), |n| format!(" `{n}`"));
                (
                    format!("its class {}{name}", i + 1),
                    class.span.clone().or_else(|| layer.span.clone()),
                )
            }
        };
        let message = format!(
            "layer {}: the pattern `{pattern}` of {what} uses {construct}, which Forager cannot \
             match as MapServer does; the expression that holds it never matches",
            crate::name(layer)
        );
        diagnostics.push(Diagnostic::warning("pattern-unsupported", message, span));
    }
}

/// The selection of one layer's classes for its features.
pub(crate) struct Selector<'a, 'm> {
    pub layer: &'a Layer,
    pub layer_index: usize,
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
    /// whose expression matches it, a class without one matching every feature; none where no
    /// class matches.
    pub fn class(&mut self, feature: &Feature) -> Option<usize> {
        if let Some(filter) = &self.layer.filter
            && !self.matches(filter, feature, Carrier::Filter)
        {
            return None;
        }
        (0..self.layer.classes.len()).find(|&i| match &self.layer.classes[i].expression {
            None => true,
            Some(expression) => self.matches(expression, feature, Carrier::Class(i)),
        })
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
                .map(|style| self.bind(style, feature, symbols))
                .collect();
            selected.push((feature, index, styles));
        }
        selected
    }

    /// `style` with each property bound to a property or computed by an expression given its
    /// value for `feature`, as bindStyle binds it; the style itself where none is.
    pub fn bind<'s>(
        &mut self,
        style: &'s Style,
        feature: &Feature,
        symbols: &[Symbol],
    ) -> Cow<'s, Style> {
        if !varies(style) {
            return Cow::Borrowed(style);
        }
        let properties = FeatureProperties(&feature.properties);
        let mut bind = Binder {
            properties: &properties,
            evaluator: self.evaluator,
        };
        let mut bound = style.clone();
        bound.color = bind.color(&style.color);
        bound.outline_color = bind.color(&style.outline_color);
        bound.angle = bind.number(&style.angle, 360.0);
        bound.size = bind.number(&style.size, 1.0);
        bound.width = bind.number(&style.width, 1.0);
        bound.outline_width = bind.number(&style.outline_width, 1.0);
        bound.offset = style.offset.as_ref().map(|[x, y]| {
            [
                bind.number(&Some(x.clone()), 0.0).expect("a value"),
                bind.number(&Some(y.clone()), 0.0).expect("a value"),
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
        Cow::Owned(bound)
    }
}

/// Whether a property the engine draws varies by feature.
fn varies(style: &Style) -> bool {
    fn bound<T>(property: &Option<Bound<T>>) -> bool {
        property
            .as_ref()
            .is_some_and(|p| !matches!(p, Bound::Value(_)))
    }
    bound(&style.color)
        || bound(&style.outline_color)
        || bound(&style.angle)
        || bound(&style.size)
        || bound(&style.width)
        || bound(&style.outline_width)
        || style
            .offset
            .as_ref()
            .is_some_and(|pair| pair.iter().any(|p| !matches!(p, Bound::Value(_))))
        || style
            .line_offset
            .as_ref()
            .is_some_and(|line| !matches!(line.distance, Bound::Value(_)))
        || matches!(style.symbol, Some(Bound::Binding { .. }))
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
}

/// A C string's bytes: up to the first NUL.
fn c_bytes(text: &str) -> &[u8] {
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
            Bound::Expr(expr) => self.evaluator.number(expr, self.properties).unwrap_or(0.0),
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
            Bound::Expr(expr) => self.evaluator.text(expr, self.properties).ok()?,
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
fn color(text: &[u8]) -> Option<Rgba> {
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
        assert!(holds(&properties, "NONE"));
        assert!(!holds(&properties, "other"));
    }
}
