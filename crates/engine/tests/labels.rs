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
    AngleMode, Bound, Class, Expr, Extent, Label, Layer, LayerStatus, Map, Op, Position,
    RenderMode, Rgba, SourceRef, Span, Style, Units,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The font set's files, test fonts of the label crate whose licence lies beside them:
/// `fonts/sans.ttf` a subset of DejaVu Sans, and `fonts/<name>.ttf` its variant
/// `dejavu-sans-<name>.ttf`.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        let fixture = match path.as_str() {
            "fonts/sans.ttf" => "dejavu-sans-subset.ttf".to_owned(),
            other => match other.strip_prefix("fonts/") {
                Some(name) => format!("dejavu-sans-{name}"),
                None => return Err(SourceError::new("no such file")),
            },
        };
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../label/tests/fixtures")
            .join(fixture);
        std::fs::read(file).map_err(|e| SourceError::new(e.to_string()))
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
        fonts: ["sans", "broken", "huge"]
            .into_iter()
            .map(|name| (name.to_owned(), format!("fonts/{name}.ttf")))
            .collect(),
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
    {"type": "Feature", "properties": {"kind": "unnamed", "capital": true},
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
    let with = |change: fn(&mut Label)| {
        let mut label = label_item();
        change(&mut label);
        label
    };
    let not_drawn: [fn(&mut Label); 18] = [
        |l| l.position = Some(Bound::Value(Position::Ul)),
        |l| l.offset = Some([Bound::Value(2.0), Bound::Value(0.0)]),
        |l| l.angle = Some(Bound::Value(30.0)),
        |l| l.angle_mode = Some(AngleMode::Follow),
        |l| l.wrap = Some(" ".to_owned()),
        |l| l.max_length = Some(4.0),
        |l| l.priority = Some(Bound::Value(5.0)),
        |l| l.expression = Some(Expr::Bool(true)),
        |l| l.outline_color = Some(Bound::Value(WHITE)),
        |l| l.shadow_color = Some(Bound::Value(WHITE)),
        |l| l.styles = vec![Style::default()],
        |l| l.min_scale_denom = Some(1000.0),
        |l| l.max_scale_denom = Some(1000.0),
        |l| {
            l.font = Some(Bound::Binding {
                binding: "font".to_owned(),
            });
        },
        |l| {
            l.size = Some(Bound::Binding {
                binding: "size".to_owned(),
            });
        },
        |l| {
            l.color = Some(Bound::Binding {
                binding: "colour".to_owned(),
            });
        },
        |l| l.font = None,
        |l| l.priority = Some(Bound::Value(0.5)),
    ];
    let mut cases: Vec<(Label, &str)> = not_drawn
        .into_iter()
        .map(|change| (with(change), "label-not-drawn"))
        .collect();
    cases.extend([
        (
            with(|l| l.font = Some(Bound::Value("gone".to_owned()))),
            "label-font-unusable",
        ),
        (
            with(|l| l.text = Some(Expr::String("it\u{2019}s".to_owned()))),
            "label-line-unsupported",
        ),
        (
            with(|l| l.text = Some(Expr::String("two\nlines".to_owned()))),
            "label-line-unsupported",
        ),
        (
            with(|l| l.text = Some(Expr::call(Op::Upper, vec![Expr::property("name")]))),
            "label-text-unsupported",
        ),
        (
            with(|l| l.text = Some(Expr::call(Op::ToString, vec![Expr::property("capital")]))),
            "label-text-unsupported",
        ),
        (
            with(|l| {
                l.font = Some(Bound::Value("broken".to_owned()));
                l.text = Some(Expr::String("1".to_owned()));
            }),
            "label-glyph-unreadable",
        ),
    ]);
    for (label, code) in cases {
        let rendered = render(&map(label.clone()), PLACES);
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(
            codes,
            [code],
            "one diagnostic for the label, whatever its features: {label:?}"
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
fn a_layer_whose_sizes_are_not_pixels_or_whose_class_has_several_labels_is_not_drawn_yet() {
    let mut sized = map(label_item());
    sized.layers[0].size_units = Some(Units::Meters);
    let mut several = map(label_item());
    several.layers[0].classes[0].labels.push(label_item());
    let mut symbol_scaled = map(label_item());
    symbol_scaled.layers[0].symbol_scale_denom = Some(10_000.0);
    for (map, count) in [(sized, 1), (several, 2), (symbol_scaled, 1)] {
        let rendered = render(&map, PLACES);
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(codes, vec!["label-not-drawn"; count]);
    }
    // A layer of pixels, and a PRIORITY of 1, the default, draw as without them.
    let mut pixels = map(label_item());
    pixels.layers[0].size_units = Some(Units::Pixels);
    pixels.layers[0].classes[0].labels[0].priority = Some(Bound::Value(1.0));
    let rendered = render(&pixels, PLACES);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.image, render(&map(label_item()), PLACES).image);
}

#[test]
fn a_null_property_draws_no_label_as_an_empty_one_does() {
    let null = r#"{"type": "Feature", "properties": {"name": null},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let rendered = render(&map(label_item()), null);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
}

#[test]
fn a_glyph_reaching_too_far_from_the_image_is_left_out_with_a_diagnostic() {
    // Verifies: ADR-0028
    // The `1` of this test font, a square around its origin with no advance, stands 16,000,000
    // pixels wide at 6000 points; the `2` beside it, about 650,000 pixels, is drawn.
    let label = Label {
        font: Some(Bound::Value("huge".to_owned())),
        text: Some(Expr::String("21".to_owned())),
        size: Some(Bound::Value(6000.0)),
        max_size: Some(6000.0),
        ..label_item()
    };
    let rendered = render(&map(label), PLACES);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-glyph-out-of-reach"]);
    let d = &rendered.diagnostics[0];
    assert_eq!(d.span, Some(Span::new("m.map", 9, 7)));
    assert!(
        d.message.contains("1 of its glyphs") && d.message.ends_with("those glyphs are left out"),
        "{}",
        d.message
    );
    let image = rendered.image.unwrap();
    assert!(
        image.pixels.chunks(4).any(|p| p != [255, 255, 255, 255]),
        "the `2` is drawn"
    );
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
