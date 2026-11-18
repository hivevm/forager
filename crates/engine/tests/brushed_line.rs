//! A line brushed with a pixmap symbol's tile, end to end, where the renderer refuses a piece of it
//! to keep agg-rust within `i32` (ADR-0049): the layer that draws it carries the warning, once for
//! the symbol's kind, and a line the renderer draws carries none.

use std::collections::BTreeMap;
use std::future::Future;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Span, Style,
    Symbol, SymbolKind, SymbolRef,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);

/// The test icon, 32 × 32 RGBA, opaque in its top half.
const PIN: &[u8] = include_bytes!("../../cli/tests/fixtures/pin.png");

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The one file `pin.png`.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        match path.as_str() {
            "pin.png" => Ok(PIN.to_vec()),
            _ => Err(SourceError::new("no such file")),
        }
    }
}

/// A map of `width` × `height` pixels, one map unit a pixel, the extent's left edge at `min_x`
/// and its top at 50, whose line layer `lines` brushes its features with the pin, a pixmap. The
/// style's `WIDTH` of 10000, which a pixmap's brush does not take, grows the class's clip buffer
/// to 10001 pixels, so that neither the engine nor the renderer cuts a line within it.
fn brushed_map(width: u32, height: u32, min_x: f64) -> Map {
    let pin = Symbol {
        name: Some("pin".to_owned()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: Some("pin.png".to_owned()),
        transparent: None,
        span: None,
    };
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        width: Some(Bound::Value(10_000.0)),
        ..Style::default()
    };
    let layer = Layer {
        name: Some("lines".into()),
        render_mode: Some(RenderMode::Line),
        status: Some(LayerStatus::On),
        source: Some(SourceRef {
            kind: "local".into(),
            ..SourceRef::default()
        }),
        classes: vec![Class {
            styles: vec![style],
            ..Class::default()
        }],
        span: Some(Span::new("m.map", 2, 1)),
        ..Layer::default()
    };
    Map {
        extent: Some(Extent {
            min_x,
            min_y: 51.0 - f64::from(height),
            max_x: min_x + f64::from(width - 1),
            max_y: 50.0,
        }),
        size: Some([width, height]),
        image_color: Some(WHITE),
        symbols: vec![pin],
        layers: vec![layer],
        ..Map::default()
    }
}

/// `map` rendered with one feature, a line of a part for each of `parts`, each part's points in
/// pixels from the image's top-left corner of a map whose extent's left edge is at `min_x`.
fn rendered(map: &Map, min_x: f64, parts: &[&[(f64, f64)]]) -> Rendered {
    let parts: Vec<String> = parts
        .iter()
        .map(|points| {
            let coordinates: Vec<String> = points
                .iter()
                .map(|&(x, y)| format!("[{}, {}]", x + min_x, 50.0 - y))
                .collect();
            format!("[{}]", coordinates.join(", "))
        })
        .collect();
    let line = format!(
        r#"{{"type": "MultiLineString", "coordinates": [{}]}}"#,
        parts.join(", ")
    );
    let sources = BTreeMap::from([("lines".to_owned(), GeoJsonSource::parse(&line).unwrap())]);
    ready(render_with_files(map, &sources, &Files, AggRenderer::new()))
}

/// How many pixels of the rendered image are not white.
fn drawn(rendered: &Rendered) -> usize {
    let image = rendered.image.as_ref().unwrap();
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|&&pixel| pixel != [255; 4])
        .count()
}

#[test]
fn a_brushed_piece_whose_join_would_pass_i32_warns_on_its_layer_once() {
    // Verifies: ADR-0049, Q-3
    // The fuzz path of render-agg's
    // `a_piece_whose_join_lies_far_past_its_short_segment_is_not_drawn_and_does_not_panic`, moved
    // 30 pixels down, which the clip buffer holds whole: its short first segment's join with the long one lies thousands of pixels away, past what
    // agg-rust multiplies in `i32`. Two such parts of one feature are two pieces refused, and the
    // layer still warns once.
    let path = [
        (94.00000786351512, -7.817504489577309e-6),
        (61.000000000000114, 33.00000000000001),
        (-8584.616083133831, -0.003023169095998576),
        (61.00000000000001, 56.17101458477057),
        (94.00000786351512, -7.817504489577309e-6),
    ];
    let moved: Vec<(f64, f64)> = path.iter().map(|&(x, y)| (x, y + 30.0)).collect();
    let map = brushed_map(123, 101, 0.0);
    let once = rendered(&map, 0.0, &[&moved]);
    let codes: Vec<&str> = once.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn"], "{:?}", once.diagnostics);
    let message = &once.diagnostics[0].message;
    for part in ["`lines`", "1 piece ", "pixmap symbol"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    assert_eq!(drawn(&once), 0);
    let twice = rendered(&map, 0.0, &[&moved, &moved]);
    let codes: Vec<&str> = twice.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn"], "{:?}", twice.diagnostics);
    assert!(twice.diagnostics[0].message.contains("2 pieces "));
}

#[test]
fn a_long_segment_after_a_short_one_at_a_gentle_angle_is_drawn_and_warns_nothing() {
    // Verifies: ADR-0049
    // The line of render-agg's `a_long_segment_after_a_short_one_at_a_gentle_angle_is_drawn`: a
    // segment of 2 pixels, then one of 2100 at 10°, whose join lies far within the bound.
    let path = [
        (10.0, 10.0),
        (12.0, 10.0),
        (2080.096281325637, 374.6611731005537),
    ];
    let map = brushed_map(2200, 400, 0.0);
    let gentle = rendered(&map, 0.0, &[&path]);
    assert!(gentle.diagnostics.is_empty(), "{:?}", gentle.diagnostics);
    // The same 53513 pixels as without the WIDTH, which only grows the clip buffer.
    assert_eq!(drawn(&gentle), 53_513);
}
