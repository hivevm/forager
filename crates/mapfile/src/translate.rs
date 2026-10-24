//! Translates the syntax tree of a mapfile into the style IR. What the IR cannot hold yet is
//! reported, never silently dropped.

use std::collections::BTreeMap;

use forager_ports::{ProjectionResolver, ResourcePath};
use forager_style_ir::{
    Align, AngleMode, Bound, Class, Diagnostic, Expr, Extent, Force, GeomTransform, Label, Layer,
    LayerProjection, LayerStatus, LineCap, LineJoin, LineOffset, Map, MinFeatureSize, Op, Position,
    RenderMode, Rgba, SourceRef, Span, Style, Symbol, SymbolKind, SymbolRef, Units,
};

use crate::expression;
use crate::init_file::InitFile;
use crate::lexer::{Token, TokenKind};
use crate::report::Report;
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

/// The files a map was read from: the main mapfile, and the symbol file and font set it names,
/// each with the path it was read from.
pub struct Files<'a> {
    pub main: &'a ResourcePath,
    pub symbol_set: Option<&'a (ResourcePath, Block)>,
    pub font_set: Option<&'a (ResourcePath, String)>,
    /// The init files the projections name by a path, by the name as written (ADR-0023).
    pub init_files: &'a BTreeMap<String, Result<InitFile, String>>,
}

/// Translates the `MAP` block of a mapfile read from `files`.
pub fn translate(
    map: &Block,
    files: &Files<'_>,
    resolver: Option<&dyn ProjectionResolver>,
    diagnostics: &mut Report,
) -> Map {
    let main = files.main;
    let symbol_set = files.symbol_set;
    let mut t = Translator {
        resolver,
        init_files: files.init_files,
        diagnostics,
        symbols: Vec::new(),
        fonts: BTreeMap::new(),
    };
    // MapServer appends the symbols of the map's SYMBOL blocks as it parses them, and those of
    // its symbol set after the whole map (loadMap, loadSymbolSet).
    let inline = map.nodes.iter().filter_map(|node| match node {
        Node::Block(b) if b.keyword == "SYMBOL" => Some((main, b)),
        _ => None,
    });
    let from_set = symbol_set.into_iter().flat_map(|(path, set)| {
        set.nodes.iter().filter_map(move |node| match node {
            Node::Block(b) if b.keyword == "SYMBOL" => Some((path, b)),
            _ => None,
        })
    });
    let blocks: Vec<_> = inline.chain(from_set).collect();
    t.symbols = blocks
        .into_iter()
        .filter_map(|(beside, b)| t.symbol_block(b, beside))
        .collect();
    if let Some((path, text)) = files.font_set {
        t.fonts = t.fonts(path, text);
    }
    let mut map = t.map(map);
    map.symbols = std::mem::take(&mut t.symbols);
    map.fonts = std::mem::take(&mut t.fonts);
    map
}

struct Translator<'a> {
    resolver: Option<&'a dyn ProjectionResolver>,
    init_files: &'a BTreeMap<String, Result<InitFile, String>>,
    diagnostics: &'a mut Report,
    /// The map's symbols, read before its layers so that a style's reference resolves.
    symbols: Vec<Symbol>,
    /// The map's fonts, read before its layers so that a label's font is checked.
    fonts: BTreeMap<String, String>,
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
                    "UNITS" => map.units = self.choice(d, &UNITS[1..]),
                    // Read with the map, before translation.
                    "SYMBOLSET" | "FONTSET" => {}
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) => match b.keyword.as_str() {
                    "SYMBOL" => {}
                    "LAYER" => {
                        if let Some(layer) = self.layer(b) {
                            map.layers.push(layer);
                        }
                    }
                    "PROJECTION" => {
                        map.projection = match self.projection(b) {
                            Some(LayerProjection::Proj4(proj4)) => Some(proj4),
                            Some(LayerProjection::FromSource) => {
                                let message = "PROJECTION AUTO of a map is not yet supported";
                                self.warn("unsupported-value", message, &b.span);
                                None
                            }
                            None => None,
                        };
                    }
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
        let label_item = item(block, "LABELITEM");
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
                    "STATUS" => {
                        layer.status = self.choice(
                            d,
                            &[
                                ("ON", LayerStatus::On),
                                ("OFF", LayerStatus::Off),
                                ("DEFAULT", LayerStatus::Default),
                            ],
                        );
                    }
                    "CONNECTIONTYPE" => {
                        connection_type = self.word(d).map(|w| w.to_ascii_lowercase());
                        source_span.get_or_insert_with(|| d.span.clone());
                    }
                    "CLASSITEM" | "FILTERITEM" | "LABELITEM" => {
                        self.string(d);
                    }
                    "FILTER" => layer.filter = self.expression(d, filter_item.as_deref()),
                    "SIZEUNITS" => layer.size_units = self.choice(d, UNITS),
                    "MINSCALE" | "MINSCALEDENOM" => layer.min_scale_denom = self.scale_denom(d),
                    "MAXSCALE" | "MAXSCALEDENOM" => layer.max_scale_denom = self.scale_denom(d),
                    "SYMBOLSCALE" | "SYMBOLSCALEDENOM" => {
                        layer.symbol_scale_denom = self.number(d, Range::at_least(1.0));
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
                    "CLASS" => {
                        let class = self.class(b, class_item.as_deref(), label_item.as_deref());
                        layer.classes.push(class);
                    }
                    "PROJECTION" => layer.projection = self.projection(b),
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

    fn class(
        &mut self,
        block: &Block,
        class_item: Option<&str>,
        label_item: Option<&str>,
    ) -> Class {
        let mut class = Class {
            span: Some(block.span.clone()),
            ..Class::default()
        };
        // A label's text falls back on the class's TEXT, then on the layer's LABELITEM, as
        // msShapeGetLabelAnnotation does; the class's TEXT holds wherever it stands.
        let class_text = block.nodes.iter().rev().find_map(|node| match node {
            Node::Directive(d) if d.keyword == "TEXT" => Some(d),
            _ => None,
        });
        let fallback = match class_text {
            Some(d) => self.text(d),
            None => label_item.map(|name| Expr::call(Op::ToString, vec![Expr::property(name)])),
        };
        for node in &block.nodes {
            match node {
                Node::Directive(d) if d.keyword == "NAME" => class.name = self.string(d),
                Node::Directive(d) if d.keyword == "EXPRESSION" => {
                    class.expression = self.expression(d, class_item);
                }
                Node::Directive(d) if d.keyword == "TEXT" => {}
                Node::Directive(d)
                    if matches!(d.keyword.as_str(), "MINSCALE" | "MINSCALEDENOM") =>
                {
                    class.min_scale_denom = self.scale_denom(d);
                }
                Node::Directive(d)
                    if matches!(d.keyword.as_str(), "MAXSCALE" | "MAXSCALEDENOM") =>
                {
                    class.max_scale_denom = self.scale_denom(d);
                }
                Node::Block(b) if b.keyword == "STYLE" => class.styles.push(self.style(b)),
                Node::Block(b) if b.keyword == "LABEL" => {
                    class.labels.push(self.label(b, fallback.as_ref()));
                }
                Node::Directive(d) => self.unhandled_directive(d),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        class
    }

    /// A label's or class's `TEXT`. One that cannot be carried over is a warning and an empty
    /// text, which draws no label, as MapServer draws none for a text it cannot evaluate.
    fn text(&mut self, d: &Directive) -> Option<Expr> {
        let [token] = d.values.as_slice() else {
            self.invalid(d, "one string or expression");
            return None;
        };
        match expression::text(token) {
            Ok(expr) => Some(expr),
            Err(problem) if problem.error => {
                self.error(problem.code, &problem.message, &d.span);
                None
            }
            Err(problem) => {
                let message = format!("{}; the label has no text", problem.message);
                self.warn(problem.code, &message, &d.span);
                Some(Expr::String(String::new()))
            }
        }
    }

    /// A `LABEL` as loadLabel reads it, its text falling back on `fallback`.
    fn label(&mut self, block: &Block, fallback: Option<&Expr>) -> Label {
        let mut label = Label {
            span: Some(block.span.clone()),
            ..Label::default()
        };
        let mut text = None;
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "TEXT" => text = self.text(d),
                    "FONT" => {
                        label.font = self.bound_string(d);
                        if let Some(Bound::Value(names)) = &label.font {
                            self.check_fonts(names, &d.span);
                        }
                    }
                    "SIZE" => label.size = self.label_size(d),
                    "MINSCALEDENOM" => label.min_scale_denom = self.scale_denom(d),
                    "MAXSCALEDENOM" => label.max_scale_denom = self.scale_denom(d),
                    "MINSIZE" => label.min_size = self.number(d, Range::above(0.0)).map(f64::trunc),
                    "MAXSIZE" => label.max_size = self.number(d, Range::above(0.0)).map(f64::trunc),
                    "COLOR" => label.color = self.bound_color(d),
                    "OUTLINECOLOR" => label.outline_color = self.bound_color(d),
                    "OUTLINEWIDTH" => {
                        label.outline_width = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "SHADOWCOLOR" => label.shadow_color = self.bound_color(d),
                    "SHADOWSIZE" => label.shadow_size = self.bound_pair(d, Range::any()),
                    "POSITION" => label.position = self.position(d),
                    "OFFSET" => label.offset = self.bound_pair(d, Range::any()),
                    "ANGLE" => self.label_angle(d, &mut label),
                    "ALIGN" => {
                        label.align = self.bound_choice(
                            d,
                            &[
                                ("LEFT", Align::Left),
                                ("CENTER", Align::Center),
                                ("RIGHT", Align::Right),
                            ],
                        );
                    }
                    "WRAP" => {
                        label.wrap = self
                            .string(d)
                            .and_then(|w| w.chars().next())
                            .map(String::from);
                    }
                    "MAXLENGTH" => {
                        label.max_length = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "BUFFER" => label.buffer = self.number(d, Range::any()).map(f64::trunc),
                    "MINDISTANCE" => {
                        label.min_distance = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "REPEATDISTANCE" => {
                        label.repeat_distance = self.number(d, Range::above(0.0)).map(f64::trunc);
                    }
                    "MINFEATURESIZE" if d.values.len() == 1 && d.values[0].is_word("AUTO") => {
                        label.min_feature_size = Some(MinFeatureSize::Auto);
                    }
                    "MINFEATURESIZE" => {
                        label.min_feature_size = self
                            .number(d, Range::above(0.0))
                            .map(|n| MinFeatureSize::Pixels(n.trunc()));
                    }
                    "MAXOVERLAPANGLE" => {
                        label.max_overlap_angle = self.number(d, Range::within(0.0, 360.0));
                    }
                    "PARTIALS" => {
                        label.partials = self.choice(d, &[("TRUE", true), ("FALSE", false)]);
                    }
                    "FORCE" => {
                        label.force = self.choice(
                            d,
                            &[
                                ("ON", Force::On),
                                ("OFF", Force::Off),
                                ("GROUP", Force::Group),
                            ],
                        );
                    }
                    "PRIORITY" => {
                        label.priority = self.bound_number(d, Range::within(1.0, 10.0), true);
                    }
                    // loadLabel reads and discards them.
                    "TYPE" | "ANTIALIAS" => {}
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) if b.keyword == "STYLE" => {
                    let mut style = self.style(b);
                    // loadLabel's default for a label's style.
                    style
                        .geom_transform
                        .get_or_insert(GeomTransform::LabelPoint);
                    label.styles.push(style);
                }
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        label.text = text.or_else(|| fallback.cloned());
        label
    }

    /// A label's `SIZE`: a number above 0, MapServer's bitmap sizes, an attribute, or an
    /// expression.
    fn label_size(&mut self, d: &Directive) -> Option<Bound<f64>> {
        // MS_TINY to MS_GIANT, src/mapserver.h.
        let bitmap = [
            ("TINY", 5.0),
            ("SMALL", 7.0),
            ("MEDIUM", 10.0),
            ("LARGE", 13.0),
            ("GIANT", 16.0),
        ];
        if let [token] = d.values.as_slice()
            && let Some(word) = token.keyword()
            && let Some((_, size)) = bitmap.iter().find(|(name, _)| *name == word)
        {
            return Some(Bound::Value(*size));
        }
        self.bound_number(d, Range::above(0.0), true)
    }

    /// A label's `ANGLE`: a number from -360 to 360, an attribute, or `AUTO`, `AUTO2`, `FOLLOW`.
    fn label_angle(&mut self, d: &Directive, label: &mut Label) {
        let modes = [
            ("AUTO", AngleMode::Auto),
            ("AUTO2", AngleMode::Auto2),
            ("FOLLOW", AngleMode::Follow),
        ];
        if let [token] = d.values.as_slice()
            && let Some(word) = token.keyword()
            && let Some((_, mode)) = modes.iter().find(|(name, _)| *name == word)
        {
            label.angle_mode = Some(*mode);
            return;
        }
        label.angle = self.bound_number(d, Range::within(-360.0, 360.0), false);
    }

    fn position(&mut self, d: &Directive) -> Option<Bound<Position>> {
        self.bound_choice(
            d,
            &[
                ("UL", Position::Ul),
                ("UC", Position::Uc),
                ("UR", Position::Ur),
                ("CL", Position::Cl),
                ("CC", Position::Cc),
                ("CR", Position::Cr),
                ("LL", Position::Ll),
                ("LC", Position::Lc),
                ("LR", Position::Lr),
                ("AUTO", Position::Auto),
            ],
        )
    }

    /// Warns of each font of a `FONT` list, `a,b` naming fallbacks, that the font set lacks.
    fn check_fonts(&mut self, names: &str, span: &Span) {
        for name in names.split(',') {
            if !self.fonts.contains_key(name) {
                let message = format!("the font set has no font `{name}`");
                self.warn("font-unknown", &message, span);
            }
        }
    }

    /// One of `choices`, or an attribute.
    fn bound_choice<T: Copy>(&mut self, d: &Directive, choices: &[(&str, T)]) -> Option<Bound<T>> {
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
        self.choice(d, choices).map(Bound::Value)
    }

    /// A string, or an attribute.
    fn bound_string(&mut self, d: &Directive) -> Option<Bound<String>> {
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
        self.string(d).map(Bound::Value)
    }

    /// A font set as msLoadFontSet reads it: a line `name path` per font, lines that start with
    /// `#`, a space, or nothing skipped; a path resolved beside the font set, and one outside the
    /// base context a warning (Q-8).
    fn fonts(&mut self, path: &ResourcePath, text: &str) -> BTreeMap<String, String> {
        let span = Span::new(path.as_str(), 1, 1);
        let mut fonts = BTreeMap::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with(['#', ' ', '\r']) {
                continue;
            }
            let Some((name, file)) = line.split_once([' ', '\t']) else {
                continue;
            };
            let file = file.trim();
            if file.is_empty() {
                continue;
            }
            match path.resolve(file) {
                Ok(font) => {
                    fonts.insert(name.to_owned(), font.as_str().to_owned());
                }
                // One font is needed only when it is drawn, so its refusal leaves it out of
                // the map rather than failing the load.
                Err(e) => {
                    let message = format!("font `{name}` at `{file}` is refused and left out: {e}");
                    self.warn("font-refused", &message, &span);
                }
            }
        }
        fonts
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
                    "MINSCALEDENOM" => style.min_scale_denom = self.scale_denom(d),
                    "MAXSCALEDENOM" => style.max_scale_denom = self.scale_denom(d),
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
                let index = *n as u32;
                if index as usize > self.symbols.len() {
                    // msDrawLineSymbol and its kin draw nothing for an index past the set.
                    let message = format!("there is no symbol {index}; the style draws nothing");
                    self.warn("symbol-unknown", &message, &d.span);
                }
                Some(Bound::Value(SymbolRef::Index(index)))
            }
            [
                Token {
                    kind: TokenKind::Str { value, .. } | TokenKind::Word(value),
                    ..
                },
            ] => {
                // msGetSymbolIndex: by name, without regard to case, the first from index 1.
                let found = self.symbols.iter().position(|symbol| {
                    symbol
                        .name
                        .as_ref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(value))
                });
                match found {
                    Some(i) => Some(Bound::Value(SymbolRef::Index(i as u32 + 1))),
                    None => {
                        let message = format!(
                            "no symbol is named `{value}`; MapServer would read it as an image \
                             file, which is not yet supported"
                        );
                        self.warn("symbol-unknown", &message, &d.span);
                        Some(Bound::Value(SymbolRef::Name(value.clone())))
                    }
                }
            }
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

    /// A scale denominator, a number of at least 0, as `getDouble` checks it.
    fn scale_denom(&mut self, d: &Directive) -> Option<f64> {
        self.number(d, Range::at_least(0.0))
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

    /// One of the keywords `choices` names. `TRUE` and `ON`, and `FALSE` and `OFF`, stand for
    /// each other, as MapServer's lexer gives them the same token values (`MS_TRUE` and `MS_ON`
    /// are 1, `MS_FALSE` and `MS_OFF` 0, src/mapserver.h) and its parser compares those.
    fn choice<T: Copy>(&mut self, d: &Directive, choices: &[(&str, T)]) -> Option<T> {
        let word = self.word(d)?;
        let alias = match word.as_str() {
            "TRUE" => "ON",
            "ON" => "TRUE",
            "FALSE" => "OFF",
            "OFF" => "FALSE",
            other => other,
        };
        let found = choices
            .iter()
            .find(|(name, _)| *name == word || *name == alias)
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
            "labelpoint" => Some(GeomTransform::LabelPoint),
            "labelpoly" => Some(GeomTransform::LabelPoly),
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
    /// resolved by the resolver the host passes in (ADR-0023); without one, or for a code it does
    /// not know, it is a load error. A first parameter that is `AUTO`, in any case, is the
    /// projection of the source, as `msProcessProjection` leaves it to the source to declare.
    ///
    /// As MapServer hands every parameter to PROJ, `init=` is matched case-sensitively, there is
    /// at most one, and its expansion follows the explicit parameters, which PROJ lets take
    /// precedence (its `pj_expand_init`).
    fn projection(&mut self, block: &Block) -> Option<LayerProjection> {
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
            if params.is_empty() && expansion.is_none() && text.eq_ignore_ascii_case("AUTO") {
                return Some(LayerProjection::FromSource);
            }
            for param in text.split_whitespace() {
                let param = param.trim_start_matches('+');
                if let Some(reference) = param.strip_prefix("init=") {
                    if expansion.is_some() {
                        let message = "a PROJECTION has at most one init=";
                        self.error("invalid-value", message, &token.span);
                        return None;
                    }
                    let resolved = match self.resolve(reference) {
                        Ok(resolved) => resolved,
                        Err(why) => {
                            let message = format!("`{param}`: {why}");
                            self.error("projection-unresolved", &message, &token.span);
                            return None;
                        }
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
        (!params.is_empty()).then(|| LayerProjection::Proj4(params.join(" ")))
    }

    /// The definition `init=<reference>` names: from the init file the mapfile names when its
    /// file name holds a `/`, else from the host's resolver, with `<file>` as the authority
    /// (ADR-0023).
    fn resolve(&self, reference: &str) -> Result<String, String> {
        // The code follows the last `:`, as PROJ splits it (`get_init_string`).
        let Some((file, code)) = reference.rsplit_once(':') else {
            return Err("a projection is named as `<authority>:<code>`".to_owned());
        };
        if file.contains('/') {
            let init_file = self
                .init_files
                .get(file)
                .ok_or_else(|| format!("the init file `{file}` was not read"))?;
            return match init_file {
                Ok(init_file) => init_file.definition(code).map(str::to_owned),
                Err(why) => Err(why.clone()),
            };
        }
        match self.resolver {
            Some(resolver) => resolver
                .resolve(file, code)
                .ok_or_else(|| "it is not a projection the resolver knows".to_owned()),
            None => Err("it needs a projection resolver, and none is given".to_owned()),
        }
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

    /// A `SYMBOL` block as loadSymbol reads it, its `IMAGE` resolved beside `beside`: the
    /// mapfile for a symbol of the map, the symbol file for one of the set.
    fn symbol_block(&mut self, block: &Block, beside: &ResourcePath) -> Option<Symbol> {
        let mut symbol = Symbol {
            name: None,
            kind: SymbolKind::Vector,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: Some(block.span.clone()),
        };
        let mut points: Option<Vec<[f64; 2]>> = None;
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "NAME" => symbol.name = self.string(d),
                    "TYPE" => {
                        symbol.kind = self.choice(
                            d,
                            &[
                                ("VECTOR", SymbolKind::Vector),
                                ("ELLIPSE", SymbolKind::Ellipse),
                                ("PIXMAP", SymbolKind::Pixmap),
                                ("SIMPLE", SymbolKind::Simple),
                                ("TRUETYPE", SymbolKind::Truetype),
                                ("HATCH", SymbolKind::Hatch),
                                ("SVG", SymbolKind::Svg),
                            ],
                        )?;
                    }
                    "FILLED" => {
                        symbol.filled = self.choice(d, &[("TRUE", true), ("FALSE", false)])?;
                    }
                    "ANCHORPOINT" => {
                        let range = Range::within(0.0, 1.0);
                        let n = self.numbers(d, 2)?;
                        if !(range.contains(n[0]) && range.contains(n[1])) {
                            self.invalid(d, "two numbers from 0 to 1");
                            return None;
                        }
                        symbol.anchor_point = Some([n[0], n[1]]);
                    }
                    "CHARACTER" => symbol.character = self.string(d),
                    "FONT" => symbol.font = self.string(d),
                    "IMAGE" => {
                        let value = self.string(d)?;
                        match beside.resolve(&value) {
                            Ok(path) => symbol.image = Some(path.as_str().to_owned()),
                            Err(e) => {
                                let message = format!("IMAGE `{value}` is refused: {e}");
                                self.error("image-refused", &message, &d.span);
                                return None;
                            }
                        }
                    }
                    "TRANSPARENT" => {
                        let n = self.number(d, Range::within(0.0, 255.0))?;
                        symbol.transparent = Some(n as u8);
                    }
                    // loadSymbol ignores it.
                    "ANTIALIAS" => {}
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) if b.keyword == "POINTS" => points = Some(self.points(b)?),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        // The checks at loadSymbol's END.
        let needs_image = matches!(symbol.kind, SymbolKind::Pixmap | SymbolKind::Svg);
        let needs_points = matches!(symbol.kind, SymbolKind::Vector | SymbolKind::Ellipse);
        if needs_image && symbol.image.is_none() {
            let message = "a PIXMAP or SVG symbol needs an IMAGE";
            self.error("invalid-value", message, &block.span);
            return None;
        }
        let points = points.unwrap_or_default();
        if needs_points && points.is_empty() {
            let message = "a VECTOR or ELLIPSE symbol needs POINTS";
            self.error("invalid-value", message, &block.span);
            return None;
        }
        if needs_points {
            let anchor = symbol.anchor_point.unwrap_or([0.5, 0.5]);
            let (paths, extent) = shape(&points, symbol.kind == SymbolKind::Vector, anchor);
            symbol.paths = paths;
            symbol.extent = Some(extent);
        }
        Some(symbol)
    }

    /// The `[x, y]` pairs of a `POINTS` block, at most MapServer's `MS_MAXVECTORPOINTS`.
    fn points(&mut self, block: &Block) -> Option<Vec<[f64; 2]>> {
        let numbers: Option<Vec<f64>> = block
            .values
            .iter()
            .map(|t| match t.kind {
                TokenKind::Number(n) => Some(n),
                _ => None,
            })
            .collect();
        match numbers {
            Some(numbers) if numbers.len() % 2 == 0 && numbers.len() <= 200 => {
                Some(numbers.chunks(2).map(|p| [p[0], p[1]]).collect())
            }
            _ => {
                let message = "POINTS expects pairs of numbers, at most 100";
                self.error("invalid-value", message, &block.span);
                None
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

/// The unit keywords of `SIZEUNITS`; a map's `UNITS` takes them without `PIXELS`.
const UNITS: &[(&str, Units)] = &[
    ("PIXELS", Units::Pixels),
    ("INCHES", Units::Inches),
    ("FEET", Units::Feet),
    ("MILES", Units::Miles),
    ("METERS", Units::Meters),
    ("KILOMETERS", Units::Kilometers),
    ("NAUTICALMILES", Units::NauticalMiles),
    ("DD", Units::Degrees),
];

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

/// A symbol's points split into lines at MapServer's pen-up `-99 -99`, and its extent, as
/// loadSymbol computes them: the largest x and y of the points whose x is not -99, and, for a
/// vector symbol, every point shifted so that none is negative, or so that the smallest is 0
/// where the anchor point is not the centre.
fn shape(points: &[[f64; 2]], vector: bool, anchor: [f64; 2]) -> (Vec<Vec<[f64; 2]>>, [f64; 2]) {
    let mut extent = [0.0_f64, 0.0_f64];
    for p in points.iter().filter(|p| p[0] != -99.0) {
        extent = [extent[0].max(p[0]), extent[1].max(p[1])];
    }
    let drawn = |p: &&[f64; 2]| p[0] != -99.0 && p[1] != -99.0;
    let mut shift = [0.0, 0.0];
    if vector {
        let min_x = points
            .iter()
            .filter(drawn)
            .map(|p| p[0])
            .fold(points[0][0], f64::min);
        let min_y = points
            .iter()
            .filter(drawn)
            .map(|p| p[1])
            .fold(points[0][1], f64::min);
        let shift_positive = anchor != [0.5, 0.5];
        if min_x < 0.0 || min_y < 0.0 || (shift_positive && (min_x != 0.0 || min_y != 0.0)) {
            shift = [min_x, min_y];
            extent = [extent[0] - min_x, extent[1] - min_y];
        }
    }
    let mut paths = vec![Vec::new()];
    for p in points {
        if p[0] == -99.0 && p[1] == -99.0 {
            paths.push(Vec::new());
        } else if let Some(path) = paths.last_mut() {
            path.push([p[0] - shift[0], p[1] - shift[1]]);
        }
    }
    paths.retain(|path| !path.is_empty());
    (paths, extent)
}
