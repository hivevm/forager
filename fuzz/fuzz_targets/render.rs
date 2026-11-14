//! The renderer on structured input: arbitrary points, lines, and polygons, an arbitrary
//! vector or ellipse symbol at the points, and a hatch over the polygons at the symbol's size,
//! the line width, the symbol's angle, and the first further style's pattern, with any `f64` —
//! NaN, infinities, and the extremes included — in an arbitrary view, drawn by the engine with
//! agg-rust behind its clipping (Q-3, ADR-0025). JSON cannot carry such numbers, so the GeoJSON
//! target never reaches these paths. Further styles of a line's dashes, caps, joins, offsets,
//! outlines, width bounds, and the symbol placed along it by a gap are read from the same bytes
//! back to front, so that they leave how an input decodes to its view, its symbol, and its shapes
//! as it was. The points carry a label of arbitrary text, size, bounds, and resolution, in a test
//! font of the label crate, drawn through the text path (ADR-0031). The second half of the bytes
//! is read once more for the features' properties, a filter, a class expression, and a computed
//! size and colour, as arbitrary trees (ADR-0016), which every layer evaluates. A pixmap symbol,
//! a small PNG every image path loads as, is drawn at the points, along the lines at a gap, and
//! tiled over the polygons, at an arbitrary size, angle, and gap, so that its resampling and the
//! tiled fill are fuzzed too (ADR-0029, ADR-0026). The bytes back to front are read once more
//! for the layers' limit on their features, which layers are drawn after the label cache, the
//! cache's edge buffer, and the pixmap as the label's labelpoint and labelcenter marker. `Input`
//! decodes as before; the existing seeds now also read a `Cache` from their tail, which changes
//! what they render.
// Verifies: Q-3, ADR-0025, ADR-0016
#![no_main]

mod expr_input;

use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};
use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Features, Geometry, Properties, ResourceLoader,
    ResourcePath, SourceError,
};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, Label, LabelTransform, Layer, LayerStatus, LineCap, LineJoin,
    LineOffset, Map, RenderMode, Rgba, Style, Symbol, SymbolKind, SymbolRef,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    width: u8,
    height: u8,
    extent: [f64; 4],
    line_width: f64,
    symbol_size: f64,
    symbol: SymbolInput,
    shapes: Vec<Shape>,
    text: String,
    label_size: f64,
    min_size: f64,
    max_size: f64,
    resolution: f64,
    pixmap: PixmapInput,
}

/// The pixmap symbol's style: its `SIZE`, `ANGLE`, `GAP`, `MINSIZE`, `MAXSIZE`, and `OFFSET`.
#[derive(Arbitrary, Debug)]
struct PixmapInput {
    size: Option<f64>,
    angle: f64,
    gap: Option<f64>,
    min_size: Option<f64>,
    max_size: Option<f64>,
    offset: Option<(f64, f64)>,
}

/// A subset of DejaVu Sans, whose licence lies beside it, as the one font of the font set.
const SANS: &[u8] = include_bytes!("../../crates/label/tests/fixtures/dejavu-sans-subset.ttf");

/// A PNG of 3 × 3 RGBA pixels, one of the pixmap target's seeds, as every pixmap's image.
const PIN: &[u8] = include_bytes!("../seeds/pixmap/rgba.png");

/// The font for every font path, and the PNG for every image path.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str().ends_with(".png") {
            Ok(PIN.to_vec())
        } else {
            Ok(SANS.to_vec())
        }
    }
}

/// The further styles of every layer's class.
#[derive(Arbitrary, Debug)]
struct Styles(Vec<LineStyle>);

#[derive(Arbitrary, Debug)]
struct LineStyle {
    color: bool,
    outline_color: bool,
    width: f64,
    pattern: Vec<f64>,
    initial_gap: Option<f64>,
    cap: u8,
    join: u8,
    line_join_max_size: Option<f64>,
    offset: Option<(f64, f64)>,
    line_offset: Option<(f64, bool)>,
    outline_width: Option<f64>,
    min_width: Option<f64>,
    max_width: Option<f64>,
    /// The symbol along the line, its `GAP`, `SIZE`, and `ANGLE`.
    marker: Option<(f64, f64, f64)>,
}

impl LineStyle {
    fn style(&self) -> Style {
        let caps = [
            LineCap::Butt,
            LineCap::Round,
            LineCap::Square,
            LineCap::Triangle,
        ];
        let joins = [
            LineJoin::None,
            LineJoin::Round,
            LineJoin::Miter,
            LineJoin::Bevel,
        ];
        Style {
            color: self
                .color
                .then_some(Bound::Value(Rgba::opaque(30, 60, 200))),
            outline_color: self
                .outline_color
                .then_some(Bound::Value(Rgba::opaque(0, 0, 0))),
            width: Some(Bound::Value(self.width)),
            pattern: self.pattern.clone(),
            initial_gap: self.initial_gap,
            line_cap: Some(caps[usize::from(self.cap) % caps.len()]),
            line_join: Some(joins[usize::from(self.join) % joins.len()]),
            line_join_max_size: self.line_join_max_size,
            offset: self.offset.map(|(x, y)| [Bound::Value(x), Bound::Value(y)]),
            line_offset: self.line_offset.map(|(distance, both_sides)| LineOffset {
                distance: Bound::Value(distance),
                both_sides,
            }),
            outline_width: self.outline_width.map(Bound::Value),
            min_width: self.min_width,
            max_width: self.max_width,
            symbol: self.marker.map(|_| Bound::Value(SymbolRef::Index(1))),
            gap: self.marker.map(|(gap, _, _)| gap),
            size: self.marker.map(|(_, size, _)| Bound::Value(size)),
            angle: self.marker.map(|(_, _, angle)| Bound::Value(angle)),
            ..Style::default()
        }
    }
}

/// The expressions of every layer, and the properties of its features in turn; none where the
/// bytes do not read as them.
#[derive(Arbitrary, Debug, Default)]
struct Expressions {
    filter: Option<expr_input::Tree>,
    class: Option<expr_input::Tree>,
    size: Option<expr_input::Tree>,
    color: Option<expr_input::Tree>,
    bound_width: bool,
    values: Vec<Vec<expr_input::Value>>,
}

/// What the label cache and the layers' limits read: every layer's `MAXFEATURES`, the layers of
/// `POSTLABELCACHE TRUE` by the bits of their positions, the map's
/// `labelcache_map_edge_buffer`, and whether the label draws the pixmap as a marker at its point
/// and at the centre of its box.
#[derive(Arbitrary, Debug, Default)]
struct Cache {
    max_features: Option<u8>,
    post_label_cache: u8,
    edge_buffer: Option<u16>,
    label_point: bool,
    label_center: bool,
}

#[derive(Arbitrary, Debug)]
struct SymbolInput {
    ellipse: bool,
    filled: bool,
    paths: Vec<Vec<(f64, f64)>>,
    extent: (f64, f64),
    anchor_point: Option<(f64, f64)>,
    angle: f64,
}

#[derive(Arbitrary, Debug)]
enum Shape {
    Point(f64, f64),
    Line(Vec<(f64, f64)>),
    Polygon(Vec<Vec<(f64, f64)>>),
}

/// The shapes as the features of every layer.
struct Shapes(Vec<Feature>);

impl FeatureSource for Shapes {
    async fn features(&self, _: &FeatureQuery) -> Result<Features, SourceError> {
        Ok(self.0.clone().into())
    }
}

fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
    points.iter().map(|&(x, y)| Coord { x, y }).collect()
}

/// A layer whose first class takes the features its expression selects and whose second takes
/// the rest, past its filter.
fn layer(name: &str, mode: RenderMode, styles: Vec<Style>, exprs: &Expressions) -> Layer {
    Layer {
        name: Some(name.into()),
        render_mode: Some(mode),
        status: Some(LayerStatus::On),
        filter: exprs.filter.as_ref().and_then(expr_input::Tree::expr),
        classes: vec![
            Class {
                expression: exprs.class.as_ref().and_then(expr_input::Tree::expr),
                styles: styles.clone(),
                ..Class::default()
            },
            Class {
                styles: styles.into_iter().rev().collect(),
                ..Class::default()
            },
        ],
        ..Layer::default()
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(input) = Input::arbitrary_take_rest(Unstructured::new(data)) else {
        return;
    };
    let reversed: Vec<u8> = data.iter().rev().copied().collect();
    let further = Styles::arbitrary_take_rest(Unstructured::new(&reversed))
        .map_or_else(|_| Vec::new(), |styles| styles.0);
    let exprs = Expressions::arbitrary_take_rest(Unstructured::new(&data[data.len() / 2..]))
        .unwrap_or_default();
    let cache = Cache::arbitrary(&mut Unstructured::new(&reversed)).unwrap_or_default();
    let features: Vec<Feature> = input
        .shapes
        .iter()
        .zip(0u64..)
        .map(|(shape, id)| Feature {
            id,
            geometry: match shape {
                Shape::Point(x, y) => Geometry::Point(Coord { x: *x, y: *y }),
                Shape::Line(points) => Geometry::LineString(coords(points)),
                Shape::Polygon(rings) => {
                    Geometry::Polygon(rings.iter().map(|r| coords(r)).collect())
                }
            },
            properties: match exprs.values.len() {
                0 => Properties::default(),
                n => expr_input::properties(&exprs.values[id as usize % n]),
            },
        })
        .collect();
    let computed = |tree: &Option<expr_input::Tree>| tree.as_ref().and_then(expr_input::Tree::expr);
    let style = Style {
        color: Some(
            computed(&exprs.color).map_or(Bound::Value(Rgba::opaque(200, 80, 80)), Bound::Expr),
        ),
        outline_color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        width: Some(if exprs.bound_width {
            Bound::Binding {
                binding: expr_input::NAMES[0].into(),
            }
        } else {
            Bound::Value(input.line_width)
        }),
        size: Some(computed(&exprs.size).map_or(Bound::Value(input.symbol_size), Bound::Expr)),
        ..Style::default()
    };
    let symbol = &input.symbol;
    let mut symbols = vec![Symbol {
        name: None,
        kind: if symbol.ellipse {
            SymbolKind::Ellipse
        } else {
            SymbolKind::Vector
        },
        paths: symbol
            .paths
            .iter()
            .map(|path| path.iter().map(|&(x, y)| [x, y]).collect())
            .collect(),
        extent: Some([symbol.extent.0, symbol.extent.1]),
        filled: symbol.filled,
        anchor_point: symbol.anchor_point.map(|(x, y)| [x, y]),
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }];
    symbols.push(Symbol {
        kind: SymbolKind::Hatch,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        ..symbols[0].clone()
    });
    symbols.push(Symbol {
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        image: Some("pin.png".to_owned()),
        ..symbols[0].clone()
    });
    let pixmap = &input.pixmap;
    let pixmap_style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(3))),
        size: pixmap.size.map(Bound::Value),
        angle: Some(Bound::Value(pixmap.angle)),
        gap: pixmap.gap,
        min_size: pixmap.min_size,
        max_size: pixmap.max_size,
        offset: pixmap
            .offset
            .map(|(x, y)| [Bound::Value(x), Bound::Value(y)]),
        ..Style::default()
    };
    let markers = [
        (cache.label_point, LabelTransform::Point),
        (cache.label_center, LabelTransform::Center),
    ];
    let label = Label {
        text: Some(Expr::String(input.text)),
        font: Some(Bound::Value("sans".to_owned())),
        size: Some(Bound::Value(input.label_size)),
        min_size: Some(input.min_size),
        max_size: Some(input.max_size),
        styles: markers
            .into_iter()
            .filter(|&(drawn, _)| drawn)
            .map(|(_, transform)| pixmap_style.clone().with_transform(transform))
            .collect(),
        ..Label::default()
    };
    let symbol_style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        angle: Some(Bound::Value(symbol.angle)),
        ..style.clone()
    };
    let hatch_style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        pattern: further.first().map_or_else(Vec::new, |s| s.pattern.clone()),
        min_size: further.first().and_then(|s| s.min_width),
        ..symbol_style.clone()
    };
    let styles: Vec<Style> = std::iter::once(style)
        .chain(further.iter().map(LineStyle::style))
        .collect();
    let mut points = layer("points", RenderMode::Point, styles.clone(), &exprs);
    for class in &mut points.classes {
        class.labels.push(label.clone());
    }
    let [min_x, min_y, max_x, max_y] = input.extent;
    let mut map = Map {
        resolution: Some(input.resolution),
        fonts: [("sans".to_owned(), "sans.ttf".to_owned())]
            .into_iter()
            .collect(),
        extent: Some(Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        }),
        size: Some([u32::from(input.width), u32::from(input.height)]),
        label_cache_edge_buffer: cache.edge_buffer.map(u32::from),
        layers: vec![
            layer("polygons", RenderMode::Polygon, styles.clone(), &exprs),
            layer("lines", RenderMode::Line, styles.clone(), &exprs),
            points,
            layer("symbols", RenderMode::Point, vec![symbol_style], &exprs),
            layer("hatches", RenderMode::Polygon, vec![hatch_style], &exprs),
            layer(
                "pixmap-points",
                RenderMode::Point,
                vec![pixmap_style.clone()],
                &exprs,
            ),
            layer(
                "pixmap-lines",
                RenderMode::Line,
                vec![pixmap_style.clone()],
                &exprs,
            ),
            layer(
                "pixmap-fills",
                RenderMode::Polygon,
                vec![pixmap_style],
                &exprs,
            ),
        ],
        symbols,
        ..Map::default()
    };
    // One bit for each layer; a layer past the eighth is drawn before the cache.
    for (layer, bit) in map.layers.iter_mut().zip(0u32..) {
        layer.max_features = cache.max_features.map(u32::from);
        layer.post_label_cache = cache.post_label_cache.checked_shr(bit).unwrap_or(0) & 1 == 1;
    }
    let sources: BTreeMap<String, Shapes> = [
        "polygons",
        "lines",
        "points",
        "symbols",
        "hatches",
        "pixmap-points",
        "pixmap-lines",
        "pixmap-fills",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), Shapes(features.clone())))
    .collect();
    let render = forager_engine::render_with_files(&map, &sources, &Files, AggRenderer::new());
    let _ = forager_ports::poll_once(render).expect("the engine waited on a port that never waits");
});
