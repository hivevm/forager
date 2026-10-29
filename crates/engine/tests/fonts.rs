//! The fonts of a render's labels, loaded through the host's resource loader (ADR-0028).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{MAX_FONT_BYTES, Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Label, Layer, LayerStatus, Map, RenderMode, SourceRef, Span,
};

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// Files in memory, recording every path asked for.
struct Files {
    files: BTreeMap<&'static str, Vec<u8>>,
    asked: RefCell<Vec<String>>,
}

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.asked.borrow_mut().push(path.as_str().to_owned());
        self.files
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

/// A map of two point layers whose labels name `font`, with the font set `fonts`.
fn map(font: &str, fonts: &[(&str, &str)]) -> Map {
    let layer = |name: &str| Layer {
        name: Some(name.into()),
        render_mode: Some(RenderMode::Point),
        status: Some(LayerStatus::On),
        source: Some(SourceRef {
            kind: "local".into(),
            ..SourceRef::default()
        }),
        classes: vec![Class {
            labels: vec![Label {
                font: Some(Bound::Value(font.to_owned())),
                span: Some(Span::new("m.map", 7, 5)),
                ..Label::default()
            }],
            ..Class::default()
        }],
        ..Layer::default()
    };
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        }),
        size: Some([10, 10]),
        fonts: fonts
            .iter()
            .map(|(name, path)| ((*name).to_owned(), (*path).to_owned()))
            .collect(),
        layers: vec![layer("a"), layer("b")],
        ..Map::default()
    }
}

fn render(map: &Map, files: &Files) -> Rendered {
    let point = r#"{"type": "Point", "coordinates": [5, 5]}"#;
    let sources: BTreeMap<String, GeoJsonSource> = ["a", "b"]
        .into_iter()
        .map(|name| (name.to_owned(), GeoJsonSource::parse(point).unwrap()))
        .collect();
    ready(render_with_files(map, &sources, files, AggRenderer::new()))
}

fn files(files: &[(&'static str, Vec<u8>)]) -> Files {
    Files {
        files: files.iter().cloned().collect(),
        asked: RefCell::new(Vec::new()),
    }
}

fn codes(rendered: &Rendered) -> Vec<&str> {
    rendered
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect()
}

#[test]
fn a_font_is_loaded_once_per_render_through_the_loader() {
    // Verifies: ADR-0028
    let files = files(&[("fonts/sans.ttf", b"font".to_vec())]);
    let rendered = render(&map("sans", &[("sans", "fonts/sans.ttf")]), &files);
    assert_eq!(codes(&rendered), Vec::<&str>::new());
    assert_eq!(*files.asked.borrow(), ["fonts/sans.ttf"]);
}

#[test]
fn the_first_usable_entry_of_the_font_list_is_taken_as_get_face_for_run_tries_them() {
    // Verifies: ADR-0028
    // An entry the font set lacks is skipped; an `xx:` prefix is stripped; a later entry is not
    // loaded once one is usable.
    let files = files(&[("serif.ttf", b"font".to_vec())]);
    let rendered = render(
        &map("missing,de:serif,gone", &[("serif", "serif.ttf")]),
        &files,
    );
    assert_eq!(codes(&rendered), Vec::<&str>::new());
    assert_eq!(*files.asked.borrow(), ["serif.ttf"]);
    // An entry prefixed `en:` is tried before the list; a name matches without regard to case.
    let files = self::files(&[("a.ttf", b"a".to_vec()), ("b.ttf", b"b".to_vec())]);
    let rendered = render(&map("a,en:B", &[("a", "a.ttf"), ("b", "b.ttf")]), &files);
    assert_eq!(codes(&rendered), Vec::<&str>::new());
    assert_eq!(*files.asked.borrow(), ["b.ttf"]);
    // A name with a colon that the font set holds as written is found after its stripped form.
    let files = self::files(&[("xy.ttf", b"font".to_vec())]);
    let rendered = render(&map("x:y", &[("x:y", "xy.ttf")]), &files);
    assert_eq!(codes(&rendered), Vec::<&str>::new());
}

#[test]
fn a_label_none_of_whose_fonts_can_be_used_is_not_drawn_with_a_diagnostic() {
    // Verifies: ADR-0028
    let files = files(&[]);
    let rendered = render(
        &map(
            "gone1,gone2",
            &[("gone1", "gone1.ttf"), ("gone2", "gone2.ttf")],
        ),
        &files,
    );
    // One for each label; each font is asked for once in the render, its failure kept.
    assert_eq!(
        codes(&rendered),
        ["label-font-unusable", "label-font-unusable"]
    );
    let d = &rendered.diagnostics[0];
    assert!(
        d.message.contains("`gone2`") && d.message.contains("gone2.ttf"),
        "{}",
        d.message
    );
    assert_eq!(d.span, Some(Span::new("m.map", 7, 5)));
    assert_eq!(*files.asked.borrow(), ["gone1.ttf", "gone2.ttf"]);
}

#[test]
fn a_file_the_font_set_names_twice_is_loaded_once() {
    // Verifies: ADR-0028
    let files = files(&[("same.ttf", b"font".to_vec())]);
    let fonts = [("one", "same.ttf"), ("two", "same.ttf")];
    let mut map = map("one", &fonts);
    map.layers[1].classes[0].labels[0].font = Some(Bound::Value("two".to_owned()));
    let rendered = render(&map, &files);
    assert_eq!(codes(&rendered), Vec::<&str>::new());
    assert_eq!(*files.asked.borrow(), ["same.ttf"]);
}

#[test]
fn a_font_past_the_size_bound_or_outside_the_base_context_is_not_used() {
    // Verifies: ADR-0028
    // Verifies: Q-9
    let files = files(&[("big.ttf", vec![0; MAX_FONT_BYTES + 1])]);
    let rendered = render(&map("big", &[("big", "big.ttf")]), &files);
    assert_eq!(
        codes(&rendered),
        ["label-font-unusable", "label-font-unusable"]
    );
    assert!(
        rendered.diagnostics[0]
            .message
            .contains("exceeds the limit")
    );
    assert_eq!(
        *files.asked.borrow(),
        ["big.ttf"],
        "the failure is kept for the render"
    );
    let rendered = render(&map("out", &[("out", "../out.ttf")]), &files);
    assert_eq!(codes(&rendered)[0], "label-font-unusable");
    assert!(
        rendered.diagnostics[0].message.contains("base context"),
        "{}",
        rendered.diagnostics[0].message
    );
}
