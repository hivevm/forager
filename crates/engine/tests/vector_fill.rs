//! A polygon filled with a vector symbol tiled, as MapServer's msDrawShadeSymbol tiles it, and the
//! tile held to Q-9's bound.

use std::collections::BTreeMap;
use std::future::Future;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Span, Style,
    Symbol, SymbolKind, SymbolRef,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

/// A square from 10 to 90 with a hole from 40 to 60.
const AREA: &str = r#"{"type": "Polygon", "coordinates": [
    [[10, 10], [90, 10], [90, 90], [10, 90], [10, 10]],
    [[40, 40], [60, 40], [60, 60], [40, 60], [40, 40]]]}"#;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a source that never waits")
}

/// msautotest's `vector arrow`: an open line and an open chevron, 3 × 2.
fn arrow() -> Symbol {
    Symbol {
        name: Some("vector arrow".to_owned()),
        kind: SymbolKind::Vector,
        paths: vec![
            vec![[0.0, 1.0], [3.0, 1.0]],
            vec![[2.0, 0.0], [3.0, 1.0], [2.0, 2.0]],
        ],
        extent: Some([3.0, 2.0]),
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }
}

/// A 101 × 101 image of the extent 0..100 of one polygon layer `areas`, in a style of the arrow
/// at `SIZE 8` and `gap`, in black.
fn arrow_map(gap: f64) -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        }),
        size: Some([101, 101]),
        image_color: Some(WHITE),
        symbols: vec![arrow()],
        layers: vec![Layer {
            name: Some("areas".into()),
            render_mode: Some(RenderMode::Polygon),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                styles: vec![Style {
                    symbol: Some(Bound::Value(SymbolRef::Index(1))),
                    size: Some(Bound::Value(8.0)),
                    gap: Some(gap),
                    color: Some(Bound::Value(BLACK)),
                    ..Style::default()
                }],
                ..Class::default()
            }],
            span: Some(Span::new("m.map", 2, 1)),
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render_area(map: &Map) -> Rendered {
    let sources = BTreeMap::from([("areas".to_owned(), GeoJsonSource::parse(AREA).unwrap())]);
    ready(render(map, &sources, AggRenderer::new()))
}

/// Whether the pixel at (`x`, `y`) is the image colour.
fn white(rendered: &Rendered, x: u32, y: u32) -> bool {
    rendered.image.as_ref().unwrap().pixel(x, y) == Some(WHITE)
}

#[test]
fn a_vector_symbol_fills_the_polygon_with_its_tile_and_leaves_the_hole() {
    let rendered = render_area(&arrow_map(0.0));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.as_ref().unwrap();
    // Inside the polygon some pixels are drawn, its tile 12 × 8 repeated; the hole and the area
    // outside it stay white.
    let drawn = (10..90)
        .flat_map(|y| (10..40).map(move |x| (x, y)))
        .filter(|&(x, y)| image.pixel(x, y) != Some(WHITE))
        .count();
    assert!(drawn > 100, "{drawn} pixels drawn");
    for (x, y) in [(50, 50), (45, 55), (5, 5), (95, 95)] {
        assert!(white(&rendered, x, y), "({x}, {y})");
    }
}

#[test]
fn a_vector_fill_whose_tile_passes_the_bound_warns_and_draws_no_fill() {
    // Verifies: Q-9
    // A GAP of 5000 asks a tile of 5000 × 5000 pixels, past 4096 a side: nothing is allocated
    // for it, and the polygon is not filled.
    let rendered = render_area(&arrow_map(5000.0));
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["symbol-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.contains("symbol `vector arrow`") && message.contains("5000 × 5000"),
        "{message}"
    );
    let image = rendered.image.as_ref().unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

#[test]
fn what_the_renderer_refuses_of_a_fill_tile_is_reported_on_the_layer_once_per_tile() {
    // Verifies: ARCH-third-party-failures-guarded, Q-3
    // The arrow with a third line out to x = 1.7e308, of which no extent is measured, as a
    // mapfile's second POINTS block measures the extent anew: the style's scale of 4 takes that
    // point past every finite number in the tile's pixels, so the tile's renderer drops it from
    // the line at each of the seamless tile's 9 centres. The layer that first fills with the tile
    // reports the 9 paths once: for two polygons, as for one, and again in a second render, which
    // builds its own tile; a second layer that finds the tile held reports nothing of it.
    let mut map = arrow_map(0.0);
    map.symbols[0].paths.push(vec![[2.0, 1.0], [1.7e308, 1.0]]);
    let mut second = map.layers[0].clone();
    second.name = Some("again".into());
    second.span = Some(Span::new("m.map", 9, 1));
    let two = r#"{"type": "MultiPolygon", "coordinates": [
        [[[10, 10], [40, 10], [40, 40], [10, 40], [10, 10]]],
        [[[60, 60], [90, 60], [90, 90], [60, 90], [60, 60]]]]}"#;
    let render = |map: &Map, areas: &str| {
        let sources = BTreeMap::from([
            ("areas".to_owned(), GeoJsonSource::parse(areas).unwrap()),
            ("again".to_owned(), GeoJsonSource::parse(areas).unwrap()),
        ]);
        ready(render(map, &sources, AggRenderer::new()))
    };
    let warned = |rendered: &Rendered| {
        let warnings: Vec<_> = rendered
            .diagnostics
            .iter()
            .map(|d| (d.code.as_str(), d.message.as_str(), d.span.clone()))
            .collect();
        let message = "layer `areas`: 9 paths are drawn without a point, a segment, or a crossing \
                       that is no finite number in pixels (Q-3)";
        let span = Some(Span::new("m.map", 2, 1));
        assert_eq!(warnings, [("path-not-drawn", message, span)]);
    };
    for areas in [AREA, two, AREA] {
        let rendered = render(&map, areas);
        warned(&rendered);
        let drawn = (61..91)
            .flat_map(|y| (10..40).map(move |x| (x, y)))
            .filter(|&(x, y)| !white(&rendered, x, y))
            .count();
        assert!(
            drawn > 50,
            "the rest of the arrow fills: {drawn} pixels drawn"
        );
    }
    let mut both = map.clone();
    both.layers.push(second);
    warned(&render(&both, AREA));
}
