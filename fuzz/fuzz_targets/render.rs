//! The renderer on structured input: arbitrary points, lines, and polygons with any `f64` —
//! NaN, infinities, and the extremes included — in an arbitrary view, drawn by the engine with
//! agg-rust behind its clipping (Q-3, ADR-0015). JSON cannot carry such numbers, so the GeoJSON
//! target never reaches these paths.
// Verifies: Q-3, ADR-0015
#![no_main]

use std::collections::BTreeMap;

use arbitrary::Arbitrary;
use forager_ports::{Coord, Feature, FeatureQuery, FeatureSource, Geometry, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::{Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, Style};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    width: u8,
    height: u8,
    extent: [f64; 4],
    line_width: f64,
    symbol_size: f64,
    shapes: Vec<Shape>,
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
    async fn features(&self, _: &FeatureQuery) -> Result<Vec<Feature>, SourceError> {
        Ok(self.0.clone())
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
        })
        .collect();
    let style = Style {
        color: Some(Bound::Value(Rgba::opaque(200, 80, 80))),
        outline_color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        width: Some(Bound::Value(input.line_width)),
        size: Some(Bound::Value(input.symbol_size)),
        ..Style::default()
    };
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
            layer("polygons", RenderMode::Polygon, style.clone()),
            layer("lines", RenderMode::Line, style.clone()),
            layer("points", RenderMode::Point, style),
        ],
        ..Map::default()
    };
    let sources: BTreeMap<String, Shapes> = ["polygons", "lines", "points"]
        .into_iter()
        .map(|name| (name.to_owned(), Shapes(features.clone())))
        .collect();
    let _ = forager_ports::poll_once(forager_engine::render(&map, &sources, AggRenderer::new()))
        .expect("the engine waited on a source that never waits");
});
