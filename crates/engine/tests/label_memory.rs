//! The memory the labels of a render take (Q-9): "A label the label cache holds keeps at most
//! 400 bytes apart from its text, and at most 32 more for each style value a feature binds to
//! it; a test measures it." What a render holds per label until the labels are drawn after the
//! last layer is measured under a counting allocator, as the difference between a render of many
//! labelled points and the same render with no label, its text counted with it, so that the test
//! is stricter than Q-9. One test, so that no other test's allocations are counted with it.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::render_with_files;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{DisplayItem, Draw, Renderer, RgbaImage};
use forager_style_ir::{
    Bound, Class, Expr, Extent, Force, GeomTransform, Label, Layer, LayerStatus, Map, Op,
    RenderMode, Rgba, SourceRef, Style, Symbol, SymbolKind, SymbolRef,
};
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The test font of the label crate, a subset of DejaVu Sans whose licence lies beside it, and
/// `pin.png`, the test icon of the command line, 32 × 32.
struct Files;

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path.as_str() == "pin.png" {
            return Ok(include_bytes!("../../cli/tests/fixtures/pin.png").to_vec());
        }
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
        Ok(std::fs::read(file).unwrap())
    }
}

/// A renderer that keeps nothing of what it is handed, so that only the engine's memory counts.
struct Discard(usize);

impl Draw for Discard {
    fn draw(&mut self, item: &DisplayItem) {
        self.0 += usize::from(matches!(item.op, forager_render::DrawOp::Glyphs { .. }));
    }
}

/// A layer's items are drawn as if onto the map.
impl Renderer for Discard {
    type Layer<'a> = &'a mut Self;

    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}
    fn layer(&mut self, _: u8) -> &mut Self {
        self
    }
    fn finish(self) -> RgbaImage {
        RgbaImage {
            width: 0,
            height: 0,
            pixels: (self.0 as u64).to_le_bytes().to_vec(),
        }
    }
}

/// The map of the points, its label drawn where `labelled`, with the style `styled` where one is
/// given: a `labelpnt` or `labelcenter` style of symbol 0, which is sized 1 by 1 and draws
/// nothing, unless the style names symbol 1, a square its size scales.
fn map(labelled: bool, styled: Option<Style>) -> Map {
    let styles = styled.into_iter().collect();
    let label = Label {
        styles,
        text: labelled.then(|| Expr::call(Op::ToString, vec![Expr::property("name")])),
        font: Some(Bound::Value("sans".to_owned())),
        // Forced, so that every label is drawn and held by the label cache, though they overlap.
        force: Some(Force::On),
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
        // Symbol 1, a vector square of side 1, which a style's SIZE scales; symbol 2, the pin.
        symbols: vec![
            Symbol {
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
            },
            Symbol {
                kind: SymbolKind::Pixmap,
                paths: Vec::new(),
                extent: None,
                filled: false,
                name: Some("pin".to_owned()),
                anchor_point: None,
                character: None,
                font: None,
                image: Some("pin.png".to_owned()),
                transparent: None,
                span: None,
            },
        ],
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
    // At a count just past a power of two, where a list that doubled as it grew would hold up to
    // three times what it fills while it moves; at 16,027, just past a step of the label cache's
    // lists, which grow by a quarter as they fill, where they hold the most past what they fill;
    // and at one well past both.
    for points in [16_027, 16_385, 20_000] {
        let sources = places(points);
        styled_labels(points, &sources);
        pinned_labels(points, &sources);
        alternating_classes(points, &sources);
    }
    labels_over_two_layers();
    labels_most_features_refuse();
    a_long_text_is_held_once_for_all_its_points();
}

/// The source of `points` labelled points inside the map, each with values for its styles.
fn places(points: usize) -> BTreeMap<String, GeoJsonSource> {
    [(
        "places".to_owned(),
        source((0..points).map(place).collect()),
    )]
    .into()
}

/// The `i`th labelled point inside the map, with values for its styles, `kept` 1 for one in ten
/// and 0 for the others, and `odd` 1 for every other.
fn place(i: usize) -> String {
    format!(
        r#"{{"type": "Feature", "properties": {{"name": "Forager {i}",
            "color": "{} 255 0", "size": "{}", "width": "{}", "kept": "{}", "odd": "{}"}},
            "geometry": {{"type": "Point", "coordinates": [{}, {}]}}}}"#,
        i % 256,
        i % 7 + 1,
        i % 5 + 1,
        u8::from(i.is_multiple_of(10)),
        i % 2,
        i % 1000,
        (i / 1000) * 40
    )
}

/// The source of the GeoJSON features `features`.
fn source(features: Vec<String>) -> GeoJsonSource {
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    );
    GeoJsonSource::parse(&geojson).unwrap()
}

fn bound<T>(name: &str) -> Bound<T> {
    Bound::Binding {
        binding: name.to_owned(),
    }
}

/// What each of `points` labels holds, without a style and with each kind of style.
fn styled_labels(points: usize, sources: &BTreeMap<String, GeoJsonSource>) {
    let (without, none) = peak(&map(false, None), sources);
    assert_eq!(none, 0);
    // About 290 to 360 bytes: the label's text, its glyph indices, its line, its box, its point,
    // and its place in the label cache, whose lists grow by a quarter as they fill; its path, of
    // thousands of bytes, is built only as it is drawn.
    // A `labelpoly` style the same for every feature is read from the map as the label is drawn,
    // and adds nothing a label holds; nor does a `labelpnt` style, whose marker the label's
    // definition holds and whose bounds are computed at each label's point as the cache tests it,
    // nor a `labelcenter` style, whose marker is placed only as the label is drawn. A style whose
    // properties are bound to the feature's adds the values bound to it, each feature's its own;
    // a `labelpnt` marker those values size is computed from them as the cache tests it.
    let style = |transform, color: Bound<Rgba>| Style {
        color: Some(color),
        geom_transform: Some(transform),
        ..Style::default()
    };
    let fixed = || Bound::Value(Rgba::opaque(0, 255, 0));
    // Each style, and the values a feature binds to it.
    for (styled, values) in [
        (None, 0),
        (Some(style(GeomTransform::LabelPoly, fixed())), 0),
        (Some(style(GeomTransform::LabelPoint, fixed())), 0),
        (Some(style(GeomTransform::LabelCenter, fixed())), 0),
        (Some(style(GeomTransform::LabelPoly, bound("color"))), 1),
        (Some(style(GeomTransform::LabelPoint, bound("color"))), 1),
        (
            Some(Style {
                size: Some(bound("size")),
                ..style(GeomTransform::LabelPoly, fixed())
            }),
            1,
        ),
        // A marker of symbol 1 whose size is each feature's own.
        (
            Some(Style {
                size: Some(bound("size")),
                symbol: Some(Bound::Value(SymbolRef::Index(1))),
                ..style(GeomTransform::LabelPoint, fixed())
            }),
            1,
        ),
        (
            Some(Style {
                width: Some(bound("width")),
                ..style(GeomTransform::LabelPoly, bound("color"))
            }),
            2,
        ),
    ] {
        let (with, drawn) = peak(&map(true, styled.clone()), sources);
        assert_eq!(drawn, points, "every label is drawn");
        let per_label = with.saturating_sub(without) / points;
        assert!(
            per_label < 400 + 32 * values,
            "{points} labels, styled {styled:?}: {per_label} bytes per label ({with} with \
             labels, {without} without)"
        );
    }
}

/// What each of `points` labels holds over the pin under it, its class's first style, sized from
/// the image the render holds once, as every point draws it: the label's definition holds its
/// marker where the style is the same for every feature, and each label its own where the
/// feature binds its size: an entry of 24 bytes, its index and its width and height, in a list
/// that grows by a quarter as it fills, held here to twice that.
fn pinned_labels(points: usize, sources: &BTreeMap<String, GeoJsonSource>) {
    let pinned = |labelled, size: Option<Bound<f64>>| {
        let mut map = map(labelled, None);
        map.layers[0].classes[0].styles = vec![Style {
            symbol: Some(Bound::Value(SymbolRef::Index(2))),
            size,
            ..Style::default()
        }];
        map
    };
    for (size, marker) in [(None, 0), (Some(bound("size")), 2 * 24)] {
        let (without, none) = peak(&pinned(false, size.clone()), sources);
        assert_eq!(none, 0);
        let (with, drawn) = peak(&pinned(true, size.clone()), sources);
        assert_eq!(drawn, points, "every label is drawn");
        let per_label = with.saturating_sub(without) / points;
        assert!(
            per_label < 400 + marker,
            "{points} labels pinned at {size:?}: {per_label} bytes per label ({with} with \
             labels, {without} without)"
        );
    }
}

/// What each of `points` labels holds where the features alternate between two classes, each
/// with a label whose style binds a colour, so that no two labels in a row share a definition
/// and each starts a run of its own.
fn alternating_classes(points: usize, sources: &BTreeMap<String, GeoJsonSource>) {
    let alternating = |labelled| {
        let mut map = map(
            labelled,
            Some(Style {
                color: Some(bound("color")),
                geom_transform: Some(GeomTransform::LabelPoly),
                ..Style::default()
            }),
        );
        let odd = |value: &str| {
            Some(Expr::call(
                Op::Eq,
                vec![
                    Expr::call(Op::ToString, vec![Expr::property("odd")]),
                    value.into(),
                ],
            ))
        };
        let mut second = map.layers[0].classes[0].clone();
        map.layers[0].classes[0].expression = odd("0");
        second.expression = odd("1");
        map.layers[0].classes.push(second);
        map
    };
    let (without, none) = peak(&alternating(false), sources);
    assert_eq!(none, 0);
    let (with, drawn) = peak(&alternating(true), sources);
    assert_eq!(drawn, points, "every label is drawn");
    let per_label = with.saturating_sub(without) / points;
    assert!(
        per_label < 400 + 32,
        "{points} labels of alternating classes: {per_label} bytes per label ({with} with \
         labels, {without} without)"
    );
}

/// What each label holds where a layer's labels fill the label cache's lists to a power of two
/// and a second layer adds one label, so that the lists grow across layers: 16,384 labels and
/// one, without a style and with a style that binds two values.
fn labels_over_two_layers() {
    const FIRST: usize = 16_384;
    let mut sources = places(FIRST);
    sources.insert("second".to_owned(), source(vec![place(FIRST)]));
    let styled = Style {
        color: Some(bound("color")),
        width: Some(bound("width")),
        geom_transform: Some(GeomTransform::LabelPoly),
        ..Style::default()
    };
    for (styled, values) in [(None, 0), (Some(styled), 2)] {
        let two_layers = |labelled| {
            let mut map = map(labelled, styled.clone());
            let mut second = map.layers[0].clone();
            second.name = Some("second".into());
            map.layers.push(second);
            map
        };
        let (without, none) = peak(&two_layers(false), &sources);
        assert_eq!(none, 0);
        let (with, drawn) = peak(&two_layers(true), &sources);
        assert_eq!(drawn, FIRST + 1, "every label is drawn");
        let per_label = with.saturating_sub(without) / drawn;
        assert!(
            per_label < 400 + 32 * values,
            "two layers, styled {styled:?}: {per_label} bytes per label ({with} with labels, \
             {without} without)"
        );
    }
}

/// What each label the label cache holds keeps where the label's `EXPRESSION` refuses most
/// features: all but one in ten of 20,000 points, and a MultiPoint of 100,000 points inside the
/// map. A feature refused holds nothing, nor any room for its label. One in ten, so that what
/// the render holds once for its labels, the font among it, counts little per label.
fn labels_most_features_refuse() {
    const POINTS: usize = 20_000;
    const VERTICES: usize = 100_000;
    let mut features: Vec<String> = (0..POINTS).map(place).collect();
    let coordinates: Vec<String> = (0..VERTICES)
        .map(|i| format!("[{}, {}]", i % 1000, i / 1000 * 10))
        .collect();
    features.push(format!(
        r#"{{"type": "Feature", "properties": {{"name": "Refused", "kept": "0"}},
            "geometry": {{"type": "MultiPoint", "coordinates": [{}]}}}}"#,
        coordinates.join(",")
    ));
    let sources = [("places".to_owned(), source(features))].into();
    let refusing = |labelled| {
        let mut map = map(labelled, None);
        map.layers[0].classes[0].labels[0].expression = Some(Expr::call(
            Op::Eq,
            vec![
                Expr::call(Op::ToString, vec![Expr::property("kept")]),
                "1".into(),
            ],
        ));
        map
    };
    let (without, none) = peak(&refusing(false), &sources);
    assert_eq!(none, 0);
    let (with, drawn) = peak(&refusing(true), &sources);
    assert_eq!(
        drawn,
        POINTS / 10,
        "the labels of the features kept are drawn"
    );
    let per_label = with.saturating_sub(without) / drawn;
    assert!(
        per_label < 400,
        "{drawn} labels of {POINTS} points and {VERTICES} refused: {per_label} bytes per label \
         ({with} with labels, {without} without)"
    );
}

/// A label of a long text on a feature of many points, with and without `MINDISTANCE`: the render
/// holds its text once for the feature, not once for each point, in the label cache and in the
/// labels it reports, which share it (Q-9, the convention that memory follows the input's size).
/// What the long text adds over a text of one word is measured, so that what each point holds
/// apart from its text, which Q-9 bounds, is not counted with it, and held to four times the
/// input's size, the maintainer's bound; a copy of the text for each point would be over a
/// thousand times its size. The text and the points are few enough that such a copy, of 160 MiB,
/// fails the assertion rather than exhaust the memory of the machine that runs the test. The
/// coordinates outweigh the text here, so the bound leaves room for some copies of it besides.
fn a_long_text_is_held_once_for_all_its_points() {
    const VERTICES: usize = 10_000;
    const TEXT: usize = 1 << 14;
    // Words of a glyph each, so that the text is laid out as any other.
    let long = "Forager ".repeat(TEXT / 8);
    for min_distance in [None, Some(10.0)] {
        let (short, _) = held(VERTICES, "Forager", min_distance);
        let (with, input) = held(VERTICES, &long, min_distance);
        let kept = with.saturating_sub(short);
        assert!(
            kept <= 4 * input,
            "MINDISTANCE {min_distance:?}: {kept} bytes for the text, for an input of {input} bytes"
        );
    }
}

/// The peak of the bytes a render allocates over the source for a label of `text` on a MultiPoint
/// of `vertices` points inside the map, unforced, which reports a label at each point, all of
/// them sharing the text; and the size of the input, the source's GeoJSON.
fn held(vertices: usize, text: &str, min_distance: Option<f64>) -> (usize, usize) {
    let coordinates: Vec<String> = (0..vertices)
        .map(|i| format!("[{}, {}]", i % 1000, i / 1000 * 10))
        .collect();
    let geojson = format!(
        r#"{{"type": "Feature", "properties": {{"name": "{text}"}},
            "geometry": {{"type": "MultiPoint", "coordinates": [{}]}}}}"#,
        coordinates.join(",")
    );
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let mut map = map(true, None);
    let label = &mut map.layers[0].classes[0].labels[0];
    label.force = None;
    label.min_distance = min_distance;
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let rendered = ready(render_with_files(&map, &sources, &Files, Discard(0)));
    let labels = &rendered.labels;
    assert_eq!(labels.len(), vertices, "{:?}", rendered.diagnostics);
    assert!(labels.iter().all(|l| Arc::ptr_eq(&l.text, &labels[0].text)));
    assert_eq!(&*labels[0].text, text);
    drop(rendered);
    (ALLOCATOR.peak_usage() - before, geojson.len())
}
