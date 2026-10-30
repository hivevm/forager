//! The memory the labels of a render take (Q-9): what a render holds per label until the labels
//! are drawn after the last layer is measured under a counting allocator, as the difference
//! between a render of many labelled points and the same render with no label. One test, so
//! that no other test's allocations are counted with it.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::render_with_files;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{DisplayItem, Renderer, RgbaImage};
use forager_style_ir::{
    Bound, Class, Expr, Extent, Label, Layer, LayerStatus, Map, Op, RenderMode, Rgba, SourceRef,
};
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The test font of the label crate, a subset of DejaVu Sans whose licence lies beside it.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
        Ok(std::fs::read(file).unwrap())
    }
}

/// A renderer that keeps nothing of what it is handed, so that only the engine's memory counts.
struct Discard(usize);

impl Renderer for Discard {
    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}
    fn draw(&mut self, item: &DisplayItem) {
        self.0 += usize::from(matches!(item.op, forager_render::DrawOp::Glyphs { .. }));
    }
    fn begin_layer(&mut self) {}
    fn end_layer(&mut self, _: u8) {}
    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: (self.0 as u64).to_le_bytes().to_vec(),
        }
    }
}

fn map(labelled: bool) -> Map {
    let label = Label {
        text: labelled.then(|| Expr::call(Op::ToString, vec![Expr::property("name")])),
        font: Some(Bound::Value("sans".to_owned())),
        ..Label::default()
    };
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1000.0,
            max_y: 1000.0,
        }),
        size: Some([1000, 1000]),
        fonts: [("sans".to_owned(), "sans.ttf".to_owned())].into(),
        layers: vec![Layer {
            name: Some("places".into()),
            render_mode: Some(RenderMode::Point),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                labels: vec![label],
                ..Class::default()
            }],
            ..Layer::default()
        }],
        ..Map::default()
    }
}

/// The peak of the bytes a render of `map` allocates over the source, and the labels it drew.
fn peak(map: &Map, sources: &BTreeMap<String, GeoJsonSource>) -> (usize, usize) {
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let drawn = {
        let rendered = ready(render_with_files(map, sources, &Files, Discard(0)));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        let pixels = rendered.image.unwrap().pixels;
        u64::from_le_bytes(pixels.try_into().unwrap())
    };
    (ALLOCATOR.peak_usage() - before, drawn as usize)
}

#[test]
fn a_render_holds_little_per_label_until_the_labels_are_drawn() {
    // Verifies: Q-9
    const POINTS: usize = 20_000;
    let features: Vec<String> = (0..POINTS)
        .map(|i| {
            format!(
                r#"{{"type": "Feature", "properties": {{"name": "Forager {i}"}},
                    "geometry": {{"type": "Point", "coordinates": [{}, {}]}}}}"#,
                i % 1000,
                (i / 1000) * 40
            )
        })
        .collect();
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    );
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let (without, none) = peak(&map(false), &sources);
    let (with, drawn) = peak(&map(true), &sources);
    assert_eq!((none, drawn), (0, POINTS), "every label is drawn");
    // About 360 bytes: the label's glyph indices, its line, its box, and its point; its path, of
    // thousands of bytes, is built only as it is drawn.
    let per_label = with.saturating_sub(without) / POINTS;
    assert!(
        per_label < 400,
        "{per_label} bytes per label ({with} with labels, {without} without)"
    );
}
