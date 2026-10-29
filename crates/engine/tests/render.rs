//! The engine end to end: a style IR and GeoJSON in, an RGBA image out, compared with a reference
//! image of Forager's own (spec of the browser tracer, Testing decisions). Set FORAGER_BLESS=1 to
//! write the references after a deliberate change, and review the images before committing them.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{MAX_IMAGE_SIDE, Rendered, render};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Span, Style,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const GREEN: Rgba = Rgba::opaque(200, 220, 180);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a source that never waits")
}

/// A 101 × 101 image of the extent 0..100, so that one map unit is one pixel.
fn map(mode: RenderMode, style: Style) -> Map {
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
            render_mode: Some(mode),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                span: Some(Span::new("m.map", 5, 3)),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                styles: vec![style],
                ..Class::default()
            }],
            span: Some(Span::new("m.map", 2, 1)),
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render_with(map: &Map, geojson: Option<&str>) -> Rendered {
    let sources: BTreeMap<String, GeoJsonSource> = geojson
        .map(|text| ("data".to_owned(), GeoJsonSource::parse(text).unwrap()))
        .into_iter()
        .collect();
    ready(render(map, &sources, AggRenderer::new()))
}

fn assert_matches_reference(image: &RgbaImage, name: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    if std::env::var_os("FORAGER_BLESS").is_some() {
        std::fs::write(&path, forager_encode::png(image).unwrap()).unwrap();
    }
    let reference = forager_encode::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    assert!(
        reference == *image,
        "{name} differs from its reference image"
    );
}

const SQUARE_WITH_HOLE: &str = r#"{"type": "Polygon", "coordinates": [
    [[10, 10], [90, 10], [90, 90], [10, 90], [10, 10]],
    [[40, 40], [60, 40], [60, 60], [40, 60], [40, 40]]]}"#;

#[test]
fn a_polygon_layer_is_filled_and_outlined() {
    let style = Style {
        color: Some(Bound::Value(GREEN)),
        outline_color: Some(Bound::Value(BLACK)),
        width: Some(Bound::Value(2.0)),
        ..Style::default()
    };
    let rendered = render_with(&map(RenderMode::Polygon, style), Some(SQUARE_WITH_HOLE));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // Map y grows upwards, pixel y downwards: (25, 25) in map units is pixel (25, 75).
    assert_eq!(image.pixel(25, 75), Some(GREEN));
    assert_eq!(image.pixel(50, 50), Some(WHITE), "the hole stays empty");
    assert_eq!(image.pixel(5, 5), Some(WHITE));
    assert_eq!(
        image.pixel(10, 50),
        Some(BLACK),
        "the outline runs along x = 10"
    );
    assert_matches_reference(&image, "polygon.png");
}

#[test]
fn a_line_layer_strokes_lines_and_outlines_polygons() {
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        width: Some(Bound::Value(3.0)),
        ..Style::default()
    };
    let collection = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {}, "geometry":
            {"type": "LineString", "coordinates": [[0, 20], [100, 20]]}},
        {"type": "Feature", "properties": {}, "geometry":
            {"type": "Polygon", "coordinates": [[[30, 40], [70, 40], [70, 80], [30, 80], [30, 40]]]}}
    ]}"#;
    let rendered = render_with(&map(RenderMode::Line, style), Some(collection));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(50, 80), Some(BLUE), "the line at y = 20");
    assert_eq!(image.pixel(30, 40), Some(BLUE), "the outline at x = 30");
    assert_eq!(
        image.pixel(50, 40),
        Some(WHITE),
        "a line layer does not fill"
    );
    assert_matches_reference(&image, "line.png");
}

#[test]
fn a_layer_without_a_source_is_skipped_with_a_diagnostic() {
    // Verifies: G-8
    let rendered = render_with(&map(RenderMode::Polygon, Style::default()), None);
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
    assert_eq!(rendered.diagnostics.len(), 1);
    let d = &rendered.diagnostics[0];
    assert_eq!(d.code, "layer-skipped");
    assert!(
        d.message.contains("`data`") && d.message.contains("local"),
        "{}",
        d.message
    );
    assert_eq!(d.span, Some(Span::new("m.map", 5, 3)));
}

#[test]
fn an_image_past_the_size_limit_is_an_error() {
    // Verifies: Q-9
    let mut big = map(RenderMode::Polygon, Style::default());
    big.size = Some([MAX_IMAGE_SIDE + 1, 10]);
    let rendered = render_with(&big, Some(SQUARE_WITH_HOLE));
    assert!(rendered.image.is_none());
    assert_eq!(rendered.diagnostics[0].code, "image-too-large");
}

#[test]
fn a_layer_without_status_is_off_as_in_mapserver() {
    let mut no_status = map(
        RenderMode::Polygon,
        Style {
            color: Some(Bound::Value(GREEN)),
            ..Style::default()
        },
    );
    no_status.layers[0].status = None;
    let rendered = render_with(&no_status, Some(SQUARE_WITH_HOLE));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.image.unwrap().pixel(25, 75), Some(WHITE));
}

#[test]
fn a_geometry_the_render_mode_cannot_draw_is_reported() {
    // Verifies: Q-3
    let line = r#"{"type": "LineString", "coordinates": [[0, 50], [100, 50]]}"#;
    let rendered = render_with(
        &map(
            RenderMode::Polygon,
            Style {
                color: Some(Bound::Value(GREEN)),
                ..Style::default()
            },
        ),
        Some(line),
    );
    assert_eq!(rendered.diagnostics.len(), 1, "{:?}", rendered.diagnostics);
    assert_eq!(rendered.diagnostics[0].code, "geometry-not-drawn");
    assert_eq!(rendered.diagnostics[0].span, Some(Span::new("m.map", 2, 1)));
}

#[test]
fn a_point_layer_marks_every_vertex() {
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(5.0)),
        ..Style::default()
    };
    let line = r#"{"type": "LineString", "coordinates": [[20, 20], [80, 80]]}"#;
    let image = render_with(&map(RenderMode::Point, style), Some(line))
        .image
        .unwrap();
    assert_eq!(image.pixel(20, 80), Some(BLUE));
    assert_eq!(image.pixel(80, 20), Some(BLUE));
    assert_eq!(
        image.pixel(50, 50),
        Some(WHITE),
        "no line between the vertices"
    );
}

#[test]
fn line_width_is_capped_at_mapservers_maxwidth() {
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        width: Some(Bound::Value(1e6)),
        ..Style::default()
    };
    let line = r#"{"type": "LineString", "coordinates": [[0, 50], [100, 50]]}"#;
    let image = render_with(&map(RenderMode::Line, style), Some(line))
        .image
        .unwrap();
    assert_eq!(image.pixel(50, 50 + 14), Some(BLUE), "inside 32 px");
    assert_eq!(image.pixel(50, 50 + 18), Some(WHITE), "outside 32 px");
}

/// A source that delivers one square and reports `count` records skipped.
struct Skipping {
    count: u64,
}

impl forager_ports::FeatureSource for Skipping {
    async fn features(
        &self,
        query: &forager_ports::FeatureQuery,
    ) -> Result<forager_ports::Features, forager_ports::SourceError> {
        let square = GeoJsonSource::parse(SQUARE_WITH_HOLE).unwrap();
        let mut delivered = square.features(query).await?;
        for i in 0..self.count {
            delivered.skipped.add(|| format!("record {i} is broken"));
        }
        Ok(delivered)
    }
}

fn render_skipping(count: u64) -> Rendered {
    let sources = BTreeMap::from([("data".to_owned(), Skipping { count })]);
    let map = map(RenderMode::Polygon, Style::default());
    ready(render(&map, &sources, AggRenderer::new()))
}

#[test]
fn a_source_that_skips_records_yields_one_warning_for_its_layer() {
    // Verifies: ADR-0025
    let rendered = render_skipping(3);
    let skipped: Vec<_> = rendered
        .diagnostics
        .iter()
        .filter(|d| d.code == "source-records-skipped")
        .collect();
    assert_eq!(skipped.len(), 1, "{:?}", rendered.diagnostics);
    let d = skipped[0];
    for part in ["`data`", "local", "3 records", "record 0 is broken"] {
        assert!(d.message.contains(part), "{part}: {}", d.message);
    }
    assert!(!d.message.contains("record 1 is broken"), "{}", d.message);
    assert_eq!(d.span, Some(Span::new("m.map", 5, 3)));
    // The layer is not skipped for it.
    assert!(
        rendered
            .diagnostics
            .iter()
            .all(|d| d.code != "layer-skipped")
    );
}

#[test]
fn a_source_that_skips_none_yields_no_such_warning() {
    // Verifies: ADR-0025
    let rendered = render_skipping(0);
    assert!(
        rendered
            .diagnostics
            .iter()
            .all(|d| d.code != "source-records-skipped"),
        "{:?}",
        rendered.diagnostics
    );
}
