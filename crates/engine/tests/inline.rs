//! A layer's inline features drawn as its source (ADR-0033).

use std::collections::BTreeMap;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::render;
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, FeatureProperty, InlineFeature, Layer, LayerStatus, Map, Op,
    RenderMode, Rgba, ShapeType, Style,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const GREEN: Rgba = Rgba::opaque(200, 220, 180);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

fn square(min: f64, max: f64) -> Vec<[f64; 2]> {
    vec![[min, min], [max, min], [max, max], [min, max], [min, min]]
}

fn feature(parts: Vec<Vec<[f64; 2]>>, kind: &str) -> InlineFeature {
    InlineFeature {
        shape_type: ShapeType::Polygon,
        parts,
        properties: vec![FeatureProperty {
            name: "Kind".to_owned(),
            value: kind.to_owned(),
        }],
        text: None,
        span: None,
    }
}

fn fill(color: Rgba) -> Style {
    Style {
        color: Some(Bound::Value(color)),
        ..Style::default()
    }
}

/// A 101 × 101 image of the extent 0..100, one map unit a pixel, of one polygon layer `data`
/// whose first class draws the features of kind `a` green, and its second every other blue.
fn map(features: Vec<InlineFeature>) -> Map {
    let kind_a = Expr::call(
        Op::Eq,
        vec![
            Expr::call(Op::ToString, vec![Expr::property("KIND")]),
            "a".into(),
        ],
    );
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        }),
        size: Some([101, 101]),
        image_color: Some(WHITE),
        layers: vec![Layer {
            name: Some("data".into()),
            render_mode: Some(RenderMode::Polygon),
            status: Some(LayerStatus::On),
            classes: vec![
                Class {
                    expression: Some(kind_a),
                    styles: vec![fill(GREEN)],
                    ..Class::default()
                },
                Class {
                    styles: vec![fill(BLUE)],
                    ..Class::default()
                },
            ],
            features,
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render_with(map: &Map, geojson: Option<&str>) -> forager_engine::Rendered {
    let sources: BTreeMap<String, GeoJsonSource> = geojson
        .map(|text| ("data".to_owned(), GeoJsonSource::parse(text).unwrap()))
        .into_iter()
        .collect();
    forager_ports::poll_once(render(map, &sources, AggRenderer::new())).unwrap()
}

#[test]
fn a_layers_inline_features_are_drawn_in_place_of_a_bound_source() {
    // Verifies: ADR-0033
    let map = map(vec![
        feature(vec![square(10.0, 30.0)], "a"),
        feature(vec![square(60.0, 80.0)], "b"),
    ]);
    let bound = r#"{"type": "Polygon", "coordinates": [
        [[40, 40], [50, 40], [50, 50], [40, 50], [40, 40]]]}"#;
    let rendered = render_with(&map, Some(bound));
    let codes: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["source-ignored"]);
    let image = rendered.image.unwrap();
    // Map y grows upwards, pixel y downwards.
    assert_eq!(image.pixel(20, 80), Some(GREEN), "the feature of kind a");
    assert_eq!(image.pixel(70, 30), Some(BLUE), "the feature of kind b");
    assert_eq!(
        image.pixel(45, 55),
        Some(WHITE),
        "the bound source is not read"
    );
}

#[test]
fn an_inline_layer_needs_no_bound_source() {
    let rendered = render_with(&map(vec![feature(vec![square(10.0, 30.0)], "a")]), None);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.image.unwrap().pixel(20, 80), Some(GREEN));
}

#[test]
fn a_feature_without_points_draws_nothing() {
    let rendered = render_with(
        &map(vec![feature(vec![], "a"), feature(vec![vec![]], "a")]),
        None,
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert!((0..101).all(|y| (0..101).all(|x| image.pixel(x, y) == Some(WHITE))));
}
