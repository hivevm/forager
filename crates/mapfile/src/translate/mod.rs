// The translation follows MapServer's code where it names it, src/mapserver.h and the functions
// of its loader and drawing, of the release pinned in scripts/fetch-msautotest.sh.
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

//! Translates the syntax tree of a mapfile into the style IR. What the IR cannot hold yet is
//! reported, never silently dropped.

use std::collections::BTreeMap;

use forager_ports::{ProjectionResolver, ResourcePath};
use forager_style_ir::{
    Bound, Class, Diagnostic, Expr, Extent, GeomTransform, Layer, LayerProjection, LayerStatus,
    Map, Op, RenderMode, Rgba, ShapeType, SourceRef, Span, Symbol, Units,
};

use crate::expression;
use crate::init_file::InitFile;
use crate::lexer::{Token, TokenKind};
use crate::report::Report;
use crate::syntax::{Block, Directive, Node};

mod feature;
mod fonts;
mod label;
mod output;
mod projection;
mod style;
mod symbol;

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
            [token] => token.string().map(str::to_owned),
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

/// The `SYMBOL` blocks of a symbol set, appended to `blocks` in the order they stand, those of
/// the sets nested in it among them, as loadSymbolSet reads them.
fn set_symbols<'a>(
    path: &'a ResourcePath,
    set: &'a Block,
    blocks: &mut Vec<(&'a ResourcePath, &'a Block)>,
) {
    for node in &set.nodes {
        match node {
            Node::Block(b) if b.keyword == "SYMBOL" => blocks.push((path, b)),
            Node::Block(b) if b.keyword == "SYMBOLSET" => set_symbols(path, b, blocks),
            _ => {}
        }
    }
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
    // MapServer joins SHAPEPATH when it opens a layer's data, so the map's last one holds
    // wherever it stands (msBuildPath3 in msSHPLayerOpen).
    let shape_path = map.nodes.iter().rev().find_map(|node| match node {
        Node::Directive(d) if d.keyword == "SHAPEPATH" => match d.values.as_slice() {
            [token] => token.string().map(ToOwned::to_owned),
            _ => None,
        },
        _ => None,
    });
    let mut t = Translator {
        resolver,
        main,
        shape_path,
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
    let mut blocks: Vec<_> = inline.collect();
    if let Some((path, set)) = symbol_set {
        set_symbols(path, set, &mut blocks);
    }
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
    /// The main mapfile, against whose directory a layer's data resolves.
    main: &'a ResourcePath,
    /// The map's `SHAPEPATH`, which a local layer's `DATA` is read beneath.
    shape_path: Option<String>,
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
                    "IMAGECOLOR" => map.image_color = self.image_color(d),
                    "IMAGETYPE" => self.output_format(&d.keyword, &d.span),
                    // MS_RESOLUTION_MIN and MS_RESOLUTION_MAX.
                    "RESOLUTION" => map.resolution = self.number(d, Range::within(10.0, 1000.0)),
                    "DEFRESOLUTION" => {
                        map.def_resolution = self.number(d, Range::within(10.0, 1000.0));
                    }
                    "UNITS" => map.units = self.choice(d, &UNITS[1..]),
                    // Read with the map, before translation.
                    "SYMBOLSET" | "FONTSET" => {}
                    // Read before the layers, whose data it places.
                    "SHAPEPATH" => {
                        self.string(d);
                    }
                    _ => self.unhandled_directive(d),
                },
                Node::Block(b) => match b.keyword.as_str() {
                    "SYMBOL" => {}
                    "OUTPUTFORMAT" => self.output_format(&b.keyword, &b.span),
                    "LAYER" => {
                        if let Some(layer) = self.layer(b) {
                            self.class_labelcenter(&layer);
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
        if output::transparent(block) {
            // agg2CreateImage clears the image of a transparent format to no colour.
            map.image_color = Some(Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            });
        }
        map
    }

    /// `IMAGETYPE` and `OUTPUTFORMAT`, of which only whether the selected format is transparent
    /// is carried, as the map's background ([`output::transparent`]).
    fn output_format(&mut self, keyword: &str, span: &Span) {
        let message = format!(
            "{keyword} is read only for whether the image starts transparent; the output format \
             is not yet supported otherwise"
        );
        self.warn("unsupported-keyword", &message, span);
    }

    fn layer(&mut self, block: &Block) -> Option<Layer> {
        let mut layer = Layer {
            span: Some(block.span.clone()),
            ..Layer::default()
        };
        let mut connection_type = None;
        let mut props = BTreeMap::new();
        let mut source_span = None;
        let mut data_span = None;
        // MapServer reads the items when it evaluates, so they hold wherever they stand.
        let class_item = item(block, "CLASSITEM");
        let label_item = item(block, "LABELITEM");
        let filter_item = item(block, "FILTERITEM");
        let mut composites = Vec::new();
        // A layer with features is an inline layer (msInitializeVirtualTable), whose item names
        // its PROCESSING "ITEMS=…" gives (msLayerGetItems).
        let inline = block
            .nodes
            .iter()
            .any(|node| matches!(node, Node::Block(b) if b.keyword == "FEATURE"));
        let item_names = feature::item_names(block);
        // The shape type of the features the TYPE read so far gives (loadLayer); none before one.
        let mut shape_type = None;
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "NAME" => layer.name = self.string(d),
                    "TYPE" => {
                        if let Some(read) = self.layer_type(&mut layer, d)? {
                            shape_type = Some(read);
                        }
                    }
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
                    "LABELCACHE" => self.label_cache(&mut layer, d),
                    "FILTER" => layer.filter = self.expression(d, filter_item.as_deref()),
                    "SIZEUNITS" => layer.size_units = self.choice(d, UNITS),
                    "MINSCALE" | "MINSCALEDENOM" => layer.min_scale_denom = self.scale_denom(d),
                    "MAXSCALE" | "MAXSCALEDENOM" => layer.max_scale_denom = self.scale_denom(d),
                    "SYMBOLSCALE" | "SYMBOLSCALEDENOM" => {
                        layer.symbol_scale_denom = self.number(d, Range::at_least(1.0));
                    }
                    "PROCESSING" if inline && feature::items_value(d).is_some() => {
                        self.string(d);
                    }
                    "CONNECTION" | "DATA" => {
                        if let Some(value) = self.string(d) {
                            props.insert(d.keyword.to_ascii_lowercase(), value);
                            source_span.get_or_insert_with(|| d.span.clone());
                            if d.keyword == "DATA" {
                                data_span = Some(d.span.clone());
                            }
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
                    "COMPOSITE" => composites.push(b),
                    "FEATURE" => {
                        let feature = self.feature(b, shape_type, &item_names);
                        layer.features.extend(feature);
                    }
                    _ => self.unhandled(&b.keyword, &b.span),
                },
            }
        }
        layer.opacity = self.composite(&composites);
        layer.source = source_span.map(|span| self.source(connection_type, props, span, data_span));
        Some(layer)
    }

    /// A layer's source of the `CONNECTIONTYPE` `connection_type`, its `CONNECTION` and `DATA` in
    /// `props`, the first of them at `span`, its `DATA` at `data_span`.
    fn source(
        &mut self,
        connection_type: Option<String>,
        mut props: BTreeMap<String, String>,
        span: Span,
        data_span: Option<Span>,
    ) -> SourceRef {
        // Without CONNECTIONTYPE, MapServer reads DATA as a local shapefile.
        let kind = connection_type.unwrap_or_else(|| "local".to_owned());
        if kind == "local"
            && let (Some(data), Some(at)) = (props.remove("data"), data_span)
        {
            match self.data_path(&data) {
                Ok(path) => {
                    props.insert("data".to_owned(), path.as_str().to_owned());
                }
                // The layer keeps no data, so the engine skips it (G-8) and the map renders.
                Err(message) => self.warn("data-refused", &message, &at),
            }
        }
        SourceRef {
            kind,
            props,
            span: Some(span),
        }
    }

    /// Sets the layer's label cache by its `LABELCACHE`, `ON` or `OFF` as loadLayer reads it
    /// (mapfile.c:4643-4646); initLayer starts it on.
    fn label_cache(&mut self, layer: &mut Layer, d: &Directive) {
        if let Some(on) = self.choice(d, &[("ON", true), ("OFF", false)]) {
            layer.label_cache = on;
        }
    }

    /// Warns of each class `STYLE` of `GEOMTRANSFORM labelcenter` of a line or a polygon layer:
    /// msDrawTransformedShape (src/mapgeomtransform.c:245) fails the layer's draw on it, and the
    /// engine draws nothing of the style. A label's style is drawn, and a point layer draws the
    /// style at its points.
    fn class_labelcenter(&mut self, layer: &Layer) {
        if !matches!(
            layer.render_mode,
            Some(RenderMode::Line | RenderMode::Polygon)
        ) {
            return;
        }
        let spans: Vec<Span> = layer
            .classes
            .iter()
            .flat_map(|class| &class.styles)
            .filter(|style| style.geom_transform == Some(GeomTransform::LabelCenter))
            .filter_map(|style| style.span.clone())
            .collect();
        for span in spans {
            let message = "GEOMTRANSFORM labelcenter is drawn only by a label's style; on a \
                           class's style of a line or polygon layer MapServer fails the layer's \
                           draw, and the style draws nothing";
            self.warn("unsupported-value", message, &span);
        }
    }

    /// Sets the render mode of a layer's `TYPE`, and gives the shape type of the features after it
    /// (loadLayer): polygon for `POLYGON`, line for `LINE`, point for every other. `None` for a
    /// `QUERY` layer, which is discarded.
    fn layer_type(&mut self, layer: &mut Layer, d: &Directive) -> Option<Option<ShapeType>> {
        let word = self.word(d);
        let shape_type = word.as_deref().map(|w| match w {
            "POLYGON" => ShapeType::Polygon,
            "LINE" => ShapeType::Line,
            _ => ShapeType::Point,
        });
        match word.as_deref() {
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
        }
        Some(shape_type)
    }

    /// A layer's opacity from its `COMPOSITE` blocks. msDrawLayer draws a layer that has one into
    /// a transparent image of its own and lays it over the map once for each block, with the
    /// block's `COMPOP` and `COMPFILTER`, at its `OPACITY` (msCompositeRasterBuffer,
    /// src/mapdraw.c). The IR carries one block that draws `src-over`, MapServer's default
    /// `COMPOP`, without a `COMPFILTER`: its `OPACITY`, as `loadLayerCompositer` reads it, a number
    /// from 0 to 100 truncated to an integer, and 100 without one. Any other block is not yet
    /// supported and is ignored, every block of a layer that has several.
    fn composite(&mut self, blocks: &[&Block]) -> Option<f64> {
        let carried = |b: &Block| {
            b.nodes.iter().all(|node| match node {
                Node::Directive(d) if d.keyword == "OPACITY" => true,
                Node::Directive(d) if d.keyword == "COMPOP" => {
                    matches!(d.values.as_slice(), [t] if t.string() == Some("src-over"))
                }
                _ => false,
            })
        };
        match blocks {
            [] => None,
            [block] if carried(block) => {
                let mut opacity = Some(100.0);
                for node in &block.nodes {
                    if let Node::Directive(d) = node
                        && d.keyword == "OPACITY"
                    {
                        opacity = self.number(d, Range::within(0.0, 100.0)).map(f64::trunc);
                    }
                }
                opacity
            }
            _ => {
                for block in blocks {
                    self.unhandled(&block.keyword, &block.span);
                }
                None
            }
        }
    }

    /// A local layer's `DATA` beneath the map's `SHAPEPATH`, against the main mapfile's directory,
    /// as `msBuildPath3` joins them, and inside the base context (Q-8). The refusal names the
    /// `SHAPEPATH` the path was joined with, which may be the part that leaves.
    fn data_path(&self, data: &str) -> Result<ResourcePath, String> {
        let refused = |e: forager_ports::PathError| format!("DATA `{data}` is refused: {e}");
        match self.shape_path.as_deref() {
            Some(shape_path) if !shape_path.is_empty() => {
                // An absolute DATA, a drive's included, ignores SHAPEPATH, as `msBuildPath` reads
                // it; the base context refuses it.
                let bytes = data.as_bytes();
                if data.starts_with(['/', '\\'])
                    || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
                {
                    return self.main.resolve(data).map_err(refused);
                }
                self.main
                    .resolve(&format!(
                        "{}/{data}",
                        shape_path.trim_end_matches(['/', '\\'])
                    ))
                    .map_err(|e| {
                        format!("DATA `{data}` beneath SHAPEPATH `{shape_path}` is refused: {e}")
                    })
            }
            _ => self.main.resolve(data).map_err(refused),
        }
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
                // loadClass refuses each TEXT that is neither a string nor an expression, the
                // ones a later TEXT replaces included.
                Node::Directive(d) if d.keyword == "TEXT" => {
                    if class_text.is_some_and(|last| !std::ptr::eq(last, d)) {
                        self.text_errors(d);
                    }
                }
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
                    class
                        .labels
                        .push(self.label(b, fallback.as_ref(), label_item));
                }
                Node::Directive(d) => self.unhandled_directive(d),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        class
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

    /// One string, or a bare word, which MapServer's lexer reads as a string.
    fn string(&mut self, d: &Directive) -> Option<String> {
        let value = match d.values.as_slice() {
            [token] => token.string(),
            _ => None,
        };
        if value.is_none() {
            self.invalid(d, "one string");
        }
        value.map(str::to_owned)
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

    /// A colour as `loadColor` reads it: `#rrggbb` or `#rrggbbaa`, its digits in either case, or
    /// three numbers from -255 to 255, each checked before it is truncated to an integer. The
    /// channels come as read, a negative one included, with the alpha, 255 where none is given.
    fn loaded_color(&mut self, d: &Directive) -> Option<([i32; 3], u8)> {
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Str { value, .. },
                    ..
                },
            ] => match Rgba::from_hex(value) {
                Some(c) => Some(([c.r, c.g, c.b].map(i32::from), c.a)),
                None => {
                    self.invalid(d, "a colour as `#rrggbb` or `#rrggbbaa`");
                    None
                }
            },
            _ => {
                let rgb = self.numbers(d, 3)?;
                if rgb.iter().any(|c| !(-255.0..=255.0).contains(c)) {
                    self.invalid(d, "three numbers from -255 to 255");
                    return None;
                }
                Some(([rgb[0], rgb[1], rgb[2]].map(|c| c.trunc() as i32), 255))
            }
        }
    }

    /// A colour of a style or a label as [`Self::loaded_color`] reads it. A channel of -1 means
    /// no colour (`MS_VALID_COLOR`, src/mapserver.h); MapServer's renderers store any other
    /// channel in a byte, so a negative one is taken modulo 256.
    fn color(&mut self, d: &Directive) -> Option<Rgba> {
        let (rgb, alpha) = self.loaded_color(d)?;
        if rgb.contains(&-1) {
            return None;
        }
        let [r, g, b] = rgb.map(channel_byte);
        Some(Rgba { r, g, b, a: alpha })
    }

    /// The map's `IMAGECOLOR` as [`Self::loaded_color`] reads it. `msPrepareImage`
    /// (src/mapdraw.c) hands it to the renderer without checking `MS_VALID_COLOR`, so each
    /// channel, -1 included, is taken modulo 256. MapServer also sets its alpha to 255; the
    /// frontend keeps the alpha of `#rrggbbaa` instead, so that a map can ask for a translucent
    /// background, and three numbers stay opaque.
    fn image_color(&mut self, d: &Directive) -> Option<Rgba> {
        let (rgb, alpha) = self.loaded_color(d)?;
        let [r, g, b] = rgb.map(channel_byte);
        Some(Rgba { r, g, b, a: alpha })
    }

    /// A style's colour, which may also be bound to a property.
    fn bound_color(&mut self, d: &Directive) -> Option<Bound<Rgba>> {
        if let Some(bound) = binding_of(d) {
            return Some(bound);
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
        if let Some(bound) = binding_of(d) {
            return Some(bound);
        }
        match d.values.as_slice() {
            [
                Token {
                    kind: TokenKind::Number(n),
                    ..
                },
            ] if range.contains(*n) => Some(Bound::Value(*n)),
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

    /// A class's or a label's `EXPRESSION` or a layer's `FILTER`; a string, regular expression, or
    /// list in it compares the attribute `item` names, the layer's `CLASSITEM`, `LABELITEM`, or
    /// `FILTERITEM`. One that cannot be carried over is a warning and a condition that never
    /// matches.
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

    /// One of `choices`, or an attribute.
    fn bound_choice<T: Copy>(&mut self, d: &Directive, choices: &[(&str, T)]) -> Option<Bound<T>> {
        if let Some(bound) = binding_of(d) {
            return Some(bound);
        }
        self.choice(d, choices).map(Bound::Value)
    }

    /// A string, or an attribute.
    fn bound_string(&mut self, d: &Directive) -> Option<Bound<String>> {
        if let Some(bound) = binding_of(d) {
            return Some(bound);
        }
        self.string(d).map(Bound::Value)
    }

    /// Two numbers inside `range`, each of which may be bound to a property.
    fn bound_pair(&mut self, d: &Directive, range: Range) -> Option<[Bound<f64>; 2]> {
        let bound = |token: &Token| match token.kind {
            TokenKind::Number(n) if range.contains(n) => Some(Bound::Value(n)),
            _ => binding(token),
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

/// The attribute a `[name]` token binds to.
fn binding<T>(token: &Token) -> Option<Bound<T>> {
    match &token.kind {
        TokenKind::Binding(name) => Some(Bound::Binding {
            binding: name.clone(),
        }),
        _ => None,
    }
}

/// The attribute a directive of one `[name]` binds to.
fn binding_of<T>(d: &Directive) -> Option<Bound<T>> {
    match d.values.as_slice() {
        [token] => binding(token),
        _ => None,
    }
}

/// A colour channel as MapServer's renderers store it, in a byte: AGG's `rgba8` keeps an `int8u`.
fn channel_byte(c: i32) -> u8 {
    c.rem_euclid(256) as u8
}

/// What a property holds after a later directive gives it `new`: loadStyle and loadLabel never
/// free a binding for a value, and bindStyle and bindLabel, src/maputil.c, draw the binding over
/// the value. A colour of `-1 -1 -1` reads as none, which bindColorAttribute replaces with the
/// bound one, and keeps the binding too; any other `None` is a value that did not read, which
/// fails the load anyway.
fn kept_binding<T>(earlier: Option<Bound<T>>, new: Option<Bound<T>>) -> Option<Bound<T>> {
    match (earlier, new) {
        (Some(binding @ Bound::Binding { .. }), Some(Bound::Value(_)) | None) => Some(binding),
        (_, new) => new,
    }
}

/// [`kept_binding`] for each of a pair's numbers.
fn kept_bindings(
    earlier: Option<[Bound<f64>; 2]>,
    new: Option<[Bound<f64>; 2]>,
) -> Option<[Bound<f64>; 2]> {
    let [x, y] = new?;
    let [earlier_x, earlier_y] = earlier.map_or([None, None], |[x, y]| [Some(x), Some(y)]);
    Some([
        kept_binding(earlier_x, Some(x))?,
        kept_binding(earlier_y, Some(y))?,
    ])
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
