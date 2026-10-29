//! The renderer on structured input: arbitrary points, lines, and polygons with any `f64` —
//! NaN, infinities, and the extremes included — in an arbitrary view, drawn by the engine with
//! agg-rust behind its clipping (Q-3, ADR-0015). JSON cannot carry such numbers, so the GeoJSON
//! target never reaches these paths. The points carry a label of arbitrary text, size, bounds, and
//! resolution, in a test font of the label crate, drawn through the text path (ADR-0028).
// Verifies: Q-3, ADR-0015
#![no_main]

use std::collections::BTreeMap;

use arbitrary::Arbitrary;
use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Features, Geometry, Properties, ResourceLoader,
    ResourcePath, SourceError,
};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, Label, Layer, LayerStatus, Map, RenderMode, Rgba, Style,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    width: u8,
    height: u8,
    extent: [f64; 4],
    line_width: f64,
    symbol_size: f64,
    shapes: Vec<Shape>,
    text: String,
    label_size: f64,
    min_size: f64,
    max_size: f64,
    resolution: f64,
}

/// A subset of DejaVu Sans, whose licence lies beside it, as the one font of the font set.
const SANS: &[u8] = include_bytes!("../../crates/label/tests/fixtures/dejavu-sans-subset.ttf");

struct Font;

impl ResourceLoader for Font {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Ok(SANS.to_vec())
    }
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

fn layer(name: &str, mode: RenderMode, style: Style) -> Layer {
    Layer {
        name: Some(name.into()),
        render_mode: Some(mode),
        status: Some(LayerStatus::On),
        classes: vec![Class {
            styles: vec![style],
            ..Class::default()
        }],
        ..Layer::default()
    }
}

fuzz_target!(|input: Input| {
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
    let label = Label {
        text: Some(Expr::String(input.text)),
        font: Some(Bound::Value("sans".to_owned())),
        size: Some(Bound::Value(input.label_size)),
        min_size: Some(input.min_size),
        max_size: Some(input.max_size),
        ..Label::default()
    };
    let mut points = layer("points", RenderMode::Point, style.clone());
    points.classes[0].labels.push(label);
    let [min_x, min_y, max_x, max_y] = input.extent;
    let map = Map {
        resolution: Some(input.resolution),
        fonts: [("sans".to_owned(), "sans.ttf".to_owned())].into(),
        extent: Some(Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        }),
        size: Some([u32::from(input.width), u32::from(input.height)]),
        layers: vec![
            layer("polygons", RenderMode::Polygon, style.clone()),
            layer("lines", RenderMode::Line, style.clone()),
            points,
        ],
        ..Map::default()
    };
    let sources: BTreeMap<String, Shapes> = ["polygons", "lines", "points"]
        .into_iter()
        .map(|name| (name.to_owned(), Shapes(features.clone())))
        .collect();
    let render = forager_engine::render_with_files(&map, &sources, &Font, AggRenderer::new());
    let _ = forager_ports::poll_once(render).expect("the engine waited on a port that never waits");
});
