//! The text path on an arbitrary font file: the input is the one font of the font set, and a
//! label of every printable ASCII character and `í`, the `?` among them, is drawn with it at a
//! point and at a size large enough to reach past the image (Q-3, ADR-0028).
// Verifies: Q-3, ADR-0028
#![no_main]

use std::collections::BTreeMap;

use forager_ports::{
    Coord, Feature, FeatureQuery, FeatureSource, Features, Geometry, Properties, ResourceLoader,
    ResourcePath, SourceError,
};
use forager_render_agg::AggRenderer;
use forager_style_ir::{Bound, Class, Expr, Extent, Label, Layer, LayerStatus, Map, RenderMode};
use libfuzzer_sys::fuzz_target;

/// One point in the middle of the image.
struct Point;

impl FeatureSource for Point {
    async fn features(&self, _: &FeatureQuery) -> Result<Features, SourceError> {
        let feature = Feature {
            id: 0,
            geometry: Geometry::Point(Coord { x: 50.0, y: 50.0 }),
            properties: Properties::default(),
        };
        Ok(vec![feature].into())
    }
}

/// The input as the font file.
struct Font<'a>(&'a [u8]);

impl ResourceLoader for Font<'_> {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Ok(self.0.to_vec())
    }
}

fuzz_target!(|data: &[u8]| {
    let text: String = (' '..='~').chain(['í']).collect();
    let layers = [12.0, 256.0]
        .into_iter()
        .map(|size| Layer {
            name: Some("point".into()),
            render_mode: Some(RenderMode::Point),
            status: Some(LayerStatus::On),
            classes: vec![Class {
                labels: vec![Label {
                    text: Some(Expr::String(text.clone())),
                    font: Some(Bound::Value("font".to_owned())),
                    size: Some(Bound::Value(size)),
                    ..Label::default()
                }],
                ..Class::default()
            }],
            ..Layer::default()
        })
        .collect();
    let map = Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        }),
        size: Some([101, 101]),
        fonts: [("font".to_owned(), "font.ttf".to_owned())].into(),
        layers,
        ..Map::default()
    };
    let sources: BTreeMap<String, Point> = [("point".to_owned(), Point)].into();
    let font = Font(data);
    let render = forager_engine::render_with_files(&map, &sources, &font, AggRenderer::new());
    let _ = forager_ports::poll_once(render).expect("the engine waited on a port that never waits");
});
