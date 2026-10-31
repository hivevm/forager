//! The labels of point layers, drawn through the reference renderer and compared with a reference
//! image of Forager's own, as the geometry of render.rs is (ADR-0028). Set FORAGER_BLESS=1 to write
//! the reference after a deliberate change, and review the image before committing it; the bless
//! reports how far it moved each image, as render.rs does.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;

mod bless;

use bless::assert_matches_reference;
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
    let not_drawn: [fn(&mut Label); 15] = [
        |l| l.angle_mode = Some(AngleMode::Follow),
        |l| l.shadow_color = Some(Bound::Value(WHITE)),
        |l| l.styles = vec![Style::default()],
        |l| {
            l.styles = vec![Style {
                symbol: Some(Bound::Value(SymbolRef::Index(1))),
                ..labelpoly(Bound::Value(WHITE))
            }];
        },
        |l| l.min_scale_denom = Some(1000.0),
        |l| l.max_scale_denom = Some(1000.0),
        |l| {
            l.font = Some(Bound::Binding {
                binding: "font".to_owned(),
            });
        },
        |l| l.font = None,
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
fn at_resolution_144_a_label_moves_away_from_the_marker_as_it_is_drawn() {
    // Verifies: ADR-0028
    // msPrepareImage's scale factor, RESOLUTION 144 over DEFRESOLUTION 72, scales the marker
    // msDrawMarkerSymbol draws and the one msGetMarkerSize gives the label cache alike, and the
    // label's size and offset: a circle of SIZE 9 under a label of SIZE 12 offset (3, 2) at 144
    // is drawn as a circle of 18 under a label of 24 offset (6, 4) at 72.
    let with_marker = |resolution: Option<f64>, marker: f64, size: f64, (x, y): (f64, f64)| {
        let mut map = map(Label {
            text: Some(Expr::String("Fen".to_owned())),
            position: Some(Bound::Value(Position::Ur)),
            offset: Some([Bound::Value(x), Bound::Value(y)]),
            size: Some(Bound::Value(size)),
            ..label_item()
        });
        map.resolution = resolution;
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
            size: Some(Bound::Value(marker)),
            color: Some(Bound::Value(BLUE)),
            ..Style::default()
        }];
        let rendered = render(&map, FEN);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        rendered.image.unwrap()
    };
    let at_144 = with_marker(Some(144.0), 9.0, 12.0, (3.0, 2.0));
    assert!(at_144 == with_marker(None, 18.0, 24.0, (6.0, 4.0)));
    assert!(
        at_144 != with_marker(None, 9.0, 24.0, (6.0, 4.0)),
        "the marker is drawn twice its size"
    );
}

#[test]
fn a_label_stands_by_the_point_pixel_ms_transform_point_computes() {
    // misc/attrbind.map's "Label 1": a circle of 13 at (-1.2827027027027027,
    // 0.7747297297297298) in EXTENT -1.5 0.1 -0.6 1.0 at SIZE 500 500, its label at UR.
    // msTransformPoint (maputil.c) multiplies by 1 / cellsize: the pixel is
    // (120.47927927927928, 124.89984984984984), where dividing by the cellsize gives
    // 124.89984984984983 down. msAddLabelGroup's marker box at that y runs from
    // 118.39984984984984 to 130.39984984984983, 11.999999999999986 tall, so msDrawLabelCache's
    // offset up is 5.999999999999993, cut to 5 by get_metrics' int; across it is 12 / 2, 6.
    let mut map = map(Label {
        text: Some(Expr::String("Fen".to_owned())),
        position: Some(Bound::Value(Position::Ur)),
        ..label_item()
    });
    map.extent = Some(Extent {
        min_x: -1.5,
        min_y: 0.1,
        max_x: -0.6,
        max_y: 1.0,
    });
    map.size = Some([500, 500]);
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
        size: Some(Bound::Value(13.0)),
        color: Some(Bound::Value(BLUE)),
        ..Style::default()
    }];
    let point = r#"{"type": "Feature", "properties": {},
        "geometry": {"type": "Point",
                     "coordinates": [-1.2827027027027027, 0.7747297297297298]}}"#;
    let ops = recorded(&map, point);
    assert_eq!(ops.len(), 1);

    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
    let data = std::fs::read(file).unwrap();
    let font = Font::new(&data).unwrap();
    let ids = font.glyphs(&['F', 'e', 'n'], true).unwrap().unwrap();
    let mut glyphs = Glyphs::new(12.0);
    let path = TextPath::new(&font, &mut glyphs, &ids).unwrap();
    let at = Point {
        x: 120.479_279_279_279_28,
        y: 124.899_849_849_849_84,
    };
    let (origin, _) = metrics(at, Anchor::Ur, &path.text_box(), 6, 5, 0.0, 0);
    let visible = [0.0, 0.0, 500.0, 500.0];
    let drawn = path
        .draw(std::slice::from_ref(&glyphs), origin, 0.0, visible)
        .path;
    assert_eq!(glyph_path(&ops[0]), drawn);
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

/// The priority each label is cached at under `priority`, for a feature with each of `values`
/// as its property `n`, by the feature's place.
fn priorities(priority: Bound<f64>, values: &[&str]) -> Vec<(String, u8)> {
    let features: Vec<String> = values
        .iter()
        .enumerate()
        .map(|(i, n)| {
            format!(
                r#"{{"type": "Feature", "properties": {{"name": "f{i}", "n": "{n}"}},
                    "geometry": {{"type": "Point", "coordinates": [{}, 30]}}}}"#,
                20 + 40 * i
            )
        })
        .collect();
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    );
    let mut map = map(Label {
        priority: Some(priority),
        size: Some(Bound::Value(6.0)),
        ..label_item()
    });
    map.size = Some([40 * values.len() as u32, 60]);
    map.extent.as_mut().unwrap().max_x = f64::from(40 * values.len() as u32 - 1);
    let rendered = render(&map, &geojson);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let mut cached: Vec<(String, u8)> = rendered
        .labels
        .iter()
        .map(|l| (l.text.clone(), l.priority))
        .collect();
    cached.sort();
    cached
}

/// `(f0, p0), (f1, p1), …` of `priorities`.
fn named(priorities: &[u8]) -> Vec<(String, u8)> {
    priorities
        .iter()
        .enumerate()
        .map(|(i, p)| (format!("f{i}"), *p))
        .collect()
}

#[test]
fn a_priority_computed_by_an_expression_is_truncated_and_held_from_1_to_10() {
    // Verifies: G-10
    let n = || Expr::call(Op::ToNumber, vec![Expr::property("n")]);
    // The frontend's form of a numeric expression, as msEvalDoubleExpression reads it: the value
    // printed with %g and read back with atof.
    let numeric = |expr: Expr| {
        Bound::Expr(Expr::call(
            Op::ToNumber,
            vec![Expr::call(
                Op::Tostring,
                vec![expr, Expr::String("%g".to_owned())],
            )],
        ))
    };
    let plus_one = || numeric(Expr::call(Op::Add, vec![n(), Expr::Number(1.0)]));
    // bindLabel assigns the double to the int priority, toward zero, and msAddLabelGroup holds it
    // from 1 to MS_MAX_LABEL_PRIORITY, 10: 4 + 1 is 5, -3 + 1 is 1, 29 + 1 is 10, 7.9 + 1 is 8,
    // -0.5 + 1 is 0 and so 1.
    assert_eq!(
        priorities(plus_one(), &["4", "-3", "29", "7.9", "-0.5"]),
        named(&[5, 1, 10, 8, 1])
    );
    // 8.9999999 + 1 prints with %g as 10, not 9.9999999. A value past an int is INT_MIN on
    // x86-64, held at 1, as is no number.
    assert_eq!(
        priorities(plus_one(), &["8.9999999", "1e10", "nan"]),
        named(&[10, 1, 1])
    );
    // An expression that fails, a division by zero, gives 0, held at 1.
    let by_zero = numeric(Expr::call(Op::Div, vec![n(), Expr::Number(0.0)]));
    assert_eq!(priorities(by_zero, &["4"]), named(&[1]));
    // A text expression is read with atof once, without %g: 7.9999999 is 7, `x` is 0 and so 1.
    let text = Bound::Expr(Expr::call(
        Op::ToNumber,
        vec![Expr::call(Op::ToString, vec![Expr::property("n")])],
    ));
    assert_eq!(
        priorities(text, &["6.7", "7.9999999", "x"]),
        named(&[6, 7, 1])
    );
}

#[test]
fn a_literal_priority_is_held_from_1_to_10_as_a_condition_given_0_is() {
    // Verifies: G-10
    // The frontend gives a condition, which MapServer evaluates to `true` or `false` and atof
    // reads as 0, or an expression it cannot read, the PRIORITY 0; msAddLabelGroup holds it at 1.
    for (value, priority) in [(0.0, 1), (4.0, 4), (30.0, 10), (-2.0, 1)] {
        assert_eq!(
            priorities(Bound::Value(value), &["1"]),
            named(&[priority]),
            "{value}"
        );
    }
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

/// Every operation the render draws, in its order.
#[derive(Clone, Default)]
struct OpRecorder(std::rc::Rc<std::cell::RefCell<Vec<forager_render::DrawOp>>>);

impl forager_render::Renderer for OpRecorder {
    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}

    fn draw(&mut self, item: &forager_render::DisplayItem) {
        self.0.borrow_mut().push(item.op.clone());
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

/// A label's style with `GEOMTRANSFORM labelpoly` in `color`.
fn labelpoly(color: Bound<Rgba>) -> Style {
    Style {
        color: Some(color),
        geom_transform: Some(forager_style_ir::GeomTransform::LabelPoly),
        ..Style::default()
    }
}

const GREEN: Rgba = Rgba::opaque(0, 255, 0);

/// The label "Fen" with `styles`.
fn fen_styled(styles: Vec<Style>) -> Label {
    Label {
        text: Some(Expr::String("Fen".to_owned())),
        styles,
        ..label_item()
    }
}

/// The render of `label` over [`FEN`], with no diagnostic, and every operation it draws.
fn draw_fen(label: Label) -> (Rendered, Vec<forager_render::DrawOp>) {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(FEN).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(
        &map(label),
        &sources,
        &Files,
        recorder.clone(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    (rendered, recorder.0.take())
}

#[test]
fn a_labelpoly_style_fills_the_text_box_grown_by_a_pixel_under_the_text() {
    // "Fen" at 12 points stands at CC of the pixel (80, 29) in the box [65.40625, 23.046875,
    // 94.59375, 34.953125]; msDrawLabelCache's labelpoly is get_metrics' box at a buffer of 1,
    // which msDrawLabelBounds fills as the ring from its lower left corner, under the text.
    let (rendered, ops) = draw_fen(fen_styled(vec![labelpoly(Bound::Value(GREEN))]));
    let p = |x, y| Point { x, y };
    assert_eq!(
        ops[0],
        forager_render::DrawOp::Fill {
            rings: vec![vec![
                p(64.40625, 22.046875),
                p(64.40625, 35.953125),
                p(95.59375, 35.953125),
                p(95.59375, 22.046875),
                p(64.40625, 22.046875),
            ]],
            color: GREEN,
            gamma: Some(0.75),
        }
    );
    assert_eq!(ops.len(), 2);
    assert!(matches!(ops[1], forager_render::DrawOp::Glyphs { .. }));
    // The box the label takes and reports is its text's alone.
    assert_eq!(
        rendered.labels[0].drawn,
        Some([65.40625, 23.046875, 94.59375, 34.953125])
    );
    // A COLOR bound to the feature's property reads as bindStyle reads it.
    let (_, ops) = draw_fen(fen_styled(vec![labelpoly(binding("outline"))]));
    assert!(matches!(
        ops[0],
        forager_render::DrawOp::Fill { color, .. } if color == Rgba::opaque(255, 255, 0)
    ));
}

#[test]
fn a_turned_labelpoly_is_moved_by_its_offset_and_outlined_at_its_width() {
    let p = |x, y| Point { x, y };
    // Moved by its OFFSET, outlined at its WIDTH, and turned with the text: msDrawShadeSymbol.
    let styled = Label {
        angle: Some(Bound::Value(20.0)),
        ..fen_styled(vec![Style {
            outline_color: Some(Bound::Value(Rgba::opaque(200, 30, 30))),
            width: Some(Bound::Value(2.0)),
            offset: Some([Bound::Value(3.0), Bound::Value(2.0)]),
            ..labelpoly(Bound::Value(GREEN))
        }])
    };
    // get_metrics turns the corners of the box grown by 1 about the point by 20 degrees, from
    // x1 = -w/2 and y1 = h/2 of the text 29.1875 by 11.90625 pixels: lower left, upper left,
    // upper right, lower right. msOffsetPolyline then shifts them by (3, 2) in pixels, unturned,
    // and msDrawShadeSymbol outlines the ring at WIDTH 2 times the scale factor of 1, in the
    // OUTLINECOLOR at the COLOR's alpha, with round caps and joins.
    let (_, ops) = draw_fen(styled.clone());
    let ring = [
        p(70.72477700368103, 42.86717686388667),
        p(65.96855938555845, 29.799576356082625),
        p(95.27522299631897, 19.132823136113334),
        p(100.03144061444155, 32.200423643917375),
        p(70.72477700368103, 42.86717686388667),
    ];
    let near = |got: &[Point]| {
        got.len() == ring.len()
            && got
                .iter()
                .zip(&ring)
                .all(|(a, b)| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9)
    };
    assert_eq!(ops.len(), 3, "{ops:?}");
    match &ops[0] {
        forager_render::DrawOp::Fill {
            rings,
            color,
            gamma,
        } => {
            assert_eq!((rings.len(), *color, *gamma), (1, GREEN, Some(0.75)));
            assert!(near(&rings[0]), "{rings:?}");
        }
        other => panic!("{other:?}"),
    }
    match &ops[1] {
        forager_render::DrawOp::Stroke {
            paths,
            closed,
            color,
            width,
            cap,
            join,
            dashes,
        } => {
            assert_eq!(
                (paths.len(), *closed, *color, *width, *cap, *join),
                (
                    1,
                    false,
                    Rgba::opaque(200, 30, 30),
                    2.0,
                    forager_render::LineCap::Round,
                    forager_render::LineJoin::Round
                )
            );
            assert!(dashes.pattern.is_empty());
            assert!(near(&paths[0]), "{paths:?}");
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(ops[2], forager_render::DrawOp::Glyphs { .. }));
    let rendered = render(&map(styled), FEN);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_matches_reference(&rendered.image.unwrap(), "label_labelpoly.png");
}

#[test]
fn a_labelpoly_takes_no_room_from_other_labels_as_in_mapservers_cache() {
    // msTestLabelCacheCollisions tests a label's text box, grown by its BUFFER, against the text
    // boxes and labelpoint styles of the labels drawn, never a labelpoly: two "Fen", 29.1875
    // pixels wide, whose texts lie a pixel apart are both drawn though their labelpolys meet.
    let fen = |buffer, styles| Label {
        text: Some(Expr::String("Fen".to_owned())),
        buffer,
        styles,
        ..label_item()
    };
    let points = [("a", 50.0, 30.0, ""), ("b", 80.1875, 30.0, "")];
    let styled = fen(None, vec![labelpoly(Bound::Value(BLUE))]);
    assert_eq!(labelled(&map(styled), &points), [1, 0]);
    // A BUFFER of 1 grows the boxes the cache tests until they meet.
    assert_eq!(labelled(&map(fen(Some(1.0), Vec::new())), &points), [1]);
}

/// A label's `MAXLENGTH` reaches its layout: the text `Fen Fen` with `WRAP ' '` is laid out on
/// one line or on two, as msLayoutTextSymbol, src/textlayout.c:638-660, breaks it, and each line
/// past the first adds `ceil(12 * 1.33)` = 16 px to the height of the box the label is drawn in.
#[test]
fn a_labels_maxlength_decides_where_its_text_breaks() {
    let one = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"name": "Fen Fen"},
         "geometry": {"type": "Point", "coordinates": [80, 30]}}
    ]}"#;
    let size = |max_length: Option<f64>, wrap: Option<&str>, text: Option<&str>| {
        let mut label = label_item();
        label.max_length = max_length;
        label.wrap = wrap.map(str::to_owned);
        if let Some(text) = text {
            label.text = Some(Expr::String(text.to_owned()));
        }
        let rendered = render(&map(label), one);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        let [x0, y0, x1, y1] = rendered.labels[0].drawn.expect("the label is drawn");
        (x1 - x0, y1 - y0)
    };
    let (fen_width, fen_height) = size(None, None, Some("Fen"));
    let two_lines = (fen_width, fen_height + 16.0);
    let one_line = size(None, None, None);
    for (max_length, lines) in [
        // `Fen Fen` holds 7 characters, no more than 7 or 50: laid out as written.
        (Some(7.0), one_line),
        (Some(50.0), one_line),
        // Past 3, the space is met when the line holds 3 characters: it breaks there for 3 and
        // not for 4.
        (Some(3.0), two_lines),
        (Some(4.0), one_line),
        // MAXLENGTH 0 is none: each wrap character breaks.
        (None, two_lines),
        (Some(0.0), two_lines),
        // Past an `int`, `(int)` gives INT_MIN, which every line already holds.
        (Some(3_000_000_000.0), two_lines),
    ] {
        assert_eq!(
            size(max_length, Some(" "), None),
            lines,
            "MAXLENGTH {max_length:?}"
        );
    }
}

/// A label's style with `GEOMTRANSFORM labelpnt`: symbol 1, a filled square of side 1, at `SIZE`
/// 8, in `RED`.
fn labelpoint() -> Style {
    Style {
        color: Some(Bound::Value(RED)),
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(8.0)),
        geom_transform: Some(forager_style_ir::GeomTransform::LabelPoint),
        ..Style::default()
    }
}

const RED: Rgba = Rgba::opaque(220, 20, 20);

/// [`map`] with symbol 1 a filled vector square of side 1.
fn map_with_square(label: Label) -> Map {
    Map {
        symbols: vec![Symbol {
            kind: SymbolKind::Vector,
            paths: vec![vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.0, 0.0],
            ]],
            extent: Some([1.0, 1.0]),
            filled: true,
            name: None,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }],
        ..map(label)
    }
}

/// The label "Fen" at `LR` with a labelpoint style.
fn fen_at_lr_with_marker() -> Label {
    Label {
        position: Some(Bound::Value(Position::Lr)),
        ..fen_styled(vec![labelpoint()])
    }
}

#[test]
fn a_labelpoint_style_draws_its_marker_at_the_point_under_the_text_moved_off_it() {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(FEN).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(
        &map_with_square(fen_at_lr_with_marker()),
        &sources,
        &Files,
        recorder.clone(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let ops = recorder.0.take();
    // msDrawMarkerSymbol draws the square, msGetMarkerSize 8 by 8, centred on the label's point,
    // the pixel (80, 29), before the text: agg2RenderVectorSymbol's points of the unit square
    // moved by half its extent, scaled by 8, and moved to the point.
    let p = |x, y| Point { x, y };
    assert_eq!(
        ops[0],
        forager_render::DrawOp::Fill {
            rings: vec![vec![
                p(76.0, 25.0),
                p(84.0, 25.0),
                p(84.0, 33.0),
                p(76.0, 33.0),
                p(76.0, 25.0),
            ]],
            color: RED,
            gamma: None,
        }
    );
    assert_eq!(ops.len(), 2, "{ops:?}");
    // computeMarkerBounds' box, [76, 25, 84, 33], moves the text off the point by half its
    // width and height, MS_MAX'd with the class's marker, of which there is none: get_metrics
    // at LR with an offset of (int)4 each way.
    assert_eq!(
        glyph_path(&ops[1]),
        &expected("Fen", 12.0, Anchor::Lr, (4, 4), 0.0)[..]
    );
    let rendered = render(&map_with_square(fen_at_lr_with_marker()), FEN);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_matches_reference(&rendered.image.unwrap(), "label_labelpoint.png");
}

#[test]
fn a_label_with_a_labelpoint_style_reports_the_box_of_its_text_and_its_marker() {
    // msDrawLabelCache's member bbox (mapdraw.c:3976-4030): the text's box, then MS_MIN and
    // MS_MAX with the marker's. "Fen" at 12 points is 29.1875 by 11.90625 pixels; at LR of the
    // pixel (80, 29), 4 and 4 away, get_metrics puts its box at x1 = 4, y1 = h + 4 from the
    // point: [84, 33, 113.1875, 44.90625]. The marker's box is [76, 25, 84, 33].
    let rendered = render(&map_with_square(fen_at_lr_with_marker()), FEN);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        rendered.labels[0].drawn,
        Some([76.0, 25.0, 113.1875, 44.90625])
    );
}

#[test]
fn a_labelpoint_marker_takes_room_from_the_labels_after_it() {
    // msTestLabelCacheCollisions tests a label's text against the text and the labelpoint
    // markers of the labels drawn (maplabel.c:781-868). "b", added last, is drawn first: its
    // marker [78, 25, 86, 33] and its text [86, 33, 115.1875, 44.90625]. "a", 3 pixels higher,
    // stands with its text at [54, 30, 83.1875, 41.90625]: clear of b's text, it meets b's
    // marker, and is left out.
    let fen = Label {
        text: Some(Expr::String("Fen".to_owned())),
        ..fen_at_lr_with_marker()
    };
    let points = [("a", 50.0, 33.0, ""), ("b", 82.0, 30.0, "")];
    assert_eq!(labelled(&map_with_square(fen.clone()), &points), [1]);
    // With b 8 pixels further right, its marker [86, 25, 94, 33] is clear of a's text too.
    let apart = [("a", 50.0, 33.0, ""), ("b", 90.0, 30.0, "")];
    assert_eq!(labelled(&map_with_square(fen), &apart), [1, 0]);
}

#[test]
fn a_label_style_whose_transform_the_frontend_dropped_is_refused_not_drawn_at_the_point() {
    // `GEOMTRANSFORM bbox` or an expression reach the IR as no transform, which msDrawLabelCache
    // fails on (mapdraw.c:4128): the label is left out with its reason, and nothing of it is
    // drawn.
    let dropped = Style {
        geom_transform: None,
        ..labelpoint()
    };
    let rendered = render(&map_with_square(fen_styled(vec![dropped])), FEN);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
    assert!(
        rendered.diagnostics[0].message.contains(
            "a STYLE other than one of GEOMTRANSFORM labelpoly, labelpnt, or labelcenter"
        ),
        "{}",
        rendered.diagnostics[0].message
    );
    assert!(rendered.labels.is_empty());
    assert_eq!(ink(&rendered.image.unwrap(), (0, 160), (0, 60)), None);
}

/// A label's style with `GEOMTRANSFORM labelcenter`: symbol 1, a filled square of side 1, at
/// `SIZE` `size`, in `RED`.
fn labelcenter(size: f64) -> Style {
    Style {
        size: Some(Bound::Value(size)),
        geom_transform: Some(forager_style_ir::GeomTransform::LabelCenter),
        ..labelpoint()
    }
}

/// The render of `label` over [`FEN`] with symbol 1 a square, with no diagnostic, and every
/// operation it draws.
fn draw_fen_with_square(label: Label) -> (Rendered, Vec<forager_render::DrawOp>) {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(FEN).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(
        &map_with_square(label),
        &sources,
        &Files,
        recorder.clone(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    (rendered, recorder.0.take())
}

/// The ring agg2RenderVectorSymbol fills for the unit square at `SIZE` 8 centred on `(x, y)`:
/// its points moved by half its extent, scaled by 8, and moved to the point.
fn square_at(x: f64, y: f64) -> Vec<Point> {
    let p = |x, y| Point { x, y };
    vec![
        p(x - 4.0, y - 4.0),
        p(x + 4.0, y - 4.0),
        p(x + 4.0, y + 4.0),
        p(x - 4.0, y + 4.0),
        p(x - 4.0, y - 4.0),
    ]
}

#[test]
fn a_labelcenter_style_draws_its_marker_at_the_centre_of_the_labelpoly_in_the_styles_order() {
    // "Fen" at 12 points, 29.1875 by 11.90625 pixels, at LR of the pixel (80, 29), nothing moving
    // it off: get_metrics at a buffer of 1 puts the labelpoly at x1 = 0, y1 = h, the text's top
    // at y1 - h = 0 below the point, at 29, and its bottom at y1 = h below it: the box
    // [80 - 1, 29 - 1, 80 + 29.1875 + 1, 29 + 11.90625 + 1] = [79, 28, 110.1875, 41.90625],
    // whose centre is (94.59375, 34.953125) (mapdraw.c:4093-4127). A labelcenter
    // style moves the text off no marker: computeMarkerBounds counts labelpoint styles alone.
    let at_lr = |styles| Label {
        position: Some(Bound::Value(Position::Lr)),
        ..fen_styled(styles)
    };
    let p = |x, y| Point { x, y };
    let labelpoly_fill = forager_render::DrawOp::Fill {
        rings: vec![vec![
            p(79.0, 28.0),
            p(79.0, 41.90625),
            p(110.1875, 41.90625),
            p(110.1875, 28.0),
            p(79.0, 28.0),
        ]],
        color: GREEN,
        gamma: Some(0.75),
    };
    let marker = forager_render::DrawOp::Fill {
        rings: vec![square_at(94.59375, 34.953125)],
        color: RED,
        gamma: None,
    };
    let text = expected("Fen", 12.0, Anchor::Lr, (0, 0), 0.0);
    // In the label's styles' order, before the text.
    let styles = vec![labelpoly(Bound::Value(GREEN)), labelcenter(8.0)];
    let (rendered, ops) = draw_fen_with_square(at_lr(styles));
    assert_eq!(ops.len(), 3, "{ops:?}");
    assert_eq!(ops[0], labelpoly_fill);
    assert_eq!(ops[1], marker);
    assert_eq!(glyph_path(&ops[2]), &text[..]);
    // The box the label reports is its text's alone: [80, 29, 109.1875, 40.90625].
    assert_eq!(
        rendered.labels[0].drawn,
        Some([80.0, 29.0, 109.1875, 40.90625])
    );
    let styles = vec![labelcenter(8.0), labelpoly(Bound::Value(GREEN))];
    let (_, ops) = draw_fen_with_square(at_lr(styles));
    assert_eq!(ops.len(), 3, "{ops:?}");
    assert_eq!(ops[0], marker);
    assert_eq!(ops[1], labelpoly_fill);
    assert_eq!(glyph_path(&ops[2]), &text[..]);
}

#[test]
fn a_turned_labels_labelcenter_marker_stands_at_the_centre_of_its_turned_polygons_box() {
    // Turned by 20 degrees at LR of the pixel (80, 29), get_metrics turns the corners of the box
    // grown by 1, from x1 = 0 and y1 = h, about the point: lower left (83.474504854011,
    // 41.4699280303438), upper left (78.71828723588843, 28.40232752253976), upper right
    // (108.02495084664895, 17.735574302570466), lower right (112.78116846477153,
    // 30.803174810374507). fastComputeBounds gives their box [78.71828723588843,
    // 17.735574302570466, 112.78116846477153, 41.4699280303438], whose centre is
    // (95.74972785032998, 29.602751166457132); the marker is drawn there, unturned.
    let turned = Label {
        position: Some(Bound::Value(Position::Lr)),
        angle: Some(Bound::Value(20.0)),
        ..fen_styled(vec![labelcenter(8.0)])
    };
    let (_, ops) = draw_fen_with_square(turned.clone());
    assert_eq!(ops.len(), 2, "{ops:?}");
    let want = square_at(95.749_727_850_329_98, 29.602_751_166_457_132);
    match &ops[0] {
        forager_render::DrawOp::Fill {
            rings,
            color,
            gamma,
        } => {
            assert_eq!((rings.len(), *color, *gamma), (1, RED, None));
            assert_eq!(rings[0].len(), want.len());
            for (got, want) in rings[0].iter().zip(&want) {
                assert!(
                    (got.x - want.x).abs() < 1e-9 && (got.y - want.y).abs() < 1e-9,
                    "{rings:?}"
                );
            }
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        glyph_path(&ops[1]),
        &expected("Fen", 12.0, Anchor::Lr, (0, 0), 20.0)[..]
    );
    let rendered = render(&map_with_square(turned), FEN);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_matches_reference(&rendered.image.unwrap(), "label_labelcenter.png");
}

#[test]
fn a_labelcenter_marker_takes_no_room_and_stays_out_of_the_box_a_label_reports() {
    // Two "Fen" at LR with a labelpoint marker of SIZE 8 and a labelcenter marker of SIZE 40.
    // At the pixel (x, 29), the labelpoint marker [x - 4, 25, x + 4, 33] moves the text 4 and 4
    // off: its box [x + 4, 33, x + 33.1875, 44.90625]. The labelpoly, get_metrics at x1 = 4 and
    // y1 = h + 4 with a buffer of 1, is [x + 3, 32, x + 34.1875, 45.90625], centred on
    // (x + 18.59375, 38.953125): the labelcenter marker spans x - 1.40625 to x + 38.59375 and
    // 18.953125 to 58.953125. "b" at 88, added last, is drawn first; "a" at 50, its labelpoint
    // marker [46, 54] across and its text [54, 83.1875], meets neither b's labelpoint marker
    // [84, 92] nor b's text [92, 121.1875], while their labelcenter markers, [48.59375, 88.59375]
    // and [86.59375, 126.59375], meet: msTestLabelCacheCollisions tests the text and the
    // labelpoint marker alone (maplabel.c:781-868), so both are drawn.
    let fen = Label {
        position: Some(Bound::Value(Position::Lr)),
        ..fen_styled(vec![labelpoint(), labelcenter(40.0)])
    };
    let points = [("a", 50.0, 30.0, ""), ("b", 88.0, 30.0, "")];
    assert_eq!(labelled(&map_with_square(fen.clone()), &points), [1, 0]);
    // Each reports the box of its text and its labelpoint marker (mapdraw.c:3976-4030), in the
    // cache's order: b's [84, 25, 121.1875, 44.90625], a's [46, 25, 83.1875, 44.90625].
    let rendered = render(&map_with_square(fen), &collection(&points));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let drawn: Vec<_> = rendered.labels.iter().map(|l| l.drawn).collect();
    assert_eq!(
        drawn,
        [
            Some([84.0, 25.0, 121.1875, 44.90625]),
            Some([46.0, 25.0, 83.1875, 44.90625])
        ]
    );
}

#[test]
fn a_labelcenter_style_whose_symbol_is_not_drawn_yet_is_refused() {
    // A labelcenter marker is msDrawMarkerSymbol's, as a labelpoint marker is: a SYMBOL that is
    // no vector or ellipse of the set, here one past the set, leaves the label out.
    let style = Style {
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        ..labelcenter(8.0)
    };
    let rendered = render(&map_with_square(fen_styled(vec![style])), FEN);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
    assert!(
        rendered.diagnostics[0]
            .message
            .contains("a STYLE of GEOMTRANSFORM labelcenter whose SYMBOL"),
        "{}",
        rendered.diagnostics[0].message
    );
}

/// Features of the name `Forager` at each of `points`, of the colour `colors`, as GeoJSON.
fn foragers(points: &[([f64; 2], &str)]) -> String {
    let features: Vec<String> = points
        .iter()
        .map(|([x, y], c)| {
            format!(
                r#"{{"type": "Feature", "properties": {{"name": "Forager", "c": "{c}"}},
                    "geometry": {{"type": "Point", "coordinates": [{x}, {y}]}}}}"#
            )
        })
        .collect();
    format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    )
}

/// `map` of `label_item` in the colour of the property `c`, its layer of `LABELCACHE OFF`.
fn cache_off() -> Map {
    let mut map = map(Label {
        color: Some(Bound::Binding {
            binding: "c".to_owned(),
        }),
        ..label_item()
    });
    map.layers[0].label_cache = false;
    map
}

/// How many pixels of `image` are of `rgba` exactly.
fn count(image: &RgbaImage, rgba: [u8; 4]) -> usize {
    image.pixels.chunks(4).filter(|p| *p == rgba).count()
}

#[test]
fn a_layer_of_labelcache_off_draws_every_label_in_the_order_of_its_features() {
    // pointLayerDrawShape draws each label with msDrawLabel as it draws its feature, untested:
    // two labels at one point are both drawn, the later over the earlier, and neither is cached.
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    for (first, second, seen, hidden) in [
        ("255 0 0", "0 0 255", blue, red),
        ("0 0 255", "255 0 0", red, blue),
    ] {
        let rendered = render(
            &cache_off(),
            &foragers(&[([80.0, 30.0], first), ([80.0, 30.0], second)]),
        );
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        assert_eq!(rendered.labels, []);
        let image = rendered.image.unwrap();
        assert!(count(&image, seen) > 0, "the later label is drawn");
        assert_eq!(count(&image, hidden), 0, "the earlier label lies under it");
    }
    // Moved apart by 3 pixels, each shows: the label cache would discard one of them.
    let rendered = render(
        &cache_off(),
        &foragers(&[([80.0, 30.0], "255 0 0"), ([83.0, 30.0], "0 0 255")]),
    );
    let image = rendered.image.unwrap();
    assert!(count(&image, red) > 0 && count(&image, blue) > 0);
}

#[test]
fn a_label_of_labelcache_off_is_drawn_past_the_images_edge() {
    // No test against the image's edge: "Forager" centred 10 pixels from the right edge is drawn
    // up to the edge, where the label cache, without PARTIALS, discards it.
    let rendered = render(&cache_off(), &foragers(&[([150.0, 30.0], "0 0 255")]));
    assert_eq!(rendered.labels, []);
    let image = rendered.image.unwrap();
    let [_, _, max_x, _] = ink(&image, (100, 160), (0, 60)).expect("the label is drawn");
    assert_eq!(max_x, 159);
    let mut cached = cache_off();
    cached.layers[0].label_cache = true;
    let rendered = render(&cached, &foragers(&[([150.0, 30.0], "0 0 255")]));
    assert_eq!(rendered.labels.len(), 1);
    assert_eq!(rendered.labels[0].drawn, None);
    assert_eq!(ink(&rendered.image.unwrap(), (0, 160), (0, 60)), None);
}

/// A polygon layer over the whole image, filled in green, as inline features.
fn green_cover() -> Layer {
    Layer {
        name: Some("cover".into()),
        render_mode: Some(RenderMode::Polygon),
        status: Some(LayerStatus::On),
        classes: vec![Class {
            styles: vec![Style {
                color: Some(Bound::Value(Rgba::opaque(0, 200, 0))),
                ..Style::default()
            }],
            ..Class::default()
        }],
        features: vec![InlineFeature {
            shape_type: ShapeType::Polygon,
            parts: vec![vec![
                [-1.0, -1.0],
                [-1.0, 61.0],
                [161.0, 61.0],
                [161.0, -1.0],
                [-1.0, -1.0],
            ]],
            properties: Vec::new(),
            text: None,
            span: None,
        }],
        ..Layer::default()
    }
}

#[test]
fn a_later_layer_covers_the_labels_of_a_layer_of_labelcache_off() {
    // The labels are drawn with their layer, so a polygon layer after it covers them; those of
    // the cache are drawn after every layer, over it.
    let green = [0, 200, 0, 255];
    let mut map = cache_off();
    map.layers.push(green_cover());
    let points = foragers(&[([80.0, 30.0], "0 0 255")]);
    let rendered = render(&map, &points);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(count(&rendered.image.unwrap(), green), 160 * 60);
    map.layers[0].label_cache = true;
    let image = render(&map, &points).image.unwrap();
    assert!(count(&image, [0, 0, 255, 255]) > 0);
}

#[test]
fn the_label_cache_does_not_test_its_labels_against_those_drawn_at_once() {
    // msTestLabelCacheCollisions knows only the cache: a cached label at the point of a label of
    // LABELCACHE OFF is drawn.
    let mut map = cache_off();
    let mut cached = map.layers[0].clone();
    cached.label_cache = true;
    map.layers.push(cached);
    let sources: BTreeMap<String, GeoJsonSource> = [(
        "places".to_owned(),
        GeoJsonSource::parse(&foragers(&[([80.0, 30.0], "0 0 255")])).unwrap(),
    )]
    .into();
    let rendered = ready(render_with_files(
        &map,
        &sources,
        &Files,
        AggRenderer::new(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.labels.len(), 1);
    assert_eq!((rendered.labels[0].layer, rendered.labels[0].class), (1, 0));
    assert!(rendered.labels[0].drawn.is_some());
}

#[test]
fn a_label_of_labelcache_off_stands_by_its_position_and_offset_alone() {
    // msDrawLabel calls get_metrics with the label's OFFSET and no marker's: a class style's
    // marker does not move the label. POSITION AUTO is a case get_metrics' switch does not name,
    // the text's box from the point up and to the right, its offset unread.
    let mut boxes = Vec::new();
    for (position, offset) in [
        (Position::Auto, [10.0, 10.0]),
        (Position::Ur, [0.0, 0.0]),
        (Position::Ur, [10.0, 0.0]),
    ] {
        let mut map = cache_off();
        let label = &mut map.layers[0].classes[0].labels[0];
        label.position = Some(Bound::Value(position));
        label.offset = Some([Bound::Value(offset[0]), Bound::Value(offset[1])]);
        map.layers[0].classes[0].styles = vec![Style {
            symbol: Some(Bound::Value(SymbolRef::Index(0))),
            size: Some(Bound::Value(30.0)),
            ..Style::default()
        }];
        let rendered = render(&map, &foragers(&[([40.0, 30.0], "0 0 255")]));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        boxes.push(ink(&rendered.image.unwrap(), (0, 160), (0, 60)));
    }
    // The point is pixel (40, 29); the text's ink starts a pixel right of it, past the F's
    // bearing, and ends on the row above it, where the bottom of the text's box, its descent
    // included, meets the point. A marker of 30 pixels
    // would move a cached label 15 pixels off.
    assert_eq!(
        boxes,
        [
            Some([41, 14, 101, 28]),
            Some([41, 14, 101, 28]),
            Some([51, 14, 111, 28])
        ]
    );
}

#[test]
fn a_label_of_labelcache_off_draws_its_styles_before_its_text() {
    // msDrawLabel draws a labelpnt style's marker at the point, a labelpoly style on get_metrics'
    // box grown by 1 pixel, and a labelcenter style's marker at that box's centre, in their
    // order, then the text.
    let mut map = map_with_square(Label {
        styles: vec![
            labelpoly(Bound::Value(GREEN)),
            labelpoint(),
            Style {
                color: Some(Bound::Value(BLUE)),
                ..labelcenter(4.0)
            },
        ],
        color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        position: Some(Bound::Value(Position::Ur)),
        ..label_item()
    });
    map.layers[0].label_cache = false;
    let rendered = render(&map, &foragers(&[([40.0, 30.0], "")]));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    let boxed = |rgba: [u8; 4]| {
        let mut b: Option<[u32; 4]> = None;
        for y in 0..60 {
            for x in 0..160 {
                if image.pixel(x, y).map(|p| [p.r, p.g, p.b, p.a]) == Some(rgba) {
                    let e = b.get_or_insert([x, y, x, y]);
                    *e = [e[0].min(x), e[1].min(y), e[2].max(x), e[3].max(y)];
                }
            }
        }
        b
    };
    // The labelpoly the text's box of a label at UR of pixel (40, 29), the box of
    // [`a_label_of_labelcache_off_stands_by_its_position_and_offset_alone`] grown; the square of
    // 8 pixels about the point, unmoved and over the labelpoly; the blue marker at the centre.
    assert_eq!(boxed([0, 255, 0, 255]), Some([39, 13, 102, 29]));
    assert_eq!(boxed([220, 20, 20, 255]), Some([36, 25, 43, 32]));
    assert_eq!(boxed([30, 60, 200, 255]), Some([69, 22, 71, 22]));
}
