//! Translates the syntax tree of a mapfile into the style IR. What the IR cannot hold yet is
//! reported, never silently dropped.

use std::collections::BTreeMap;

use forager_style_ir::{
    Class, Diagnostic, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Span, Style,
};

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

pub fn translate(map: &Block, diagnostics: &mut Vec<Diagnostic>) -> Map {
    let mut t = Translator { diagnostics };
    t.map(map)
}

struct Translator<'a> {
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
                    _ => self.unhandled(&d.keyword, &d.span),
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
                    "CONNECTION" | "DATA" => {
                        if let Some(value) = self.string(d) {
                            props.insert(d.keyword.to_ascii_lowercase(), value);
                            source_span.get_or_insert_with(|| d.span.clone());
                        }
                    }
                    _ => self.unhandled(&d.keyword, &d.span),
                },
                Node::Block(b) => match b.keyword.as_str() {
                    "CLASS" => layer.classes.push(self.class(b)),
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

    fn class(&mut self, block: &Block) -> Class {
        let mut class = Class {
            span: Some(block.span.clone()),
            ..Class::default()
        };
        for node in &block.nodes {
            match node {
                Node::Directive(d) if d.keyword == "NAME" => class.name = self.string(d),
                Node::Block(b) if b.keyword == "STYLE" => class.styles.push(self.style(b)),
                Node::Directive(Directive { keyword, span, .. })
                | Node::Block(Block { keyword, span, .. }) => self.unhandled(keyword, span),
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
                    "COLOR" => style.color = self.color(d),
                    "OUTLINECOLOR" => style.outline_color = self.color(d),
                    "WIDTH" => style.width = self.numbers(d, 1).map(|n| n[0]),
                    "SIZE" => style.size = self.numbers(d, 1).map(|n| n[0]),
                    _ => self.unhandled(&d.keyword, &d.span),
                },
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        style
    }

    /// A proj4 string from the parameters of a PROJECTION block. `init=` needs a resolver the
    /// host injects, which the frontend does not have yet, so it is a load error.
    fn projection(&mut self, block: &Block) -> Option<String> {
        let mut params = Vec::new();
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
                if param.to_ascii_lowercase().starts_with("init=") {
                    let message =
                        format!("`{param}` needs a projection resolver, and none is given");
                    self.error("projection-unresolved", &message, &token.span);
                    return None;
                }
                params.push(format!("+{param}"));
            }
        }
        (!params.is_empty()).then(|| params.join(" "))
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
            [
                Token {
                    kind: TokenKind::Binding(_),
                    ..
                },
            ] => {
                let message = "a colour bound to an attribute is not yet supported";
                self.warn("unsupported-value", message, &d.span);
                None
            }
            _ => {
                let rgb = self.numbers(d, 3)?;
                if rgb.iter().all(|&c| c == -1.0) {
                    return None;
                }
                if rgb
                    .iter()
                    .any(|&c| !(0.0..=255.0).contains(&c) || c.fract() != 0.0)
                {
                    self.invalid(d, "three integers from 0 to 255");
                    return None;
                }
                Some(Rgba::opaque(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8))
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
