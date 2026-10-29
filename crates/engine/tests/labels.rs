//! The labels of point layers, drawn through the reference renderer and compared with a reference
//! image of Forager's own, as the geometry of render.rs is (ADR-0028). Set FORAGER_BLESS=1 to write
//! the reference after a deliberate change, and review the image before committing it.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, Label, Layer, LayerStatus, Map, Op, Position, RenderMode, Rgba,
    SourceRef, Span,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The font set's one file, a subset of DejaVu Sans whose licence lies beside it.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str() != "fonts/sans.ttf" {
            return Err(SourceError::new("no such file"));
        }
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
        Ok(std::fs::read(file).unwrap())
    }
}

/// A 160 × 60 image of the extent 0..159 × 0..59, one map unit a pixel, with a point layer whose
/// one label is `label`.
fn map(label: Label) -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 159.0,
            max_y: 59.0,
        }),
        size: Some([160, 60]),
        image_color: Some(WHITE),
        fonts: [("sans".to_owned(), "fonts/sans.ttf".to_owned())].into(),
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

/// A label of the layer's `LABELITEM` `name`, as the frontend writes it.
fn label_item() -> Label {
    Label {
        text: Some(Expr::call(Op::ToString, vec![Expr::property("name")])),
        font: Some(Bound::Value("sans".to_owned())),
        size: Some(Bound::Value(12.0)),
        color: Some(Bound::Value(BLUE)),
        span: Some(Span::new("m.map", 9, 7)),
        ..Label::default()
    }
}

const PLACES: &str = r#"{"type": "FeatureCollection", "features": [
    {"type": "Feature", "properties": {"name": "Forager"},
     "geometry": {"type": "Point", "coordinates": [40, 30]}},
    {"type": "Feature", "properties": {"name": "Fen"},
     "geometry": {"type": "Point", "coordinates": [125, 45]}},
    {"type": "Feature", "properties": {"kind": "unnamed"},
     "geometry": {"type": "Point", "coordinates": [125, 12]}}
]}"#;

fn render(map: &Map, geojson: &str) -> Rendered {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(geojson).unwrap())].into();
    ready(render_with_files(map, &sources, &Files, AggRenderer::new()))
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

/// The box of the pixels that are not white within `[x0, x1) × [y0, y1)`, `None` when all are.
fn ink(image: &RgbaImage, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> Option<[u32; 4]> {
    let mut found: Option<[u32; 4]> = None;
    for y in y0..y1 {
        for x in x0..x1 {
            if image.pixel(x, y) != Some(WHITE) {
                let b = found.get_or_insert([x, y, x, y]);
                *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    found
}

#[test]
fn a_point_layer_draws_each_features_label_centred_on_its_point() {
    // Verifies: ADR-0028
    let rendered = render(&map(label_item()), PLACES);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // Map y grows upwards: the point (40, 30) is pixel (40, 29), (125, 45) is pixel (125, 14).
    for ((x, y), area) in [
        ((40.0, 29.0), ((0, 85), (0, 60))),
        ((125.0, 14.0), ((85, 160), (0, 30))),
    ] {
        let [x0, y0, x1, y1] = ink(&image, area.0, area.1).expect("a label is drawn");
        let centre = (f64::from(x0 + x1 + 1) / 2.0, f64::from(y0 + y1 + 1) / 2.0);
        assert!(
            (centre.0 - (x + 0.5)).abs() <= 1.0 && (centre.1 - (y + 0.5)).abs() <= 1.0,
            "the label's ink centred at {centre:?}, its point at ({x}, {y})"
        );
    }
    // Its colour, where a glyph covers a pixel fully: the stem of the F of "Forager".
    assert!(image.pixels.chunks(4).any(|p| p == [30, 60, 200, 255]));
    assert_eq!(
        ink(&image, (85, 160), (30, 60)),
        None,
        "a feature without the property gets no label"
    );
    assert_matches_reference(&image, "label.png");
}

#[test]
fn a_label_is_drawn_from_its_text_its_class_text_or_its_labelitem_alike() {
    // The frontend gives a label its TEXT, else its class's, else the LABELITEM; a string and a
    // concatenation draw as the property they spell does.
    let one = r#"{"type": "Feature", "properties": {"name": "Fen", "a": "F", "b": "en"},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let reference = render(&map(label_item()), one).image.unwrap();
    for text in [
        Expr::String("Fen".to_owned()),
        Expr::call(
            Op::Concat,
            vec![
                Expr::call(Op::ToString, vec![Expr::property("a")]),
                Expr::call(Op::ToString, vec![Expr::property("b")]),
            ],
        ),
    ] {
        let rendered = render(
            &map(Label {
                text: Some(text),
                ..label_item()
            }),
            one,
        );
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        assert_eq!(rendered.image.unwrap(), reference);
    }
}

#[test]
fn a_label_that_cannot_be_drawn_yet_is_left_out_with_its_reason() {
    // Verifies: ADR-0028
    let cases = [
        (
            Label {
                font: Some(Bound::Value("gone".to_owned())),
                ..label_item()
            },
            "label-font-unusable",
        ),
        (
            Label {
                text: Some(Expr::String("it\u{2019}s".to_owned())),
                ..label_item()
            },
            "label-line-unsupported",
        ),
        (
            Label {
                text: Some(Expr::String("two\nlines".to_owned())),
                ..label_item()
            },
            "label-line-unsupported",
        ),
        (
            Label {
                text: Some(Expr::call(Op::Upper, vec![Expr::property("name")])),
                ..label_item()
            },
            "label-text-unsupported",
        ),
        (
            Label {
                position: Some(Bound::Value(Position::Ul)),
                ..label_item()
            },
            "label-not-drawn",
        ),
        (
            Label {
                color: Some(Bound::Binding {
                    binding: "colour".to_owned(),
                }),
                ..label_item()
            },
            "label-not-drawn",
        ),
    ];
    for (label, code) in cases {
        let rendered = render(&map(label), PLACES);
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(
            codes,
            [code],
            "one diagnostic for the label, whatever its features"
        );
        assert_eq!(rendered.diagnostics[0].span, Some(Span::new("m.map", 9, 7)));
        let image = rendered.image.unwrap();
        assert!(
            image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]),
            "{code}"
        );
    }
}

#[test]
fn a_label_of_a_line_layer_is_not_drawn_yet_and_says_so() {
    let mut map = map(label_item());
    map.layers[0].render_mode = Some(RenderMode::Line);
    let line = r#"{"type": "Feature", "properties": {"name": "Fen"},
        "geometry": {"type": "LineString", "coordinates": [[10, 10], [100, 40]]}}"#;
    let rendered = render(&map, line);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
}
