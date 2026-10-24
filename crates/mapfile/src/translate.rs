//! Translates the syntax tree of a mapfile into the style IR. What the IR cannot hold yet is
//! reported, never silently dropped.

use std::collections::BTreeMap;

use forager_ports::ProjectionResolver;
use forager_style_ir::{
    Bound, Class, Diagnostic, Expr, Extent, GeomTransform, Layer, LayerStatus, LineCap, LineJoin,
    LineOffset, Map, RenderMode, Rgba, SourceRef, Span, Style, SymbolRef, Units,
};

use crate::expression;
use crate::lexer::{Token, TokenKind};
use crate::syntax::{Block, Directive, Node};

/// Keywords out of Forager's scope (specification, Non-Goals): read and discarded.
const DISCARDED: &[&str] = &[
    "FOOTER",
    "HEADER",
    "IDENTIFY",
    "JOIN",
    "QUERYMAP",
    "TEMPLATE",
    "TOLERANCE",
    "TOLERANCEUNITS",
    "WEB",
];

/// The string value of the last `keyword` directive of a block, the one MapServer keeps, read
/// without a diagnostic; the directive reports its own when the block's loop reaches it.
fn item(block: &Block, keyword: &str) -> Option<String> {
    block.nodes.iter().rev().find_map(|node| match node {
        Node::Directive(d) if d.keyword == keyword => match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Str { value, .. } | TokenKind::Word(value),
                    ..
                },
            ] => Some(value.clone()),
            _ => None,
        },
        _ => None,
    })
}

pub fn translate(
    map: &Block,
    resolver: Option<&dyn ProjectionResolver>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Map {
    let mut t = Translator {
        resolver,
        diagnostics,
    };
    t.map(map)
}

struct Translator<'a> {
    resolver: Option<&'a dyn ProjectionResolver>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl Translator<'_> {
    fn map(&mut self, block: &Block) -> Map {
        let mut map = Map {
            span: Some(block.span.clone()),
            ..Map::default()
        };
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "NAME" => map.name = self.string(d),
                    "EXTENT" => map.extent = self.extent(d),
                    "SIZE" => {
                        map.size = self
                            .numbers(d, 2)
                            .map(|n| [n[0].max(0.0) as u32, n[1].max(0.0) as u32]);
                    }
                    "IMAGECOLOR" => map.image_color = self.color(d),
                    // MS_RESOLUTION_MIN and MS_RESOLUTION_MAX.
                    "RESOLUTION" => map.resolution = self.number(d, Range::within(10.0, 1000.0)),
                    "DEFRESOLUTION" => {
                        map.def_resolution = self.number(d, Range::within(10.0, 1000.0));
                    }
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) => match b.keyword.as_str() {
                    "LAYER" => {
                        if let Some(layer) = self.layer(b) {
                            map.layers.push(layer);
                        }
                    }
                    "PROJECTION" => map.projection = self.projection(b),
                    _ => self.unhandled(&b.keyword, &b.span),
                },
            }
        }
        map
    }

    fn layer(&mut self, block: &Block) -> Option<Layer> {
        let mut layer = Layer {
            span: Some(block.span.clone()),
            ..Layer::default()
        };
        let mut connection_type = None;
        let mut props = BTreeMap::new();
        let mut source_span = None;
        // MapServer reads the items when it evaluates, so they hold wherever they stand.
        let class_item = item(block, "CLASSITEM");
        let filter_item = item(block, "FILTERITEM");
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "NAME" => layer.name = self.string(d),
                    "TYPE" => match self.word(d).as_deref() {
                        Some("POINT") => layer.render_mode = Some(RenderMode::Point),
                        Some("LINE") => layer.render_mode = Some(RenderMode::Line),
                        Some("POLYGON" | "CIRCLE") => layer.render_mode = Some(RenderMode::Polygon),
                        Some("RASTER") => layer.render_mode = Some(RenderMode::Raster),
                        Some("ANNOTATION") => layer.render_mode = Some(RenderMode::Annotation),
                        Some("QUERY") => {
                            self.warn("discarded-layer", "a QUERY layer is discarded", &d.span);
                            return None;
                        }
                        Some(other) => {
                            let message = format!("layer type {other} is not yet supported");
                            self.warn("unsupported-value", &message, &d.span);
                        }
                        None => {}
                    },
                    "STATUS" => match self.word(d).as_deref() {
                        Some("ON") => layer.status = Some(LayerStatus::On),
                        Some("OFF") => layer.status = Some(LayerStatus::Off),
                        Some("DEFAULT") => layer.status = Some(LayerStatus::Default),
                        Some(_) => self.invalid(d, "ON, OFF, or DEFAULT"),
                        None => {}
                    },
                    "CONNECTIONTYPE" => {
                        connection_type = self.word(d).map(|w| w.to_ascii_lowercase());
                        source_span.get_or_insert_with(|| d.span.clone());
                    }
                    "CLASSITEM" | "FILTERITEM" => {
                        self.string(d);
                    }
                    "FILTER" => layer.filter = self.expression(d, filter_item.as_deref()),
                    "SIZEUNITS" => {
                        layer.size_units = self.choice(
                            d,
                            &[
                                ("PIXELS", Units::Pixels),
                                ("INCHES", Units::Inches),
                                ("FEET", Units::Feet),
                                ("MILES", Units::Miles),
                                ("METERS", Units::Meters),
                                ("KILOMETERS", Units::Kilometers),
                                ("NAUTICALMILES", Units::NauticalMiles),
                                ("DD", Units::Degrees),
                            ],
                        );
                    }
                    "CONNECTION" | "DATA" => {
                        if let Some(value) = self.string(d) {
                            props.insert(d.keyword.to_ascii_lowercase(), value);
                            source_span.get_or_insert_with(|| d.span.clone());
                        }
                    }
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) => match b.keyword.as_str() {
                    "CLASS" => layer.classes.push(self.class(b, class_item.as_deref())),
                    _ => self.unhandled(&b.keyword, &b.span),
                },
            }
        }
        if let Some(span) = source_span {
            layer.source = Some(SourceRef {
                // Without CONNECTIONTYPE, MapServer reads DATA as a local shapefile.
                kind: connection_type.unwrap_or_else(|| "local".to_owned()),
                props,
                span: Some(span),
            });
        }
        Some(layer)
    }

    fn class(&mut self, block: &Block, class_item: Option<&str>) -> Class {
        let mut class = Class {
            span: Some(block.span.clone()),
            ..Class::default()
        };
        for node in &block.nodes {
            match node {
                Node::Directive(d) if d.keyword == "NAME" => class.name = self.string(d),
                Node::Directive(d) if d.keyword == "EXPRESSION" => {
                    class.expression = self.expression(d, class_item);
                }
                Node::Block(b) if b.keyword == "STYLE" => class.styles.push(self.style(b)),
                Node::Directive(d) => self.unhandled_directive(d),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        class
    }

    fn style(&mut self, block: &Block) -> Style {
        let mut style = Style {
            span: Some(block.span.clone()),
            ..Style::default()
        };
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "COLOR" => style.color = self.bound_color(d),
                    "OUTLINECOLOR" => style.outline_color = self.bound_color(d),
                    // The ranges are those loadStyle checks.
                    "WIDTH" => {
                        let range = Range::at_least(0.0);
                        style.width = self.bound_number(d, range, false);
                    }
                    "SIZE" => style.size = self.bound_number(d, Range::above(0.0), true),
                    "ANGLE" if d.values.len() == 1 && d.values[0].is_word("AUTO") => {
                        style.auto_angle = true;
                    }
                    "ANGLE" => {
                        let range = Range::within(-360.0, 360.0);
                        style.angle = self.bound_number(d, range, false);
                    }
                    "SYMBOL" => style.symbol = self.symbol(d),
                    "OFFSET" => self.offset(d, &mut style),
                    "POLAROFFSET" => {
                        style.polar_offset = self.bound_pair(d, Range::any());
                    }
                    "GAP" => style.gap = self.number(d, Range::any()),
                    "INITIALGAP" => style.initial_gap = self.number(d, Range::at_least(0.0)),
                    "LINECAP" => {
                        style.line_cap = self.choice(
                            d,
                            &[
                                ("BUTT", LineCap::Butt),
                                ("ROUND", LineCap::Round),
                                ("SQUARE", LineCap::Square),
                                ("TRIANGLE", LineCap::Triangle),
                            ],
                        );
                    }
                    "LINEJOIN" => {
                        style.line_join = self.choice(
                            d,
                            &[
                                ("NONE", LineJoin::None),
                                ("ROUND", LineJoin::Round),
                                ("MITER", LineJoin::Miter),
                                ("BEVEL", LineJoin::Bevel),
                            ],
                        );
                    }
                    "LINEJOINMAXSIZE" => {
                        style.line_join_max_size = self.number(d, Range::above(0.0));
                    }
                    // loadStyle truncates an opacity and clamps it to 0..100.
                    "OPACITY" => {
                        style.opacity =
                            self.bound_number(d, Range::any(), false).map(|o| match o {
                                Bound::Value(n) => Bound::Value(n.trunc().clamp(0.0, 100.0)),
                                bound => bound,
                            });
                    }
                    "OUTLINEWIDTH" => {
                        style.outline_width = self.bound_number(d, Range::at_least(0.0), false);
                    }
                    "MINSIZE" => style.min_size = self.number(d, Range::at_least(0.0)),
                    "MAXSIZE" => style.max_size = self.number(d, Range::above(0.0)),
                    "MINWIDTH" => style.min_width = self.number(d, Range::at_least(0.0)),
                    "MAXWIDTH" => style.max_width = self.number(d, Range::above(0.0)),
                    "GEOMTRANSFORM" => style.geom_transform = self.geom_transform(d),
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) if b.keyword == "PATTERN" => style.pattern = self.pattern(b),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        style
    }

    /// `SYMBOL` by its index, at least 0, by its name, or bound to a property. The name is
    /// resolved against the symbol set when symbols are carried over.
    fn symbol(&mut self, d: &Directive) -> Option<Bound<SymbolRef>> {
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Number(n),
                    ..
                },
            ] if *n >= 0.0 && *n <= f64::from(u32::MAX) => {
                Some(Bound::Value(SymbolRef::Index(*n as u32)))
            }
            [
                Token {
                    kind: TokenKind::Str { value, .. } | TokenKind::Word(value),
                    ..
                },
            ] => Some(Bound::Value(SymbolRef::Name(value.clone()))),
            [
                Token {
                    kind: TokenKind::Binding(name),
                    ..
                },
            ] => Some(Bound::Binding {
                binding: name.clone(),
            }),
            _ => {
                self.invalid(
                    d,
                    "a symbol index of at least 0, a symbol name, or an attribute",
                );
                None
            }
        }
    }

    /// `OFFSET x y`. MapServer writes a line drawn parallel to the feature's as a y of -99, or
    /// -999 for both sides (`MS_STYLE_SINGLE_SIDED_OFFSET`, `MS_STYLE_DOUBLE_SIDED_OFFSET`);
    /// the IR states it as a line offset.
    fn offset(&mut self, d: &Directive, style: &mut Style) {
        let Some([x, y]) = self.bound_pair(d, Range::any()) else {
            return;
        };
        match y {
            Bound::Value(n) if n == -99.0 || n == -999.0 => {
                style.line_offset = Some(LineOffset {
                    distance: x,
                    both_sides: n == -999.0,
                });
            }
            y => style.offset = Some([x, y]),
        }
    }

    /// Two numbers inside `range`, each of which may be bound to a property.
    fn bound_pair(&mut self, d: &Directive, range: Range) -> Option<[Bound<f64>; 2]> {
        let bound = |token: &Token| match &token.kind {
            TokenKind::Number(n) if range.contains(*n) => Some(Bound::Value(*n)),
            TokenKind::Binding(name) => Some(Bound::Binding {
                binding: name.clone(),
            }),
            _ => None,
        };
        match d.values.as_slice() {
            [a, b] if bound(a).is_some() && bound(b).is_some() => Some([bound(a)?, bound(b)?]),
            _ => {
                self.invalid(d, &format!("two numbers{range} or attributes"));
                None
            }
        }
    }

    /// One number inside `range`.
    fn number(&mut self, d: &Directive, range: Range) -> Option<f64> {
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Number(n),
                    ..
                },
            ] if range.contains(*n) => Some(*n),
            _ => {
                self.invalid(d, &format!("a number{range}"));
                None
            }
        }
    }

    /// One of the keywords `choices` names.
    fn choice<T: Copy>(&mut self, d: &Directive, choices: &[(&str, T)]) -> Option<T> {
        let word = self.word(d)?;
        let found = choices
            .iter()
            .find(|(name, _)| *name == word)
            .map(|(_, v)| *v);
        if found.is_none() {
            let names: Vec<&str> = choices.iter().map(|(name, _)| *name).collect();
            self.invalid(d, &names.join(", "));
        }
        found
    }

    /// `PATTERN` with 2 to 10 numbers, as loadStyle takes it.
    fn pattern(&mut self, block: &Block) -> Vec<f64> {
        let numbers: Option<Vec<f64>> = block
            .values
            .iter()
            .map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        match numbers {
            Some(numbers) if (2..=10).contains(&numbers.len()) => numbers,
            _ => {
                self.error(
                    "invalid-value",
                    "PATTERN expects 2 to 10 numbers",
                    &block.span,
                );
                Vec::new()
            }
        }
    }

    /// `GEOMTRANSFORM` by name; MapServer's other transforms and its expressions are not yet
    /// carried over.
    fn geom_transform(&mut self, d: &Directive) -> Option<GeomTransform> {
        let name = match d.values.as_slice() {
            // An unquoted word is a string to MapServer's lexer.
            [
                Token {
                    kind: TokenKind::Str { value, .. } | TokenKind::Word(value),
                    ..
                },
            ] => value.to_ascii_lowercase(),
            [
                Token {
                    kind: TokenKind::Expression(_),
                    ..
                },
            ] => String::new(),
            _ => {
                self.invalid(d, "a transform name or an expression");
                return None;
            }
        };
        let transform = match name.as_str() {
            "start" => Some(GeomTransform::Start),
            "end" => Some(GeomTransform::End),
            "vertices" => Some(GeomTransform::Vertices),
            "centroid" => Some(GeomTransform::Centroid),
            _ => None,
        };
        if transform.is_none() {
            let message = "this GEOMTRANSFORM is not yet supported, and the style is drawn as \
                           if it had none";
            self.warn("unsupported-value", message, &d.span);
        }
        transform
    }

    /// A proj4 string from the parameters of a PROJECTION block. `init=<authority>:<code>` is
    /// resolved by the resolver the host passes in (ADR-0018); without one, or for a code it does
    /// not know, it is a load error.
    ///
    /// As MapServer hands every parameter to PROJ, `init=` is matched case-sensitively, there is
    /// at most one, and its expansion follows the explicit parameters, which PROJ lets take
    /// precedence (its `pj_expand_init`).
    fn projection(&mut self, block: &Block) -> Option<String> {
        let mut params = Vec::new();
        let mut expansion: Option<Vec<String>> = None;
        for token in &block.values {
            let text = match &token.kind {
                TokenKind::Str { value, .. } | TokenKind::Word(value) => value.trim(),
                _ => {
                    self.error(
                        "invalid-value",
                        "a projection parameter is a string",
                        &token.span,
                    );
                    return None;
                }
            };
            if text.eq_ignore_ascii_case("AUTO") {
                self.warn(
                    "unsupported-value",
                    "PROJECTION AUTO is not yet supported",
                    &token.span,
                );
                return None;
            }
            for param in text.split_whitespace() {
                let param = param.trim_start_matches('+');
                if let Some(reference) = param.strip_prefix("init=") {
                    if expansion.is_some() {
                        let message = "a PROJECTION has at most one init=";
                        self.error("invalid-value", message, &token.span);
                        return None;
                    }
                    let resolved = reference
                        .split_once(':')
                        .and_then(|(authority, code)| self.resolver?.resolve(authority, code));
                    let Some(resolved) = resolved else {
                        let message = match self.resolver {
                            Some(_) => format!("`{param}` is not a projection the resolver knows"),
                            None => {
                                format!("`{param}` needs a projection resolver, and none is given")
                            }
                        };
                        self.error("projection-unresolved", &message, &token.span);
                        return None;
                    };
                    expansion = Some(
                        resolved
                            .split_whitespace()
                            .map(|p| format!("+{}", p.trim_start_matches('+')))
                            .collect(),
                    );
                    continue;
                }
                params.push(format!("+{param}"));
            }
        }
        params.extend(expansion.unwrap_or_default());
        (!params.is_empty()).then(|| params.join(" "))
    }

    /// A directive not carried over, unless the parser has already warned that it is unknown.
    fn unhandled_directive(&mut self, d: &Directive) {
        if d.known {
            self.unhandled(&d.keyword, &d.span);
        }
    }

    fn unhandled(&mut self, keyword: &str, span: &Span) {
        if DISCARDED.contains(&keyword) {
            let message = format!("{keyword} is out of Forager's scope and is discarded");
            self.warn("discarded-keyword", &message, span);
        } else {
            let message = format!("{keyword} is not yet supported and is ignored");
            self.warn("unsupported-keyword", &message, span);
        }
    }

    fn string(&mut self, d: &Directive) -> Option<String> {
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Str { value, .. } | TokenKind::Word(value),
                    ..
                },
            ] => Some(value.clone()),
            _ => {
                self.invalid(d, "one string");
                None
            }
        }
    }

    /// A bare word value in upper case.
    fn word(&mut self, d: &Directive) -> Option<String> {
        match d.values.as_slice() {
            [token] if token.keyword().is_some() => token.keyword(),
            _ => {
                self.invalid(d, "one keyword");
                None
            }
        }
    }

    fn numbers(&mut self, d: &Directive, count: usize) -> Option<Vec<f64>> {
        let numbers: Vec<f64> = d
            .values
            .iter()
            .filter_map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        if numbers.len() == count && d.values.len() == count {
            Some(numbers)
        } else {
            self.invalid(d, &format!("{count} number(s)"));
            None
        }
    }

    fn extent(&mut self, d: &Directive) -> Option<Extent> {
        self.numbers(d, 4).map(|n| Extent {
            min_x: n[0],
            min_y: n[1],
            max_x: n[2],
            max_y: n[3],
        })
    }

    /// `r g b` with each in 0..=255, `-1 -1 -1` for no colour, or `"#rrggbb[aa]"`.
    /// A colour as `loadColor` reads it: `#rrggbb`, `#rrggbbaa`, or three numbers from -255 to
    /// 255, each truncated to an integer. A channel of -1 means no colour; MapServer's renderers
    /// store any other channel in a byte, so a negative one is taken modulo 256.
    fn color(&mut self, d: &Directive) -> Option<Rgba> {
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Str { value, .. },
                    ..
                },
            ] => Rgba::from_hex(value).or_else(|| {
                self.invalid(d, "a colour as `#rrggbb` or `#rrggbbaa`");
                None
            }),
            _ => {
                let rgb = self.numbers(d, 3)?;
                if rgb.iter().any(|c| !(-255.0..=255.0).contains(c)) {
                    self.invalid(d, "three numbers from -255 to 255");
                    return None;
                }
                let rgb: Vec<i32> = rgb.iter().map(|c| c.trunc() as i32).collect();
                if rgb.contains(&-1) {
                    return None;
                }
                let byte = |c: i32| c.rem_euclid(256) as u8;
                Some(Rgba::opaque(byte(rgb[0]), byte(rgb[1]), byte(rgb[2])))
            }
        }
    }

    /// A style's colour, which may also be bound to a property.
    fn bound_color(&mut self, d: &Directive) -> Option<Bound<Rgba>> {
        if let [
            Token {
                kind: TokenKind::Binding(name),
                ..
            },
        ] = d.values.as_slice()
        {
            return Some(Bound::Binding {
                binding: name.clone(),
            });
        }
        self.color(d).map(Bound::Value)
    }

    /// A style's number inside `range`, which may also be bound to a property, and, where
    /// `expression` allows, be an expression. An expression that cannot be carried over is a
    /// warning and the number 0, what MapServer's evaluation yields for one it cannot parse.
    fn bound_number(
        &mut self,
        d: &Directive,
        range: Range,
        expression: bool,
    ) -> Option<Bound<f64>> {
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Number(n),
                    ..
                },
            ] if range.contains(*n) => Some(Bound::Value(*n)),
            [
                Token {
                    kind: TokenKind::Binding(name),
                    ..
                },
            ] => Some(Bound::Binding {
                binding: name.clone(),
            }),
            [
                Token {
                    kind: TokenKind::Expression(text),
                    ..
                },
            ] if expression => match expression::number_value(text) {
                Ok(expr) => Some(Bound::Expr(expr)),
                Err(problem) if problem.error => {
                    self.error(problem.code, &problem.message, &d.span);
                    None
                }
                Err(problem) => {
                    let message = format!("{}; the {} is 0", problem.message, d.keyword);
                    self.warn(problem.code, &message, &d.span);
                    Some(Bound::Value(0.0))
                }
            },
            _ => {
                let what = if expression {
                    ", an attribute, or an expression"
                } else {
                    " or an attribute"
                };
                self.invalid(d, &format!("a number{range}{what}"));
                None
            }
        }
    }
    fn expression(&mut self, d: &Directive, item: Option<&str>) -> Option<Expr> {
        let [token] = d.values.as_slice() else {
            self.invalid(d, "one expression");
            return None;
        };
        match expression::translate(token, item) {
            Ok(expr) => Some(expr),
            Err(problem) if problem.error => {
                self.error(problem.code, &problem.message, &d.span);
                None
            }
            Err(problem) => {
                let message = format!("{}; the {} never matches", problem.message, d.keyword);
                self.warn(problem.code, &message, &d.span);
                Some(Expr::Bool(false))
            }
        }
    }

    fn invalid(&mut self, d: &Directive, expected: &str) {
        let message = format!("{} expects {expected}", d.keyword);
        self.error("invalid-value", &message, &d.span);
    }

    fn warn(&mut self, code: &str, message: &str, span: &Span) {
        self.diagnostics
            .push(Diagnostic::warning(code, message, Some(span.clone())));
    }

    fn error(&mut self, code: &str, message: &str, span: &Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, Some(span.clone())));
    }
}

/// The values a number may take, as MapServer's `msCheckNumber` checks them.
#[derive(Clone, Copy)]
enum Range {
    AtLeast(f64),
    Above(f64),
    Within(f64, f64),
}

impl Range {
    fn any() -> Self {
        Range::Within(f64::MIN, f64::MAX)
    }

    fn at_least(min: f64) -> Self {
        Range::AtLeast(min)
    }

    fn above(min: f64) -> Self {
        Range::Above(min)
    }

    fn within(min: f64, max: f64) -> Self {
        Range::Within(min, max)
    }

    fn contains(self, n: f64) -> bool {
        match self {
            Range::AtLeast(min) => n >= min,
            Range::Above(min) => n > min,
            Range::Within(min, max) => (min..=max).contains(&n),
        }
    }
}

impl std::fmt::Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Range::AtLeast(min) => write!(f, " of at least {min}"),
            Range::Above(min) => write!(f, " greater than {min}"),
            Range::Within(min, max) if *min == f64::MIN && *max == f64::MAX => Ok(()),
            Range::Within(min, max) => write!(f, " from {min} to {max}"),
        }
    }
}
