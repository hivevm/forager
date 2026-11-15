//! A TrueType symbol at a point: its glyph read from the render's fonts through the port the
//! symbolizer asks, drawn as MapServer's drawGlyphMarker draws it, and what is reported where the
//! font or the character is missing (ADR-draft-truetype-marker-glyph-through-a-port).

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
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

struct Files(BTreeMap<&'static str, Vec<u8>>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

fn files() -> Files {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
    Files([("sans.ttf", std::fs::read(path).unwrap())].into())
}

/// A 41 × 41 image of the extent 0..40 with a point layer at (20, 20) drawing the TrueType symbol
/// of `font` and `character` in black at `SIZE 20`; the font set names `sans`.
fn map(font: &str, character: &str) -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 40.0,
            max_y: 40.0,
        }),
        size: Some([41, 41]),
        image_color: Some(WHITE),
        fonts: [("sans".to_owned(), "sans.ttf".to_owned())]
            .into_iter()
            .collect(),
        symbols: vec![Symbol {
            name: Some("ttf".to_owned()),
            kind: SymbolKind::Truetype,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: Some(character.to_owned()),
            font: Some(font.to_owned()),
            image: None,
            transparent: None,
            span: None,
        }],
        layers: vec![Layer {
            name: Some("points".into()),
            render_mode: Some(RenderMode::Point),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                styles: vec![Style {
                    symbol: Some(Bound::Value(SymbolRef::Index(1))),
                    size: Some(Bound::Value(20.0)),
                    color: Some(Bound::Value(BLACK)),
                    ..Style::default()
                }],
                ..Class::default()
            }],
            span: Some(Span::new("m.map", 3, 1)),
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render(map: &Map) -> Rendered {
    let point = r#"{"type": "Point", "coordinates": [20, 20]}"#;
    let sources = BTreeMap::from([("points".to_owned(), GeoJsonSource::parse(point).unwrap())]);
    ready(render_with_files(
        map,
        &sources,
        &files(),
        AggRenderer::new(),
    ))
}

/// How many pixels are not the image colour.
fn drawn(rendered: &Rendered) -> usize {
    let image = rendered.image.as_ref().unwrap();
    image
        .pixels
        .chunks(4)
        .filter(|p| *p != [255, 255, 255, 255])
        .count()
}

fn codes(rendered: &Rendered) -> Vec<&str> {
    rendered.diagnostics.iter().map(|d| &*d.code).collect()
}

#[test]
fn a_truetype_symbol_draws_its_glyph_centred_on_its_point() {
    let rendered = render(&map("sans", "A"));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert!(drawn(&rendered) > 20, "{} pixels drawn", drawn(&rendered));
    // Centred on (20, 20) by its control box: the corners stay white.
    let image = rendered.image.as_ref().unwrap();
    for (x, y) in [(1, 1), (39, 1), (1, 39), (39, 39)] {
        assert_eq!(image.pixel(x, y), Some(WHITE), "({x}, {y})");
    }
}

#[test]
fn a_character_the_font_lacks_draws_glyph_0_and_warns() {
    // Verifies: ADR-draft-truetype-marker-glyph-through-a-port
    let rendered = render(&map("sans", "☃"));
    assert_eq!(codes(&rendered), ["symbol-glyph-missing"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.contains("symbol `ttf`")
            && message.contains("U+2603")
            && message.contains("glyph 0"),
        "{message}"
    );
    assert!(drawn(&rendered) > 0, "glyph 0 is drawn");
}

#[test]
fn a_font_the_font_set_lacks_draws_nothing_and_warns() {
    // Verifies: ADR-draft-truetype-marker-glyph-through-a-port
    let rendered = render(&map("serif", "A"));
    assert_eq!(codes(&rendered), ["symbol-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.contains("symbol `ttf`") && message.contains("no font `serif`"),
        "{message}"
    );
    assert_eq!(drawn(&rendered), 0);
}
