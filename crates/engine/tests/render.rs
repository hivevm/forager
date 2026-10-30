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
    Bound, Class, Extent, Layer, LayerStatus, LineCap, LineJoin, LineOffset, Map, RenderMode, Rgba,
    SourceRef, Span, Style, Symbol, SymbolKind, SymbolRef,
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
    map_with(mode, vec![style])
}

/// [`map`] with a class of several styles.
fn map_with(mode: RenderMode, styles: Vec<Style>) -> Map {
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
                styles,
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

const RED: Rgba = Rgba::opaque(220, 30, 30);

fn line_style(color: Rgba, width: f64) -> Style {
    Style {
        color: Some(Bound::Value(color)),
        width: Some(Bound::Value(width)),
        ..Style::default()
    }
}

fn render_lines(styles: Vec<Style>, geojson: &str) -> RgbaImage {
    let rendered = render_with(&map_with(RenderMode::Line, styles), Some(geojson));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    rendered.image.unwrap()
}

const HORIZONTAL: &str = r#"{"type": "LineString", "coordinates": [[10, 50], [90, 50]]}"#;

#[test]
fn a_pattern_dashes_a_line_with_its_cap() {
    let style = Style {
        pattern: vec![12.0, 6.0, 6.0, 6.0],
        line_cap: Some(LineCap::Butt),
        ..line_style(BLUE, 3.0)
    };
    let image = render_lines(vec![style], HORIZONTAL);
    // From x = 10: a dash to 22, a gap to 28, a dash to 34, a gap to 40, a dash to 52.
    assert_eq!(image.pixel(15, 50), Some(BLUE), "the first dash");
    assert_eq!(image.pixel(25, 50), Some(WHITE), "the first gap");
    assert_eq!(image.pixel(31, 50), Some(BLUE), "the second dash");
    assert_eq!(image.pixel(37, 50), Some(WHITE), "the second gap");
    assert_eq!(
        image.pixel(8, 50),
        Some(WHITE),
        "a butt cap ends at the line"
    );
    assert_matches_reference(&image, "line_pattern.png");
}

#[test]
fn an_initial_gap_starts_the_pattern_that_far_before_its_end() {
    let style = Style {
        pattern: vec![10.0, 10.0],
        initial_gap: Some(5.0),
        line_cap: Some(LineCap::Butt),
        ..line_style(BLUE, 3.0)
    };
    let image = render_lines(vec![style], HORIZONTAL);
    // The line starts 5 pixels into the gap: blank to 15, a dash to 25, blank to 35.
    assert_eq!(image.pixel(12, 50), Some(WHITE));
    assert_eq!(image.pixel(20, 50), Some(BLUE));
    assert_eq!(image.pixel(30, 50), Some(WHITE));
}

#[test]
fn the_cap_and_the_join_of_a_style_shape_a_wide_line() {
    let corner = r#"{"type": "LineString", "coordinates": [[20, 80], [80, 80], [80, 20]]}"#;
    let style = |cap, join| Style {
        line_cap: Some(cap),
        line_join: Some(join),
        ..line_style(BLUE, 10.0)
    };
    let square = render_lines(vec![style(LineCap::Square, LineJoin::Miter)], corner);
    // The corner is pixel (80, 20); the outer corner of a miter join is (85, 15).
    assert_eq!(square.pixel(84, 16), Some(BLUE), "a miter join");
    assert_eq!(square.pixel(17, 20), Some(BLUE), "a square cap");
    let butt = render_lines(vec![style(LineCap::Butt, LineJoin::Bevel)], corner);
    assert_eq!(butt.pixel(84, 16), Some(WHITE), "a bevel join");
    assert_eq!(butt.pixel(17, 20), Some(WHITE), "a butt cap");
    assert_matches_reference(&square, "line_cap_join.png");
}

#[test]
fn offsets_shift_a_line_or_draw_it_parallel() {
    let parallel = |distance, both_sides| Style {
        line_offset: Some(LineOffset {
            distance: Bound::Value(distance),
            both_sides,
        }),
        ..line_style(RED, 2.0)
    };
    let shifted = Style {
        offset: Some([Bound::Value(0.0), Bound::Value(-30.0)]),
        ..line_style(GREEN, 2.0)
    };
    let styles = vec![
        line_style(BLUE, 2.0),
        parallel(10.0, false),
        parallel(-40.0, true),
        shifted,
    ];
    let image = render_lines(styles, HORIZONTAL);
    // The line runs rightwards at pixel row 50: the parallel at +10 lies below it, at row 60.
    assert_eq!(image.pixel(50, 50), Some(BLUE));
    assert_eq!(image.pixel(50, 60), Some(RED), "parallel at 10");
    assert_eq!(image.pixel(50, 30), Some(RED), "both sides at 20 above");
    assert_eq!(image.pixel(50, 70), Some(RED), "both sides at 20 below");
    assert_eq!(image.pixel(50, 20), Some(GREEN), "shifted 30 up");
    assert_matches_reference(&image, "line_offset.png");
}

#[test]
fn an_outline_is_drawn_under_the_lines_of_every_feature() {
    // A line layer draws every feature's outline before any feature's line, as MapServer does.
    let crossing = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {}, "geometry":
            {"type": "LineString", "coordinates": [[10, 50], [90, 50]]}},
        {"type": "Feature", "properties": {}, "geometry":
            {"type": "LineString", "coordinates": [[50, 10], [50, 90]]}}
    ]}"#;
    let style = Style {
        outline_color: Some(Bound::Value(BLACK)),
        outline_width: Some(Bound::Value(2.0)),
        line_cap: Some(LineCap::Butt),
        ..line_style(GREEN, 6.0)
    };
    let image = render_lines(vec![style], crossing);
    assert_eq!(image.pixel(30, 50), Some(GREEN), "the line");
    assert_eq!(
        image.pixel(30, 46),
        Some(BLACK),
        "its outline, 2 pixels beyond"
    );
    assert_eq!(image.pixel(30, 43), Some(WHITE));
    assert_eq!(
        image.pixel(50, 46),
        Some(GREEN),
        "the second line's outline stays under the first line"
    );
    assert_matches_reference(&image, "line_outline.png");
}

#[test]
fn a_polygon_style_with_an_offset_moves_its_fill() {
    let style = Style {
        color: Some(Bound::Value(GREEN)),
        offset: Some([Bound::Value(5.0), Bound::Value(5.0)]),
        ..Style::default()
    };
    let image = render_with(&map(RenderMode::Polygon, style), Some(SQUARE_WITH_HOLE))
        .image
        .unwrap();
    // The square spans pixels 10..90; moved 5 right and 5 down.
    assert_eq!(image.pixel(12, 50), Some(WHITE));
    assert_eq!(image.pixel(93, 50), Some(GREEN));
    assert_eq!(image.pixel(50, 93), Some(GREEN));
}

#[test]
fn a_polygon_without_a_color_draws_its_outline_as_a_styled_line() {
    let style = Style {
        outline_color: Some(Bound::Value(BLACK)),
        width: Some(Bound::Value(3.0)),
        pattern: vec![10.0, 10.0],
        line_cap: Some(LineCap::Butt),
        ..Style::default()
    };
    let image = render_with(&map(RenderMode::Polygon, style), Some(SQUARE_WITH_HOLE))
        .image
        .unwrap();
    // The outer ring starts at pixel (10, 90) and runs rightwards: a dash, then a gap.
    assert_eq!(image.pixel(15, 90), Some(BLACK));
    assert_eq!(image.pixel(25, 90), Some(WHITE));
}

#[test]
fn a_ring_in_a_line_layer_is_stroked_open_at_its_first_vertex() {
    // MapServer strokes a ring as a line back to its first point: caps there, no join.
    let square = r#"{"type": "Polygon", "coordinates":
        [[[20, 20], [80, 20], [80, 80], [20, 80], [20, 20]]]}"#;
    let style = Style {
        line_cap: Some(LineCap::Butt),
        line_join: Some(LineJoin::Miter),
        ..line_style(BLUE, 10.0)
    };
    let image = render_lines(vec![style], square);
    // The first vertex, map (20, 20), is pixel (20, 80); the next corner is pixel (80, 80).
    assert_eq!(
        image.pixel(17, 83),
        Some(WHITE),
        "no join at the first vertex"
    );
    assert_eq!(image.pixel(83, 83), Some(BLUE), "a miter join at the next");
}

#[test]
fn a_geometry_a_layer_cannot_draw_counts_once_whatever_its_styles() {
    let collection = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {}, "geometry": {"type": "Point", "coordinates": [5, 5]}},
        {"type": "Feature", "properties": {}, "geometry": {"type": "LineString",
            "coordinates": [[0, 50], [100, 50]]}}
    ]}"#;
    let styles = vec![line_style(BLUE, 1.0), line_style(RED, 1.0)];
    let rendered = render_with(&map_with(RenderMode::Line, styles), Some(collection));
    assert_eq!(rendered.diagnostics.len(), 1, "{:?}", rendered.diagnostics);
    let message = &rendered.diagnostics[0].message;
    assert!(message.contains("1 point geometry"), "{message}");
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

/// An arrow 4 wide and 6 high, pointing up, with its pen lifted between shaft and head; and a
/// circle and an ellipse twice as wide as high.
fn symbols() -> Vec<Symbol> {
    let symbol = |kind, paths, extent, filled| Symbol {
        name: None,
        kind,
        paths,
        extent: Some(extent),
        filled,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    };
    let arrow = vec![
        vec![[0.0, 2.0], [2.0, 0.0], [4.0, 2.0]],
        vec![[2.0, 0.0], [2.0, 6.0]],
    ];
    let head = vec![vec![[0.0, 3.0], [2.0, 0.0], [4.0, 3.0], [2.0, 2.0]]];
    vec![
        symbol(SymbolKind::Vector, arrow, [4.0, 6.0], false),
        symbol(SymbolKind::Vector, head, [4.0, 3.0], true),
        symbol(
            SymbolKind::Ellipse,
            vec![vec![[1.0, 1.0]]],
            [1.0, 1.0],
            true,
        ),
        symbol(
            SymbolKind::Ellipse,
            vec![vec![[2.0, 1.0]]],
            [2.0, 1.0],
            false,
        ),
    ]
}

/// Draws `style` of symbol `index` at three points, at the angles 0, 45, and 90 by a layer each:
/// at the pixels (25, 25), (50, 50), and (75, 75).
fn render_symbol(index: u32, style: Style) -> RgbaImage {
    let mut map = map(RenderMode::Point, Style::default());
    map.symbols = symbols();
    let layer = map.layers.remove(0);
    for (i, angle) in [0.0, 45.0, 90.0].into_iter().enumerate() {
        let mut layer = layer.clone();
        layer.name = Some(format!("data{i}"));
        layer.classes[0].styles = vec![Style {
            symbol: Some(Bound::Value(SymbolRef::Index(index))),
            angle: Some(Bound::Value(angle)),
            ..style.clone()
        }];
        map.layers.push(layer);
    }
    let points = [(25, 75), (50, 50), (75, 25)];
    let sources: BTreeMap<String, GeoJsonSource> = points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let point = format!(r#"{{"type": "Point", "coordinates": [{x}, {y}]}}"#);
            (format!("data{i}"), GeoJsonSource::parse(&point).unwrap())
        })
        .collect();
    let rendered = ready(render(&map, &sources, AggRenderer::new()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    rendered.image.unwrap()
}

#[test]
fn a_vector_symbol_not_filled_is_stroked_at_its_size_and_angle() {
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(24.0)),
        width: Some(Bound::Value(2.0)),
        ..Style::default()
    };
    let image = render_symbol(1, style);
    assert_eq!(image.pixel(25, 30), Some(BLUE), "the upright shaft");
    assert_eq!(image.pixel(20, 30), Some(WHITE), "beside the shaft");
    assert_matches_reference(&image, "symbol_vector.png");
}

#[test]
fn a_filled_vector_symbol_is_filled_and_outlined_at_its_size_and_angle() {
    let style = Style {
        color: Some(Bound::Value(GREEN)),
        outline_color: Some(Bound::Value(BLACK)),
        size: Some(Bound::Value(20.0)),
        width: Some(Bound::Value(1.5)),
        ..Style::default()
    };
    let image = render_symbol(2, style);
    assert_eq!(image.pixel(25, 25), Some(GREEN), "inside the upright head");
    assert_matches_reference(&image, "symbol_vector_filled.png");
}

#[test]
fn an_ellipse_is_drawn_at_its_size_and_angle() {
    let circle = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(12.0)),
        ..Style::default()
    };
    let image = render_symbol(3, circle);
    assert_eq!(image.pixel(50, 50), Some(BLUE));
    assert_eq!(image.pixel(50, 57), Some(WHITE), "a radius of 6");
    assert_matches_reference(&image, "symbol_circle.png");
    let ellipse = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(10.0)),
        width: Some(Bound::Value(2.0)),
        ..Style::default()
    };
    let image = render_symbol(4, ellipse);
    assert_eq!(image.pixel(25, 25), Some(WHITE), "not filled");
    assert_eq!(image.pixel(75, 65), Some(BLUE), "turned by 90°, 20 wide");
    assert_matches_reference(&image, "symbol_ellipse.png");
}

#[test]
fn a_point_outside_the_extent_draws_no_symbol() {
    let mut map = map(
        RenderMode::Point,
        Style {
            color: Some(Bound::Value(BLUE)),
            size: Some(Bound::Value(40.0)),
            symbol: Some(Bound::Value(SymbolRef::Index(3))),
            ..Style::default()
        },
    );
    map.symbols = symbols();
    let outside = r#"{"type": "Point", "coordinates": [101, 50]}"#;
    let image = render_with(&map, Some(outside)).image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

#[test]
fn a_symbol_index_past_the_last_draws_nothing() {
    let mut map = map(
        RenderMode::Point,
        Style {
            color: Some(Bound::Value(BLUE)),
            size: Some(Bound::Value(40.0)),
            symbol: Some(Bound::Value(SymbolRef::Index(9))),
            ..Style::default()
        },
    );
    map.symbols = symbols();
    let inside = r#"{"type": "Point", "coordinates": [50, 50]}"#;
    let image = render_with(&map, Some(inside)).image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

#[test]
fn a_point_outside_the_extent_draws_no_square() {
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(40.0)),
        ..Style::default()
    };
    let outside = r#"{"type": "Point", "coordinates": [100.5, 50]}"#;
    let image = render_with(&map(RenderMode::Point, style), Some(outside))
        .image
        .unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

/// An L from pixel (10, 20) right to (90, 20), then down to (90, 90).
const ELL: &str = r#"{"type": "LineString", "coordinates": [[10, 80], [90, 80], [90, 10]]}"#;

/// A line layer of [`symbols`] drawing `styles` along [`ELL`].
fn render_markers(styles: Vec<Style>) -> Rendered {
    let mut map = map_with(RenderMode::Line, styles);
    map.symbols = symbols();
    render_with(&map, Some(ELL))
}

#[test]
fn symbols_stand_along_a_line_by_gap_and_initial_gap() {
    // The filled head, SIZE 9: a negative GAP of 20 from the INITIALGAP 10 places it at 10, 30,
    // 50, and 70 along the first segment, pixels (20, 20) to (80, 20), and carries 10 into the
    // second, (90, 30) to (90, 90), each head turned with its segment. The circle, SIZE 4, a
    // positive GAP of 25 without INITIALGAP: from 12.5, at x = 22.5, 47.5, 72.5, upright.
    let heads = Style {
        color: Some(Bound::Value(BLUE)),
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        size: Some(Bound::Value(9.0)),
        gap: Some(-20.0),
        initial_gap: Some(10.0),
        ..Style::default()
    };
    let circles = Style {
        color: Some(Bound::Value(GREEN)),
        symbol: Some(Bound::Value(SymbolRef::Index(3))),
        size: Some(Bound::Value(4.0)),
        gap: Some(25.0),
        ..Style::default()
    };
    let rendered = render_markers(vec![line_style(BLACK, 1.0), heads, circles]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // A head points up on the first segment: its apex above the line, its notch below.
    for x in [40, 60] {
        assert_eq!(image.pixel(x, 20), Some(BLUE), "a head at ({x}, 20)");
        assert_eq!(
            image.pixel(x, 18),
            Some(BLUE),
            "towards the apex at ({x}, 20)"
        );
        assert_eq!(image.pixel(x, 22), Some(WHITE), "in the notch at ({x}, 20)");
    }
    assert_ne!(
        image.pixel(30, 20),
        Some(WHITE),
        "the line between two heads"
    );
    assert_eq!(
        image.pixel(72, 20),
        Some(GREEN),
        "a circle, drawn over the line"
    );
    // On the second the heads turn by -90°, clockwise on the image: their apex to the right.
    for y in [50, 70] {
        assert_eq!(
            image.pixel(92, y),
            Some(BLUE),
            "towards the apex of the head at (90, {y})"
        );
        assert_eq!(
            image.pixel(90, y - 2),
            Some(BLUE),
            "an arm of the head at (90, {y})"
        );
    }
    assert_matches_reference(&image, "line_markers.png");
}

#[test]
fn a_polygon_outlined_with_a_symbol_and_a_gap_marks_its_rings() {
    // msDrawShadeSymbol hands a polygon without a COLOR to msDrawLineSymbol: the circles stand
    // along the outer ring from pixel (10, 90) rightwards, from 10 every 20. A filled symbol of
    // a style with only an OUTLINECOLOR is outlined, not filled (computeSymbolStyle).
    let style = Style {
        outline_color: Some(Bound::Value(BLUE)),
        symbol: Some(Bound::Value(SymbolRef::Index(3))),
        size: Some(Bound::Value(6.0)),
        gap: Some(20.0),
        ..Style::default()
    };
    let mut map = map(RenderMode::Polygon, style);
    map.symbols = symbols();
    let image = render_with(&map, Some(SQUARE_WITH_HOLE)).image.unwrap();
    assert_matches_reference(&image, "polygon_markers.png");
}

#[test]
fn a_tiny_gap_draws_no_markers_and_says_so() {
    // Verifies: Q-9
    let heads = Style {
        color: Some(Bound::Value(BLUE)),
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        size: Some(Bound::Value(9.0)),
        gap: Some(-0.0001),
        ..Style::default()
    };
    let rendered = render_markers(vec![heads]);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["line-markers-over-limit"]);
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

/// A polygon layer of [`SQUARE_WITH_HOLE`] whose class draws `styles`, with a hatch symbol at
/// index 1.
fn render_hatched(styles: Vec<Style>) -> Rendered {
    render_hatched_on(styles, SQUARE_WITH_HOLE)
}

/// [`render_hatched`] on the polygon of a GeoJSON geometry.
fn render_hatched_on(styles: Vec<Style>, geojson: &str) -> Rendered {
    let mut map = map_with(RenderMode::Polygon, styles);
    map.symbols = vec![Symbol {
        name: Some("hatch".into()),
        kind: SymbolKind::Hatch,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }];
    render_with(&map, Some(geojson))
}

fn hatch_style(color: Rgba, size: f64, width: f64, angle: f64, pattern: Vec<f64>) -> Style {
    Style {
        color: Some(Bound::Value(color)),
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(size)),
        width: Some(Bound::Value(width)),
        angle: Some(Bound::Value(angle)),
        pattern,
        ..Style::default()
    }
}

#[test]
fn a_hatch_fills_a_polygon_with_lines_at_its_angles_and_pattern() {
    let styles = vec![
        Style {
            color: Some(Bound::Value(GREEN)),
            ..Style::default()
        },
        hatch_style(BLUE, 10.0, 3.0, 45.0, vec![6.0, 3.0]),
        hatch_style(BLACK, 8.0, 2.0, 0.0, vec![]),
    ];
    let rendered = render_hatched(styles);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // The lines are counted from the map's origin, pixel (0, 100): ANGLE 0 lays one at y = 84,
    // 2 wide, over rows 83 and 84, inside the square only.
    assert_eq!(image.pixel(15, 84), Some(BLACK));
    assert_eq!(image.pixel(15, 83), Some(BLACK));
    assert_eq!(image.pixel(5, 84), Some(WHITE), "clipped to the polygon");
    for y in 40..60 {
        for x in 40..60 {
            assert_eq!(image.pixel(x, y), Some(WHITE), "({x}, {y}) in the hole");
        }
    }
    assert!(
        (11..89).any(|x| image.pixel(x, 30) == Some(BLUE)),
        "the diagonal dashes"
    );
    assert_matches_reference(&image, "polygon_hatch.png");
}

#[test]
fn a_hatch_past_its_bound_is_not_drawn_and_reported() {
    // Verifies: Q-9
    // A spacing of a billionth of a pixel would lay some 1e11 lines, dashes of a billionth of a
    // pixel cut the lines into some 1e11 pieces: neither is drawn, and the layer says so.
    let styles = vec![
        Style {
            color: Some(Bound::Value(GREEN)),
            ..Style::default()
        },
        Style {
            min_size: Some(1e-9),
            ..hatch_style(BLACK, 1e-9, 1.0, 30.0, vec![])
        },
        hatch_style(BLACK, 10.0, 1.0, 0.0, vec![1e-9, 1e-9]),
    ];
    let rendered = render_hatched(styles);
    assert_eq!(rendered.diagnostics.len(), 1, "{:?}", rendered.diagnostics);
    let diagnostic = &rendered.diagnostics[0];
    assert_eq!(diagnostic.code, "hatch-not-drawn");
    assert!(
        diagnostic.message.contains("2 hatch fills are not drawn"),
        "{}",
        diagnostic.message
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(15, 84), Some(GREEN));
}

#[test]
fn a_hatch_of_a_polygon_past_its_bound_on_vertices_is_not_drawn_and_reported() {
    // Verifies: Q-9
    // A circle of 17 000 vertices, past the bound of 16 384, under a hatch 10 apart.
    let points: Vec<String> = (0..=17_000)
        .map(|i| {
            let t = f64::from(i % 17_000) / 17_000.0 * std::f64::consts::TAU;
            format!("[{}, {}]", 50.0 + 40.0 * t.cos(), 50.0 + 40.0 * t.sin())
        })
        .collect();
    let circle = format!(
        r#"{{"type": "Polygon", "coordinates": [[{}]]}}"#,
        points.join(", ")
    );
    let rendered = render_hatched_on(vec![hatch_style(BLACK, 10.0, 1.0, 45.0, vec![])], &circle);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["hatch-not-drawn"]);
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|px| px == [255, 255, 255, 255]));
}
