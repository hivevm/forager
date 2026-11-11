//! The images one render holds decoded come to at most 256 MiB (Q-9): a render whose styles draw
//! five images of 4096 × 4096 pixels, 64 MiB each decoded, in turn holds four, and the fifth,
//! which would pass the bound, is refused before it is decoded. Measured under a counting
//! allocator; one test, so that no other test's allocations are counted with it.

use std::collections::BTreeMap;
use std::future::Future;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::render_with_files;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{DisplayItem, Draw, Renderer, RgbaImage};
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Style, Symbol,
    SymbolKind, SymbolRef,
};
use forager_symbolize::MAX_DECODED_IMAGE_BYTES;
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

const MIB: usize = 1024 * 1024;
const SIDE: u32 = 4096;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The same PNG under every path.
struct Same(Vec<u8>);

impl ResourceLoader for Same {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Ok(self.0.clone())
    }
}

/// A renderer that draws nothing, so that only what the engine holds is counted.
struct Nothing;

impl Draw for Nothing {
    fn draw(&mut self, _: &DisplayItem) {}
}

impl Renderer for Nothing {
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

/// A map of one point layer whose five styles each draw the pixmap symbol of the image `{i}.png`,
/// at its own size.
fn map() -> Map {
    let symbols = (0..5).map(|i| Symbol {
        name: Some(format!("{i}")),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: Some(format!("{i}.png")),
        transparent: None,
        span: None,
    });
    // MAXSIZE at the image's height: by MapServer's default of 500, computeSymbolStyle would
    // scale the image to 500.
    let styles = (1..=5)
        .map(|index| Style {
            symbol: Some(Bound::Value(SymbolRef::Index(index))),
            max_size: Some(f64::from(SIDE)),
            ..Style::default()
        })
        .collect();
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        }),
        size: Some([11, 11]),
        symbols: symbols.collect(),
        layers: vec![Layer {
            name: Some("places".into()),
            render_mode: Some(RenderMode::Point),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                styles,
                ..Class::default()
            }],
            ..Layer::default()
        }],
        ..Map::default()
    }
}

#[test]
fn the_image_that_would_pass_256_mib_is_refused_before_it_is_decoded() {
    // Verifies: Q-9, ADR-0029
    let file = forager_encode::png(&RgbaImage {
        width: SIDE,
        height: SIDE,
        pixels: vec![0; SIDE as usize * SIDE as usize * 4],
    })
    .unwrap();
    let files = Same(file);
    let decoded = SIDE as usize * SIDE as usize * 4;
    assert_eq!(4 * decoded, MAX_DECODED_IMAGE_BYTES);
    let point = r#"{"type": "Point", "coordinates": [5, 5]}"#;
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(point).unwrap())].into();
    let map = map();

    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let rendered = ready(render_with_files(&map, &sources, &files, Nothing));
    // Four images held and a fifth decoded would come to 320 MiB.
    let peak = ALLOCATOR.peak_usage() - before;
    assert!(
        peak < MAX_DECODED_IMAGE_BYTES + 4 * MIB,
        "a peak of {peak} bytes for four images held and one refused"
    );

    let messages: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| {
            assert_eq!(d.code, "pixmap-not-drawn", "{d:?}");
            d.message.as_str()
        })
        .collect();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].contains("symbol `4`"), "{}", messages[0]);
    assert!(messages[0].contains("4.png"), "{}", messages[0]);
    assert!(
        messages[0].contains(&format!(
            "past the limit of {MAX_DECODED_IMAGE_BYTES} bytes"
        )),
        "{}",
        messages[0]
    );
}
