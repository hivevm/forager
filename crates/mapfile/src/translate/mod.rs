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
use forager_style_ir::c::{atoi, int_cast};
use forager_style_ir::{
    Bound, Class, Diagnostic, Expr, Extent, Fonts, GeomTransform, Layer, LayerProjection,
    LayerStatus, MAX_RESOLUTION, MIN_RESOLUTION, Map, Op, RenderMode, Rgba, ShapeType, SourceRef,
    Span, Symbol, Units,
};

use crate::MAX_DEFAULT_BYTES;
use crate::budget::Budget;
use crate::expression;
use crate::init_file::InitFile;
use crate::keywords;
use crate::lexer::{Token, TokenKind};
use crate::report::Report;
use crate::syntax::{Block, Directive, Node};

use self::defaults::{Defaults, substituted};
use self::hash_table::HashTable;

mod defaults;
mod feature;
mod fonts;
mod hash_table;
mod label;
mod output;
mod projection;
mod style;
mod symbol;

/// The range MapServer's loader holds `RESOLUTION` and `DEFRESOLUTION` to, the style document's.
const RESOLUTION: Range = Range::Within(MIN_RESOLUTION, MAX_RESOLUTION);

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

/// The keys of `WEB` `METADATA` the map carries.
const WEB_METADATA_READ: &[&str] = &["labelcache_map_edge_buffer", MAX_FEATURES_KEY];
/// The key of the metadata and of the output format's option that limit the features a layer
/// draws, as `msLayerGetMaxFeaturesToDraw` reads it.
const MAX_FEATURES_KEY: &str = "maxfeaturestodraw";

/// Whether the layer `block`'s features are never read: a `FEATURE` makes the layer inline as
/// `loadLayer` reads it (src/mapfile.c), and a `CONNECTIONTYPE GRATICULE` after the last one
/// makes it a graticule again, which `msInitializeVirtualTable` (src/maplayer.c) does not read
/// features for; so that the style IR carries features only where they are the layer's source
/// (ADR-0018).
fn exempts_features(block: &Block) -> bool {
    block
        .nodes
        .iter()
        .rev()
        .find_map(|node| match node {
            Node::Directive(d) if d.keyword == "CONNECTIONTYPE" => {
                Some(match d.values.as_slice() {
                    [token] => token
                        .keyword()
                        .is_some_and(|kind| kind.eq_ignore_ascii_case("graticule")),
                    _ => false,
                })
            }
            Node::Block(b) if b.keyword == "FEATURE" => Some(false),
            _ => None,
        })
        .unwrap_or(false)
}

/// The files a map was read from: the main mapfile, and the symbol file and font set it names,
/// each with the path it was read from.
pub struct Files<'a> {
    pub main: &'a ResourcePath,
    pub symbol_set: Option<&'a (ResourcePath, Block)>,
    pub font_set: Option<&'a (ResourcePath, String)>,
    /// The init files the projections name by a path, by the name as written (ADR-0019).
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
        names: BTreeMap::new(),
        numbered: Vec::new(),
        fonts: Fonts::default(),
        defaults_added: Budget::new(MAX_DEFAULT_BYTES),
        max_features_fallback: MaxFeaturesFallback {
            web: None,
            format: -1,
        },
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
    for (beside, b) in blocks {
        if let Some(symbol) = t.symbol_block(b, beside) {
            t.push_symbol(symbol);
        }
    }
    if let Some((path, text)) = files.font_set {
        t.fonts = t.fonts(path, text);
    }
    let mut map = t.map(map);
    t.image_symbols(&mut map);
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
    /// The position among `symbols` of the first of each name, by its `symbol_name_key`, so that
    /// a name is found as `ms_get_symbol_index` finds it without scanning them (Q-9).
    names: BTreeMap<String, usize>,
    /// The symbol indexes the styles give, each with its directive's span, checked against the
    /// symbols once those the styles name are appended.
    numbered: Vec<(i32, Span)>,
    /// The map's fonts, read before its layers so that a label's font is checked.
    fonts: Fonts,
    /// What the defaults of `VALIDATION` have put into the load's expressions, held to
    /// [`MAX_DEFAULT_BYTES`] (Q-9).
    defaults_added: Budget,
    /// The limits on a layer's features that `msLayerGetMaxFeaturesToDraw` falls back on.
    max_features_fallback: MaxFeaturesFallback,
}

/// The limits on a layer's features that `msLayerGetMaxFeaturesToDraw` falls back on, where the
/// layer's own metadata gives none.
struct MaxFeaturesFallback {
    /// The map's `maxfeaturestodraw` metadata, read by `atoi`, which replaces the layer's
    /// `MAXFEATURES`.
    web: Option<i32>,
    /// The selected output format's `MAXFEATURESTODRAW`, read by `atoi`, -1 without one, which
    /// holds where the limit is below 0.
    format: i32,
}

impl Translator<'_> {
    fn map(&mut self, block: &Block) -> Map {
        let mut map = Map {
            span: Some(block.span.clone()),
            ..Map::default()
        };
        // MapServer applies them after the whole map is read, so each holds wherever it stands.
        let webs: Vec<&Block> = block
            .nodes
            .iter()
            .filter_map(|node| match node {
                Node::Block(b) if b.keyword == "WEB" => Some(b),
                _ => None,
            })
            .collect();
        let web = Defaults::of(webs.iter().copied(), self.diagnostics);
        let web_metadata = HashTable::of(webs, "METADATA", self.diagnostics);
        self.max_features_fallback = MaxFeaturesFallback {
            web: web_metadata
                .get(MAX_FEATURES_KEY)
                .map(|v| atoi(v.as_bytes())),
            format: output::format_option(block, MAX_FEATURES_KEY)
                .map_or(-1, |v| atoi(v.as_bytes())),
        };
        for node in &block.nodes {
            match node {
                Node::Directive(d) => match d.keyword.as_str() {
                    "NAME" => map.name = self.string(d),
                    "EXTENT" => map.extent = self.extent(d),
                    "SIZE" => map.size = self.map_size(d),
                    "IMAGECOLOR" => map.image_color = self.image_color(d),
                    "IMAGETYPE" => self.output_format(&d.keyword, &d.span),
                    "RESOLUTION" => map.resolution = self.number(d, RESOLUTION),
                    "DEFRESOLUTION" => map.def_resolution = self.number(d, RESOLUTION),
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
                        if let Some(layer) = self.layer(b, &web) {
                            self.class_labelcenter(&layer);
                            map.layers.push(layer);
                        }
                    }
                    "WEB" => self.web(b),
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
        // msDrawLabelCache reads the gutter as MS_ABS(atoi(value)) (src/mapdraw.c:3488-3498).
        map.label_cache_edge_buffer = web_metadata
            .get("labelcache_map_edge_buffer")
            .map(|v| atoi(v.as_bytes()).unsigned_abs())
            .filter(|&gutter| gutter > 0);
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

    /// `WEB`, read for its `VALIDATION` and, of its `METADATA`, for the keys the map carries
    /// ([`WEB_METADATA_READ`]); the other keys of its `METADATA` may set what MapServer draws, so
    /// they are not yet supported, and its other keywords serve MapServer's CGI and templates and
    /// are out of scope.
    fn web(&mut self, block: &Block) {
        for node in &block.nodes {
            match node {
                // Read by Defaults; its other keys only check a runtime substitution.
                Node::Block(b) if b.keyword == "VALIDATION" => {}
                Node::Block(b) if b.keyword == "METADATA" => {
                    let message = format!(
                        "METADATA of WEB is read for {} alone; its other keys are not yet \
                         supported and are ignored",
                        WEB_METADATA_READ.join(" and ")
                    );
                    self.warn("unsupported-keyword", &message, &b.span);
                }
                Node::Directive(d) if !d.known => {}
                Node::Directive(Directive { keyword, span, .. })
                | Node::Block(Block { keyword, span, .. }) => {
                    let message =
                        format!("{keyword} of WEB is out of Forager's scope and is discarded");
                    self.warn("discarded-keyword", &message, span);
                }
            }
        }
    }

    /// `IMAGETYPE` and `OUTPUTFORMAT`, of which only whether the selected format is transparent
    /// is carried, as the map's background ([`output::transparent`]), and its
    /// `MAXFEATURESTODRAW`, as the layers' limit ([`Self::features_to_draw`]).
    fn output_format(&mut self, keyword: &str, span: &Span) {
        let message = format!(
            "{keyword} is read only for whether the image starts transparent and for its \
             MAXFEATURESTODRAW; the output format is not yet supported otherwise"
        );
        self.warn("unsupported-keyword", &message, span);
    }

    fn layer(&mut self, block: &Block, web: &Defaults) -> Option<Layer> {
        let mut layer = Layer {
            span: Some(block.span.clone()),
            ..Layer::default()
        };
        let mut connection_type = None;
        // initLayer: no quota.
        let mut max_features = -1;
        let mut props = BTreeMap::new();
        let mut source_span = None;
        let mut data_span = None;
        // MapServer reads the items when it evaluates, so they hold wherever they stand.
        let class_item = item(block, "CLASSITEM");
        let label_item = item(block, "LABELITEM");
        let filter_item = item(block, "FILTERITEM");
        let defaults = Defaults::of([block], self.diagnostics);
        let tables = [&defaults, web];
        let mut composites = Vec::new();
        // A layer with features is an inline layer (msInitializeVirtualTable), whose item names
        // its PROCESSING "ITEMS=…" gives (msLayerGetItems), unless its connection type is one
        // MapServer exempts, whose features are never read.
        let exempt = exempts_features(block);
        let inline = !exempt
            && block
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
                    "POSTLABELCACHE" => self.post_label_cache(&mut layer, d),
                    "MAXFEATURES" => max_features = self.max_features(d),
                    "FILTER" => layer.filter = self.defaulted(d, filter_item.as_deref(), &tables),
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
                        let class =
                            self.class(b, class_item.as_deref(), label_item.as_deref(), tables);
                        layer.classes.push(class);
                    }
                    "PROJECTION" => layer.projection = self.projection(b),
                    // Read by Defaults; its other keys only check a runtime substitution.
                    "VALIDATION" => {}
                    "COMPOSITE" => composites.push(b),
                    "METADATA" => self.layer_metadata(&b.span),
                    "FEATURE" => {
                        let feature = self.feature(b, shape_type, &item_names);
                        if exempt {
                            let message = "a FEATURE before the layer's CONNECTIONTYPE GRATICULE is never \
                                           read, as in MapServer, and is left out";
                            self.warn("features-ignored", message, &b.span);
                        } else {
                            layer.features.extend(feature);
                        }
                    }
                    _ => self.unhandled(&b.keyword, &b.span),
                },
            }
        }
        self.settle_layer(&mut layer, block, &composites, max_features);
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

    /// What a layer settles once its block is read: its opacity from its `COMPOSITE`s; its label
    /// cache off where it is drawn after the cache, as loadLayer turns it off where POSTLABELCACHE
    /// is TRUE (mapfile.c:4755-4759) and msDrawLayer turns it off again as it draws such a layer,
    /// whatever LABELCACHE says after it (mapdraw.c:854-856); and its limit on its features.
    fn settle_layer(
        &mut self,
        layer: &mut Layer,
        block: &Block,
        composites: &[&Block],
        max_features: i32,
    ) {
        layer.opacity = self.composite(composites);
        layer.label_cache &= !layer.post_label_cache;
        let metadata = HashTable::of([block], "METADATA", self.diagnostics);
        layer.max_features = self.features_to_draw(&metadata, max_features);
    }

    /// Warns of a layer's `METADATA`, read for [`MAX_FEATURES_KEY`] alone
    /// ([`Self::features_to_draw`]).
    fn layer_metadata(&mut self, span: &Span) {
        let message = format!(
            "METADATA of a layer is read for {MAX_FEATURES_KEY} alone; its other keys are not yet \
             supported and are ignored"
        );
        self.warn("unsupported-keyword", &message, span);
    }

    /// A layer's `MAXFEATURES`, a number above 0 as getInteger checks it, truncated to an `int`;
    /// -1, no quota, where it does not read, which fails the load.
    fn max_features(&mut self, d: &Directive) -> i32 {
        self.number(d, Range::above(0.0)).map_or(-1, int_cast)
    }

    /// The most features a layer of the `metadata` draws, as `msLayerGetMaxFeaturesToDraw` of
    /// src/maplayer.c settles it: its `MAXFEATURES`, replaced by its `maxfeaturestodraw` metadata
    /// or else the map's, read by `atoi`; below 0, the output format's `MAXFEATURESTODRAW`; below 0
    /// still, none.
    fn features_to_draw(&self, metadata: &HashTable<'_>, max_features: i32) -> Option<u32> {
        let MaxFeaturesFallback { web, format } = self.max_features_fallback;
        let layer = metadata.get(MAX_FEATURES_KEY).map(|v| atoi(v.as_bytes()));
        let n = layer.or(web).unwrap_or(max_features);
        u32::try_from(if n < 0 { format } else { n }).ok()
    }

    /// Sets the layer's label cache by its `LABELCACHE`, `ON` or `OFF` as loadLayer reads it
    /// (mapfile.c:4643-4646); initLayer starts it on.
    fn label_cache(&mut self, layer: &mut Layer, d: &Directive) {
        if let Some(on) = self.choice(d, &[("ON", true), ("OFF", false)]) {
            layer.label_cache = on;
        }
    }

    /// Sets whether the layer is drawn after the label cache by its `POSTLABELCACHE`, `TRUE` or
    /// `FALSE` as loadLayer reads it with getSymbol (mapfile.c:4755-4759); initLayer starts it
    /// false.
    fn post_label_cache(&mut self, layer: &mut Layer, d: &Directive) {
        if let Some(post) = self.choice(d, &[("TRUE", true), ("FALSE", false)]) {
            layer.post_label_cache = post;
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
        // loadLayer's getSymbol, src/mapfile.c: a word of the token value of one of its choices
        // stands for it, so `TYPE off` is a point layer.
        let types = [
            "POINT",
            "LINE",
            "RASTER",
            "POLYGON",
            "ANNOTATION",
            "QUERY",
            "CIRCLE",
            "CHART",
            "TILEINDEX",
        ];
        let word = self
            .word(d)
            .map(|w| keywords::chosen(&w, &types).map_or(w, str::to_owned));
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
                        // getInteger's `(int)` cast, of a number already within 0 to 100.
                        opacity = self
                            .number(d, Range::within(0.0, 100.0))
                            .map(|n| f64::from(int_cast(n)));
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
        outer: [&Defaults; 2],
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
        let own = Defaults::of([block], self.diagnostics);
        let mut label_left_out = false;
        for node in &block.nodes {
            match node {
                Node::Directive(d) if d.keyword == "NAME" => class.name = self.string(d),
                Node::Directive(d) if d.keyword == "EXPRESSION" => {
                    class.expression = self.defaulted(d, class_item, &[&own, outer[0], outer[1]]);
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
                Node::Block(b) if b.keyword == "STYLE" => class.styles.push(self.style(b, false)),
                Node::Block(b) if b.keyword == "VALIDATION" => {}
                Node::Block(b) if b.keyword == "LABEL" => {
                    match self.label(b, fallback.as_ref(), label_item) {
                        Some(label) => class.labels.push(label),
                        None => label_left_out = true,
                    }
                }
                Node::Directive(d) => self.unhandled_directive(d),
                Node::Block(b) => self.unhandled(&b.keyword, &b.span),
            }
        }
        // Leaving out one of a class's labels must not draw another: the engine draws no label of
        // a class of several.
        if label_left_out {
            class.labels.clear();
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

    /// A map's `SIZE` as loadMap reads it (src/mapfile.c:6824-6829): two numbers each checked by
    /// getInteger against 1 and the map's `MAXSIZE`, an `int`, and cast to an `int`. A number below
    /// 1 or above INT_MAX, past any `MAXSIZE`, fails the load; the engine bounds the rest (Q-9).
    fn map_size(&mut self, d: &Directive) -> Option<[u32; 2]> {
        let numbers = self.numbers(d, 2)?;
        // msCheckNumber compares the double with the bounds before the cast
        // (src/mapfile.c:276-290, 312-317); `MAXSIZE` is at most INT_MAX.
        let side = |n: f64| {
            Some(n)
                .filter(|n| (1.0..=f64::from(i32::MAX)).contains(n))
                .and_then(|n| u32::try_from(int_cast(n)).ok())
        };
        match (side(numbers[0]), side(numbers[1])) {
            (Some(width), Some(height)) => Some([width, height]),
            _ => {
                self.invalid(d, "two numbers of at least 1 within an `int`");
                None
            }
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
                // loadColor's `(int)` cast, of a channel already within -255 to 255.
                Some(([rgb[0], rgb[1], rgb[2]].map(int_cast), 255))
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
        self.defaulted(d, item, &[])
    }

    /// [`Self::expression`] with the `%name%`s the defaults of `tables` give replaced first.
    fn defaulted(
        &mut self,
        d: &Directive,
        item: Option<&str>,
        tables: &[&Defaults],
    ) -> Option<Expr> {
        // The load has failed on the defaults' bound and said so once (Q-9).
        if self.defaults_added.passed() {
            return None;
        }
        let [token] = d.values.as_slice() else {
            self.invalid(d, "one expression");
            return None;
        };
        let translated = substituted(token, tables, &mut self.defaults_added)
            .and_then(|token| expression::translate(&token, item));
        match translated {
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

    /// One of the keywords `choices` names, compared by token value as `getSymbol` compares it, so
    /// that the words MapServer's lexer gives one value stand for each other: `OFF`, `FALSE`,
    /// `INCHES`, and `POINT` all for `NONE` (see [`keywords::stands_for`]).
    fn choice<T: Copy>(&mut self, d: &Directive, choices: &[(&str, T)]) -> Option<T> {
        let word = self.word(d)?;
        let found = choices
            .iter()
            .find(|(name, _)| keywords::stands_for(&word, name))
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
