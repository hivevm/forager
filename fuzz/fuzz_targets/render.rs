//! The renderer on structured input: arbitrary points, lines, and polygons, an arbitrary
//! vector or ellipse symbol at the points, and a hatch over the polygons at the symbol's size,
//! the line width, the symbol's angle, and the first further style's pattern, with any `f64` —
//! NaN, infinities, and the extremes included — in an arbitrary view, drawn by the engine with
//! agg-rust behind its clipping (Q-3, ADR-0015). JSON cannot carry such numbers, so the GeoJSON
//! target never reaches these paths. Further styles of a line's dashes, caps, joins, offsets,
//! outlines, and width bounds are read from the same bytes back to front, so that they leave how
//! an input decodes to its view, its symbol, and its shapes as it was.
// Verifies: Q-3, ADR-0015
#![no_main]

use std::collections::BTreeMap;

use arbitrary::{Arbitrary, Unstructured};
use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Features, Geometry, Properties, SourceError,
};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, LineCap, LineJoin, LineOffset, Map, RenderMode, Rgba,
    Style, Symbol, SymbolKind, SymbolRef,
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
            ..Style::default()
        }
    }
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

fn layer(name: &str, mode: RenderMode, styles: Vec<Style>) -> Layer {
    Layer {
        name: Some(name.into()),
        render_mode: Some(mode),
        status: Some(LayerStatus::On),
        classes: vec![Class {
            styles,
            ..Class::default()
        }],
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
            properties: Properties::default(),
        })
        .collect();
    let style = Style {
        color: Some(Bound::Value(Rgba::opaque(200, 80, 80))),
        outline_color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        width: Some(Bound::Value(input.line_width)),
        size: Some(Bound::Value(input.symbol_size)),
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
    let [min_x, min_y, max_x, max_y] = input.extent;
    let map = Map {
        extent: Some(Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        }),
        size: Some([u32::from(input.width), u32::from(input.height)]),
        layers: vec![
            layer("polygons", RenderMode::Polygon, styles.clone()),
            layer("lines", RenderMode::Line, styles.clone()),
            layer("points", RenderMode::Point, styles),
            layer("symbols", RenderMode::Point, vec![symbol_style]),
            layer("hatches", RenderMode::Polygon, vec![hatch_style]),
        ],
        symbols,
        ..Map::default()
    };
    let sources: BTreeMap<String, Shapes> = ["polygons", "lines", "points", "symbols", "hatches"]
        .into_iter()
        .map(|name| (name.to_owned(), Shapes(features.clone())))
        .collect();
    let _ = forager_ports::poll_once(forager_engine::render(&map, &sources, AggRenderer::new()))
        .expect("the engine waited on a source that never waits");
});
