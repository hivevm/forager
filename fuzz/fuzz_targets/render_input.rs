//! What the render target builds of its bytes, apart from the target, so that a test of the fuzz
//! workspace decodes each seed as the target does (fuzz/tests/render_seeds.rs).

use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};
use forager_engine::Rendered;
use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Features, Geometry, Properties, ResourceLoader,
    ResourcePath, SourceError,
};
use forager_render::Renderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, GeomTransform, Label, LabelTransform, Layer, LayerProjection,
    LayerStatus, LineCap, LineJoin, LineOffset, MAX_MAP_ANGLE, Map, NumberOp, Preprocessing,
    RenderMode, Rgba, Style, Symbol, SymbolKind, SymbolRef, TransformNumber, TransformShape,
    TransformTree,
};

use crate::expr_input;

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

/// The name and the path of the font the fuzz bytes supply.
pub const INPUT_FONT_NAME: &str = "input";
const INPUT_FONT: &str = "input.ttf";

/// The proj4 string of the map and of its layers where they are projected.
const LONGLAT: &str = "+proj=longlat +datum=WGS84 +no_defs";

/// The PNG for every image path, the font of the fuzz bytes at [`INPUT_FONT`], and the test font
/// for every other font path.
struct Files {
    font: Vec<u8>,
}

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str().ends_with(".png") {
            Ok(PIN.to_vec())
        } else if path.as_str() == INPUT_FONT {
            Ok(self.font.clone())
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

/// A layer's `GEOMTRANSFORM`: each step a function of the shape the steps before it give, from
/// `[shape]`, at most eight of them.
#[derive(Arbitrary, Debug, Default)]
struct TransformInput(Vec<StepInput>);

#[derive(Arbitrary, Debug)]
enum StepInput {
    Generalize(NumberInput),
    Densify(NumberInput),
    Simplify(NumberInput),
    Simplifypt(NumberInput),
    Buffer(NumberInput),
    Outer,
    Inner,
    SmoothSia(NumberInput, NumberInput, Option<bool>),
}

/// A number: written, or an operation of `[map_cellsize]` and a written number.
#[derive(Arbitrary, Debug)]
enum NumberInput {
    Number(f64),
    Cellsize(u8, f64),
}

impl NumberInput {
    fn number(&self) -> TransformNumber {
        match self {
            NumberInput::Number(n) => TransformNumber::Number(*n),
            NumberInput::Cellsize(op, n) => {
                let ops = [
                    NumberOp::Add,
                    NumberOp::Sub,
                    NumberOp::Mul,
                    NumberOp::Div,
                    NumberOp::Mod,
                    NumberOp::Pow,
                ];
                TransformNumber::Op(
                    ops[usize::from(*op) % ops.len()],
                    Box::new(TransformNumber::MapCellsize),
                    Box::new(TransformNumber::Number(*n)),
                )
            }
        }
    }
}

impl TransformInput {
    fn tree(&self) -> Option<TransformTree> {
        if self.0.is_empty() {
            return None;
        }
        let mut shape = TransformShape::Shape;
        for step in self.0.iter().take(8) {
            let inner = Box::new(shape);
            shape = match step {
                StepInput::Generalize(n) => TransformShape::Generalize(inner, n.number()),
                StepInput::Densify(n) => TransformShape::Densify(inner, n.number()),
                StepInput::Simplify(n) => TransformShape::Simplify(inner, n.number()),
                StepInput::Simplifypt(n) => TransformShape::Simplifypt(inner, n.number()),
                StepInput::Buffer(n) => TransformShape::Buffer(inner, n.number()),
                StepInput::Outer => TransformShape::Outer(inner),
                StepInput::Inner => TransformShape::Inner(inner),
                StepInput::SmoothSia(size, iterations, preprocessing) => {
                    TransformShape::SmoothSia {
                        shape: inner,
                        size: size.number(),
                        iterations: iterations.number(),
                        preprocessing: preprocessing.map(|all| {
                            if all {
                                Preprocessing::All
                            } else {
                                Preprocessing::Angle
                            }
                        }),
                    }
                }
            };
        }
        Some(TransformTree::Drawn(shape))
    }
}

/// What the fidelity spec added to the renderer: the map's `ANGLE`, and whether the map and its
/// layers are in a projection, which a rotated map reprojects through its geotransform; whether a
/// circle layer is drawn; whether the first style is `ANTIALIAS FALSE`; the TrueType symbol; and a
/// class style's `GEOMTRANSFORM` tree on the polygon and line layers.
#[derive(Arbitrary, Debug, Default)]
struct Fidelity {
    /// The map's `ANGLE`, as [`Fidelity::angle`] reads it.
    angle: Option<f64>,
    projected: bool,
    circle: bool,
    aliased: bool,
    truetype: Option<TrueTypeInput>,
    transform: TransformInput,
}

impl Fidelity {
    /// The map's `ANGLE` in whole degrees: none outside ±[`MAX_MAP_ANGLE`], which the readers
    /// drop, or for no number, and 0, which rotates nothing, below half a degree, so that a huge
    /// angle leaves the map unrotated rather than without a geotransform. The proj target fuzzes
    /// the geotransform of any angle.
    fn angle(&self) -> Option<f64> {
        self.angle
            .filter(|angle| (-MAX_MAP_ANGLE..=MAX_MAP_ANGLE).contains(angle))
            .map(f64::round)
    }
}

/// A TrueType symbol: its character, its font, the test font or the bytes given, and its style's
/// `SIZE`, `ANGLE`, `GAP` along a line, and whether it has an `OUTLINECOLOR` of an outline width.
#[derive(Arbitrary, Debug)]
struct TrueTypeInput {
    character: String,
    font: Option<Vec<u8>>,
    size: Option<f64>,
    angle: f64,
    gap: Option<f64>,
    outline_width: Option<f64>,
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

/// The map the render target draws, the features of each of its layers, and the files it loads,
/// as the target decodes them from its bytes.
pub struct Decoded {
    pub map: Map,
    /// The features of every layer, each with its index among them as its id.
    pub features: Vec<Feature>,
    files: Files,
}

impl Decoded {
    /// The map drawn by the engine with `renderer`.
    pub fn render<R: Renderer>(&self, renderer: R) -> Rendered {
        let sources: BTreeMap<String, Shapes> = self
            .map
            .layers
            .iter()
            .filter_map(|layer| layer.name.clone())
            .map(|name| (name, Shapes(self.features.clone())))
            .collect();
        let render = forager_engine::render_with_files(&self.map, &sources, &self.files, renderer);
        forager_ports::poll_once(render).expect("the engine waited on a port that never waits")
    }
}

/// What the render target builds of `data`, or `None` where its bytes do not read as an `Input`.
pub fn decode(data: &[u8]) -> Option<Decoded> {
    let input = Input::arbitrary_take_rest(Unstructured::new(data)).ok()?;
    let reversed: Vec<u8> = data.iter().rev().copied().collect();
    let further = Styles::arbitrary_take_rest(Unstructured::new(&reversed))
        .map_or_else(|_| Vec::new(), |styles| styles.0);
    let exprs = Expressions::arbitrary_take_rest(Unstructured::new(&data[data.len() / 2..]))
        .unwrap_or_default();
    let cache = Cache::arbitrary(&mut Unstructured::new(&reversed)).unwrap_or_default();
    let transform =
        TransformInput::arbitrary_take_rest(Unstructured::new(&data[data.len() / 3 * 2..]))
            .unwrap_or_default()
            .tree();
    let head: Vec<u8> = data[..data.len() / 3].iter().rev().copied().collect();
    let fidelity = Fidelity::arbitrary_take_rest(Unstructured::new(&head)).unwrap_or_default();
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
        aliased: fidelity.aliased,
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
    let truetype = fidelity.truetype.as_ref();
    if let Some(glyph) = truetype {
        let font = if glyph.font.is_some() {
            INPUT_FONT_NAME
        } else {
            "sans"
        };
        symbols.push(Symbol {
            kind: SymbolKind::Truetype,
            paths: Vec::new(),
            extent: None,
            filled: false,
            character: Some(glyph.character.clone()),
            font: Some(font.to_owned()),
            ..symbols[0].clone()
        });
    }
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
    let class_transform = fidelity.transform.tree().map(|tree| Style {
        geom_transform: Some(GeomTransform::Expression(tree)),
        ..styles[0].clone()
    });
    let mut points = layer("points", RenderMode::Point, styles.clone(), &exprs);
    for class in &mut points.classes {
        class.labels.push(label.clone());
    }
    let [min_x, min_y, max_x, max_y] = input.extent;
    let mut map = Map {
        resolution: Some(input.resolution),
        fonts: [
            ("sans".to_owned(), "sans.ttf".to_owned()),
            (INPUT_FONT_NAME.to_owned(), INPUT_FONT.to_owned()),
        ]
        .into_iter()
        .collect(),
        angle: fidelity.angle(),
        projection: fidelity.projected.then(|| LONGLAT.to_owned()),
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
    for layer in &mut map.layers[..2] {
        layer.geom_transform = transform.clone();
        if let Some(style) = &class_transform {
            for class in &mut layer.classes {
                class.styles.push(style.clone());
            }
        }
    }
    if fidelity.circle {
        let layer = layer("circles", RenderMode::Circle, styles.clone(), &exprs);
        map.layers.push(layer);
    }
    if let Some(glyph) = truetype {
        let glyph_style = Style {
            symbol: Some(Bound::Value(SymbolRef::Index(4))),
            size: glyph.size.map(Bound::Value),
            angle: Some(Bound::Value(glyph.angle)),
            outline_color: glyph
                .outline_width
                .map(|_| Bound::Value(Rgba::opaque(0, 0, 0))),
            outline_width: glyph.outline_width.map(Bound::Value),
            ..styles[0].clone()
        };
        let along = Style {
            gap: glyph.gap,
            ..glyph_style.clone()
        };
        map.layers.extend([
            layer(
                "glyph-points",
                RenderMode::Point,
                vec![glyph_style.clone()],
                &exprs,
            ),
            layer("glyph-lines", RenderMode::Line, vec![along], &exprs),
            layer(
                "glyph-fills",
                RenderMode::Polygon,
                vec![glyph_style],
                &exprs,
            ),
        ]);
    }
    if fidelity.projected {
        for layer in &mut map.layers {
            layer.projection = Some(LayerProjection::Proj4(LONGLAT.to_owned()));
        }
    }
    // One bit for each layer; a layer past the eighth is drawn before the cache.
    for (layer, bit) in map.layers.iter_mut().zip(0u32..) {
        layer.max_features = cache.max_features.map(u32::from);
        layer.post_label_cache = cache.post_label_cache.checked_shr(bit).unwrap_or(0) & 1 == 1;
    }
    let files = Files {
        font: truetype
            .and_then(|glyph| glyph.font.clone())
            .unwrap_or_default(),
    };
    Some(Decoded {
        map,
        features,
        files,
    })
}
