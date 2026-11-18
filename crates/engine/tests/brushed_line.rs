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

/// The pin, a pixmap symbol.
fn pin() -> Symbol {
    Symbol {
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
    }
}

/// A filled circle, an ellipse symbol.
fn dot() -> Symbol {
    Symbol {
        name: Some("dot".to_owned()),
        kind: SymbolKind::Ellipse,
        paths: vec![vec![[1.0, 1.0]]],
        extent: Some([1.0, 1.0]),
        filled: true,
        image: None,
        ..pin()
    }
}

/// A map of `width` × `height` pixels, one map unit a pixel, its top-left corner at (0, 50),
/// whose line layer `lines` brushes its features with `symbol`, a symbol other than a pixmap in
/// blue at `SIZE 10`, a pixmap at its own size. The style's `WIDTH` of 10000,
/// which a brush does not take, grows the class's clip buffer to 10001 pixels, so that neither the
/// engine nor the renderer cuts a line within it.
fn brushed_map(width: u32, height: u32, symbol: Symbol) -> Map {
    let mut style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        width: Some(Bound::Value(10_000.0)),
        ..Style::default()
    };
    if symbol.kind != SymbolKind::Pixmap {
        style.color = Some(Bound::Value(Rgba::opaque(30, 60, 200)));
        style.size = Some(Bound::Value(10.0));
    }
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
            min_x: 0.0,
            min_y: 51.0 - f64::from(height),
            max_x: f64::from(width - 1),
            max_y: 50.0,
        }),
        size: Some([width, height]),
        image_color: Some(WHITE),
        symbols: vec![symbol],
        layers: vec![layer],
        ..Map::default()
    }
}

/// `map` rendered with one feature, a line of a part for each of `parts`, each part's points in
/// pixels from the image's top-left corner.
fn rendered(map: &Map, parts: &[&[(f64, f64)]]) -> Rendered {
    let parts: Vec<String> = parts
        .iter()
        .map(|points| {
            let coordinates: Vec<String> = points
                .iter()
                .map(|&(x, y)| format!("[{x}, {}]", 50.0 - y))
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

/// The fuzz path of render-agg's
/// `a_piece_whose_join_lies_far_past_its_short_segment_is_not_drawn_and_does_not_panic`, moved 30
/// pixels down, which the clip buffer holds whole: its short first segment's join with the long
/// one lies thousands of pixels away, past what agg-rust multiplies in `i32`.
fn far_join() -> Vec<(f64, f64)> {
    let path = [
        (94.00000786351512, -7.817504489577309e-6),
        (61.000000000000114, 33.00000000000001),
        (-8584.616083133831, -0.003023169095998576),
        (61.00000000000001, 56.17101458477057),
        (94.00000786351512, -7.817504489577309e-6),
    ];
    path.iter().map(|&(x, y)| (x, y + 30.0)).collect()
}

/// Asserts that `rendered` carries exactly one diagnostic, the warning `code` for `pieces`
/// pieces of `kind`, and draws nothing.
fn warns_once(rendered: &Rendered, code: &str, pieces: &str, kind: &str) {
    let codes: Vec<&str> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, [code], "{:?}", rendered.diagnostics);
    let message = &rendered.diagnostics[0].message;
    for part in ["`lines`", pieces, kind] {
        assert!(message.contains(part), "{part}: {message}");
    }
    assert_eq!(drawn(rendered), 0);
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
    // Two such parts of one feature are two pieces refused, and the layer still warns once.
    let path = far_join();
    let map = brushed_map(123, 101, pin());
    let once = rendered(&map, &[&path]);
    warns_once(&once, "pixmap-not-drawn", "1 piece ", "pixmap symbol");
    let twice = rendered(&map, &[&path, &path]);
    warns_once(&twice, "pixmap-not-drawn", "2 pieces ", "pixmap symbol");
}

#[test]
fn a_piece_brushed_with_an_ellipse_symbol_warns_symbol_not_drawn_once() {
    // Verifies: ADR-0049, Q-3
    let path = far_join();
    let map = brushed_map(123, 101, dot());
    let twice = rendered(&map, &[&path, &path]);
    warns_once(&twice, "symbol-not-drawn", "2 pieces ", "ellipse");
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
    let map = brushed_map(2200, 400, pin());
    let gentle = rendered(&map, &[&path]);
    assert!(gentle.diagnostics.is_empty(), "{:?}", gentle.diagnostics);
    // The same 53513 pixels as without the WIDTH, which only grows the clip buffer.
    assert_eq!(drawn(&gentle), 53_513);
}
