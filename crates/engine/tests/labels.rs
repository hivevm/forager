// The tests of the marker a pixmap holds under a label work their values out from MapServer's
// msAddLabelGroup, src/maplabel.c, and msGetMarkerSize, src/mapsymbol.c, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The labels of point layers, drawn through the reference renderer and compared with a reference
//! image of Forager's own, as the geometry of render.rs is (ADR-0031). Set FORAGER_BLESS=1 to write
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
    AngleMode, Bound, Class, Expr, Extent, FeatureProperty, InlineFeature, Label, LabelTransform,
    Layer, LayerStatus, Map, Op, Position, RenderMode, Rgba, ShapeType, SourceRef, Span, Style,
    Symbol, SymbolKind, SymbolRef, Units,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The font set's files, test fonts of the label crate whose licence lies beside them:
/// `fonts/sans.ttf` a subset of DejaVu Sans, and `fonts/<name>.ttf` its variant
/// `dejavu-sans-<name>.ttf`; and `pin.png`, the test icon of the command line, 32 × 32.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str() == "pin.png" {
            return Ok(include_bytes!("../../cli/tests/fixtures/pin.png").to_vec());
        }
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
    // Verifies: ADR-0031
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
fn a_label_finds_its_features_text_by_the_features_place_among_all_the_layers_features() {
    // The labels read an inline feature's TEXT beside the port, at the place its id names among
    // the layer's features (ARCHITECTURE.md, inline.rs); this holds only while the inline source
    // numbers every feature by that place, those out of view included (msINLINELayerNextShape).
    let sources = BTreeMap::<String, GeoJsonSource>::new();
    let one = r#"{"type": "Feature", "properties": {"name": "Fen"},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let reference = render(&map(label_item()), one).image.unwrap();
    let without_text = || {
        let mut map = map(Label {
            text: None,
            ..label_item()
        });
        map.layers[0].source = None;
        map
    };
    let render = |features: Vec<InlineFeature>| {
        let mut map = without_text();
        map.layers[0].features = features;
        ready(render_with_files(
            &map,
            &sources,
            &Files,
            AggRenderer::new(),
        ))
    };
    // Two features out of view with a TEXT, then the one in view with its own: its text is drawn.
    let rendered = render(vec![
        inline_point([500.0, 500.0], "a", Some("Other")),
        inline_point([600.0, 600.0], "a", Some("Others")),
        inline_point([80.0, 30.0], "a", Some("Fen")),
    ]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.image.unwrap(), reference);
    // A feature out of view with a TEXT, then the one in view without: nothing is drawn, the
    // text of the first lending its class no label.
    let rendered = render(vec![
        inline_point([500.0, 500.0], "a", Some("Other")),
        inline_point([80.0, 30.0], "a", None),
    ]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        ink(rendered.image.as_ref().unwrap(), (0, 160), (0, 60)),
        None
    );
}

#[test]
fn a_label_that_cannot_be_drawn_yet_is_left_out_with_its_reason() {
    // Verifies: ADR-0031
    let with = |change: fn(&mut Label)| {
        let mut label = label_item();
        change(&mut label);
        label
    };
    let not_drawn: [fn(&mut Label); 14] = [
        |l| l.angle_mode = Some(AngleMode::Follow),
        |l| l.shadow_color = Some(WHITE),
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

/// The messages of the diagnostics of `rendered`, each with its code.
fn messages(rendered: &Rendered) -> Vec<(&str, &str)> {
    rendered
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect()
}

/// A feature of one point whose `name` is the JSON string `name`, escaped as JSON writes it.
fn named_at_40_30(name: &str) -> String {
    format!(
        r#"{{"type": "Feature", "properties": {{"name": "{name}"}},
            "geometry": {{"type": "Point", "coordinates": [40, 30]}}}}"#
    )
}

#[test]
fn a_label_refused_for_its_text_quotes_at_most_64_characters_of_it() {
    // Verifies: ADR-0031
    // A text of 1 MiB, 1,048,574 characters, whose second line ends in U+2019, read by the label
    // of each of 100 layers of one source: each label is reported once, and its message names the
    // line and the character and quotes the text's first 64 characters, its length after them,
    // rather than the whole text for every label.
    const LAYERS: usize = 100;
    let geojson = named_at_40_30(&format!("Forager\\n{}\u{2019}", "x".repeat((1 << 20) - 11)));
    let mut layers = map(label_item());
    layers.layers = vec![layers.layers[0].clone(); LAYERS];
    let rendered = render(&layers, &geojson);
    // Each message's size first, so that a failure does not print the texts it quotes.
    for (code, message) in messages(&rendered) {
        assert!(message.len() < 200, "{code}: {} bytes", message.len());
    }
    let quote = format!("Forager\n{}", "x".repeat(56));
    let expected = format!(
        "line 2 of the text `{quote}\u{2026}` (1048574 characters) holds U+2019, which needs a \
         shaper; the label is not drawn"
    );
    assert_eq!(
        messages(&rendered),
        vec![("label-line-unsupported", expected.as_str()); LAYERS]
    );
    // A text of 64 characters or fewer is quoted whole.
    let short = map(Label {
        text: Some(Expr::String("two\nlin\u{2019}s".to_owned())),
        ..label_item()
    });
    assert_eq!(
        messages(&render(&short, PLACES)),
        [(
            "label-line-unsupported",
            "line 2 of the text `two\nlin\u{2019}s` holds U+2019, which needs a shaper; the \
             label is not drawn"
        )]
    );
    // A glyph its font cannot draw quotes the text the same way: `1`, glyph 3 of the font
    // `broken`, which cannot read it.
    let mut broken = map(label_item());
    broken.layers[0].classes[0].labels[0].font = Some(Bound::Value("broken".to_owned()));
    let rendered = render(&broken, &named_at_40_30(&"1".repeat(1 << 20)));
    for (code, message) in messages(&rendered) {
        assert!(message.len() < 200, "{code}: {} bytes", message.len());
    }
    let expected = format!(
        "its font cannot draw glyph 3 of `{}\u{2026}` (1048576 characters); the label is not \
         drawn for a text that holds it",
        "1".repeat(64)
    );
    assert_eq!(
        messages(&rendered),
        [("label-glyph-unreadable", expected.as_str())]
    );
}

#[test]
fn a_labelpoly_of_a_symbol_that_is_no_hatch_is_left_out() {
    // A labelpoly whose symbol is in the set but is no hatch: msDrawShadeSymbol tiles it
    // (maprendering.c:823-), which is not drawn yet, so it is refused rather than filled plain.
    let mut tiled = map(label_item());
    tiled.layers[0].classes[0].labels[0].styles = vec![Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        ..labelpoly(Bound::Value(WHITE))
    }];
    tiled.symbols = vec![Symbol {
        name: Some("square".to_owned()),
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
        anchor_point: None,
        character: None,
        font: None,
        image: None,
        transparent: None,
        span: None,
    }];
    let rendered = render(&tiled, PLACES);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
    assert!(
        rendered
            .image
            .unwrap()
            .pixels
            .chunks(4)
            .all(|p| p == [255, 255, 255, 255])
    );
}

#[test]
fn a_class_with_several_labels_is_not_drawn_yet() {
    let mut several = map(label_item());
    several.layers[0].classes[0].labels.push(label_item());
    let rendered = render(&several, PLACES);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["label-not-drawn"; 2]);
}

#[test]
fn a_labels_size_is_scaled_by_its_layers_scale_factor() {
    // msPrepareImage: a label inherits its layer's scale factor (msUpdateClassScaleFactor), and
    // msPopulateTextSymbolForLabelAndString scales its SIZE by it, bounded by MINSIZE and MAXSIZE
    // times the resolution factor. The map is at 1 : 72 × 39.3701 (159 meters over 159 pixels).
    let scale = 72.0 * 39.3701;
    let drawn = |map: &Map| {
        let rendered = render(map, PLACES);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        rendered.image
    };
    // A SYMBOLSCALEDENOM of twice the map's scale: a factor of 2, so SIZE 12 draws as 24.
    let mut symbol_scaled = map(label_item());
    symbol_scaled.layers[0].symbol_scale_denom = Some(2.0 * scale);
    let twice = map(Label {
        size: Some(Bound::Value(24.0)),
        ..label_item()
    });
    assert_eq!(drawn(&symbol_scaled), drawn(&twice));
    // SIZEUNITS meters at a meter a pixel: a factor of 1 at any RESOLUTION, so at 144 dpi the
    // label draws as at 72, where the resolution factor alone would double it.
    let mut sized = map(label_item());
    sized.resolution = Some(144.0);
    sized.layers[0].size_units = Some(Units::Meters);
    assert_eq!(drawn(&sized), drawn(&map(label_item())));
}

#[test]
fn a_layer_of_pixels_and_the_default_priority_draw_as_without_them() {
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

/// One point at (80, 30) whose property `name` is `name`, a NUL in it written `\u0000`.
fn named_point(name: &str) -> String {
    format!(
        r#"{{"type": "Feature", "properties": {{"name": "{}"}},
        "geometry": {{"type": "Point", "coordinates": [80, 30]}}}}"#,
        name.replace('\0', "\\u0000")
    )
}

#[test]
fn a_propertys_text_ends_at_its_first_nul_as_mapservers_c_string_does() {
    // msEvalTextExpressionInternal copies each attribute value with msStrdup
    // and puts it in place of its binding with msReplaceSubstring, which reads strlen of it: the
    // value ends at its first NUL, and the text around the binding goes on after it.
    let cut = render(&map(label_item()), &named_point("ab\0cd"));
    assert!(cut.diagnostics.is_empty(), "{:?}", cut.diagnostics);
    let whole = render(&map(label_item()), &named_point("ab"));
    assert!(cut.image == whole.image, "`ab\\0cd` draws other than `ab`");
    let texts: Vec<_> = cut.labels.iter().map(|l| &*l.text).collect();
    assert_eq!(texts, ["ab"]);
    // A value that starts with a NUL is empty, and msEvalTextExpressionInternal gives no text.
    let empty = render(&map(label_item()), &named_point("\0abc"));
    assert!(empty.diagnostics.is_empty(), "{:?}", empty.diagnostics);
    assert_eq!(ink(empty.image.as_ref().unwrap(), (0, 160), (0, 60)), None);
    assert!(empty.labels.is_empty(), "{:?}", empty.labels);
    // A binding inside a text: its value ends at the NUL, the text after the binding stays.
    let around = |name: &str| {
        let label = Label {
            text: Some(Expr::call(
                Op::Concat,
                vec![
                    Expr::call(Op::ToString, vec![Expr::property("name")]),
                    Expr::String("x".to_owned()),
                ],
            )),
            ..label_item()
        };
        render(&map(label), &named_point(name))
    };
    let cut = around("ab\0cd");
    assert!(cut.diagnostics.is_empty(), "{:?}", cut.diagnostics);
    let abx = render(&map(label_item()), &named_point("abx"));
    assert!(cut.image == abx.image, "`[name]x` draws other than `abx`");
    let texts: Vec<_> = cut.labels.iter().map(|l| &*l.text).collect();
    assert_eq!(texts, ["abx"]);
}

#[test]
fn a_labels_text_reads_a_feature_as_its_bound_properties_do() {
    // Verifies: Q-9
    // The text takes bindLabel's reading of a feature, as `bind` does: a property matches
    // whatever its case, as MapServer finds an attribute in a `.dbf` or an OGR source, and a
    // boolean prints as OGR gives it, 1 or 0.
    let of = |expr: Expr, geojson: &str| {
        let rendered = render(
            &map(Label {
                text: Some(expr),
                ..label_item()
            }),
            geojson,
        );
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        let texts: Vec<String> = rendered.labels.iter().map(|l| l.text.to_string()).collect();
        (texts, rendered.image)
    };
    let property = |name: &str| Expr::call(Op::ToString, vec![Expr::property(name)]);
    let forager = named_point("Forager");
    let (upper, upper_image) = of(property("NAME"), &forager);
    let (lower, lower_image) = of(property("name"), &forager);
    assert_eq!((&upper, &lower), (&vec!["Forager".to_owned()], &upper));
    assert!(
        upper_image == lower_image,
        "`[NAME]` draws other than `[name]`"
    );
    for (value, expected) in [("true", "1"), ("false", "0")] {
        let capital = format!(
            r#"{{"type": "Feature", "properties": {{"capital": {value}}},
            "geometry": {{"type": "Point", "coordinates": [80, 30]}}}}"#
        );
        let (texts, _) = of(property("Capital"), &capital);
        assert_eq!(texts, [expected], "{value}");
    }
    // A text that would pass Q-9's 16 MiB leaves the label out, with the diagnostic of the bound:
    // four times 4 MiB and a byte is 16 MiB and four bytes.
    let long = "x".repeat(4 * 1024 * 1024 + 1);
    let four = Expr::call(Op::Concat, vec![property("name"); 4]);
    let rendered = render(
        &map(Label {
            text: Some(four),
            ..label_item()
        }),
        &named_point(&long),
    );
    let codes: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["text-too-long"]);
    assert!(rendered.labels.is_empty());
    assert_eq!(
        ink(rendered.image.as_ref().unwrap(), (0, 160), (0, 60)),
        None
    );
}

#[test]
fn an_inline_features_text_ends_at_its_first_nul() {
    // msShapeGetLabelAnnotation takes the feature's TEXT with msStrdup, which
    // ends at its first NUL.
    let no_text = || {
        map(Label {
            text: None,
            ..label_item()
        })
    };
    let cut = render_inline(no_text(), Some("ab\0cd"), "Other");
    assert!(cut.diagnostics.is_empty(), "{:?}", cut.diagnostics);
    let whole = render_inline(no_text(), Some("ab"), "Other");
    assert!(cut.image == whole.image, "`ab\\0cd` draws other than `ab`");
    let texts: Vec<_> = cut.labels.iter().map(|l| &*l.text).collect();
    assert_eq!(texts, ["ab"]);
    let empty = render_inline(no_text(), Some("\0abc"), "Other");
    assert!(empty.diagnostics.is_empty(), "{:?}", empty.diagnostics);
    assert_eq!(ink(empty.image.as_ref().unwrap(), (0, 160), (0, 60)), None);
    assert!(empty.labels.is_empty(), "{:?}", empty.labels);
}

#[test]
fn a_glyph_reaching_too_far_from_the_image_is_left_out_with_a_diagnostic() {
    // Verifies: ADR-0031
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
fn a_glyph_at_the_reach_with_an_outline_is_drawn_or_left_out_with_a_diagnostic() {
    // Verifies: ADR-0031
    // The `1` of the huge test font, a square about 8,000,000 pixels wide at 3000 points, outlined
    // 3 wide, moved right step by step across the reach: its right edge passes `MAX_GLYPH_REACH`
    // beyond the image, where the outline once let the text path keep a glyph the renderer then
    // dropped. The square covers the image while it is drawn; past the reach it is left out with a
    // diagnostic, never dropped without one.
    let one = r#"{"type": "Feature", "properties": {},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let (mut drawn, mut left_out) = (0, 0);
    for dx in 194_370..194_410 {
        let label = Label {
            font: Some(Bound::Value("huge".to_owned())),
            text: Some(Expr::String("1".to_owned())),
            size: Some(Bound::Value(3000.0)),
            max_size: Some(3000.0),
            offset: Some([Bound::Value(f64::from(dx)), Bound::Value(0.0)]),
            outline_color: Some(Bound::Value(Rgba::opaque(240, 200, 0))),
            outline_width: Some(3.0),
            // Partial, so that the label cache keeps it though it leaves the image.
            partials: Some(true),
            ..label_item()
        };
        let rendered = render(&map(label), one);
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        let image = rendered.image.unwrap();
        let ink = image.pixels.chunks(4).any(|p| p != [255, 255, 255, 255]);
        match (ink, codes.as_slice()) {
            (true, []) => drawn += 1,
            (false, ["label-glyph-out-of-reach"]) => left_out += 1,
            other => panic!("at an offset of {dx}: {other:?}"),
        }
    }
    assert!(
        drawn > 0 && left_out > 0,
        "{drawn} drawn, {left_out} left out"
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

impl forager_render::Draw for LabelRecorder {
    fn draw(&mut self, item: &forager_render::DisplayItem) {
        if let forager_render::DrawOp::Glyphs { color, .. } = &item.op {
            self.0
                .borrow_mut()
                .push((item.feature_id.unwrap(), color.unwrap()));
        }
    }
}

/// A layer's items are drawn as if onto the map.
impl forager_render::Renderer for LabelRecorder {
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

#[test]
fn each_feature_is_labelled_by_the_class_that_draws_it() {
    // Verifies: ADR-0016
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

impl forager_render::Draw for GlyphRecorder {
    fn draw(&mut self, item: &forager_render::DisplayItem) {
        if matches!(item.op, forager_render::DrawOp::Glyphs { .. }) {
            self.0.borrow_mut().push(item.op.clone());
        }
    }
}

/// A layer's items are drawn as if onto the map.
impl forager_render::Renderer for GlyphRecorder {
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
    "align": "right", "bad": "zz", "op": "3e9"},
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
    // Verifies: ADR-0031
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
    // Verifies: ADR-0031
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
    // Verifies: ADR-0031
    // msPrepareImage's scale factor, RESOLUTION 144 over DEFRESOLUTION 72, scales the marker
    // msDrawMarkerSymbol draws and the one msGetMarkerSize gives the label cache alike, and the
    // label's size and offset: a circle of SIZE 9 under a label of SIZE 6 offset (3, 2) at 144
    // is drawn as a circle of 18 under a label of 12 offset (6, 4) at 72. The label stays inside
    // the image, which a label without PARTIALS must (msTestLabelCacheCollisions, maplabel.c:790).
    let marked = |resolution: Option<f64>, marker: f64, size: f64, (x, y): (f64, f64)| {
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
        map
    };
    let with_marker = |resolution: Option<f64>, marker: f64, size: f64, offset: (f64, f64)| {
        let rendered = render(&marked(resolution, marker, size, offset), FEN);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        rendered.image.unwrap()
    };
    let at_144 = with_marker(Some(144.0), 9.0, 6.0, (3.0, 2.0));
    assert!(at_144 == with_marker(None, 18.0, 12.0, (6.0, 4.0)));
    assert!(
        at_144 != with_marker(None, 9.0, 12.0, (6.0, 4.0)),
        "the marker is drawn twice its size"
    );
    // At 144, msGetMarkerSize's size is the int 9 × 2 = 18, the circle 18 by 18
    // (mapsymbol.c:753, 785-786); msAddLabelGroup's marker box runs from the point less 9 to it
    // plus 8, w - 1 (maplabel.c:367-371), so msDrawLabelCache's marker offset is 17 / 2 = 8.5
    // across and up (mapdraw.c:3572-3575). The label's OFFSET (3, 2) × 2 is (6, 4)
    // (mapdraw.c:3708-3711), and get_metrics takes 8.5 + 6 = 14.5 and 8.5 + 4 = 12.5 as its ints,
    // 14 and 12 (mapdraw.c:3768-3769, maplabel.c:1019): "Fen" at 6 × 2 = 12 at UR of the pixel
    // (80, 29), 14 across and 12 up, its box 29.1875 by 11.90625 from (94, 17) to (123.1875,
    // 5.09375).
    let map = marked(Some(144.0), 9.0, 6.0, (3.0, 2.0));
    let ops = recorded(&map, FEN);
    assert_eq!(ops.len(), 1);
    assert_eq!(
        glyph_path(&ops[0]),
        expected("Fen", 12.0, Anchor::Ur, (14, 12), 0.0)
    );
    assert_eq!(
        render(&map, FEN).labels[0].drawn,
        Some([94.0, 5.09375, 123.1875, 17.0])
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
    // Verifies: ADR-0031
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
    // Verifies: ADR-0031
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
    // Verifies: ADR-0031
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
    // Verifies: ADR-0031
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
    // Verifies: ADR-0031
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
        .map(|l| (l.text.to_string(), l.priority))
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
    // label of a point layer, at CC too; an SVG or TrueType symbol has no size yet, so
    // the label is left out rather than placed without that box.
    for kind in [SymbolKind::Svg, SymbolKind::Truetype] {
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
    // Verifies: Q-9, ADR-0032
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

/// The render reports every label of its cache in the order msDrawLabelCache walks it, the
/// highest priority first, the one drawn with the box it was drawn in, which holds its ink, and
/// the one a label of a higher priority made it discard without. Two labels of one priority that
/// meet are left to the cache's own tests.
#[test]
fn the_render_reports_its_labels_drawn_and_discarded_in_the_caches_order() {
    // Three features named "Fen", each label 29.1875 by 11.90625 pixels at 12 points: one of
    // PRIORITY 7 at the pixel (40, 29); a MultiPoint of PRIORITY 3 at (80, 29) and (120, 29), a
    // label for each point; and one of PRIORITY 3 at (42, 29), over the first. msDrawLabelCache
    // walks the priorities from the highest, and each slot from its last label
    // (mapdraw.c:3500-3504): the label at (40, 29), then the one at (42, 29), which meets it and
    // is discarded, then (120, 29) and (80, 29). At CC, get_metrics puts the box at x1 = -w / 2
    // and y1 = h / 2 of the point (maplabel.c:1049-1051, 1109-1117): from x - 14.59375 to
    // x + 14.59375 across and from 29 - 5.953125 to 29 + 5.953125 down.
    let geojson = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"name": "Fen", "p": "7"},
         "geometry": {"type": "Point", "coordinates": [40, 30]}},
        {"type": "Feature", "properties": {"name": "Fen", "p": "3"},
         "geometry": {"type": "MultiPoint", "coordinates": [[80, 30], [120, 30]]}},
        {"type": "Feature", "properties": {"name": "Fen", "p": "3"},
         "geometry": {"type": "Point", "coordinates": [42, 30]}}
    ]}"#;
    let rendered = render(
        &map(Label {
            priority: Some(binding("p")),
            ..label_item()
        }),
        geojson,
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let labels: Vec<_> = rendered
        .labels
        .iter()
        .map(|l| (l.priority, l.layer, l.class, &*l.text, l.drawn))
        .collect();
    assert_eq!(
        labels,
        [
            (
                7,
                0,
                0,
                "Fen",
                Some([25.40625, 23.046875, 54.59375, 34.953125])
            ),
            (3, 0, 0, "Fen", None),
            (
                3,
                0,
                0,
                "Fen",
                Some([105.40625, 23.046875, 134.59375, 34.953125])
            ),
            (
                3,
                0,
                0,
                "Fen",
                Some([65.40625, 23.046875, 94.59375, 34.953125])
            ),
        ]
    );
    // The ink of each label drawn lies within its box, to the pixel.
    let image = rendered.image.unwrap();
    for label in &rendered.labels {
        let Some([x0, y0, x1, y1]) = label.drawn else {
            continue;
        };
        let [ix0, iy0, ix1, iy1] = ink(&image, (x0 as u32 - 2, x1 as u32 + 3), (18, 42)).unwrap();
        assert!(
            x0.floor() <= f64::from(ix0)
                && y0.floor() <= f64::from(iy0)
                && f64::from(ix1) <= x1.ceil()
                && f64::from(iy1) <= y1.ceil(),
            "the ink {:?} lies outside the box {:?}",
            [ix0, iy0, ix1, iy1],
            [x0, y0, x1, y1]
        );
    }
}

/// Every operation the render draws, in its order.
#[derive(Clone, Default)]
struct OpRecorder(std::rc::Rc<std::cell::RefCell<Vec<forager_render::DrawOp>>>);

impl forager_render::Draw for OpRecorder {
    fn draw(&mut self, item: &forager_render::DisplayItem) {
        self.0.borrow_mut().push(item.op.clone());
    }
}

/// A layer's items are drawn as if onto the map.
impl forager_render::Renderer for OpRecorder {
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

/// A label's style with `GEOMTRANSFORM labelpoly` in `color`.
fn labelpoly(color: Bound<Rgba>) -> Style<LabelTransform> {
    Style {
        color: Some(color),
        ..Style::new(LabelTransform::Poly)
    }
}

const GREEN: Rgba = Rgba::opaque(0, 255, 0);

/// The label "Fen" with `styles`.
fn fen_styled(styles: Vec<Style<LabelTransform>>) -> Label {
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
fn a_label_styles_opacity_past_an_int_is_rounded_as_a_class_styles_is() {
    // Verifies: ADR-0036
    // bindStyle rounds a label's style as it rounds a class's: MS_NINT(atof("3e9")) in an int is
    // -1294967296, and the alpha MS_NINT(-1294967296 × 2.55) keeps 179 in a byte.
    let style = Style {
        opacity: Some(binding("op")),
        ..labelpoly(Bound::Value(GREEN))
    };
    let (_, ops) = draw_fen(fen_styled(vec![style]));
    assert!(
        matches!(
            ops[0],
            forager_render::DrawOp::Fill { color, .. } if color == Rgba { a: 179, ..GREEN }
        ),
        "{:?}",
        ops[0]
    );
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
fn labelpoint() -> Style<LabelTransform> {
    Style {
        color: Some(Bound::Value(RED)),
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(8.0)),
        ..Style::new(LabelTransform::Point)
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
fn a_labelpoint_marker_of_a_bound_size_takes_each_features_size_in_the_cache() {
    // Each feature's SIZE sizes its own marker as the cache tests it: at the pixel (20, 29),
    // size 8 gives the box [16, 25, 24, 33], which moves the text 4 and 4 off the point to
    // [24, 33, 53.1875, 44.90625]; at the pixel (100, 29), size 4 gives [98, 27, 102, 31], and
    // the text 2 and 2 off it, [102, 31, 131.1875, 42.90625]. Each member's box is both; the
    // label added last is reported first.
    let fen = Label {
        position: Some(Bound::Value(Position::Lr)),
        ..fen_styled(vec![Style {
            size: Some(binding("size")),
            ..labelpoint()
        }])
    };
    let geojson = r#"{"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"size": "8"},
         "geometry": {"type": "Point", "coordinates": [20, 30]}},
        {"type": "Feature", "properties": {"size": "4"},
         "geometry": {"type": "Point", "coordinates": [100, 30]}}]}"#;
    let rendered = render(&map_with_square(fen), geojson);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let drawn: Vec<_> = rendered.labels.iter().map(|l| l.drawn).collect();
    assert_eq!(
        drawn,
        [
            Some([98.0, 27.0, 131.1875, 42.90625]),
            Some([16.0, 25.0, 53.1875, 44.90625]),
        ]
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
fn a_cached_label_of_line_breaks_only_with_a_labelpnt_style_is_refused() {
    // Does msDrawLabelCache test and draw the labelpnt marker of a member whose text has no box?
    // It computes the marker by computeMarkerBounds and tests it before the text
    // (mapdraw.c:3607-3646), and draws a labelpnt style at the member's point whatever its text
    // (4056-4065). But a text of line breaks is no NULL annotext (msAddLabelGroup,
    // maplabel.c:267-282): its path is laid out (mapdraw.c:3651, 3688-3693) on a box
    // msLayoutTextSymbol leaves at FLT_MAX and -FLT_MAX (textlayout.c:817-819), which get_metrics
    // places and msTestLabelCacheCollisions tests (mapdraw.c:3789, 3898) before the marker is
    // drawn. That box is not ported through the cache, so the label is left out with its reason
    // rather than drawn or dropped in silence.
    let label = Label {
        text: Some(Expr::String("\n".to_owned())),
        ..fen_styled(vec![labelpoint()])
    };
    let rendered = render(&map_with_square(label), FEN);
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
    assert!(
        rendered.diagnostics[0]
            .message
            .contains("its text is of line breaks only"),
        "{}",
        rendered.diagnostics[0].message
    );
    assert!(rendered.labels.is_empty());
    assert_eq!(ink(&rendered.image.unwrap(), (0, 160), (0, 60)), None);
}

/// The render of `label` over the features `geojson` with [`map_with_square`], and every
/// operation it draws.
fn draw_square(label: Label, geojson: &str) -> (Rendered, Vec<forager_render::DrawOp>) {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(geojson).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(
        &map_with_square(label),
        &sources,
        &Files,
        recorder.clone(),
    ));
    (rendered, recorder.0.take())
}

#[test]
fn a_cached_label_without_text_with_a_labelpnt_style_draws_its_marker_alone() {
    // msShapeGetLabelAnnotation gives no text for an empty LABELITEM value (maputil.c:973-976),
    // nor where no text is set at all (980); msAddLabelGroup keeps such a label for its labelpnt
    // style (maplabel.c:267-277). msDrawLabelCache tests and draws its marker, and skips all of
    // its text (mapdraw.c:3607-3649); the member's box is the marker's (3994-4027), and its
    // labelpnt style is drawn at its point (4056-4065).
    let empty = r#"{"type": "Feature", "properties": {"name": ""},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let no_text = Label {
        text: None,
        ..label_item()
    };
    for label in [label_item(), no_text] {
        let styles = vec![labelpoint()];
        let (rendered, ops) = draw_square(Label { styles, ..label }, empty);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        // The square of msGetMarkerSize 8 by 8 centred on the pixel (80, 29), as
        // a_labelpoint_style_draws_its_marker_at_the_point_under_the_text_moved_off_it draws it.
        let p = |x, y| Point { x, y };
        assert_eq!(
            ops,
            [forager_render::DrawOp::Fill {
                rings: vec![vec![
                    p(76.0, 25.0),
                    p(84.0, 25.0),
                    p(84.0, 33.0),
                    p(76.0, 33.0),
                    p(76.0, 25.0),
                ]],
                color: RED,
                gamma: None,
            }]
        );
        assert_eq!(rendered.labels.len(), 1);
        assert_eq!(&*rendered.labels[0].text, "");
        assert_eq!(rendered.labels[0].drawn, Some([76.0, 25.0, 84.0, 33.0]));
    }
}

#[test]
fn a_cached_label_without_text_with_a_labelpoly_or_labelcenter_style_is_refused() {
    // msDrawLabelCache draws a labelpoly or labelcenter style only with an annotext; a label
    // without one, kept for its labelpnt style (maplabel.c:267-277), falls to the last branch of
    // the style loop, which fails the map with "Labels only support LABELPNT, LABELPOLY and
    // LABELCENTER GEOMTRANSFORMS" (mapdraw.c:4056-4133, 540, 593). The label is left out with its
    // reason rather than drawn in part.
    let empty = r#"{"type": "Feature", "properties": {"name": ""},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let no_text = Label {
        text: None,
        ..label_item()
    };
    for label in [label_item(), no_text] {
        for boxed in [labelpoly(Bound::Value(GREEN)), labelcenter(8.0)] {
            let styles = vec![labelpoint(), boxed];
            let label = Label {
                styles,
                ..label.clone()
            };
            let (rendered, ops) = draw_square(label, empty);
            let codes: Vec<&str> = rendered
                .diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect();
            assert_eq!(codes, ["label-not-drawn"]);
            assert!(
                rendered.diagnostics[0]
                    .message
                    .contains("its text is empty, where msDrawLabelCache fails the map"),
                "{}",
                rendered.diagnostics[0].message
            );
            assert_eq!(ops, []);
            assert_eq!(rendered.labels, []);
        }
    }
}

/// A label of no text source and no `FONT`, whose one style is `labelpoint`.
fn marker_alone() -> Label {
    Label {
        text: None,
        font: None,
        styles: vec![labelpoint()],
        ..label_item()
    }
}

#[test]
fn a_cached_label_of_a_marker_alone_without_a_font_is_tested_and_drawn() {
    // MapServer needs no FONT for a label without text: loadLabel requires none, a label's style
    // is labelpnt by default (mapfile.c:2058-2061), and a font is resolved only to lay out a text,
    // in msComputeTextPath, which msDrawLabelCache calls only with an annotext
    // (mapdraw.c:3651). As a_cached_label_without_text_with_a_labelpnt_style_draws_its_marker
    // _alone draws it with a FONT: the square of 8 by 8 centred on the pixel (80, 29), its box.
    let (rendered, ops) = draw_square(marker_alone(), &collection(&[("", 80.0, 30.0, "")]));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        ops,
        [forager_render::DrawOp::Fill {
            rings: vec![square_at(80.0, 29.0)],
            color: RED,
            gamma: None,
        }]
    );
    assert_eq!(rendered.labels.len(), 1);
    assert_eq!(rendered.labels[0].drawn, Some([76.0, 25.0, 84.0, 33.0]));
    // The marker is tested (msTestLabelCacheCollisions, mapdraw.c:3632-3646): the one added last
    // is placed first, and the other, 4 pixels off it, is left out unless FORCE is on.
    let close = collection(&[("", 80.0, 30.0, ""), ("", 84.0, 30.0, "")]);
    let drawn = |label: Label| -> Vec<Option<[f64; 4]>> {
        let (rendered, _) = draw_square(label, &close);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        rendered.labels.iter().map(|l| l.drawn).collect()
    };
    let (first, second) = (
        Some([76.0, 25.0, 84.0, 33.0]),
        Some([80.0, 25.0, 88.0, 33.0]),
    );
    assert_eq!(drawn(marker_alone()), [second, None]);
    let forced = Label {
        force: Some(forager_style_ir::Force::On),
        ..marker_alone()
    };
    assert_eq!(drawn(forced), [second, first]);
}

#[test]
fn a_marker_alone_without_a_font_is_drawn_at_the_point_under_labelcache_off() {
    // msDrawLabel computes a text path only where it has a text (haveLabelText,
    // mapdraw.c:2710-2727), and draws the labelpnt styles at the point whatever the text
    // (2736-2748).
    let geojson = collection(&[("", 80.0, 30.0, "")]);
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let mut map = map_with_square(marker_alone());
    map.layers[0].label_cache = false;
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        recorder.0.take(),
        [forager_render::DrawOp::Fill {
            rings: vec![square_at(80.0, 29.0)],
            color: RED,
            gamma: None,
        }]
    );
    assert_eq!(rendered.labels, []);
}

#[test]
fn a_text_of_line_breaks_only_lays_out_nothing_and_needs_no_font() {
    // msLayoutTextSymbol looks a face up for each run (textlayout.c:802-803), but a run of no
    // glyph takes nothing of it, and msDrawLabel draws the labelpnt styles (mapdraw.c:2736-2748).
    let geojson = collection(&[("\\n", 80.0, 30.0, "")]);
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let mut map = map_with_square(Label {
        font: None,
        styles: vec![labelpoint()],
        ..label_item()
    });
    map.layers[0].label_cache = false;
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        recorder.0.take(),
        [forager_render::DrawOp::Fill {
            rings: vec![square_at(80.0, 29.0)],
            color: RED,
            gamma: None,
        }]
    );
}

#[test]
fn a_label_with_a_text_and_no_font_is_refused_for_the_built_in_font() {
    // MapServer lays such a text out in its built-in font (msGetFontFace of no key, through
    // get_face_for_run in msLayoutTextSymbol), which is not bundled: the reason says so.
    for cache in [true, false] {
        let mut map = map_with_square(Label {
            font: None,
            ..label_item()
        });
        map.layers[0].label_cache = cache;
        let rendered = render(&map, FEN);
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(codes, ["label-not-drawn"], "LABELCACHE {cache}");
        let message = &rendered.diagnostics[0].message;
        assert!(
            message.contains("it names no FONT; MapServer's built-in font is not bundled"),
            "{message}"
        );
        assert_eq!(rendered.labels, []);
    }
}

#[test]
fn an_inline_features_own_text_without_a_font_is_refused_for_the_built_in_font() {
    // A label of no text source reads the feature's TEXT (msShapeGetLabelAnnotation), which
    // MapServer lays out in its built-in font where the label names no FONT.
    let label = Label {
        text: None,
        font: None,
        ..label_item()
    };
    let rendered = render_inline(map(label), Some("Fen"), "");
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(message.contains("it names no FONT"), "{message}");
    assert_eq!(rendered.labels, []);
}

#[test]
fn a_text_label_without_a_font_or_styles_whose_features_are_all_empty_reports_nothing() {
    // A label whose text is empty for every feature lays out nothing, so it needs no font and
    // draws nothing; nor is it refused for one, as no text of it is laid out.
    let label = Label {
        font: None,
        ..label_item()
    };
    let rendered = render(
        &map(label),
        &collection(&[("", 40.0, 30.0, ""), ("", 80.0, 30.0, "")]),
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.labels, []);
    assert_eq!(
        ink(rendered.image.as_ref().unwrap(), (0, 160), (0, 60)),
        None
    );
}

#[test]
fn a_label_without_a_font_is_drawn_for_the_features_without_text_and_refused_for_the_others() {
    // msDrawLabelCache settles a font member by member, for a member with an annotext
    // (mapdraw.c:3651): of a LABELITEM empty for one feature only (maputil.c:973-976), that
    // feature's marker is drawn as a marker alone's, and the other's text is refused, once.
    let label = Label {
        font: None,
        styles: vec![labelpoint()],
        ..label_item()
    };
    let (rendered, ops) = draw_square(
        label,
        &collection(&[("Fen", 40.0, 30.0, ""), ("", 80.0, 30.0, "")]),
    );
    let codes: Vec<&str> = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, ["label-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.ends_with(
            "it names no FONT; MapServer's built-in font is not bundled; the label is not drawn \
             where a feature gives it a text"
        ),
        "{message}"
    );
    assert_eq!(
        ops,
        [forager_render::DrawOp::Fill {
            rings: vec![square_at(80.0, 29.0)],
            color: RED,
            gamma: None,
        }]
    );
    assert_eq!(rendered.labels.len(), 1);
    assert_eq!(rendered.labels[0].drawn, Some([76.0, 25.0, 84.0, 33.0]));
}

#[test]
fn the_marker_of_a_cached_label_without_text_is_tested_and_takes_room() {
    // msDrawLabelCache tests the marker of a member without text against the labels drawn,
    // unless FORCE is on (mapdraw.c:3632-3646), and msTestLabelCacheCollisions tests a later label
    // against that marker (maplabel.c:845-861). As in
    // a_labelpoint_marker_takes_room_from_the_labels_after_it, "Fen" at LR of the pixel (50, 26)
    // takes its marker [46, 22, 54, 30] and its text [54, 30, 83.1875, 41.90625]; the label
    // without text at the pixel (82, 29) has its marker at [78, 25, 86, 33], which meets that
    // text.
    let label = || Label {
        position: Some(Bound::Value(Position::Lr)),
        styles: vec![labelpoint()],
        ..label_item()
    };
    let drawn = |label: Label, points: &[(&str, f64, f64, &str)]| -> Vec<Option<[f64; 4]>> {
        let (rendered, _) = draw_square(label, &collection(points));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        rendered.labels.iter().map(|l| l.drawn).collect()
    };
    let fen = Some([46.0, 22.0, 83.1875, 41.90625]);
    let marker = Some([78.0, 25.0, 86.0, 33.0]);
    // Added last, "Fen" is drawn first, and the marker after it is left out.
    let fen_last = [("", 82.0, 30.0, ""), ("Fen", 50.0, 33.0, "")];
    assert_eq!(drawn(label(), &fen_last), [fen, None]);
    // Forced, the marker is drawn untested.
    let forced = Label {
        force: Some(forager_style_ir::Force::On),
        ..label()
    };
    assert_eq!(drawn(forced, &fen_last), [fen, marker]);
    // Added last, the marker is drawn first, and "Fen" after it is left out.
    let fen_first = [("Fen", 50.0, 33.0, ""), ("", 82.0, 30.0, "")];
    assert_eq!(drawn(label(), &fen_first), [marker, None]);
}

#[test]
fn a_cached_label_without_text_or_labelpnt_style_draws_and_reports_nothing() {
    // msAddLabelGroup skips a label without text that has no labelpnt style (maplabel.c:267-277),
    // a labelpoly one or none.
    let empty = r#"{"type": "Feature", "properties": {"name": ""},
        "geometry": {"type": "Point", "coordinates": [80, 30]}}"#;
    let no_text = Label {
        text: None,
        ..label_item()
    };
    for label in [label_item(), no_text] {
        for styles in [vec![], vec![labelpoly(Bound::Value(GREEN))]] {
            let (rendered, ops) = draw_square(
                Label {
                    styles,
                    ..label.clone()
                },
                empty,
            );
            assert!(
                rendered.diagnostics.is_empty(),
                "{:?}",
                rendered.diagnostics
            );
            assert_eq!(ops, []);
            assert_eq!(rendered.labels, []);
        }
    }
}

#[test]
fn a_label_styles_angle_auto_turns_it_by_its_angle_alone() {
    // `ANGLE AUTO` sets only the style's autoangle, its angle left as it is (loadStyle,
    // mapfile.c:2773-2775); computeMarkerBounds turns a labelpnt marker by the angle
    // (mapdraw.c:3297), msDrawMarkerSymbol draws it at the angle (computeSymbolStyle,
    // maprendering.c:76), and msDrawShadeSymbol hatches a labelpoly at it (maprendering.c:821):
    // none reads autoangle. So a label's style of `ANGLE AUTO` draws as it does without it.
    let turned = |auto_angle, angle: Option<f64>| Style {
        auto_angle,
        angle: angle.map(Bound::Value),
        ..labelpoint()
    };
    for angle in [None, Some(30.0)] {
        let with_auto = Label {
            styles: vec![turned(true, angle), labelcenter(6.0)],
            ..fen_at_lr_with_marker()
        };
        let without = Label {
            styles: vec![turned(false, angle), labelcenter(6.0)],
            ..fen_at_lr_with_marker()
        };
        let mut auto_center = labelcenter(6.0);
        auto_center.auto_angle = true;
        let with_auto_center = Label {
            styles: vec![turned(false, angle), auto_center],
            ..fen_at_lr_with_marker()
        };
        let (drawn, ops) = draw_fen_with_square(with_auto);
        let (expected, expected_ops) = draw_fen_with_square(without);
        assert_eq!(ops, expected_ops, "ANGLE {angle:?}");
        assert_eq!(drawn.labels, expected.labels, "ANGLE {angle:?}");
        assert_eq!(draw_fen_with_square(with_auto_center).1, expected_ops);
    }
}

/// [`map`] with symbol 1 a hatch.
fn map_with_hatch(label: Label) -> Map {
    Map {
        symbols: vec![Symbol {
            kind: SymbolKind::Hatch,
            paths: Vec::new(),
            extent: None,
            filled: false,
            name: Some("hatch".to_owned()),
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

/// A labelpoly of symbol 1 in `GREEN`, its lines `SIZE` 5 apart, `WIDTH` 1, dashed by `pattern`.
fn hatched_labelpoly(pattern: Vec<f64>) -> Style<LabelTransform> {
    Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: Some(Bound::Value(5.0)),
        width: Some(Bound::Value(1.0)),
        pattern,
        ..labelpoly(Bound::Value(GREEN))
    }
}

#[test]
fn a_labelpoly_style_of_a_hatch_hatches_the_box_as_ms_draw_shade_symbol_does() {
    // "Fen" at CC of the pixel (80, 29): msDrawLabelBounds gives msDrawShadeSymbol the ring of
    // the labelpoly [64.40625, 22.046875, 95.59375, 35.953125] (see
    // `a_labelpoly_style_fills_the_text_box_grown_by_a_pixel_under_the_text`), and its hatch
    // branch (maprendering.c:803-822) calls msHatchPolygon at a spacing of 5 and a width of 1,
    // the angle 0, the pattern as written. msHatchPolygon (mapagg.cpp:1133-1153) grows the box by
    // 0.7072: ox = 63.69905, oy = 21.339675, pw = (int)(31.1875 + 1.4144) + 1 = 33. createHatch's
    // case of angle 0 (mapagg.cpp:947-956) counts the lines from the image's refpt, the map's
    // origin at the pixel (0, 59) (msPrepareImage, mapdraw.c:262-265): y0 = 5 - fmod(oy - 59, 5)
    // = 7.660325, less 5 as oy - 59 < 0, so the lines at 2.660325, 7.660325, and 12.660325 below
    // oy, below ph = 16: y = 24, 29, and 34, each from ox to ox + 33.
    let label = Label {
        text: Some(Expr::String("Fen".to_owned())),
        styles: vec![hatched_labelpoly(Vec::new())],
        ..label_item()
    };
    assert_hatched(&map_with_hatch(label.clone()), [24.0, 29.0, 34.0]);
    // A layer of LABELCACHE OFF and OPACITY 50 draws its labels at once into an image of its own
    // (msDrawLayer), at the same box, as msDrawLabel computes it alike (mapdraw.c:2838-2860);
    // that image's refpt stays at (0, 0) (agg2CreateImage), so y0 = 5 - fmod(oy, 5) = 3.660325:
    // y = 25, 30, and 35.
    let mut own = map_with_hatch(label);
    own.layers[0].label_cache = false;
    own.layers[0].opacity = Some(50.0);
    assert_hatched(&own, [25.0, 30.0, 35.0]);
}

/// Asserts that `map` draws over [`FEN`], without a diagnostic, the hatch of
/// [`hatched_labelpoly`] on the labelpoly of "Fen" at CC of (80, 29), its lines at `ys`, and then
/// the text.
fn assert_hatched(map: &Map, ys: [f64; 3]) {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(FEN).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let ops = recorder.0.take();
    assert_eq!(ops.len(), 2, "{ops:?}");
    let forager_render::DrawOp::Hatch(hatch) = &ops[0] else {
        panic!("a hatch first: {:?}", ops[0]);
    };
    let p = |x, y| Point { x, y };
    assert_eq!(
        hatch.rings,
        [vec![
            p(64.40625, 22.046875),
            p(64.40625, 35.953125),
            p(95.59375, 35.953125),
            p(95.59375, 22.046875),
            p(64.40625, 22.046875),
        ]]
    );
    let (ox, oy) = (64.40625 - 0.7072, 22.046875 - 0.7072);
    let expected = ys.map(|y| [p(ox, y), p(ox + 33.0, y)]);
    assert_eq!(hatch.lines.len(), expected.len(), "{:?}", hatch.lines);
    for (line, want) in hatch.lines.iter().zip(&expected) {
        for (got, want) in line.iter().zip(want) {
            assert!(
                (got.x - want.x).abs() < 1e-9 && (got.y - want.y).abs() < 1e-9,
                "{line:?} is not {want:?} (oy {oy})"
            );
        }
    }
    assert_eq!(
        (
            hatch.color,
            hatch.width,
            hatch.pattern.as_slice(),
            hatch.gamma
        ),
        (GREEN, 1.0, &[][..], Some(0.75))
    );
    assert!(matches!(ops[1], forager_render::DrawOp::Glyphs { .. }));
}

#[test]
fn a_labelpoly_hatch_past_its_bounds_is_reported_with_its_layer_as_a_polygons_is() {
    // Verifies: Q-9
    // The same hatch dashed by 0.00001 and 0.00001: its three lines of 33 pixels walk some
    // 9.9 million dashes and gaps, past Hatch::MAX_DASHES, so the hatch is refused, and the
    // layer reports it with `hatch-not-drawn`, as it reports a polygon's; the text is drawn. So
    // too where the label is drawn at once, of a layer of LABELCACHE OFF.
    for cache in [true, false] {
        let label = Label {
            text: Some(Expr::String("Fen".to_owned())),
            styles: vec![hatched_labelpoly(vec![0.00001, 0.00001])],
            ..label_item()
        };
        let mut map = map_with_hatch(label);
        map.layers[0].label_cache = cache;
        map.layers[0].span = Some(Span::new("m.map", 3, 3));
        let rendered = render(&map, FEN);
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(codes, ["hatch-not-drawn"], "LABELCACHE {cache}");
        let d = &rendered.diagnostics[0];
        assert_eq!(d.span, Some(Span::new("m.map", 3, 3)));
        assert!(
            d.message
                .starts_with("layer `places`: 1 hatch fill is not drawn (Q-9)"),
            "{}",
            d.message
        );
        // The text alone, in BLUE.
        let image = rendered.image.unwrap();
        assert_eq!(count(&image, [0, 255, 0, 255]), 0, "LABELCACHE {cache}");
        assert!(
            ink(&image, (0, 160), (0, 60)).is_some(),
            "LABELCACHE {cache}"
        );
    }
}

/// A label's style with `GEOMTRANSFORM labelcenter`: symbol 1, a filled square of side 1, at
/// `SIZE` `size`, in `RED`.
fn labelcenter(size: f64) -> Style<LabelTransform> {
    Style {
        size: Some(Bound::Value(size)),
        geom_transform: LabelTransform::Center,
        ..labelpoint()
    }
}

#[test]
fn a_label_styles_symbol_by_name_draws_as_the_same_symbol_by_index() {
    // msGetSymbolIndex resolves a style's SYMBOL "name" to the first symbol of that name,
    // without regard to case (mapsymbol.c), so a labelpoly of a hatch and a labelpnt or
    // labelcenter of a vector draw the same by name as by index.
    let by_name = |style: Style<LabelTransform>, name: &str| Style {
        symbol: Some(Bound::Value(SymbolRef::Name(name.to_owned()))),
        ..style
    };
    fn named_square(label: Label) -> Map {
        let mut map = map_with_square(label);
        map.symbols[0].name = Some("sq".to_owned());
        map
    }
    let draw = |map: &Map| {
        let sources: BTreeMap<String, GeoJsonSource> =
            [("places".to_owned(), GeoJsonSource::parse(FEN).unwrap())].into();
        let recorder = OpRecorder::default();
        let rendered = ready(render_with_files(map, &sources, &Files, recorder.clone()));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        recorder.0.take()
    };
    let cases = [
        (named_square as fn(Label) -> Map, labelpoint(), "SQ"),
        (named_square, labelcenter(8.0), "Sq"),
        (map_with_hatch, hatched_labelpoly(Vec::new()), "HATCH"),
    ];
    let mut drawn = Vec::new();
    for (with_symbols, style, name) in cases {
        let by_index = draw(&with_symbols(fen_styled(vec![style.clone()])));
        let named = draw(&with_symbols(fen_styled(vec![by_name(style, name)])));
        assert_eq!(by_index.len(), 2, "{name}: {by_index:?}");
        assert_eq!(named, by_index, "{name}");
        drawn.push(by_index);
    }
    // The labelpnt marker by index, as a_labelpoint_style_draws_its_marker_at_the_point_under_the
    // _text_moved_off_it pins it: the square of 8 by 8 centred on the pixel (80, 29).
    assert_eq!(
        drawn[0][0],
        forager_render::DrawOp::Fill {
            rings: vec![square_at(80.0, 29.0)],
            color: RED,
            gamma: None,
        }
    );
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
    // no vector, ellipse, or pixmap of the set, here one past the set, leaves the label out.
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

/// The map of the label "Fen" with a style of `transform` of the pixmap `pin`, symbol 2, whose
/// image `missing.png` is not among the files.
fn fen_over_missing_pixmap(transform: LabelTransform) -> Map {
    let mut map = map_with_square(fen_styled(vec![Style {
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        geom_transform: transform,
        ..labelcenter(8.0)
    }]));
    map.symbols.push(Symbol {
        name: Some("pin".to_owned()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        image: Some("missing.png".to_owned()),
        ..map.symbols[0].clone()
    });
    map
}

/// The codes of `rendered`'s diagnostics, and whether the first names the symbol `pin` and its
/// image `missing.png`.
fn names_missing_pin(rendered: &Rendered) -> (Vec<&str>, bool) {
    let codes = rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    let named = rendered
        .diagnostics
        .first()
        .is_some_and(|d| d.message.contains("symbol `pin`") && d.message.contains("missing.png"));
    (codes, named)
}

#[test]
fn a_labelcenter_pixmap_that_does_not_load_leaves_its_marker_out_and_warns() {
    // msDrawMarkerSymbol draws a labelcenter style's pixmap (src/maprendering.c:980), and fails
    // where msPreloadImageSymbol cannot load it; Forager leaves the marker out, draws the label's
    // text as the label without the style draws it, and warns with the symbol and its image.
    let rendered = render(&fen_over_missing_pixmap(LabelTransform::Center), FEN);
    assert_eq!(
        names_missing_pin(&rendered),
        (vec!["pixmap-not-drawn"], true),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.labels.len(), 1);
    assert!(rendered.labels[0].drawn.is_some());
    let unstyled = render(&map_with_square(fen_styled(Vec::new())), FEN);
    assert!(
        unstyled.diagnostics.is_empty(),
        "{:?}",
        unstyled.diagnostics
    );
    let (image, unstyled) = (rendered.image.unwrap(), unstyled.image.unwrap());
    assert!(
        ink(&image, (0, 160), (0, 60)).is_some(),
        "the text is drawn"
    );
    assert!(image == unstyled, "the marker is left out");
}

#[test]
fn a_pixmap_neither_a_point_nor_its_label_drawn_at_once_draws_is_reported_once() {
    // A point layer of LABELCACHE OFF draws its markers and its labels' styles into one image,
    // each on a canvas of its own; a pixmap whose image does not load, left undrawn by both, is
    // reported once for the layer.
    let mut map = fen_over_missing_pixmap(LabelTransform::Center);
    map.layers[0].label_cache = false;
    map.layers[0].classes[0].styles = vec![Style {
        symbol: Some(Bound::Value(SymbolRef::Index(2))),
        ..Style::default()
    }];
    let rendered = render(&map, FEN);
    assert_eq!(
        names_missing_pin(&rendered),
        (vec!["pixmap-not-drawn"], true),
        "{:?}",
        rendered.diagnostics
    );
}

#[test]
fn a_pixmap_neither_a_point_nor_its_cached_label_draws_is_reported_once() {
    // The label cache draws a label's styles after its layer, on a canvas of its own; a pixmap
    // whose image does not load, left undrawn by a style of the layer and by the label, is
    // reported once for the layer. The class's first style is not the pin, so the label holds no
    // marker of it and is drawn.
    let mut map = fen_over_missing_pixmap(LabelTransform::Center);
    assert!(map.layers[0].label_cache);
    map.layers[0].classes[0].styles = vec![
        Style::default(),
        Style {
            symbol: Some(Bound::Value(SymbolRef::Index(2))),
            ..Style::default()
        },
    ];
    let rendered = render(&map, FEN);
    assert_eq!(
        names_missing_pin(&rendered),
        (vec!["pixmap-not-drawn"], true),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.labels.len(), 1);
    assert!(rendered.labels[0].drawn.is_some());
}

#[test]
fn a_labelpnt_pixmap_that_does_not_load_leaves_the_label_out() {
    // msGetMarkerSize sizes a labelpnt style's marker in computeMarkerBounds and fails where
    // msPreloadImageSymbol does (src/mapsymbol.c:736-738), and the map with it; Forager leaves
    // the label out, with a warning that names the symbol and its image.
    let rendered = render(&fen_over_missing_pixmap(LabelTransform::Point), FEN);
    assert_eq!(
        names_missing_pin(&rendered),
        (vec!["label-not-drawn"], true),
        "{:?}",
        rendered.diagnostics
    );
    assert!(rendered.labels.is_empty(), "{:?}", rendered.labels);
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
    let apart = foragers(&[([80.0, 30.0], "255 0 0"), ([83.0, 30.0], "0 0 255")]);
    let rendered = render(&cache_off(), &apart);
    let image = rendered.image.unwrap();
    assert!(count(&image, red) > 0 && count(&image, blue) > 0);
    // Drawn in the order of the features: "Fen", 29.1875 by 11.90625 pixels, at CC of the pixel
    // (80, 29) in red, its box from (65.40625, 23.046875), then at CC of (83, 29), 3 pixels
    // across, in blue (maplabel.c:1049-1051, 1109-1117). The F's outline starts at its box's top,
    // 1.578125 right of its left edge, as Forager's text stack draws the test font; the font's
    // own left side bearing, 201 of 2048 units at 16 pixels, is 1.5703125, so the value is
    // Forager's, not one derived from MapServer, and pins the placement against a move.
    let mut fen = cache_off();
    fen.layers[0].classes[0].labels[0].text = Some(Expr::String("Fen".to_owned()));
    let drawn: Vec<(Option<Rgba>, Vec<PathCommand>)> = recorded(&fen, &apart)
        .iter()
        .map(|op| match op {
            forager_render::DrawOp::Glyphs { path, color, .. } => (*color, path.clone()),
            other => panic!("{other:?}"),
        })
        .collect();
    let starts: Vec<_> = drawn
        .iter()
        .map(|(color, path)| (*color, path[0]))
        .collect();
    assert_eq!(
        starts,
        [
            (
                Some(Rgba::opaque(255, 0, 0)),
                PathCommand::MoveTo(Point {
                    x: 66.984375,
                    y: 23.046875
                })
            ),
            (
                Some(Rgba::opaque(0, 0, 255)),
                PathCommand::MoveTo(Point {
                    x: 69.984375,
                    y: 23.046875
                })
            ),
        ]
    );
    assert_eq!(drawn[0].1, expected("Fen", 12.0, Anchor::Cc, (0, 0), 0.0));
    assert_eq!(drawn[1].1, expected("Fen", 12.0, Anchor::Cc, (3, 0), 0.0));
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

/// The box of the pixels of `image` that are of `rgba` exactly, `None` when none is.
fn boxed(image: &RgbaImage, rgba: [u8; 4]) -> Option<[u32; 4]> {
    let mut found: Option<[u32; 4]> = None;
    for y in 0..image.height {
        for x in 0..image.width {
            if image.pixel(x, y).map(|p| [p.r, p.g, p.b, p.a]) == Some(rgba) {
                let b = found.get_or_insert([x, y, x, y]);
                *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    found
}

const PURE_GREEN: [u8; 4] = [0, 255, 0, 255];

#[test]
fn a_label_of_labelcache_off_stands_by_its_position_and_offset_alone() {
    // msDrawLabel calls get_metrics with the label's OFFSET and no marker's: a class style's
    // marker of 30 pixels, which would move a cached label 15 pixels off, does not move it.
    // POSITION AUTO is a case get_metrics' switch does not name, x1 = y1 = 0, its offset unread.
    // The label's labelpoly, filled, shows where get_metrics puts the text's box: "Fen" at 12
    // points, 29.1875 by 11.90625 pixels.
    let mut boxes = Vec::new();
    for (position, offset) in [
        (Position::Auto, [10.0, 10.0]),
        (Position::Ur, [0.0, 0.0]),
        (Position::Ur, [10.0, 0.0]),
    ] {
        let mut map = cache_off();
        let label = &mut map.layers[0].classes[0].labels[0];
        label.text = Some(Expr::String("Fen".to_owned()));
        label.position = Some(Bound::Value(position));
        label.offset = Some([Bound::Value(offset[0]), Bound::Value(offset[1])]);
        label.styles = vec![labelpoly(Bound::Value(GREEN))];
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
        boxes.push(boxed(&rendered.image.unwrap(), PURE_GREEN).expect("a labelpoly"));
    }
    // The point (40, 30) is pixel (40, 29): the extent's 159 units over 160 pixels give a cell of
    // one (msTransformPoint). At UR, x1 = ox and y1 = -oy (get_metrics), so the text's box has
    // its lower left corner at (40 + ox, 29) (maplabel.c:1038-1040, 1109-1117); the labelpoly is
    // that box grown by 1, [39 + ox, 16.09375, 70.1875 + ox, 30]. agg2RenderPolygon fills it
    // through gamma_linear(0, 0.75), which turns a cover of 0.75 or more full (mapagg.cpp:275-283,
    // 862): the row 16, covered 0.90625, is wholly green, the column 70 + ox, covered 0.1875, is
    // not. The green box runs over the columns from 39 + ox to 69 + ox and the rows from 16 to 29.
    assert_eq!(boxes[1], [39, 16, 69, 29]);
    assert_eq!(boxes[2], [49, 16, 79, 29]);
    assert_eq!(boxes[0], boxes[1], "AUTO stands as UR at offset 0");
    assert_eq!(
        boxes[2],
        [boxes[1][0] + 10, boxes[1][1], boxes[1][2] + 10, boxes[1][3]],
        "an OFFSET of 10 moves it 10 pixels"
    );
}

#[test]
fn a_label_of_labelcache_off_draws_its_styles_before_its_text() {
    // msDrawLabel draws a labelpnt style's marker at the point, a labelpoly style on get_metrics'
    // box grown by 1 pixel, and a labelcenter style's marker at that box's centre, in their
    // order, then the text.
    let mut map = map_with_square(Label {
        text: Some(Expr::String("Fen".to_owned())),
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
    // The labelpoly at UR of pixel (40, 29), as in
    // [`a_label_of_labelcache_off_stands_by_its_position_and_offset_alone`]: its left edge at
    // x = 39, its bottom at y = 30.
    let poly = boxed(&image, PURE_GREEN).expect("a labelpoly");
    assert_eq!(poly, [39, 16, 69, 29]);
    // The square of 8 pixels centred on the point, from 36 to 44 and from 25 to 33, unmoved and
    // drawn over the labelpoly.
    assert_eq!(boxed(&image, [220, 20, 20, 255]), Some([36, 25, 43, 32]));
    // The labelcenter marker at the centre of the labelpoly's box, after it.
    let [x0, y0, x1, y1] = boxed(&image, [30, 60, 200, 255]).expect("a labelcenter marker");
    let centre = |a: u32, b: u32| f64::from(a + b + 1) / 2.0;
    assert!((centre(x0, x1) - centre(poly[0], poly[2])).abs() <= 1.0);
    assert!((centre(y0, y1) - centre(poly[1], poly[3])).abs() <= 1.0);
    // As drawn: the labelpoly [39, 16.09375, 70.1875, 30] from its lower left corner
    // (mapdraw.c:4093-4127), the square of 8 at the point, the square of 4 at the labelpoly's
    // centre (54.59375, 23.046875), half its side 2 on each side, then the text.
    let sources: BTreeMap<String, GeoJsonSource> = [(
        "places".to_owned(),
        GeoJsonSource::parse(&foragers(&[([40.0, 30.0], "")])).unwrap(),
    )]
    .into();
    let recorder = OpRecorder::default();
    ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    let ops = recorder.0.take();
    let p = |x, y| Point { x, y };
    let fill = |ring: Vec<Point>, color: Rgba, gamma: Option<f64>| forager_render::DrawOp::Fill {
        rings: vec![ring],
        color,
        gamma,
    };
    assert_eq!(ops.len(), 4, "{ops:?}");
    assert_eq!(
        ops[..3],
        [
            fill(
                vec![
                    p(39.0, 16.09375),
                    p(39.0, 30.0),
                    p(70.1875, 30.0),
                    p(70.1875, 16.09375),
                    p(39.0, 16.09375),
                ],
                GREEN,
                Some(0.75)
            ),
            fill(square_at(40.0, 29.0), RED, None),
            fill(
                vec![
                    p(52.59375, 21.046875),
                    p(56.59375, 21.046875),
                    p(56.59375, 25.046875),
                    p(52.59375, 25.046875),
                    p(52.59375, 21.046875),
                ],
                BLUE,
                None
            ),
        ]
    );
    // The text at UR of (40, 29), [`expected`]'s point 40 pixels to the left.
    assert_eq!(
        glyph_path(&ops[3]),
        expected("Fen", 12.0, Anchor::Ur, (-40, 0), 0.0)
    );
}

#[test]
fn a_label_of_labelcache_off_draws_its_styles_bound_to_each_feature() {
    // A labelpoly whose COLOR is bound to the property `c` draws, drawn at once, in each
    // feature's own colour: at UR of the pixel (40, 29) in green, its left edge at x = 39, and at
    // UR of the pixel (110, 29) in magenta, its left edge at x = 109.
    let mut map = map(Label {
        styles: vec![labelpoly(binding("c"))],
        position: Some(Bound::Value(Position::Ur)),
        ..label_item()
    });
    map.layers[0].label_cache = false;
    let points = [([40.0, 30.0], "0 255 0"), ([110.0, 30.0], "255 0 255")];
    let rendered = render(&map, &foragers(&points));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    let green = boxed(&image, PURE_GREEN).expect("a green labelpoly");
    let magenta = boxed(&image, [255, 0, 255, 255]).expect("a magenta labelpoly");
    assert_eq!([green[0], green[3]], [39, 29]);
    assert_eq!([magenta[0], magenta[3]], [109, 29]);
}

#[test]
fn a_label_of_labelcache_off_without_text_draws_its_labelpnt_styles() {
    // msDrawLabel draws a labelpnt style at the point whatever the text (mapdraw.c:2714-2748),
    // an empty one or one of line breaks; the cache draws the empty one's alike, and refuses the
    // other (a_cached_label_of_line_breaks_only_with_a_labelpnt_style_is_refused).
    for text in ["", "\n"] {
        let label = || Label {
            text: Some(Expr::String(text.to_owned())),
            ..fen_styled(vec![labelpoint()])
        };
        let mut map = map_with_square(label());
        map.layers[0].label_cache = false;
        let rendered = render(&map, &foragers(&[([40.0, 30.0], "")]));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        let image = rendered.image.unwrap();
        // The square of 8 pixels centred on pixel (40, 29), and nothing else.
        assert_eq!(boxed(&image, [220, 20, 20, 255]), Some([36, 25, 43, 32]));
        assert_eq!(ink(&image, (0, 160), (0, 60)), Some([36, 25, 43, 32]));
        let cached = render(&map_with_square(label()), &foragers(&[([40.0, 30.0], "")]));
        let square = text.is_empty().then_some([36, 25, 43, 32]);
        assert_eq!(ink(&cached.image.unwrap(), (0, 160), (0, 60)), square);
    }
}

#[test]
fn a_label_of_labelcache_off_with_no_text_set_draws_its_labelpnt_styles() {
    // Without TEXT or LABELITEM, msShapeGetLabelAnnotation gives NULL (maputil.c:965-982), which
    // pointLayerDrawShape passes to msDrawLabel (mapdraw.c:2087-2105), and msDrawLabel draws a
    // labelpnt style at the point without a text (2714-2748).
    let mut map = map_with_square(Label {
        text: None,
        ..fen_styled(vec![labelpoint()])
    });
    map.layers[0].label_cache = false;
    let rendered = render(&map, &foragers(&[([40.0, 30.0], "")]));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // The square of 8 pixels centred on pixel (40, 29), and nothing else.
    assert_eq!(boxed(&image, [220, 20, 20, 255]), Some([36, 25, 43, 32]));
    assert_eq!(ink(&image, (0, 160), (0, 60)), Some([36, 25, 43, 32]));
}

#[test]
fn a_label_of_labelcache_off_without_text_and_a_labelpoly_is_refused() {
    // On an empty text msDrawLabel fails the layer's draw at a labelpoly style
    // (mapdraw.c:2793-2799); on one of line breaks it draws it on a box left at FLT_MAX: the
    // label is left out with its reason.
    for text in ["", "\n"] {
        let mut map = map_with_square(Label {
            text: Some(Expr::String(text.to_owned())),
            ..fen_styled(vec![labelpoint(), labelpoly(Bound::Value(GREEN))])
        });
        map.layers[0].label_cache = false;
        let rendered = render(&map, &foragers(&[([40.0, 30.0], "")]));
        let codes: Vec<&str> = rendered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert_eq!(codes, ["label-not-drawn"]);
        assert!(
            rendered.diagnostics[0]
                .message
                .contains("its text is empty or of line breaks only"),
            "{}",
            rendered.diagnostics[0].message
        );
        assert_eq!(ink(&rendered.image.unwrap(), (0, 160), (0, 60)), None);
    }
}

/// [`cache_off`] with its layer at `OPACITY 50`.
fn cache_off_at_half() -> Map {
    let mut map = cache_off();
    map.layers[0].opacity = Some(50.0);
    map
}

#[test]
fn the_labels_of_a_layer_of_labelcache_off_are_drawn_into_its_own_image() {
    // msDrawLabel draws into the image the layer is drawn in, its own where it has an opacity,
    // which is laid over the map at MS_NINT(50 × 2.55) = 128 of 255 after its labels: a glyph's
    // pure blue over white is laid over as 255 - 255 × 128 / 255 = 127 in red and green.
    let rendered = render(
        &cache_off_at_half(),
        &foragers(&[([80.0, 30.0], "0 0 255")]),
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert_eq!(
        count(&image, [0, 0, 255, 255]),
        0,
        "no glyph is drawn opaque"
    );
    assert!(count(&image, [127, 127, 255, 255]) > 0);
}

/// An item a [`LayerRecorder`] is handed: its feature and whether it is a label's glyphs.
type Drawn = (Option<u64>, bool);

/// What a [`LayerRecorder`] is asked to draw: an item onto the map, or a layer of its own.
#[derive(Clone, Debug, PartialEq)]
enum Recorded {
    Item(Drawn),
    Layer { opacity: u8, items: Vec<Drawn> },
}

fn drawn(item: &forager_render::DisplayItem) -> Drawn {
    (
        item.feature_id,
        matches!(item.op, forager_render::DrawOp::Glyphs { .. }),
    )
}

/// A renderer that keeps each item's feature and kind, and the layers it begins.
#[derive(Clone, Default)]
struct LayerRecorder(std::rc::Rc<std::cell::RefCell<Vec<Recorded>>>);

/// A layer of a [`LayerRecorder`]: it keeps its items in the layer it recorded last.
struct RecordedLayer<'a>(&'a LayerRecorder);

impl forager_render::Draw for RecordedLayer<'_> {
    fn draw(&mut self, item: &forager_render::DisplayItem) {
        match self.0.0.borrow_mut().last_mut() {
            Some(Recorded::Layer { items, .. }) => items.push(drawn(item)),
            _ => unreachable!("a layer is recorded when it begins"),
        }
    }
}

impl forager_render::Draw for LayerRecorder {
    fn draw(&mut self, item: &forager_render::DisplayItem) {
        self.0.borrow_mut().push(Recorded::Item(drawn(item)));
    }
}

impl forager_render::Renderer for LayerRecorder {
    type Layer<'a> = RecordedLayer<'a>;

    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}

    fn layer(&mut self, opacity: u8) -> RecordedLayer<'_> {
        self.0.borrow_mut().push(Recorded::Layer {
            opacity,
            items: Vec::new(),
        });
        RecordedLayer(self)
    }

    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        }
    }
}

#[test]
fn a_layer_of_labelcache_off_with_an_opacity_draws_each_label_into_its_image_after_its_feature() {
    // msDrawLayer draws a layer of OPACITY 50 into an image of its own, laid over the map at
    // MS_NINT(50 × 2.55), and pointLayerDrawShape draws each feature's label into it right after
    // the feature's point, before the next feature.
    let mut map = map(label_item());
    map.layers[0].opacity = Some(50.0);
    map.layers[0].label_cache = false;
    map.layers[0].classes[0].styles = vec![Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(4.0)),
        ..Style::default()
    }];
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(PLACES).unwrap())].into();
    let recorder = LayerRecorder::default();
    let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.labels, []);
    let recorded = recorder.0.borrow().clone();
    let [Recorded::Layer { opacity, items }] = recorded.as_slice() else {
        panic!("expected one layer, got {recorded:?}");
    };
    assert_eq!(*opacity, 127);
    // The items in order, a run of one feature's items of one kind taken once: the third
    // feature has no name and so no label.
    let mut runs = items.clone();
    runs.dedup();
    assert_eq!(
        runs,
        [
            (Some(0), false),
            (Some(0), true),
            (Some(1), false),
            (Some(1), true),
            (Some(2), false),
        ]
    );
}

/// What a layer of `LABELCACHE OFF` draws of one multipoint at `points`: its class's square of 20
/// pixels in green, of the `GEOMTRANSFORM` `transform`, and the label "Fen" at each point. Each
/// operation in order, a marker as `M` at the x of its centre, the label's glyphs as `L` at the x
/// of their first point.
fn multipoint_off(
    points: &[[f64; 2]],
    transform: Option<forager_style_ir::GeomTransform>,
) -> Vec<(char, f64)> {
    let mut map = map(Label {
        text: Some(Expr::String("Fen".to_owned())),
        ..label_item()
    });
    map.layers[0].label_cache = false;
    map.layers[0].classes[0].styles = vec![Style {
        color: Some(Bound::Value(GREEN)),
        size: Some(Bound::Value(20.0)),
        geom_transform: transform,
        ..Style::default()
    }];
    let coordinates: Vec<String> = points.iter().map(|[x, y]| format!("[{x}, {y}]")).collect();
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{{"type": "Feature", "properties": {{}},
            "geometry": {{"type": "MultiPoint", "coordinates": [{}]}}}}]}}"#,
        coordinates.join(", ")
    );
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    recorder
        .0
        .take()
        .iter()
        .map(|op| match op {
            forager_render::DrawOp::Fill { rings, .. } => {
                ('M', f64::midpoint(rings[0][0].x, rings[0][2].x))
            }
            forager_render::DrawOp::Glyphs { path, .. } => match path[0] {
                PathCommand::MoveTo(p) => ('L', p.x),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect()
}

/// The operations a multipoint at x 40 and 45 is drawn by: each point's marker, then its label.
fn each_point_marker_then_label(drawn: &[(char, f64)]) {
    // msDrawLabel puts the label at its position, CC by default (mapfile.c:1598), of its point
    // and moves it nowhere for the marker (mapdraw.c:2803-2808): "Fen" at 12 points, 29.1875
    // pixels across by its font's advances, its box from x - 14.59375 (maplabel.c:1049-1051,
    // 1109-1117), from 25.40625 at the first point and 30.40625 at the second. The F's outline
    // starts 1.578125 right of the box's left edge, as Forager's text stack draws the test font;
    // the font's own left side bearing, 201 of 2048 units at 16 pixels, is 1.5703125, so that
    // part of the value is Forager's, not one derived from MapServer, as in
    // a_layer_of_labelcache_off_draws_every_label_in_the_order_of_its_features.
    assert_eq!(
        drawn,
        [
            ('M', 40.0),
            ('L', 26.984_375),
            ('M', 45.0),
            ('L', 31.984_375)
        ]
    );
}

#[test]
fn a_multipoint_of_labelcache_off_draws_each_points_marker_then_its_label() {
    // pointLayerDrawShape (src/mapdraw.c:2062-2105) draws at each point of the shape every
    // style's marker by msDrawMarkerSymbol, then each label msGetLabelStatus passes by
    // msDrawLabel, then the next point's markers: the second point's marker, 5 pixels on and
    // 20 pixels across, covers the first point's label.
    each_point_marker_then_label(&multipoint_off(&[[40.0, 30.0], [45.0, 30.0]], None));
}

#[test]
fn a_multipoint_of_labelcache_off_draws_a_styles_marker_at_each_point_whatever_its_transform() {
    // msDrawMarkerSymbol draws every style at every point whatever its GEOMTRANSFORM
    // (src/mapdraw.c:2072-2085): a style of GEOMTRANSFORM "start" too, each before that point's
    // label.
    let start = Some(forager_style_ir::GeomTransform::Start);
    each_point_marker_then_label(&multipoint_off(&[[40.0, 30.0], [45.0, 30.0]], start));
}

#[test]
fn a_point_of_a_multipoint_outside_the_extent_draws_neither_marker_nor_label() {
    // msPointInRect skips a point outside the map's extent before its markers and its labels
    // (src/mapdraw.c:2065-2066).
    let drawn = multipoint_off(&[[40.0, 30.0], [200.0, 30.0], [45.0, 30.0]], None);
    each_point_marker_then_label(&drawn);
}

/// The map of a label "Fen" at LC over the pixmap `image` of SIZE `size` and ANGLE `angle`,
/// symbol 1, which its class draws first.
fn over_pixmap(image: &str, size: Option<f64>, angle: Option<f64>) -> Map {
    let mut map = map(Label {
        position: Some(Bound::Value(Position::Lc)),
        ..label_item()
    });
    map.symbols = vec![Symbol {
        name: Some("pin".to_owned()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: Some(image.to_owned()),
        transparent: None,
        span: None,
    }];
    map.layers[0].classes[0].styles = vec![Style {
        symbol: Some(Bound::Value(SymbolRef::Index(1))),
        size: size.map(Bound::Value),
        angle: angle.map(Bound::Value),
        ..Style::default()
    }];
    map
}

/// The boxes the label cache drew `map`'s labels of `points` in, from the last added to the
/// first, `None` for one it discarded.
fn drawn_boxes(map: &Map, points: &[(&str, f64, f64, &str)]) -> Vec<Option<[f64; 4]>> {
    let sources: BTreeMap<String, GeoJsonSource> = [(
        "places".to_owned(),
        GeoJsonSource::parse(&collection(points)).unwrap(),
    )]
    .into();
    let rendered = ready(render_with_files(map, &sources, &Files, AggRenderer::new()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    rendered.labels.iter().map(|l| l.drawn).collect()
}

#[test]
fn a_label_moves_away_from_a_pixmap_marker_of_the_size_of_its_image() {
    // Verifies: G-10
    // msAddLabelGroup (src/maplabel.c:343-376) holds the box of msGetMarkerSize
    // (src/mapsymbol.c:720-799) of the class's first style about the point, here the pixel
    // (80, 14). The 32 × 32 pin without a SIZE: msSymbolGetDefaultSize gives its height, 32, so
    // `size` is 32 and the marker 32 / 32 × 32 = 32 wide and 32 high (776-781); its box
    // [80 - 16, 14 - 16, 64 + 31, -2 + 31] = [64, -2, 95, 29]. At SIZE 55 ANGLE 45, the angle
    // not read there, it is 55 / 32 × 32 = 55 by 55: [52.5, -13.5, 106.5, 40.5].
    // msDrawLabelCache moves the label at LC down by half its box's span, 31 / 2 = 15.5 and
    // 54 / 2 = 27, cut to 15 and 27 by get_metrics' int, and MARKER_SLOP's 2: its text, 11.90625
    // high, from 14 + 15 + 2 = 31 and from 14 + 27 + 2 = 43. A circle of the same size gives the
    // same box, and the label the same place.
    let point = [("Fen", 80.0, 45.0, "")];
    let circle = |size: f64| {
        let mut map = over_pixmap("pin.png", Some(size), None);
        map.symbols[0] = Symbol {
            name: Some("circle".to_owned()),
            kind: SymbolKind::Ellipse,
            paths: vec![vec![[1.0, 1.0]]],
            extent: Some([1.0, 1.0]),
            filled: true,
            image: None,
            ..map.symbols[0].clone()
        };
        map
    };
    let unscaled = over_pixmap("pin.png", None, None);
    let turned = over_pixmap("pin.png", Some(55.0), Some(45.0));
    for (map, marker, same) in [
        (&unscaled, [64.0_f64, -2.0, 95.0, 29.0], circle(32.0)),
        (&turned, [52.5, -13.5, 106.5, 40.5], circle(55.0)),
    ] {
        let top = 14.0 + ((marker[3] - marker[1]) / 2.0).trunc() + 2.0;
        let boxes = drawn_boxes(map, &point);
        let label = Some([65.40625, top, 94.59375, top + 11.90625]);
        assert_eq!(boxes, [label]);
        assert_eq!(boxes, drawn_boxes(&same, &point));
        // A second feature at the pixel row 40, added last, its marker as wide as the first's
        // and as far right of the first label's far edge as its own centre is: its near edge
        // just past that edge, 95, keeps the label; one pixel left, 94, meets it.
        let half = (marker[2] - marker[0] + 1.0) / 2.0;
        for (near, kept) in [(95.0, label), (94.0, None)] {
            let two = [("Fen", 80.0, 45.0, ""), ("Fen", near + half, 19.0, "")];
            assert_eq!(drawn_boxes(map, &two)[1], kept, "near edge {near}");
        }
    }
    // A marker outside its scale range is not drawn, but msAddLabelGroup sizes it all the same
    // (src/maplabel.c:344-352): the label stands where the unscaled pin puts it.
    let mut unseen = over_pixmap("pin.png", None, None);
    unseen.layers[0].classes[0].styles[0].max_scale_denom = Some(1.0);
    assert_eq!(drawn_boxes(&unseen, &point), drawn_boxes(&unscaled, &point));
}

#[test]
fn each_label_moves_away_from_its_own_feature_s_pixmap_marker() {
    // Verifies: G-10
    // A first style whose SIZE each feature binds sizes each feature's marker by its own value
    // (src/maplabel.c:350): at the pixel row 14, a pin of 55 moves its label at LC down to 43, one
    // of 32 to 31, whichever feature comes last.
    let mut map = over_pixmap("pin.png", None, None);
    map.layers[0].classes[0].styles[0].size = Some(binding("s"));
    let features = |sizes: [&str; 2]| {
        let [a, b] = sizes;
        format!(
            r#"{{"type": "FeatureCollection", "features": [
                {{"type": "Feature", "properties": {{"name": "Fen", "s": "{a}"}},
                  "geometry": {{"type": "Point", "coordinates": [40, 45]}}}},
                {{"type": "Feature", "properties": {{"name": "Fen", "s": "{b}"}},
                  "geometry": {{"type": "Point", "coordinates": [120, 45]}}}}]}}"#
        )
    };
    for (sizes, tops) in [(["55", "32"], [31.0, 43.0]), (["32", "55"], [43.0, 31.0])] {
        let sources: BTreeMap<String, GeoJsonSource> = [(
            "places".to_owned(),
            GeoJsonSource::parse(&features(sizes)).unwrap(),
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
        let found: Vec<_> = rendered
            .labels
            .iter()
            .map(|l| l.drawn.map(|b| b[1]))
            .collect();
        assert_eq!(found, tops.map(Some), "sizes {sizes:?}");
    }
}

#[test]
fn the_label_over_a_pixmap_that_does_not_load_is_left_out_and_the_map_drawn() {
    // msGetMarkerSize fails where msPreloadImageSymbol does (src/mapsymbol.c:736-738), and so
    // msAddLabelGroup (src/maplabel.c:350-352) and the map; Forager leaves the label out, with
    // a warning that names the symbol and its image, and draws the rest of the map.
    let mut map = over_pixmap("missing.png", None, None);
    map.layers.push(Layer {
        name: Some("marks".into()),
        classes: vec![Class {
            styles: vec![Style {
                size: Some(Bound::Value(9.0)),
                color: Some(Bound::Value(BLUE)),
                ..Style::default()
            }],
            ..Class::default()
        }],
        ..map.layers[0].clone()
    });
    let fen = collection(&[("Fen", 80.0, 30.0, "")]);
    let sources: BTreeMap<String, GeoJsonSource> = ["places", "marks"]
        .map(|name| (name.to_owned(), GeoJsonSource::parse(&fen).unwrap()))
        .into();
    let rendered = ready(render_with_files(
        &map,
        &sources,
        &Files,
        AggRenderer::new(),
    ));
    assert!(rendered.labels.is_empty(), "{:?}", rendered.labels);
    let label = rendered
        .diagnostics
        .iter()
        .find(|d| d.code == "label-not-drawn")
        .unwrap_or_else(|| panic!("{:?}", rendered.diagnostics));
    assert!(
        label.message.contains("symbol `pin`") && label.message.contains("missing.png"),
        "{}",
        label.message
    );
    let image = rendered.image.unwrap();
    // The square of 9 at the pixel (80, 29).
    assert_eq!(ink(&image, (0, 160), (0, 60)), Some([75, 24, 84, 33]));
}

/// A point layer `name` of a square of `size` pixels in `color` at its points, of
/// `POSTLABELCACHE TRUE` where `post`, labelled with `text` where one is given.
fn post_layer(name: &str, post: bool, size: f64, color: Rgba, text: Option<&str>) -> Layer {
    Layer {
        name: Some(name.into()),
        render_mode: Some(RenderMode::Point),
        status: Some(LayerStatus::On),
        source: Some(SourceRef {
            kind: "local".into(),
            ..SourceRef::default()
        }),
        // On, as the frontend carries a mapfile's POSTLABELCACHE layer: post_label_cache overrides
        // it.
        label_cache: true,
        post_label_cache: post,
        classes: vec![Class {
            styles: vec![Style {
                color: Some(Bound::Value(color)),
                size: Some(Bound::Value(size)),
                ..Style::default()
            }],
            labels: text
                .map(|text| Label {
                    text: Some(Expr::String(text.to_owned())),
                    ..label_item()
                })
                .into_iter()
                .collect(),
            ..Class::default()
        }],
        ..Layer::default()
    }
}

/// The operations of `layers` over one point at the pixel (80, 29), each layer reading it: a
/// square's width for each marker, `L` for each label's text; and the render's cached labels by
/// their layers.
fn post_render(layers: Vec<Layer>) -> (Vec<(char, f64)>, Vec<usize>) {
    let mut map = map(label_item());
    let names: Vec<String> = layers.iter().filter_map(|l| l.name.clone()).collect();
    map.layers = layers;
    let point = r#"{"type": "FeatureCollection", "features": [{"type": "Feature",
        "properties": {}, "geometry": {"type": "Point", "coordinates": [80, 30]}}]}"#;
    let sources: BTreeMap<String, GeoJsonSource> = names
        .into_iter()
        .map(|name| (name, GeoJsonSource::parse(point).unwrap()))
        .collect();
    let recorder = OpRecorder::default();
    let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let ops = recorder
        .0
        .take()
        .iter()
        .map(|op| match op {
            forager_render::DrawOp::Fill { rings, .. } => ('M', rings[0][2].x - rings[0][0].x),
            forager_render::DrawOp::Glyphs { .. } => ('L', 0.0),
            other => panic!("{other:?}"),
        })
        .collect();
    let cached = rendered.labels.iter().map(|label| label.layer).collect();
    (ops, cached)
}

#[test]
fn a_layer_of_postlabelcache_is_drawn_after_the_cached_labels_its_own_at_once() {
    // msDrawMap skips a layer of POSTLABELCACHE TRUE in its first loop (src/mapdraw.c:456),
    // draws the label cache (593), then draws it (604-611) through msDrawLayer, which turns its
    // label cache off (854-856): listed first, its 20-pixel square is drawn after the plain
    // layer's 10-pixel square and that layer's cached label, and its own label at once after its
    // square, never cached.
    let (ops, cached) = post_render(vec![
        post_layer("over", true, 20.0, GREEN, Some("Up")),
        post_layer("under", false, 10.0, BLUE, Some("Fen")),
    ]);
    assert_eq!(ops, [('M', 10.0), ('L', 0.0), ('M', 20.0), ('L', 0.0)]);
    assert_eq!(cached, [1], "the plain layer's label alone is cached");
    // A style document that turns the layer's label cache off draws its labels at once alike, as
    // msDrawLayer turns the cache off for it either way (src/mapdraw.c:850-856).
    let mut over = post_layer("over", true, 20.0, GREEN, Some("Up"));
    over.label_cache = false;
    let (ops, cached) = post_render(vec![
        over,
        post_layer("under", false, 10.0, BLUE, Some("Fen")),
    ]);
    assert_eq!(ops, [('M', 10.0), ('L', 0.0), ('M', 20.0), ('L', 0.0)]);
    assert_eq!(cached, [1]);
}

#[test]
fn layers_of_postlabelcache_are_drawn_after_the_others_in_their_listed_order() {
    // Listed post, plain, post: the plain layer first, then the cache, then the two others as
    // the second loop of msDrawMap takes them, in their order (src/mapdraw.c:604-611).
    let (ops, cached) = post_render(vec![
        post_layer("first", true, 20.0, GREEN, None),
        post_layer("plain", false, 10.0, BLUE, None),
        post_layer("second", true, 30.0, GREEN, None),
    ]);
    assert_eq!(ops, [('M', 10.0), ('M', 20.0), ('M', 30.0)]);
    assert!(cached.is_empty());
}

#[test]
fn the_maps_label_cache_edge_buffer_keeps_a_cached_label_from_the_edge() {
    // msDrawLabelCache hands the map's gutter to labelInImage for every label that may not be
    // partial: "Fen" at (80, 29), its box about 9 pixels high around y 29, is drawn without one
    // and left out with a gutter of 29, which its top crosses.
    let glyphs = |gutter: Option<u32>| {
        let mut map = map(label_item());
        map.label_cache_edge_buffer = gutter;
        let sources: BTreeMap<String, GeoJsonSource> =
            [("places".to_owned(), GeoJsonSource::parse(FEN).unwrap())].into();
        let recorder = GlyphRecorder::default();
        let rendered = ready(render_with_files(&map, &sources, &Files, recorder.clone()));
        (recorder.0.take().len(), rendered.labels.len())
    };
    let (drawn, cached) = glyphs(None);
    assert!(
        drawn > 0 && cached == 1,
        "{drawn} glyph operations, {cached} labels"
    );
    assert_eq!(glyphs(Some(29)).0, 0, "left out inside the gutter");
}

#[test]
fn a_label_offset_past_an_int_is_cast_as_c_casts_it() {
    // Verifies: ADR-0036
    // `(int)` of an `OFFSET` past an `int` gives INT_MIN on x86-64, both where the label binds it
    // and where its offset is scaled: the label stands 2^31 pixels left of the point, where `as`
    // would saturate it to the right. Partial, so that the cache keeps it off the image.
    let drawn = |offset: f64, resolution: Option<f64>| {
        let label = Label {
            offset: Some([Bound::Value(offset), Bound::Value(0.0)]),
            partials: Some(true),
            ..label_item()
        };
        let mut map = map(label);
        map.resolution = resolution;
        let rendered = render(&map, FEN);
        rendered.labels[0].drawn.expect("the label is kept")
    };
    let left = [
        -2_147_483_582.593_75,
        23.046_875,
        -2_147_483_553.406_25,
        34.953_125,
    ];
    assert_eq!(drawn(3e9, None), left, "bound past an int");
    assert_eq!(drawn(-3e9, None), left, "bound below an int");
    // 1.5e9 binds within an int, and doubles past it at a resolution of 144.
    let scaled = drawn(1.5e9, Some(144.0));
    // The label doubles too, its box about the point INT_MIN pixels left.
    assert_eq!(
        scaled,
        [
            -2_147_483_597.187_5,
            17.109_375,
            -2_147_483_538.812_5,
            40.890_625
        ],
        "scaled past an int"
    );
}
