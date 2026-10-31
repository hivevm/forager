//! The labels of point layers, drawn through the reference renderer and compared with a reference
//! image of Forager's own, as the geometry of render.rs is (ADR-0028). Set FORAGER_BLESS=1 to write
//! the reference after a deliberate change, and review the image before committing it.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_label::{Anchor, Font, Glyphs, TextPath, metrics};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render::{PathCommand, Point};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    AngleMode, Bound, Class, Expr, Extent, FeatureProperty, InlineFeature, Label, Layer,
    LayerStatus, Map, Op, Position, RenderMode, Rgba, ShapeType, SourceRef, Span, Style, Symbol,
    SymbolKind, SymbolRef, Units,
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

/// `map` with its layer read from one inline feature at (80, 30) of the `TEXT` `text` and the
/// property `name` of the value `name`, rendered without a source bound to it.
fn render_inline(mut map: Map, text: Option<&str>, name: &str) -> Rendered {
    map.layers[0].source = None;
    map.layers[0].features = vec![InlineFeature {
        shape_type: ShapeType::Point,
        parts: vec![vec![[80.0, 30.0]]],
        properties: vec![FeatureProperty {
            name: "name".to_owned(),
            value: name.to_owned(),
        }],
        text: text.map(str::to_owned),
        span: None,
    }];
    let sources = BTreeMap::<String, GeoJsonSource>::new();
    ready(render_with_files(
        &map,
        &sources,
        &Files,
        AggRenderer::new(),
    ))
}

#[test]
fn an_inline_features_text_is_its_labels_text_after_the_label_class_and_labelitem() {
    // msShapeGetLabelAnnotation takes the label's TEXT, else the class's, else the
    // LABELITEM's value where it is not empty, else the feature's TEXT; the frontend folds the
    // first three into the label's text.
    let one = r#"{"type": "Feature", "properties": {"name": "Fen"},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let reference = render(&map(label_item()), one).image.unwrap();
    let no_text = || {
        map(Label {
            text: None,
            ..label_item()
        })
    };
    let cases = [
        // The feature's TEXT where the label has no text.
        (no_text(), Some("Fen"), "Other"),
        // The label's or the class's TEXT before the feature's.
        (
            map(Label {
                text: Some(Expr::String("Fen".to_owned())),
                ..label_item()
            }),
            Some("Other"),
            "Other",
        ),
        // The LABELITEM's value before the feature's.
        (map(label_item()), Some("Other"), "Fen"),
    ];
    for (map, text, name) in cases {
        let rendered = render_inline(map, text, name);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        assert_eq!(rendered.image.unwrap(), reference, "{text:?} {name:?}");
    }
    // Neither the label nor the feature has a text: no label, and nothing to report, as
    // MapServer skips it.
    let rendered = render_inline(no_text(), None, "Fen");
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        ink(rendered.image.as_ref().unwrap(), (0, 160), (0, 60)),
        None
    );
    // Nor does a feature of a source other than inline features have one.
    let rendered = render(&no_text(), one);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        ink(rendered.image.as_ref().unwrap(), (0, 160), (0, 60)),
        None
    );
    // A known divergence (docs/semantics/feature.md): a LABELITEM whose value is empty, or that
    // names an item the layer's ITEMS does not hold, draws no label, where MapServer gives such
    // an item "" and falls back on the feature's TEXT, as the style IR does not tell a label's
    // text from its LABELITEM.
    let rendered = render_inline(map(label_item()), Some("Fen"), "");
    assert_eq!(
        ink(rendered.image.as_ref().unwrap(), (0, 160), (0, 60)),
        None
    );
}

/// A point inline feature at `at` of the property `k` of the value `k` and the `TEXT` `text`.
fn inline_point(at: [f64; 2], k: &str, text: Option<&str>) -> InlineFeature {
    InlineFeature {
        shape_type: ShapeType::Point,
        parts: vec![vec![at]],
        properties: vec![FeatureProperty {
            name: "k".to_owned(),
            value: k.to_owned(),
        }],
        text: text.map(str::to_owned),
        span: None,
    }
}

#[test]
fn a_label_reads_the_text_of_the_features_its_class_draws_by_their_place() {
    let one = r#"{"type": "Feature", "properties": {"name": "Fen"},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let reference = render(&map(label_item()), one).image.unwrap();
    let sources = BTreeMap::<String, GeoJsonSource>::new();
    // The first feature out of view: the second's text is read, not the first's.
    let mut placed = map(Label {
        text: None,
        ..label_item()
    });
    placed.layers[0].source = None;
    placed.layers[0].features = vec![
        inline_point([500.0, 500.0], "a", Some("Other")),
        inline_point([80.0, 30.0], "a", Some("Fen")),
    ];
    let rendered = ready(render_with_files(
        &placed,
        &sources,
        &Files,
        AggRenderer::new(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.image.unwrap(), reference);
    // A label without a text, and without a FONT, whose class draws only features without a
    // TEXT, is skipped as MapServer skips it, and not reported; another class's feature with a
    // TEXT does not make it a label.
    let is_a = Expr::call(
        Op::Eq,
        vec![
            Expr::call(Op::ToString, vec![Expr::property("k")]),
            "a".into(),
        ],
    );
    let mut classes = map(label_item());
    let layer = &mut classes.layers[0];
    layer.source = None;
    layer.classes = vec![
        Class {
            expression: Some(is_a),
            labels: vec![Label {
                text: Some(Expr::String("Fen".to_owned())),
                ..label_item()
            }],
            ..Class::default()
        },
        Class {
            labels: vec![Label {
                text: None,
                font: None,
                ..label_item()
            }],
            ..Class::default()
        },
    ];
    layer.features = vec![
        inline_point([80.0, 30.0], "a", Some("Fen")),
        inline_point([20.0, 10.0], "b", None),
    ];
    let rendered = ready(render_with_files(
        &classes,
        &sources,
        &Files,
        AggRenderer::new(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.image.unwrap(), reference);
}

#[test]
fn a_label_that_cannot_be_drawn_yet_is_left_out_with_its_reason() {
    // Verifies: ADR-0028
    let with = |change: fn(&mut Label)| {
        let mut label = label_item();
        change(&mut label);
        label
    };
    let not_drawn: [fn(&mut Label); 16] = [
        |l| l.angle_mode = Some(AngleMode::Follow),
        |l| l.max_length = Some(4.0),
        |l| l.shadow_color = Some(Bound::Value(WHITE)),
        |l| l.styles = vec![Style::default()],
        |l| l.min_scale_denom = Some(1000.0),
        |l| l.max_scale_denom = Some(1000.0),
        |l| {
            l.font = Some(Bound::Binding {
                binding: "font".to_owned(),
            });
        },
        |l| l.font = None,
        |l| l.priority = Some(Bound::Value(0.5)),
        |l| l.size = Some(Bound::Expr(Expr::property("x"))),
        |l| l.color = Some(Bound::Expr(Expr::property("x"))),
        |l| l.outline_color = Some(Bound::Expr(Expr::property("x"))),
        |l| l.angle = Some(Bound::Expr(Expr::property("x"))),
        |l| l.position = Some(Bound::Expr(Expr::property("x"))),
        |l| l.align = Some(Bound::Expr(Expr::property("x"))),
        |l| l.offset = Some([Bound::Value(1.0), Bound::Expr(Expr::property("x"))]),
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
            with(|l| l.text = Some(Expr::String("two\nlin\u{2019}s".to_owned()))),
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
        // Partial, so that the label cache keeps it though it leaves the image.
        partials: Some(true),
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

/// A renderer that keeps the colour and the feature of each label it draws.
#[derive(Clone, Default)]
struct LabelRecorder(std::rc::Rc<std::cell::RefCell<Vec<(u64, Rgba)>>>);

impl forager_render::Renderer for LabelRecorder {
    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}

    fn draw(&mut self, item: &forager_render::DisplayItem) {
        if let forager_render::DrawOp::Glyphs { color, .. } = &item.op {
            self.0
                .borrow_mut()
                .push((item.feature_id.unwrap(), color.unwrap()));
        }
    }

    fn begin_layer(&mut self) {}
    fn end_layer(&mut self, _: u8) {}
    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        }
    }
}

#[test]
fn each_feature_is_labelled_by_the_class_that_draws_it() {
    // Verifies: ADR-0031
    // The first feature takes the second class, the second the first, and the third no class:
    // it is not labelled. The labels are added in the order of the features, not of the classes,
    // as msDrawShape adds them, and draw last added first, as the label cache draws them.
    const RED: Rgba = Rgba::opaque(200, 30, 30);
    let named = |name: &str, color: Rgba| Class {
        expression: Some(Expr::call(
            Op::Eq,
            vec![
                Expr::call(Op::ToString, vec![Expr::property("name")]),
                name.into(),
            ],
        )),
        labels: vec![Label {
            color: Some(Bound::Value(color)),
            ..label_item()
        }],
        ..Class::default()
    };
    let mut map = map(label_item());
    map.layers[0].classes = vec![named("Fen", RED), named("Forager", BLUE)];
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(PLACES).unwrap())].into();
    let recorder = LabelRecorder::default();
    let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(*recorder.0.borrow(), vec![(1, RED), (0, BLUE)]);
}

/// A renderer that keeps each label's display operation.
#[derive(Clone, Default)]
struct GlyphRecorder(std::rc::Rc<std::cell::RefCell<Vec<forager_render::DrawOp>>>);

impl forager_render::Renderer for GlyphRecorder {
    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}

    fn draw(&mut self, item: &forager_render::DisplayItem) {
        if matches!(item.op, forager_render::DrawOp::Glyphs { .. }) {
            self.0.borrow_mut().push(item.op.clone());
        }
    }

    fn begin_layer(&mut self) {}
    fn end_layer(&mut self, _: u8) {}
    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        }
    }
}

/// The display operations of the labels of `map` over `geojson`, with no diagnostic.
fn recorded(map: &Map, geojson: &str) -> Vec<forager_render::DrawOp> {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(geojson).unwrap())].into();
    let recorder = GlyphRecorder::default();
    let rendered = ready(render_with_files(map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    recorder.0.take()
}

/// One feature named "Fen" at the pixel (80, 29).
const FEN: &str = r#"{"type": "Feature", "properties": {"name": "Fen", "pos": "ul",
    "x": "2.5", "y": "-3.5", "size": "14", "angle": "30", "outline": "255 255 0",
    "align": "right", "bad": "zz"},
    "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;

/// The path of `text` in the test font at a glyph size of `size`, laid out on one line and drawn
/// where the label crate places it at `anchor` of the pixel (80, 29), `ox` and `oy` away, turned
/// by `degrees`.
fn expected(
    text: &str,
    size: f64,
    anchor: Anchor,
    (ox, oy): (i32, i32),
    degrees: f64,
) -> Vec<PathCommand> {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
    let data = std::fs::read(file).unwrap();
    let font = Font::new(&data).unwrap();
    let chars: Vec<char> = text.chars().collect();
    let ids = font.glyphs(&chars, true).unwrap().unwrap();
    let mut glyphs = Glyphs::new(size);
    let path = TextPath::new(&font, &mut glyphs, &ids).unwrap();
    let rotation = degrees * 0.017_453_292_519_943_295;
    let (origin, _) = metrics(
        Point { x: 80.0, y: 29.0 },
        anchor,
        &path.text_box(),
        ox,
        oy,
        rotation,
        0,
    );
    let visible = [0.0, 0.0, 160.0, 60.0];
    path.draw(std::slice::from_ref(&glyphs), origin, rotation, visible)
        .path
}

fn glyph_path(op: &forager_render::DrawOp) -> &[PathCommand] {
    match op {
        forager_render::DrawOp::Glyphs { path, .. } => path,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_label_stands_at_its_position_offset_and_angle_as_the_placement_puts_it() {
    // Verifies: ADR-0028
    let fen = || Label {
        text: Some(Expr::String("Fen".to_owned())),
        ..label_item()
    };
    for (position, anchor) in [
        (Position::Ul, Anchor::Ul),
        (Position::Uc, Anchor::Uc),
        (Position::Ur, Anchor::Ur),
        (Position::Cl, Anchor::Cl),
        (Position::Cc, Anchor::Cc),
        (Position::Cr, Anchor::Cr),
        (Position::Ll, Anchor::Ll),
        (Position::Lc, Anchor::Lc),
        (Position::Lr, Anchor::Lr),
        // Nothing else is in the way: AUTO takes its first position, UL.
        (Position::Auto, Anchor::Ul),
    ] {
        let label = Label {
            position: Some(Bound::Value(position)),
            offset: Some([Bound::Value(3.0), Bound::Value(-2.0)]),
            angle: Some(Bound::Value(20.0)),
            ..fen()
        };
        let ops = recorded(&map(label), FEN);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            glyph_path(&ops[0]),
            expected("Fen", 12.0, anchor, (3, -2), 20.0),
            "{position:?}"
        );
    }
}

#[test]
fn a_label_moves_away_from_the_marker_its_class_draws_first() {
    // Verifies: ADR-0028
    // A circle 9 pixels across: its box in the marker cache spans 8, so the label at UR stands
    // 4 + 3 = 7 across and 4 + 2 = 6 up; at CC the marker does not count. An outline adds half
    // its width, 2 / 2.
    let with_marker = |position, outline: Option<Rgba>| {
        let mut map = map(Label {
            text: Some(Expr::String("Fen".to_owned())),
            position: Some(Bound::Value(position)),
            offset: Some([Bound::Value(3.0), Bound::Value(2.0)]),
            outline_color: outline.map(Bound::Value),
            outline_width: Some(2.0),
            ..label_item()
        });
        map.symbols = vec![Symbol {
            name: Some("circle".to_owned()),
            kind: SymbolKind::Ellipse,
            paths: vec![vec![[1.0, 1.0]]],
            extent: Some([1.0, 1.0]),
            filled: true,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }];
        map.layers[0].classes[0].styles = vec![Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(9.0)),
            color: Some(Bound::Value(BLUE)),
            ..Style::default()
        }];
        recorded(&map, FEN)
    };
    let ur = with_marker(Position::Ur, None);
    assert_eq!(
        glyph_path(&ur[0]),
        expected("Fen", 12.0, Anchor::Ur, (7, 6), 0.0)
    );
    let cc = with_marker(Position::Cc, None);
    assert_eq!(
        glyph_path(&cc[0]),
        expected("Fen", 12.0, Anchor::Cc, (3, 2), 0.0)
    );
    let outlined = with_marker(Position::Ur, Some(WHITE));
    assert_eq!(
        glyph_path(&outlined[0]),
        expected("Fen", 12.0, Anchor::Ur, (8, 7), 0.0)
    );
}

#[test]
fn bound_properties_draw_as_their_values_do_once_bound_label_reads_them() {
    // Verifies: ADR-0028
    let fen = || Label {
        text: Some(Expr::String("Fen".to_owned())),
        ..label_item()
    };
    let bound = Label {
        position: Some(binding("pos")),
        // 2.5 and -3.5 rounded half to even: 2 and -4.
        offset: Some([binding("x"), binding("y")]),
        size: Some(binding("size")),
        angle: Some(binding("angle")),
        outline_color: Some(binding("outline")),
        align: Some(binding("align")),
        ..fen()
    };
    let literal = Label {
        position: Some(Bound::Value(Position::Ul)),
        offset: Some([Bound::Value(2.0), Bound::Value(-4.0)]),
        size: Some(Bound::Value(14.0)),
        angle: Some(Bound::Value(30.0)),
        outline_color: Some(Bound::Value(Rgba::opaque(255, 255, 0))),
        align: Some(Bound::Value(forager_style_ir::Align::Right)),
        ..fen()
    };
    assert_eq!(recorded(&map(bound), FEN), recorded(&map(literal), FEN));
    // MapServer's position constant, 101 for UL, reads as its name does.
    let numbered = r#"{"type": "Feature", "properties": {"pos": "101"},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let by_number = Label {
        position: Some(binding("pos")),
        ..fen()
    };
    let ul = Label {
        position: Some(Bound::Value(Position::Ul)),
        ..fen()
    };
    assert_eq!(
        recorded(&map(by_number), numbered),
        recorded(&map(ul), numbered)
    );
}

#[test]
fn a_bound_position_or_alignment_that_names_none_is_not_drawn_yet() {
    // MapServer keeps the value an earlier feature or the mapfile left, which the style IR does
    // not carry; an alignment matters only to several lines.
    let codes = |label: Label| {
        let rendered = render(&map(label), FEN);
        rendered
            .diagnostics
            .iter()
            .map(|d| d.code.clone())
            .collect::<Vec<_>>()
    };
    let fen = |text: &str| Label {
        text: Some(Expr::String(text.to_owned())),
        ..label_item()
    };
    assert_eq!(
        codes(Label {
            position: Some(binding("bad")),
            ..fen("Fen")
        }),
        ["label-binding-unsettled"]
    );
    assert!(
        codes(Label {
            align: Some(binding("bad")),
            ..fen("Fen")
        })
        .is_empty()
    );
    assert_eq!(
        codes(Label {
            align: Some(binding("bad")),
            ..fen("F\nen")
        }),
        ["label-binding-unsettled"]
    );
}

#[test]
fn a_wrapped_turned_and_outlined_label_matches_its_reference() {
    // Verifies: ADR-0028
    // Three lines centred, turned by 15 degrees, at LR with an offset, outlined 2 pixels wide in
    // yellow: the outline is drawn first, as the contour of the glyphs, then the fill.
    let label = Label {
        text: Some(Expr::String("Fen Forager l".to_owned())),
        wrap: Some(" ".to_owned()),
        align: Some(Bound::Value(forager_style_ir::Align::Center)),
        position: Some(Bound::Value(Position::Lr)),
        offset: Some([Bound::Value(4.0), Bound::Value(-20.0)]),
        angle: Some(Bound::Value(15.0)),
        outline_color: Some(Bound::Value(Rgba::opaque(240, 200, 0))),
        outline_width: Some(2.0),
        // Partial, so that the label cache keeps it though its box leaves the image at the top.
        partials: Some(true),
        ..label_item()
    };
    let one = r#"{"type": "Feature", "properties": {},
        "geometry": {"type": "Point", "coordinates": [30, 30]}}"#;
    let rendered = render(&map(label), one);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert!(
        image.pixels.chunks(4).any(|p| p == [240, 200, 0, 255]),
        "the outline"
    );
    assert!(
        image.pixels.chunks(4).any(|p| p == [30, 60, 200, 255]),
        "the fill"
    );
    assert_matches_reference(&image, "label_wrapped_outlined.png");
}

/// A property bound to the feature's property `name`.
fn binding<T>(name: &str) -> Bound<T> {
    Bound::Binding {
        binding: name.to_owned(),
    }
}

#[test]
fn the_outline_of_a_glyph_just_outside_the_image_still_paints_its_edge() {
    // Verifies: ADR-0028
    // An `l` left of the image's left edge, at CL of the pixel (0, 29), its box ending a pixel
    // left of the image: its glyph lies wholly outside, yet its outline, 7 wide, reaches
    // 3.5 pixels beyond it, into column 0.
    let mut map = map(Label {
        text: Some(Expr::String("l".to_owned())),
        position: Some(Bound::Value(Position::Cl)),
        offset: Some([Bound::Value(-1.0), Bound::Value(0.0)]),
        outline_color: Some(Bound::Value(Rgba::opaque(240, 200, 0))),
        outline_width: Some(6.0),
        // Partial, so that the label cache keeps it outside the image.
        partials: Some(true),
        ..label_item()
    });
    map.layers[0].classes[0].labels[0].size = Some(Bound::Value(10.0));
    let one = r#"{"type": "Feature", "properties": {},
        "geometry": {"type": "Point", "coordinates": [0, 30]}}"#;
    let rendered = render(&map, one);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert!(
        ink(&image, (0, 1), (0, 60)).is_some(),
        "the outline paints column 0"
    );
    let unoutlined = render(
        &{
            let mut m = map.clone();
            m.layers[0].classes[0].labels[0].outline_color = None;
            m
        },
        one,
    );
    assert_eq!(
        ink(&unoutlined.image.unwrap(), (0, 160), (0, 60)),
        None,
        "the glyph alone lies outside"
    );
}

#[test]
fn the_outline_width_is_scaled_by_the_glyph_size_over_the_size_and_rounded_half_to_even() {
    // Verifies: ADR-0028
    // SIZE 10 held at MINSIZE 15: 3 × (15 / 10) = 4.5, MS_NINT gives 4, AGG's contour 5.
    let label = Label {
        text: Some(Expr::String("Fen".to_owned())),
        size: Some(Bound::Value(10.0)),
        min_size: Some(15.0),
        outline_color: Some(Bound::Value(WHITE)),
        outline_width: Some(3.0),
        ..label_item()
    };
    let ops = recorded(&map(label), FEN);
    let forager_render::DrawOp::Glyphs { outline, .. } = &ops[0] else {
        panic!("{:?}", ops[0]);
    };
    assert_eq!(outline.map(|o| o.width), Some(5.0));
}

#[test]
fn a_label_moves_away_from_a_simple_marker_without_points_as_from_one_pixel_a_size() {
    // Verifies: ADR-0028
    // initSymbol sizes a symbol without POINTS 1 by 1: SIZE 10 draws it 10 by 10, so the label
    // at UR stands (10 - 1) / 2 + 3 = 7.5, cut to 7, across and up.
    let mut map = map(Label {
        text: Some(Expr::String("Fen".to_owned())),
        position: Some(Bound::Value(Position::Ur)),
        offset: Some([Bound::Value(3.0), Bound::Value(3.0)]),
        ..label_item()
    });
    map.symbols = vec![Symbol {
        name: Some("simple".to_owned()),
        kind: SymbolKind::Simple,
        paths: vec![],
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }];
    map.layers[0].classes[0].styles = vec![Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(10.0)),
        color: Some(Bound::Value(BLUE)),
        ..Style::default()
    }];
    let ops = recorded(&map, FEN);
    assert_eq!(
        glyph_path(&ops[0]),
        expected("Fen", 12.0, Anchor::Ur, (7, 7), 0.0)
    );
}

/// The features labelled, in the order their labels are drawn, of `map` over points named by
/// `name` at `(x, y)` each, with properties `extra`.
fn labelled(map: &Map, points: &[(&str, f64, f64, &str)]) -> Vec<u64> {
    let features: Vec<String> = points
        .iter()
        .map(|(name, x, y, extra)| {
            format!(
                r#"{{"type": "Feature", "properties": {{"name": "{name}"{extra}}},
                    "geometry": {{"type": "Point", "coordinates": [{x}, {y}]}}}}"#
            )
        })
        .collect();
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    );
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let recorder = LabelRecorder::default();
    let rendered = ready(render_with_files(map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    recorder.0.take().into_iter().map(|(id, _)| id).collect()
}

#[test]
fn of_two_labels_that_overlap_the_label_cache_keeps_the_one_added_last() {
    // Verifies: G-10
    let points = [("Forager", 60.0, 30.0, ""), ("Fen", 75.0, 32.0, "")];
    assert_eq!(labelled(&map(label_item()), &points), [1]);
    // Far enough apart, both, the last added first.
    let apart = [("Forager", 50.0, 30.0, ""), ("Fen", 120.0, 30.0, "")];
    assert_eq!(labelled(&map(label_item()), &apart), [1, 0]);
    // A buffer grows the boxes until they meet.
    let buffered = Label {
        buffer: Some(14.0),
        ..label_item()
    };
    assert_eq!(labelled(&map(buffered), &apart), [1]);
    // Forced, both are drawn.
    let forced = Label {
        force: Some(forager_style_ir::Force::On),
        ..label_item()
    };
    assert_eq!(labelled(&map(forced), &points), [1, 0]);
    let rendered = render(&map(label_item()), &collection(&points));
    assert_matches_reference(&rendered.image.unwrap(), "label_collision.png");
}

/// The points as a feature collection.
fn collection(points: &[(&str, f64, f64, &str)]) -> String {
    let features: Vec<String> = points
        .iter()
        .map(|(name, x, y, _)| {
            format!(
                r#"{{"type": "Feature", "properties": {{"name": "{name}"}},
                    "geometry": {{"type": "Point", "coordinates": [{x}, {y}]}}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    )
}

#[test]
fn a_higher_priority_is_placed_first_and_a_bound_one_is_held_from_1_to_10() {
    // Verifies: G-10
    let points = [
        ("Forager", 60.0, 30.0, r#", "p": "7""#),
        ("Fen", 75.0, 32.0, r#", "p": "3""#),
    ];
    let bound = Label {
        priority: Some(binding("p")),
        ..label_item()
    };
    assert_eq!(labelled(&map(bound.clone()), &points), [0]);
    // 30 is held at 10, an empty value is 1.
    let held = [
        ("Forager", 60.0, 30.0, r#", "p": "30""#),
        ("Fen", 75.0, 32.0, r#", "p": "9""#),
    ];
    assert_eq!(labelled(&map(bound.clone()), &held), [0]);
    let empty = [
        ("Forager", 60.0, 30.0, r#", "p": "2""#),
        ("Fen", 75.0, 32.0, r#", "p": """#),
    ];
    assert_eq!(labelled(&map(bound), &empty), [0]);
    // A literal priority places the label among those of other layers' priorities.
    let mut two = map(Label {
        priority: Some(Bound::Value(4.0)),
        ..label_item()
    });
    let mut second = two.layers[0].clone();
    second.classes[0].labels[0].priority = None;
    two.layers.push(second);
    let sources: BTreeMap<String, GeoJsonSource> = [(
        "places".to_owned(),
        GeoJsonSource::parse(&collection(&[("Forager", 60.0, 30.0, "")])).unwrap(),
    )]
    .into();
    let recorder = LabelRecorder::default();
    ready(render_with_files(&two, &sources, &Files, recorder.clone()));
    assert_eq!(recorder.0.take().len(), 1, "the label of priority 4 only");
}

#[test]
fn a_label_leaving_the_image_is_left_out_unless_partials_or_force_allow_it() {
    // Verifies: G-10
    let edge = [("Forager", 2.0, 30.0, "")];
    assert_eq!(labelled(&map(label_item()), &edge), Vec::<u64>::new());
    for label in [
        Label {
            partials: Some(true),
            ..label_item()
        },
        Label {
            force: Some(forager_style_ir::Force::On),
            ..label_item()
        },
    ] {
        assert_eq!(labelled(&map(label), &edge), [0]);
    }
}

#[test]
fn a_label_is_left_out_where_it_meets_the_marker_of_another_feature() {
    // Verifies: G-10
    // A square marker 9 pixels across under each point; a label at CC lies over its own.
    let mut map = map(label_item());
    map.layers[0].classes[0].styles = vec![Style {
        size: Some(Bound::Value(9.0)),
        color: Some(Bound::Value(BLUE)),
        ..Style::default()
    }];
    let points = [("Fen", 60.0, 30.0, ""), ("Forager", 110.0, 30.0, "")];
    assert_eq!(labelled(&map, &points), [1, 0], "each over its own marker");
    // Without the markers, the label of Fen, added last, would be drawn; with them, each label
    // meets the other's marker.
    let near = [("Forager", 60.0, 30.0, ""), ("Fen", 70.0, 30.0, "")];
    assert_eq!(labelled(&map, &near), Vec::<u64>::new());
    // A text of line breaks only draws nothing, yet its marker is cached.
    let blank = [("Forager", 60.0, 30.0, ""), (r"\n", 80.0, 30.0, "")];
    assert_eq!(labelled(&map, &blank), Vec::<u64>::new());
}

#[test]
fn a_labels_expression_decides_for_which_features_it_is_drawn() {
    // Verifies: G-10
    let label = Label {
        expression: Some(Expr::call(
            Op::Eq,
            vec![
                Expr::call(Op::ToString, vec![Expr::property("name")]),
                "Fen".into(),
            ],
        )),
        ..label_item()
    };
    let apart = [("Forager", 40.0, 30.0, ""), ("Fen", 120.0, 30.0, "")];
    assert_eq!(labelled(&map(label.clone()), &apart), [1]);
    // A label left out by its expression takes no place in the cache.
    let overlapping = [("Fen", 60.0, 30.0, ""), ("Forager", 75.0, 32.0, "")];
    assert_eq!(labelled(&map(label), &overlapping), [0]);
}

#[test]
fn min_distance_leaves_out_a_label_near_one_of_the_same_text() {
    // Verifies: G-10
    let spaced = Label {
        min_distance: Some(40.0),
        ..label_item()
    };
    let points = [
        ("Fen", 30.0, 30.0, ""),
        ("Fen", 90.0, 30.0, ""),
        ("Fan", 130.0, 30.0, ""),
    ];
    assert_eq!(labelled(&map(spaced.clone()), &points), [2, 1]);
    assert_eq!(labelled(&map(label_item()), &points), [2, 1, 0]);
}

#[test]
fn a_label_over_a_marker_whose_size_is_not_computed_yet_is_not_drawn() {
    // Verifies: G-10
    // The label cache holds the box of the marker of the class's first style beneath every
    // label of a point layer, at CC too; a pixmap, SVG, or TrueType symbol has no size yet, so
    // the label is left out rather than placed without that box.
    for kind in [SymbolKind::Pixmap, SymbolKind::Svg, SymbolKind::Truetype] {
        let mut map = map(label_item());
        map.symbols = vec![Symbol {
            name: Some("unsized".to_owned()),
            kind,
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
        map.layers[0].classes[0].styles = vec![Style {
            symbol: Some(Bound::Value(SymbolRef::Index(1))),
            size: Some(Bound::Value(9.0)),
            ..Style::default()
        }];
        let sources: BTreeMap<String, GeoJsonSource> = [(
            "places".to_owned(),
            GeoJsonSource::parse(&collection(&[("Fen", 80.0, 30.0, "")])).unwrap(),
        )]
        .into();
        let recorder = GlyphRecorder::default();
        let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
        assert!(recorder.0.take().is_empty(), "{kind:?}");
        let label = rendered
            .diagnostics
            .iter()
            .find(|d| d.code == "label-not-drawn")
            .unwrap_or_else(|| panic!("{kind:?}: {:?}", rendered.diagnostics));
        assert!(
            label.message.contains("which the label cache holds"),
            "{}",
            label.message
        );
    }
}

#[test]
fn the_label_cache_holds_at_most_100000_labels_and_reports_those_past_it_once() {
    // Verifies: Q-9, ADR-0035
    // One feature of 100,001 points: the first 100,000 at one point, where all but one collide,
    // and the last alone, where it would be drawn first. Past the limit it is left out untested.
    let mut points = vec!["[40, 30]"; forager_label::MAX_LABELS];
    points.push("[130, 30]");
    let geojson = format!(
        r#"{{"type": "Feature", "properties": {{"name": "Fen"}},
            "geometry": {{"type": "MultiPoint", "coordinates": [{}]}}}}"#,
        points.join(",")
    );
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let recorder = GlyphRecorder::default();
    let rendered = ready(render_with_files(
        &map(label_item()),
        &sources,
        &Files,
        recorder.clone(),
    ));
    assert_eq!(
        recorder.0.take().len(),
        1,
        "the point past the limit is not drawn"
    );
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["labels-over-limit"]);
    let d = &rendered.diagnostics[0];
    assert!(
        d.message.contains("100000") && d.message.contains("1 labels past it"),
        "{}",
        d.message
    );
    assert_eq!(d.span, Some(Span::new("m.map", 9, 7)));
    // At the limit, nothing is left out: the lone point is drawn.
    let at_limit = geojson.replace("[40, 30],[130, 30]", "[130, 30]");
    let sources: BTreeMap<String, GeoJsonSource> = [(
        "places".to_owned(),
        GeoJsonSource::parse(&at_limit).unwrap(),
    )]
    .into();
    let recorder = GlyphRecorder::default();
    let rendered = ready(render_with_files(
        &map(label_item()),
        &sources,
        &Files,
        recorder.clone(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(recorder.0.take().len(), 2);
}

/// The render reports every label of its cache in the order msDrawLabelCache walks it, the last
/// added first, the one drawn with the box it was drawn in, which holds its ink to the pixel, and
/// the one it discarded without.
#[test]
fn the_render_reports_its_labels_drawn_and_discarded_in_the_caches_order() {
    let overlapping = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"name": "Forager"},
         "geometry": {"type": "Point", "coordinates": [40, 30]}},
        {"type": "Feature", "properties": {"name": "Fen"},
         "geometry": {"type": "Point", "coordinates": [42, 30]}}
    ]}"#;
    let rendered = render(&map(label_item()), overlapping);
    let labels: Vec<_> = rendered
        .labels
        .iter()
        .map(|l| {
            (
                l.priority,
                l.layer,
                l.class,
                l.text.as_str(),
                l.drawn.is_some(),
            )
        })
        .collect();
    assert_eq!(
        labels,
        [(1, 0, 0, "Fen", true), (1, 0, 0, "Forager", false)]
    );
    let [x0, y0, x1, y1] = rendered.labels[0].drawn.unwrap();
    let image = rendered.image.unwrap();
    let [ix0, iy0, ix1, iy1] = ink(&image, (0, 160), (0, 60)).unwrap();
    assert!(
        x0.floor() <= f64::from(ix0) && y0.floor() <= f64::from(iy0),
        "the ink {:?} starts before the box {:?}",
        [ix0, iy0, ix1, iy1],
        [x0, y0, x1, y1]
    );
    assert!(
        f64::from(ix1) <= x1.ceil() && f64::from(iy1) <= y1.ceil(),
        "the ink {:?} ends past the box {:?}",
        [ix0, iy0, ix1, iy1],
        [x0, y0, x1, y1]
    );
}
