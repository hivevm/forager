// Some expectations of these tests follow MapServer's src/maplayer.c and src/mapdraw.c of the
// release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The engine end to end: a style IR and GeoJSON in, an RGBA image out, compared with a reference
//! image of Forager's own (spec of the browser tracer, Testing decisions). Set FORAGER_BLESS=1 to
//! write the references after a deliberate change, and review the images before committing them;
//! what the bless reports is told in bless/mod.rs.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

mod bless;

use bless::assert_matches_reference;
use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{MAX_FEATURES_PER_LAYER, MAX_IMAGE_SIDE, Rendered, render};
use forager_render::{DrawOp, RgbaImage};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, GeomTransform, Layer, LayerStatus, LineCap, LineJoin, LineOffset,
    Map, NotDrawn, NumberOp, Op, RenderMode, Rgba, SourceRef, Span, Style, Symbol, SymbolKind,
    SymbolRef, TransformNumber, TransformShape, TransformTree, Units,
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
    // Verifies: ADR-0022
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
    // Verifies: ADR-0022
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

/// A source that delivers `count` points, all at one place.
struct Many {
    count: usize,
}

impl forager_ports::FeatureSource for Many {
    async fn features(
        &self,
        _query: &forager_ports::FeatureQuery,
    ) -> Result<forager_ports::Features, forager_ports::SourceError> {
        let point = forager_ports::Feature {
            id: 0,
            geometry: forager_style_ir::Geometry::Point(forager_style_ir::Coord {
                x: 50.0,
                y: 50.0,
            }),
            properties: forager_ports::Properties::default(),
        };
        Ok(vec![point; self.count].into())
    }
}

#[test]
fn a_layer_of_more_features_than_the_limit_is_skipped() {
    // Verifies: Q-9
    // Q-9 bounds a layer to 1,000,000 features.
    assert_eq!(MAX_FEATURES_PER_LAYER, 1_000_000);
    let map = map(
        RenderMode::Point,
        Style {
            size: Some(Bound::Value(9.0)),
            color: Some(Bound::Value(BLUE)),
            ..Style::default()
        },
    );
    let white = |image: &RgbaImage| image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]);
    // The style paints its point, so a white image is a layer not drawn.
    let one = BTreeMap::from([("data".to_owned(), Many { count: 1 })]);
    let rendered = ready(render(&map, &one, AggRenderer::new()));
    assert!(!white(&rendered.image.unwrap()));
    let count = MAX_FEATURES_PER_LAYER + 1;
    let sources = BTreeMap::from([("data".to_owned(), Many { count })]);
    let rendered = ready(render(&map, &sources, AggRenderer::new()));
    assert_eq!(rendered.diagnostics.len(), 1, "{:?}", rendered.diagnostics);
    let d = &rendered.diagnostics[0];
    assert_eq!(d.code, "layer-skipped");
    let reason = format!("{count} features exceed the limit of {MAX_FEATURES_PER_LAYER}");
    assert!(
        d.message.contains("`data`") && d.message.contains(&reason),
        "{}",
        d.message
    );
    assert_eq!(d.span, Some(Span::new("m.map", 5, 3)));
    assert!(white(&rendered.image.unwrap()));
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
fn at_resolution_144_a_marker_is_drawn_twice_its_size_at_72() {
    // msPrepareImage's scale factor, RESOLUTION 144 over the default DEFRESOLUTION 72, which
    // pointLayerDrawShape hands msDrawMarkerSymbol and computeSymbolStyle scales the size and
    // the outline by: the filled head of SIZE 10 and WIDTH 1.5 at 144 is that of SIZE 20 and
    // WIDTH 3 at 72.
    let head = |size: f64, width: f64| Style {
        color: Some(Bound::Value(GREEN)),
        outline_color: Some(Bound::Value(BLACK)),
        size: Some(Bound::Value(size)),
        width: Some(Bound::Value(width)),
        ..Style::default()
    };
    let drawn = |style: Style, resolution: Option<f64>| {
        let mut map = map(RenderMode::Point, style);
        map.symbols = symbols();
        map.resolution = resolution;
        map.layers[0].classes[0].styles[0].symbol = Some(Bound::Value(SymbolRef::Index(2)));
        let rendered = render_with(&map, Some(r#"{"type": "Point", "coordinates": [50, 50]}"#));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        rendered.image.unwrap()
    };
    let at_144 = drawn(head(10.0, 1.5), Some(144.0));
    assert!(
        at_144 == drawn(head(20.0, 3.0), None),
        "twice the size at 72"
    );
    assert!(at_144 != drawn(head(10.0, 1.5), None), "not the size at 72");
    // At 144, computeSymbolStyle's scale is SIZE 10 × 2 over the head's sizey 3, 20 / 3, and its
    // outline WIDTH 1.5 × 2 = 3 (maprendering.c:62-65, 69). agg2RenderVectorSymbol moves the
    // head's centre (2, 1.5) to the origin, scales it, and moves it to the pixel (50, 50)
    // (mapagg.cpp:615-623): (0, 3), (2, 0), (4, 3), (2, 2) become (36.67, 60), (50, 40),
    // (63.33, 60), (50, 53.33), filled, then stroked open 3 wide.
    let mut map = map(RenderMode::Point, head(10.0, 1.5));
    map.symbols = symbols();
    map.resolution = Some(144.0);
    map.layers[0].classes[0].styles[0].symbol = Some(Bound::Value(SymbolRef::Index(2)));
    let ops = recorded(&map, r#"{"type": "Point", "coordinates": [50, 50]}"#);
    let head_at_144 = [
        (36.666_666_666_666_664, 60.0),
        (50.0, 40.0),
        (63.333_333_333_333_336, 60.0),
        (50.0, 53.333_333_333_333_336),
    ];
    let near = |path: &[forager_render::Point]| {
        path.len() == head_at_144.len()
            && path
                .iter()
                .zip(head_at_144)
                .all(|(p, (x, y))| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9)
    };
    match &ops[..] {
        [
            DrawOp::Fill { rings, color, .. },
            DrawOp::Stroke {
                paths,
                closed: false,
                color: outline,
                width,
                ..
            },
        ] => {
            assert!(
                *color == GREEN && rings.len() == 1 && near(&rings[0]),
                "{ops:?}"
            );
            assert!(
                *outline == BLACK && paths.len() == 1 && near(&paths[0]),
                "{ops:?}"
            );
            assert_eq!(*width, 3.0);
        }
        _ => panic!("{ops:?}"),
    }
}

/// A renderer that keeps every operation it is asked to draw.
#[derive(Clone, Default)]
struct OpRecorder(std::rc::Rc<std::cell::RefCell<Vec<DrawOp>>>);

impl forager_render::Draw for OpRecorder {
    fn draw(&mut self, item: &forager_render::DisplayItem) {
        self.0.borrow_mut().push(item.op.clone());
    }
}

/// A layer's items are drawn as if onto the map.
impl forager_render::Renderer for OpRecorder {
    type Layer<'a> = &'a mut Self;

    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}

    fn layer(&mut self, _: u8) -> &mut Self {
        self
    }
    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        }
    }
}

/// The operations the render of `map` over `geojson` draws, with no diagnostic.
fn recorded(map: &Map, geojson: &str) -> Vec<DrawOp> {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("data".to_owned(), GeoJsonSource::parse(geojson).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render(map, &sources, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    recorder.0.take()
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

/// The markers a point layer of `LABELCACHE` `label_cache` draws of a multipoint at x 40 and 45
/// with a class of two squares, of 20 pixels in blue and of 10 in green: each operation in
/// order, as the colour of its fill and the x of its centre.
fn multipoint_markers(label_cache: bool) -> Vec<(Rgba, f64)> {
    let square = |color, size| Style {
        color: Some(Bound::Value(color)),
        size: Some(Bound::Value(size)),
        ..Style::default()
    };
    let mut map = map_with(
        RenderMode::Point,
        vec![square(BLUE, 20.0), square(GREEN, 10.0)],
    );
    map.layers[0].label_cache = label_cache;
    let multipoint = r#"{"type": "MultiPoint", "coordinates": [[40, 50], [45, 50]]}"#;
    recorded(&map, multipoint)
        .iter()
        .map(|op| match op {
            DrawOp::Fill { rings, color, .. } => {
                (*color, f64::midpoint(rings[0][0].x, rings[0][2].x))
            }
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn a_multipoint_draws_each_points_markers_before_the_next_points_whatever_its_labelcache() {
    // MapServer's pointLayerDrawShape draws every style's marker at one point before the next
    // point's, whatever the layer's LABELCACHE: the second point's blue square, 5 pixels on and
    // 20 across, covers the first point's green one.
    let point_by_point = [(BLUE, 40.0), (GREEN, 40.0), (BLUE, 45.0), (GREEN, 45.0)];
    assert_eq!(multipoint_markers(true), point_by_point);
    assert_eq!(multipoint_markers(false), point_by_point);
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

#[test]
fn the_markers_of_a_point_layer_past_the_budget_are_left_out_and_said_so() {
    // Verifies: Q-9
    // A filled head of 4 points at each of 2,000,000 vertices of one feature inside the map: 8
    // million points, past the million a layer's markers may amount to, and none is drawn.
    let head = Style {
        color: Some(Bound::Value(BLUE)),
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        size: Some(Bound::Value(9.0)),
        ..Style::default()
    };
    let mut map = map(RenderMode::Point, head);
    map.symbols = symbols();
    let coordinates = vec!["[50, 50]"; 2_000_000].join(",");
    let geojson = format!(r#"{{"type": "MultiPoint", "coordinates": [{coordinates}]}}"#);
    let rendered = render_with(&map, Some(&geojson));
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["line-markers-over-limit"]);
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

#[test]
fn the_stand_in_squares_of_a_point_layer_spend_none_of_the_budget() {
    // Verifies: Q-9
    // A style without a symbol, which MapServer does not draw, and for which Forager draws its
    // square of 4 points: at 260,000 features, 1,040,000 points, past the million a layer's
    // markers may amount to, every square is drawn, the last feature's at (10, 10) among them.
    let square = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(3.0)),
        ..Style::default()
    };
    const FEATURES: usize = 260_000;
    let point = |at: &str| {
        format!(r#"{{"type": "Feature", "geometry": {{"type": "Point", "coordinates": {at}}}}}"#)
    };
    let mut features = vec![point("[50, 50]"); FEATURES - 1];
    features.push(point("[10, 10]"));
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    );
    let rendered = render_with(&map(RenderMode::Point, square), Some(&geojson));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(50, 50), Some(BLUE));
    assert_eq!(image.pixel(10, 90), Some(BLUE), "the last feature's square");
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
    // A comb of 17 000 teeth, 17 002 vertices past the bound of 16 384, under a hatch 10 apart:
    // each vertex 80 pixels from the one before, so that msTransformShapeSimplify keeps them all.
    let mut points: Vec<String> = (0..=17_000)
        .map(|i| {
            let x = 10.0 + 80.0 * f64::from(i) / 17_000.0;
            let y = if i % 2 == 0 { 10.0 } else { 90.0 };
            format!("[{x}, {y}]")
        })
        .collect();
    points.push("[10, 10]".into());
    let comb = format!(
        r#"{{"type": "Polygon", "coordinates": [[{}]]}}"#,
        points.join(", ")
    );
    let rendered = render_hatched_on(vec![hatch_style(BLACK, 10.0, 1.0, 45.0, vec![])], &comb);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["hatch-not-drawn"]);
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|px| px == [255, 255, 255, 255]));
}

/// Two squares, the second over the first from (40, 40) to (80, 80); `o` of the second is 25.
const OVERLAPPING_SQUARES: &str = r#"{"type": "FeatureCollection", "features": [
    {"type": "Feature", "properties": {"o": "100"}, "geometry": {"type": "Polygon",
     "coordinates": [[[10, 10], [60, 10], [60, 60], [10, 60], [10, 10]]]}},
    {"type": "Feature", "properties": {"o": "25"}, "geometry": {"type": "Polygon",
     "coordinates": [[[40, 40], [80, 40], [80, 80], [40, 80], [40, 40]]]}}
]}"#;

#[test]
fn a_style_s_opacity_replaces_the_alpha_of_its_colours() {
    // Verifies: G-6
    // loadStyle gives an OPACITY of 50 the alpha MS_NINT(50 × 2.55) = 127, the product lying
    // just below 127.5 in binary, its `#rrggbbaa` alpha replaced; AGG premultiplies red to 126
    // and blends it over white at a cover of 255: (255 × 128 + 126 × 256) >> 8 = 253, and
    // (255 × 128) >> 8 = 127.
    let style = Style {
        color: Some(Bound::Value(Rgba {
            a: 10,
            ..Rgba::opaque(255, 0, 0)
        })),
        opacity: Some(Bound::Value(50.0)),
        ..Style::default()
    };
    let image = render_with(&map(RenderMode::Polygon, style), Some(OVERLAPPING_SQUARES))
        .image
        .unwrap();
    assert_eq!(image.pixel(20, 80), Some(Rgba::opaque(253, 127, 127)));
    // Where the squares overlap, each blends on its own: (253 × 128 + 126 × 256) >> 8 = 252,
    // (127 × 128) >> 8 = 63.
    assert_eq!(image.pixel(50, 50), Some(Rgba::opaque(252, 63, 63)));
    assert_matches_reference(&image, "style_opacity.png");

    // A bound opacity is applied per feature: the second square at 25.
    let bound = Style {
        color: Some(Bound::Value(Rgba::opaque(255, 0, 0))),
        opacity: Some(Bound::Binding {
            binding: "o".into(),
        }),
        ..Style::default()
    };
    let image = render_with(&map(RenderMode::Polygon, bound), Some(OVERLAPPING_SQUARES))
        .image
        .unwrap();
    assert_eq!(image.pixel(20, 80), Some(Rgba::opaque(255, 0, 0)));
    // MS_NINT(25 × 2.55) = 64; red premultiplied to 63: (255 × 191 + 63 × 256) >> 8 = 253,
    // (255 × 191) >> 8 = 190.
    assert_eq!(image.pixel(70, 30), Some(Rgba::opaque(253, 190, 190)));
}

#[test]
fn a_layer_with_an_opacity_is_drawn_apart_and_laid_over_at_it() {
    // Verifies: G-6
    // The layer's squares are drawn opaque into an image of their own, which is laid over the
    // map at MS_NINT(50 × 2.55) = 127: the overlap is as light as the rest, unlike a style's
    // opacity. Black at alpha 255 over white at a cover of 127: alpha (255 × 128) >> 8 = 127,
    // (255 × 128) >> 8 = 127.
    let style = Style {
        color: Some(Bound::Value(BLACK)),
        ..Style::default()
    };
    let mut half = map(RenderMode::Polygon, style);
    half.layers[0].opacity = Some(50.0);
    let image = render_with(&half, Some(OVERLAPPING_SQUARES)).image.unwrap();
    assert_eq!(image.pixel(20, 80), Some(Rgba::opaque(127, 127, 127)));
    assert_eq!(image.pixel(50, 50), Some(Rgba::opaque(127, 127, 127)));
    assert_eq!(image.pixel(5, 5), Some(WHITE));
    assert_matches_reference(&image, "layer_opacity.png");

    // At 0 msDrawLayer skips the layer.
    half.layers[0].opacity = Some(0.0);
    let image = render_with(&half, Some(OVERLAPPING_SQUARES)).image.unwrap();
    assert_eq!(image.pixel(50, 50), Some(WHITE));
}

#[test]
fn a_transparent_background_keeps_the_colour_of_what_is_drawn_on_it() {
    // Verifies: G-6
    // The image of a transparent output format starts with no colour, and the alpha is divided
    // out of its premultiplied colours as saveAsPNG divides it: an edge of the line is the
    // line's grey at a lower alpha, not a darker grey.
    let mut lines = map(
        RenderMode::Line,
        line_style(Rgba::opaque(128, 128, 128), 5.0),
    );
    lines.image_color = Some(Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    });
    let line = r#"{"type": "LineString", "coordinates": [[10, 20], [90, 60]]}"#;
    let image = render_with(&lines, Some(line)).image.unwrap();
    assert_eq!(
        image.pixel(5, 5),
        Some(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0
        })
    );
    assert_eq!(image.pixel(50, 60), Some(Rgba::opaque(128, 128, 128)));
    // Down the column x = 50, across the line: each edge keeps the grey, divided out of its
    // premultiplied colour and truncated. At the top a cover of 22 gives alpha
    // (255 × 23) >> 8 = 22, the colour (128 × 23) >> 8 = 11 and the alpha
    // 255 - ((233 × 255) >> 8) = 23 in the buffer, and 11 / (23 / 255) = 121 in the image.
    let grey = |c: u8, a: u8| {
        Some(Rgba {
            r: c,
            g: c,
            b: c,
            a,
        })
    };
    let column: Vec<_> = (55..64).map(|y| image.pixel(50, y)).collect();
    assert_eq!(
        column,
        [
            grey(0, 0),
            grey(121, 23),
            grey(126, 247),
            grey(128, 255),
            grey(128, 255),
            grey(128, 255),
            grey(128, 255),
            grey(127, 140),
            grey(0, 0),
        ]
    );
    assert_matches_reference(&image, "transparent_background.png");
}

#[test]
fn a_hatch_in_a_layer_of_its_own_counts_its_lines_from_that_image_s_corner() {
    // Verifies: G-6
    // msDrawLayer draws a layer with a COMPOSITE block into an image msImageCreate makes, whose
    // refpt stays at (0, 0): ANGLE 0 lays the lines of SIZE 8 at y = 0, 8, …, 80, not at
    // y = 100, 92, …, 84 from the map's origin as on the map's own image.
    let hatch = hatch_style(BLACK, 8.0, 2.0, 0.0, vec![]);
    let on_map = render_hatched(vec![hatch.clone()]).image.unwrap();
    assert_eq!(on_map.pixel(15, 84), Some(BLACK));
    assert_eq!(on_map.pixel(15, 80), Some(WHITE));

    let mut map = map_with(RenderMode::Polygon, vec![hatch]);
    map.layers[0].opacity = Some(100.0);
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
    let apart = render_with(&map, Some(SQUARE_WITH_HOLE)).image.unwrap();
    assert_eq!(apart.pixel(15, 80), Some(BLACK));
    assert_eq!(apart.pixel(15, 84), Some(WHITE));
}

#[test]
fn a_layer_opacity_outside_0_to_100_is_bounded_and_one_not_finite_is_none() {
    let style = Style {
        color: Some(Bound::Value(BLACK)),
        ..Style::default()
    };
    let mut layer_map = map(RenderMode::Polygon, style);
    let codes = |m: &Map| -> Vec<String> {
        render_with(m, Some(SQUARE_WITH_HOLE))
            .diagnostics
            .iter()
            .map(|d| d.code.clone())
            .collect()
    };
    layer_map.layers[0].opacity = Some(150.0);
    assert_eq!(codes(&layer_map), ["opacity-out-of-range"]);
    let image = render_with(&layer_map, Some(SQUARE_WITH_HOLE))
        .image
        .unwrap();
    assert_eq!(image.pixel(25, 75), Some(BLACK), "drawn at 100");
    layer_map.layers[0].opacity = Some(-3.0);
    assert_eq!(codes(&layer_map), ["opacity-out-of-range"]);
    let image = render_with(&layer_map, Some(SQUARE_WITH_HOLE))
        .image
        .unwrap();
    assert_eq!(image.pixel(25, 75), Some(WHITE), "drawn at 0: skipped");
    layer_map.layers[0].opacity = Some(f64::NAN);
    assert_eq!(codes(&layer_map), ["opacity-out-of-range"]);
    let image = render_with(&layer_map, Some(SQUARE_WITH_HOLE))
        .image
        .unwrap();
    assert_eq!(image.pixel(25, 75), Some(BLACK), "drawn without an opacity");
}

/// A style drawing symbol `index` of [`symbols`] in `color` at `size` where `transform` places
/// it, turned there by `ANGLE AUTO`.
fn transformed(transform: GeomTransform, index: u32, color: Rgba, size: f64) -> Style {
    Style {
        color: Some(Bound::Value(color)),
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        size: Some(Bound::Value(size)),
        auto_angle: true,
        geom_transform: Some(transform),
        ..Style::default()
    }
}

#[test]
fn transforms_draw_symbols_at_the_start_the_end_and_the_vertices_of_a_line() {
    // On ELL, pixels (10, 20) to (90, 20) to (90, 90): the circle at the start, the arrow at the
    // end turned with the last segment, the filled head at the corner turned by -45°. They are
    // drawn with the first pass of the line layer's cache, so the wide line of the last style,
    // drawn from the cache after them, covers the circle where it runs. A style transformed to
    // the label point draws nothing on a line.
    let red_square = Style {
        color: Some(Bound::Value(Rgba::opaque(255, 0, 0))),
        size: Some(Bound::Value(30.0)),
        geom_transform: Some(GeomTransform::LabelPoint),
        ..Style::default()
    };
    let rendered = render_markers(vec![
        line_style(BLACK, 1.0),
        transformed(GeomTransform::Start, 3, BLACK, 9.0),
        transformed(GeomTransform::End, 1, BLUE, 12.0),
        transformed(GeomTransform::Vertices, 2, BLUE, 12.0),
        red_square,
        line_style(GREEN, 3.0),
    ]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(
        image.pixel(10, 20),
        Some(GREEN),
        "the wide line over the circle"
    );
    assert_eq!(
        image.pixel(10, 23),
        Some(BLACK),
        "the circle beside the line"
    );
    assert_eq!(image.pixel(10, 26), Some(WHITE), "past the circle's radius");
    // The arrow, upright at the angle 0, is turned by -90° on the downward segment, clockwise on
    // the image: its shaft lies across the end, its apex to the right.
    assert_ne!(
        image.pixel(95, 90),
        Some(WHITE),
        "the end's arrow towards its apex"
    );
    assert_eq!(image.pixel(90, 96), Some(WHITE), "nothing past the end");
    assert!(
        image.pixels.chunks(4).all(|p| p != [255, 0, 0, 255]),
        "the label point transform draws nothing"
    );
    assert_matches_reference(&image, "geomtransform_line.png");
}

#[test]
fn a_bbox_transform_strokes_the_line_s_bounds_held_near_the_image() {
    // msDrawTransformedShape's bbox: the line runs from pixel
    // (-50, -20) to (150, 60), its bounds held by the padding (int)(MAX(5, -1) + 3) = 8 to
    // x = -8 and 101 + 8 = 109 and y = -8, and kept at y = 60 below; msDrawShadeSymbol strokes
    // the ring of an OUTLINECOLOR without a COLOR as a line 5 pixels wide. Only its lower side,
    // y = 60, falls on the image, rows 58 to 61 covered whole and 57 and 62 by half; the line
    // itself, drawn after it in green, crosses it.
    let ring = Style {
        outline_color: Some(Bound::Value(BLACK)),
        width: Some(Bound::Value(5.0)),
        geom_transform: Some(GeomTransform::Bbox),
        ..Style::default()
    };
    let rendered = render_with(
        &map_with(RenderMode::Line, vec![ring, line_style(GREEN, 1.0)]),
        Some(r#"{"type": "LineString", "coordinates": [[-50, 120], [150, 40]]}"#),
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    for x in [0, 20, 100] {
        for y in 58..=61 {
            assert_eq!(
                image.pixel(x, y),
                Some(BLACK),
                "the lower side at ({x}, {y})"
            );
        }
        assert_eq!(
            image.pixel(x, 56),
            Some(WHITE),
            "above the lower side at {x}"
        );
        assert_eq!(
            image.pixel(x, 64),
            Some(WHITE),
            "below the lower side at {x}"
        );
    }
    for (x, y) in [(0, 45), (50, 0), (100, 0), (100, 50)] {
        assert_eq!(image.pixel(x, y), Some(WHITE), "no side within ({x}, {y})");
    }
    assert_matches_reference(&image, "geomtransform_bbox.png");
}

#[test]
fn a_centroid_transform_draws_at_the_centre_of_a_polygon_s_rings() {
    // msGetPolygonCentroid: the square and its hole, both centred on (50, 50).
    let fill = Style {
        color: Some(Bound::Value(GREEN)),
        ..Style::default()
    };
    let mut map = map_with(
        RenderMode::Polygon,
        vec![fill, transformed(GeomTransform::Centroid, 3, BLACK, 10.0)],
    );
    map.symbols = symbols();
    let rendered = render_with(&map, Some(SQUARE_WITH_HOLE));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(50, 50), Some(BLACK), "the centroid in the hole");
    assert_eq!(image.pixel(50, 57), Some(WHITE), "the hole past the circle");
    assert_eq!(image.pixel(20, 20), Some(GREEN));
    assert_matches_reference(&image, "geomtransform_centroid.png");
}

#[test]
fn a_polar_offset_moves_a_symbol_at_a_point_by_distance_and_angle() {
    // POLAROFFSET [d] [a] bound to 20 and 90: the circle 20 pixels up from the point, counter-
    // clockwise from rightwards. A point layer draws its symbol at the point whatever its
    // GEOMTRANSFORM, as pointLayerDrawShape does.
    let style = Style {
        polar_offset: Some([
            Bound::Binding {
                binding: "d".into(),
            },
            Bound::Binding {
                binding: "a".into(),
            },
        ]),
        ..transformed(GeomTransform::End, 3, BLUE, 8.0)
    };
    let mut map = map(RenderMode::Point, style);
    map.symbols = symbols();
    let point = r#"{"type": "Feature", "properties": {"d": 20, "a": 90},
        "geometry": {"type": "Point", "coordinates": [50, 50]}}"#;
    let rendered = render_with(&map, Some(point));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(50, 30), Some(BLUE));
    assert_eq!(image.pixel(50, 50), Some(WHITE));
    assert_matches_reference(&image, "symbol_polar_offset.png");
}

#[test]
fn a_transformed_first_style_with_an_outline_draws_once_as_its_outline_prepares_it() {
    // msDrawVectorLayer prepares the first style of a cached line layer for its outline before
    // msDrawShape draws its transform: the circle at the start in its OUTLINECOLOR, red, outlined
    // in its COLOR, blue, at a width of 1 + 2 × 2 and a size of 8 + 2. The cache's later pass of
    // that style skips it, so it is not drawn again in blue.
    let red = Rgba::opaque(255, 0, 0);
    let start = Style {
        outline_color: Some(Bound::Value(red)),
        outline_width: Some(Bound::Value(2.0)),
        ..transformed(GeomTransform::Start, 3, BLUE, 8.0)
    };
    let rendered = render_markers(vec![start, line_style(BLACK, 1.0)]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(
        image.pixel(10, 18),
        Some(red),
        "the fill, in the outline colour"
    );
    assert_eq!(
        image.pixel(10, 25),
        Some(BLUE),
        "the outline, in the colour"
    );
    assert_eq!(image.pixel(10, 30), Some(WHITE), "past the outline");
    assert_matches_reference(&image, "geomtransform_outline.png");
}

/// A 2 × 2 image of four pixels given in reading order.
fn image_of(pixels: [[u8; 4]; 4]) -> RgbaImage {
    RgbaImage {
        width: 2,
        height: 2,
        pixels: pixels.concat(),
    }
}

#[test]
fn a_bless_reports_how_far_it_moved_the_image_it_replaces() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bless");
    std::fs::create_dir_all(&dir).unwrap();
    let moved = dir.join("moved.png");
    let kept = dir.join("kept.png");
    let before = image_of([
        [255, 255, 255, 255],
        [200, 220, 180, 255],
        [0, 0, 0, 255],
        [30, 60, 200, 128],
    ]);
    for path in [&moved, &kept] {
        std::fs::write(path, forager_encode::png(&before).unwrap()).unwrap();
    }
    let after = image_of([
        [255, 255, 255, 255],
        [202, 220, 179, 255],
        [0, 0, 0, 255],
        // R moves in two pixels, by 2 and by 1: the report gives the larger, not their sum.
        [29, 63, 200, 127],
    ]);
    assert_eq!(
        bless::bless(&moved, &after).as_deref(),
        Some("moved.png: 2 of 4 pixels moved, by at most R 2 G 3 B 1 A 1")
    );
    assert_eq!(
        forager_encode::decode_png(&std::fs::read(&moved).unwrap()).unwrap(),
        after,
        "the bless wrote the new image"
    );
    assert_eq!(
        bless::bless(&kept, &before),
        None,
        "an image that did not move reports nothing"
    );
}

#[test]
fn a_bless_reports_an_image_that_changed_its_size_is_new_or_replaces_a_broken_one() {
    let wide = RgbaImage {
        width: 4,
        height: 1,
        pixels: [[0_u8, 0, 0, 255]; 4].concat(),
    };
    let square = image_of([[0, 0, 0, 255]; 4]);
    assert_eq!(
        bless::report("x.png", &wide, &square).as_deref(),
        Some("x.png: 4×1 became 2×2")
    );
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bless");
    std::fs::create_dir_all(&dir).unwrap();
    let new = dir.join("new.png");
    let _ = std::fs::remove_file(&new);
    assert_eq!(
        bless::bless(&new, &square).as_deref(),
        Some("new.png: new, 2×2")
    );
    let broken = dir.join("broken.png");
    std::fs::write(&broken, b"not a png").unwrap();
    assert_eq!(
        bless::bless(&broken, &square).as_deref(),
        Some("broken.png: replaced an undecodable reference, 2×2")
    );
    assert_eq!(
        forager_encode::decode_png(&std::fs::read(&broken).unwrap()).unwrap(),
        square,
        "the bless repaired the reference"
    );
}

/// The image of a line layer drawing a thin green line along [`ELL`], then `style`.
fn render_after_a_line(style: Style) -> RgbaImage {
    let rendered = render_markers(vec![line_style(GREEN, 1.0), style]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    rendered.image.unwrap()
}

#[test]
fn a_later_transformed_style_with_only_an_outlinecolor_draws_its_marker_once_as_written() {
    // lineLayerDrawShape hands every style with a GEOMTRANSFORM to msDrawTransformedShape in the
    // cached layer's first pass, with no test of its colour, and the cache's passes skip it, so
    // the condition of the later passes does not apply: the style is drawn once, as written, its
    // marker at the corner outlined in its OUTLINECOLOR and not filled. No outline pass prepares
    // it, which would swap its colours and fill the head in black: msDrawVectorLayer prepares
    // only the first style, and only with an OUTLINEWIDTH above 0.
    let line = render_after_a_line(Style::default());
    let head = Style {
        color: None,
        outline_color: Some(Bound::Value(BLACK)),
        ..transformed(GeomTransform::Vertices, 2, BLUE, 12.0)
    };
    let image = render_after_a_line(head);
    assert!(image != line, "the head is drawn");
    assert_eq!(image.pixel(91, 28), Some(BLACK), "the head's outline");
    assert_eq!(
        image.pixel(91, 21),
        Some(WHITE),
        "the head's inside, unfilled"
    );
}

#[test]
fn a_documents_symbol_name_that_no_symbol_carries_draws_nothing_and_warns_once() {
    // A style document has no image file to read an unknown name as, so the style draws
    // nothing, also over a polygon with a COLOR; the layer warns once for the name, however many
    // of its styles name it.
    let unknown = || Style {
        color: Some(Bound::Value(GREEN)),
        symbol: Some(Bound::Value(SymbolRef::Name("nowhere".to_owned()))),
        ..Style::default()
    };
    let map = map_with(RenderMode::Polygon, vec![unknown(), unknown()]);
    let rendered = render_with(&map, Some(SQUARE_WITH_HOLE));
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["symbol-unknown"]);
    assert!(
        rendered.diagnostics[0].message.contains("`nowhere`"),
        "{}",
        rendered.diagnostics[0].message
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(25, 75), Some(WHITE));
}

/// The height in pixels of what is not white in `image`.
fn ink_height(image: &RgbaImage) -> u32 {
    let rows: Vec<u32> = (0..image.height)
        .filter(|&y| (0..image.width).any(|x| image.pixel(x, y) != Some(WHITE)))
        .collect();
    rows.last().map_or(0, |last| last - rows[0] + 1)
}

/// A point layer of the filled circle of [`symbols`] at `SIZE` 10 in red, at the centre of a map
/// of 101 × 101 pixels over `side` map units, at `resolution`, its layer changed by `layer`.
fn circle_at_scale(side: f64, resolution: f64, layer: impl Fn(&mut Layer)) -> RgbaImage {
    let style = Style {
        color: Some(Bound::Value(RED)),
        symbol: Some(Bound::Value(SymbolRef::Index(3))),
        size: Some(Bound::Value(10.0)),
        ..Style::default()
    };
    let mut map = map(RenderMode::Point, style);
    map.symbols = symbols();
    map.extent = Some(Extent {
        min_x: 0.0,
        min_y: 0.0,
        max_x: side,
        max_y: side,
    });
    map.resolution = Some(resolution);
    layer(&mut map.layers[0]);
    let centre = side / 2.0;
    let point = format!(r#"{{"type": "Point", "coordinates": [{centre}, {centre}]}}"#);
    let rendered = render_with(&map, Some(&point));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    rendered.image.unwrap()
}

#[test]
fn a_symbolscaledenom_scales_a_layers_styles_by_the_maps_scale() {
    // msPrepareImage (mapdraw.c): a layer's scale factor is its SYMBOLSCALEDENOM over the map's
    // scale, times RESOLUTION over DEFRESOLUTION. At 1:50000, 101 pixels across, 144 dpi, and
    // meters: 25000 / 50000 × 144 / 72 = 1, so the circle of SIZE 10 is 10 pixels high, where the
    // resolution factor alone, 2, would make it 20.
    let side = 50_000.0 * 100.0 / (144.0 * 39.3701);
    let scaled = circle_at_scale(side, 144.0, |layer| {
        layer.symbol_scale_denom = Some(25_000.0);
    });
    assert!(
        (9..=11).contains(&ink_height(&scaled)),
        "{}",
        ink_height(&scaled)
    );
    let unscaled = circle_at_scale(side, 144.0, |_| {});
    assert!(
        (19..=21).contains(&ink_height(&unscaled)),
        "{}",
        ink_height(&unscaled)
    );
}

#[test]
fn sizeunits_size_a_layers_styles_in_map_units_by_the_cellsize() {
    // msPrepareImage: a layer of SIZEUNITS other than pixels scales its styles by the inches of
    // its unit over those of the map's, over the cellsize: 50 meters over 100 pixels is 0.5 per
    // pixel, so a SIZE of 10 meters is 20 pixels, whatever the resolution.
    for resolution in [72.0, 144.0] {
        let sized = circle_at_scale(50.0, resolution, |layer| {
            layer.size_units = Some(Units::Meters);
        });
        assert!(
            (19..=21).contains(&ink_height(&sized)),
            "{}",
            ink_height(&sized)
        );
    }
}

#[test]
fn a_style_of_a_geomtransform_expression_draws_nothing_and_is_reported() {
    // MapServer evaluates such an expression by msGeomTransformShape, most of its functions through
    // GEOS, which the engine has not: the style draws nothing, with a warning naming it, and the
    // class's other style draws as alone.
    let plain = line_style(BLUE, 3.0);
    let line = r#"{"type": "LineString", "coordinates": [[0, 20], [100, 20]]}"#;
    let alone = render_with(&map_with(RenderMode::Line, vec![plain.clone()]), Some(line));
    let transformed = Style {
        geom_transform: Some(GeomTransform::Expression(not_drawn(NotDrawn::Buffer))),
        ..line_style(GREEN, 9.0)
    };
    let rendered = render_with(
        &map_with(RenderMode::Line, vec![plain, transformed]),
        Some(line),
    );
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["geomtransform-not-drawn"]);
    assert!(
        rendered.diagnostics[0]
            .message
            .contains("of the function buffer"),
        "{}",
        rendered.diagnostics[0].message
    );
    assert_eq!(
        rendered.image, alone.image,
        "the other style draws as alone"
    );
}

#[test]
fn a_class_style_of_labelcenter_in_a_line_or_polygon_layer_draws_nothing_and_is_reported() {
    // msDrawTransformedShape (src/mapgeomtransform.c:245) fails the draw of a line or a polygon
    // layer on a class's style of labelcenter, which msDrawLabelCache alone draws: the style draws
    // nothing, with a warning, whichever document wrote it, and the class's other style draws as
    // alone. A point layer draws the style at its points and is not warned.
    let line = r#"{"type": "LineString", "coordinates": [[0, 20], [100, 20]]}"#;
    let polygon =
        r#"{"type": "Polygon", "coordinates": [[[10, 10], [90, 10], [90, 90], [10, 10]]]}"#;
    let centred = Style {
        geom_transform: Some(GeomTransform::LabelCenter),
        ..line_style(GREEN, 9.0)
    };
    for (mode, shape) in [(RenderMode::Line, line), (RenderMode::Polygon, polygon)] {
        let plain = line_style(BLUE, 3.0);
        let alone = render_with(&map_with(mode, vec![plain.clone()]), Some(shape));
        let rendered = render_with(&map_with(mode, vec![plain, centred.clone()]), Some(shape));
        assert_eq!(codes(&rendered), ["geomtransform-not-drawn"], "{mode:?}");
        assert!(
            rendered.diagnostics[0].message.contains("labelcenter"),
            "{}",
            rendered.diagnostics[0].message
        );
        assert_eq!(
            rendered.image, alone.image,
            "{mode:?}: the other style draws"
        );
    }
    let rendered = render_with(&map_with(RenderMode::Point, vec![centred]), Some(line));
    assert_eq!(codes(&rendered), Vec::<&str>::new());
}

#[test]
fn a_point_layer_draws_a_style_of_a_geomtransform_expression_at_its_points() {
    // pointLayerDrawShape draws every style at each point whatever its transform: a point layer's
    // style of an expression draws its markers, and nothing is reported.
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(5.0)),
        geom_transform: Some(GeomTransform::Expression(not_drawn(NotDrawn::Buffer))),
        ..Style::default()
    };
    let line = r#"{"type": "LineString", "coordinates": [[20, 20], [80, 80]]}"#;
    let rendered = render_with(&map(RenderMode::Point, style), Some(line));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(20, 80), Some(BLUE));
    assert_eq!(image.pixel(80, 20), Some(BLUE));
}

fn not_drawn(reason: NotDrawn) -> TransformTree {
    TransformTree::NotDrawn(reason)
}

/// `generalize([shape], tolerance)`.
fn generalized(tolerance: TransformNumber) -> TransformTree {
    TransformTree::Drawn(TransformShape::Generalize(
        Box::new(TransformShape::Shape),
        tolerance,
    ))
}

/// `densify([shape], tolerance)`.
fn densified(tolerance: f64) -> TransformTree {
    TransformTree::Drawn(TransformShape::Densify(
        Box::new(TransformShape::Shape),
        TransformNumber::Number(tolerance),
    ))
}

fn codes(rendered: &Rendered) -> Vec<&str> {
    rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

#[test]
fn a_layer_of_a_not_drawn_transform_draws_nothing_and_warns_its_reason() {
    // Verifies: ADR-0036
    // MapServer applies the layer's GEOMTRANSFORM to each shape msLayerNextShape reads; simplify
    // needs GEOS, which Forager has not: the layer draws nothing, with the warning naming the
    // reason.
    let mut map = map(RenderMode::Line, line_style(BLUE, 3.0));
    let line = r#"{"type": "LineString", "coordinates": [[0, 20], [100, 20]]}"#;
    for (reason, named) in [
        (NotDrawn::Simplify, "not_drawn: simplify"),
        (NotDrawn::Binding, "not_drawn: binding"),
        (NotDrawn::Syntax, "not_drawn: syntax"),
    ] {
        map.layers[0].geom_transform = Some(not_drawn(reason));
        let rendered = render_with(&map, Some(line));
        assert_eq!(codes(&rendered), ["geomtransform-not-drawn"]);
        let message = &rendered.diagnostics[0].message;
        assert!(message.contains(named), "{message}");
        let image = rendered.image.unwrap();
        assert_eq!(
            image.pixel(50, 80),
            Some(WHITE),
            "nothing of the layer is drawn"
        );
    }
}

/// A line up from (0, 20) to (50, 60) and down to (100, 20).
const PEAK: &str = r#"{"type": "LineString", "coordinates": [[0, 20], [50, 60], [100, 20]]}"#;

#[test]
fn a_layer_of_generalize_draws_the_generalized_line() {
    // msGeneralize keeps the first point, each at least the tolerance from the last kept, and the
    // last: by 200 the peak is dropped, and the line runs straight along y = 20, pixel row 80.
    let mut map = map(RenderMode::Line, line_style(BLUE, 3.0));
    let plain = render_with(&map, Some(PEAK)).image.unwrap();
    assert_ne!(
        plain.pixel(25, 60),
        Some(WHITE),
        "the peak's slope is drawn"
    );
    assert_eq!(plain.pixel(50, 80), Some(WHITE));
    map.layers[0].geom_transform = Some(generalized(TransformNumber::Number(200.0)));
    let rendered = render_with(&map, Some(PEAK));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(50, 80), Some(BLUE), "the generalized line");
    assert_eq!(image.pixel(25, 60), Some(WHITE), "the peak is dropped");
}

#[test]
fn map_cellsize_is_the_cellsize_in_meters_of_the_layer() {
    // msGeomTransformShape binds [map_cellsize] to the map's cellsize, 1 here, times the inches
    // of the map's unit over those of the layer's, meters as initLayer sets them: in a map of
    // meters a tolerance of 10 keeps the peak; in one of degrees it is 10 * 4374754 / 39.3701,
    // about 1.1 million, and drops it.
    let mut map = map(RenderMode::Line, line_style(BLUE, 3.0));
    map.layers[0].geom_transform = Some(generalized(TransformNumber::Op(
        NumberOp::Mul,
        Box::new(TransformNumber::MapCellsize),
        Box::new(TransformNumber::Number(10.0)),
    )));
    let meters = render_with(&map, Some(PEAK)).image.unwrap();
    assert_ne!(meters.pixel(25, 60), Some(WHITE), "the peak is kept");
    map.units = Some(Units::Degrees);
    let rendered = render_with(&map, Some(PEAK));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let degrees = rendered.image.unwrap();
    assert_eq!(degrees.pixel(25, 60), Some(WHITE), "the peak is dropped");
    assert_eq!(degrees.pixel(50, 80), Some(BLUE));
}

#[test]
fn a_layer_whose_geomtransform_adds_too_many_points_draws_nothing_with_a_warning() {
    // densify by 0.000001 would add a hundred million points to the line, past the 1 000 000 a
    // layer's transforms may add.
    let mut map = map(RenderMode::Line, line_style(BLUE, 3.0));
    map.layers[0].geom_transform = Some(densified(0.000001));
    let rendered = render_with(&map, Some(PEAK));
    assert_eq!(codes(&rendered), ["geomtransform-over-limit"]);
    let image = rendered.image.unwrap();
    assert_eq!(
        image.pixel(25, 60),
        Some(WHITE),
        "nothing of the layer is drawn"
    );
}

#[test]
fn a_layer_whose_transform_fails_on_a_shape_draws_nothing_with_a_warning() {
    // msDensify returns NULL for a point shape, and the parser fails ("Executing densify
    // failed."); msDrawMap then makes no map, where Forager draws nothing of the layer.
    let mut map = map(RenderMode::Point, line_style(BLUE, 3.0));
    map.layers[0].geom_transform = Some(densified(5.0));
    let point = r#"{"type": "Point", "coordinates": [50, 50]}"#;
    let rendered = render_with(&map, Some(point));
    assert_eq!(codes(&rendered), ["geomtransform-not-drawn"]);
    assert!(
        rendered.diagnostics[0]
            .message
            .contains("densify on a point"),
        "{}",
        rendered.diagnostics[0].message
    );
}

/// Two lines along y = 20 and y = 50, pixel rows 80 and 50, and a point, which densify fails,
/// each with its `kind`.
const LINES_THEN_A_POINT: &str = r#"{"type": "FeatureCollection", "features": [
    {"type": "Feature", "properties": {"kind": "line"},
     "geometry": {"type": "LineString", "coordinates": [[0, 20], [100, 20]]}},
    {"type": "Feature", "properties": {"kind": "line"},
     "geometry": {"type": "LineString", "coordinates": [[0, 50], [100, 50]]}},
    {"type": "Feature", "properties": {"kind": "point"},
     "geometry": {"type": "Point", "coordinates": [50, 50]}}]}"#;

#[test]
fn a_shape_past_the_layers_limit_or_its_filter_is_never_transformed() {
    // msLayerNextShape transforms a shape after the filter passes it (src/maplayer.c:437-449),
    // and msDrawVectorLayer reads none past the first its class finds beyond MAXFEATURES
    // (src/mapdraw.c:1282-1286): the point, which densify fails, is transformed only where the
    // limit reads up to it and the filter passes it.
    let transformed = |max_features: Option<u32>, filter: Option<Expr>| {
        let mut map = map(RenderMode::Line, line_style(BLUE, 3.0));
        map.layers[0].geom_transform = Some(densified(5.0));
        map.layers[0].max_features = max_features;
        map.layers[0].filter = filter;
        render_with(&map, Some(LINES_THEN_A_POINT))
    };
    let failed = |rendered: &Rendered| codes(rendered).contains(&"geomtransform-not-drawn");
    // MAXFEATURES 1: the second line is read and transformed, its class found past the limit;
    // the point is never read.
    let one = transformed(Some(1), None);
    assert!(!failed(&one), "{:?}", one.diagnostics);
    let image = one.image.unwrap();
    assert_eq!(image.pixel(50, 80), Some(BLUE), "the first line");
    assert_eq!(image.pixel(25, 50), Some(WHITE), "past the limit");
    // MAXFEATURES 2: the point is read after the second line, and fails.
    assert!(failed(&transformed(Some(2), None)));
    assert!(failed(&transformed(None, None)));
    // A filter that leaves the point out: it is never transformed.
    let lines = Expr::call(
        Op::Eq,
        vec![
            Expr::call(Op::ToString, vec![Expr::property("kind")]),
            "line".into(),
        ],
    );
    let filtered = transformed(None, Some(lines));
    assert!(!failed(&filtered), "{:?}", filtered.diagnostics);
    assert_eq!(filtered.image.unwrap().pixel(25, 50), Some(BLUE));
}
