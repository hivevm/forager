//! A layer of the render mode circle: each feature of one part of two points drawn as the circle
//! MapServer's circleLayerDrawShape draws of it.

use std::collections::BTreeMap;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, GeomTransform, InlineFeature, Label, Layer, LayerProjection, LayerStatus,
    Map, RenderMode, Rgba, ShapeType, Style,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn feature(points: &[[f64; 2]]) -> InlineFeature {
    InlineFeature {
        shape_type: ShapeType::Point,
        parts: vec![points.to_vec()],
        properties: Vec::new(),
        text: None,
        span: None,
    }
}

/// A 101 × 101 image of the extent 0..100, one map unit a pixel, of one circle layer of
/// `features` filled black.
fn map(features: Vec<InlineFeature>) -> Map {
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
            name: Some("circles".into()),
            render_mode: Some(RenderMode::Circle),
            status: Some(LayerStatus::On),
            classes: vec![Class {
                styles: vec![Style {
                    color: Some(Bound::Value(BLACK)),
                    ..Style::default()
                }],
                ..Class::default()
            }],
            features,
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn rendered(map: &Map) -> Rendered {
    let sources = BTreeMap::<String, GeoJsonSource>::new();
    forager_ports::poll_once(render(map, &sources, AggRenderer::new())).unwrap()
}

/// The diagnostics of `rendered`, as code and message.
fn reports(rendered: &Rendered) -> Vec<(&str, &str)> {
    rendered
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect()
}

#[test]
fn a_feature_of_one_part_of_two_points_is_drawn_as_a_circle() {
    // The centre (30, 30) at the pixel (30, 70), the radius 10 pixels.
    let mut map = map(vec![feature(&[[20.0, 20.0], [40.0, 40.0]])]);
    map.layers[0].classes[0].labels = vec![Label {
        text: Some("circle".into()),
        ..Label::default()
    }];
    let rendered = rendered(&map);
    // A circle layer draws no label, as circleLayerDrawShape draws none.
    assert_eq!(
        reports(&rendered),
        [(
            "label-not-drawn",
            "a circle layer draws no label, as MapServer draws none; the label is not drawn"
        )]
    );
    assert!(rendered.labels.is_empty());
    let image = rendered.image.unwrap();
    for (x, y) in [(30, 70), (21, 70), (39, 70), (30, 61), (30, 79)] {
        assert_eq!(image.pixel(x, y), Some(BLACK), "({x}, {y}) inside");
    }
    for (x, y) in [(18, 70), (42, 70), (30, 58), (30, 82), (22, 62)] {
        assert_eq!(image.pixel(x, y), Some(WHITE), "({x}, {y}) outside");
    }
}

#[test]
fn a_feature_that_is_no_circle_draws_nothing_and_the_others_draw() {
    let rendered = rendered(&map(vec![
        // Three points, which a polygon would fill about (60, 60).
        feature(&[[50.0, 50.0], [70.0, 50.0], [60.0, 70.0]]),
        // One point.
        feature(&[[60.0, 60.0]]),
        // A radius of 0.
        feature(&[[60.0, 58.0], [60.0, 58.0]]),
        // A circle about (20, 80), drawn after them.
        feature(&[[15.0, 75.0], [25.0, 85.0]]),
    ]));
    assert_eq!(
        reports(&rendered),
        [
            (
                "geometry-not-drawn",
                "layer `circles` cannot draw 2 multipoint geometries in its render mode"
            ),
            (
                "geometry-not-drawn",
                "layer `circles` cannot draw 1 point geometry in its render mode"
            ),
        ]
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(20, 20), Some(BLACK), "the circle");
    let black = (0..101)
        .flat_map(|y| (0..101).map(move |x| (x, y)))
        .filter(|&(x, y)| image.pixel(x, y) == Some(BLACK))
        .count();
    // The circle of radius 5 alone: some 78 pixels.
    assert!((60..100).contains(&black), "{black} black pixels");
}

#[test]
fn a_feature_of_two_points_blocks_of_a_point_each_draws_nothing() {
    // loadFeature adds each POINTS block as a part of the shape (msAddLine), and
    // circleLayerDrawShape draws only a shape of one part of two points: the two points of the
    // first circle above, written as two blocks, draw nothing, and the layer warns of them.
    let two_blocks = InlineFeature {
        parts: vec![vec![[20.0, 20.0]], vec![[40.0, 40.0]]],
        ..feature(&[])
    };
    let rendered = rendered(&map(vec![two_blocks]));
    assert_eq!(
        reports(&rendered),
        [(
            "geometry-not-drawn",
            "layer `circles` cannot draw 1 multipoint geometry in its render mode"
        )]
    );
    let image = rendered.image.unwrap();
    assert!(
        (0..101).all(|y| (0..101).all(|x| image.pixel(x, y) == Some(WHITE))),
        "nothing drawn"
    );
}

#[test]
fn a_style_of_a_geomtransform_draws_its_circle_unwarned() {
    // circleLayerDrawShape draws every style in its scale range, whatever its GEOMTRANSFORM,
    // before msDrawShape would look at one.
    let mut map = map(vec![feature(&[[20.0, 20.0], [40.0, 40.0]])]);
    map.layers[0].classes[0].styles[0].geom_transform = Some(GeomTransform::LabelCenter);
    let rendered = rendered(&map);
    assert_eq!(reports(&rendered), []);
    assert_eq!(rendered.image.unwrap().pixel(30, 70), Some(BLACK));
}

#[test]
fn only_the_centre_of_a_circle_is_reprojected() {
    // Verifies: ADR-0037
    // From Mercator into Mercator of scale 2, the centre (10, -20) lands at (20, -40); the
    // radius of 10 stays 10 map units, as circleLayerDrawShape reprojects only the centre and
    // divides the radius by the map's cellsize, where a whole reprojection would make it 20.
    let mut map = map(vec![feature(&[[0.0, -30.0], [20.0, -10.0]])]);
    map.extent = Some(Extent {
        min_x: -30.0,
        min_y: -90.0,
        max_x: 70.0,
        max_y: 10.0,
    });
    map.projection = Some("+proj=merc +datum=WGS84 +k_0=2".to_owned());
    map.layers[0].projection = Some(LayerProjection::Proj4("+proj=merc +datum=WGS84".to_owned()));
    let rendered = rendered(&map);
    assert_eq!(reports(&rendered), []);
    let image = rendered.image.unwrap();
    // The centre at the pixel (50, 50).
    for (x, y) in [(50, 50), (41, 50), (59, 50)] {
        assert_eq!(image.pixel(x, y), Some(BLACK), "({x}, {y}) inside");
    }
    for (x, y) in [(38, 50), (62, 50), (50, 38), (50, 62)] {
        assert_eq!(image.pixel(x, y), Some(WHITE), "({x}, {y}) outside");
    }
}
