use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::{LayerStatus, RenderMode, Rgba, Severity, Span};

use super::{Loaded, load};

/// Files in memory, keyed by their path inside the base context.
struct Files(BTreeMap<&'static str, &'static str>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn load_files(files: &[(&'static str, &'static str)]) -> Loaded {
    let loader = Files(files.iter().copied().collect());
    block_on(load(&ResourcePath::new(files[0].0).unwrap(), &loader))
}

fn codes(loaded: &Loaded) -> Vec<&str> {
    loaded.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

const WORLD: &str = r##"MAP
  NAME "world"
  EXTENT -180 -90 180 90
  SIZE 400 200
  IMAGECOLOR 255 255 255
  PROJECTION
    "proj=longlat"
    "datum=WGS84"
  END
  LAYER
    NAME "countries"
    TYPE POLYGON
    STATUS ON
    DATA "countries.shp"
    CLASS
      NAME "all"
      STYLE
        COLOR 200 220 180
        OUTLINECOLOR "#000000"
        WIDTH 0.5
      END
    END
  END
END
"##;

#[test]
fn a_mapfile_translates_into_the_style_ir() {
    // Verifies: G-2
    let loaded = load_files(&[("world.map", WORLD)]);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let map = loaded.document.unwrap().map;
    assert_eq!(map.name.as_deref(), Some("world"));
    assert_eq!(map.size, Some([400, 200]));
    assert_eq!(map.image_color, Some(Rgba::opaque(255, 255, 255)));
    assert_eq!(
        map.projection.as_deref(),
        Some("+proj=longlat +datum=WGS84")
    );
    let layer = &map.layers[0];
    assert_eq!(layer.name.as_deref(), Some("countries"));
    assert_eq!(layer.render_mode, Some(RenderMode::Polygon));
    assert_eq!(layer.status, Some(LayerStatus::On));
    let source = layer.source.as_ref().unwrap();
    assert_eq!(source.kind, "local");
    assert_eq!(source.props["data"], "countries.shp");
    let style = &layer.classes[0].styles[0];
    assert_eq!(style.color, Some(Rgba::opaque(200, 220, 180)));
    assert_eq!(style.outline_color, Some(Rgba::opaque(0, 0, 0)));
    assert_eq!(style.width, Some(0.5));
    assert_eq!(style.span, Some(Span::new("world.map", 17, 7)));
}

#[test]
fn keywords_not_yet_supported_and_out_of_scope_are_warnings() {
    let loaded = load_files(&[(
        "m.map",
        "MAP\n  WEB TEMPLATE 'x.html' END\n  LAYER NAME 'a' LABELITEM 'n' END\nEND\n",
    )]);
    assert!(loaded.document.is_some());
    assert_eq!(codes(&loaded), ["discarded-keyword", "unsupported-keyword"]);
    assert!(
        loaded
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
    assert_eq!(loaded.diagnostics[1].span, Some(Span::new("m.map", 3, 18)));
}

#[test]
fn a_query_layer_is_discarded() {
    let loaded = load_files(&[("m.map", "MAP LAYER NAME 'q' TYPE QUERY END END")]);
    assert!(loaded.document.as_ref().unwrap().map.layers.is_empty());
    assert_eq!(codes(&loaded), ["discarded-layer"]);
}

#[test]
fn an_include_is_spliced_in_with_its_own_spans() {
    let loaded = load_files(&[
        ("m.map", "MAP\n  INCLUDE 'layers/a.inc'\nEND\n"),
        ("layers/a.inc", "LAYER NAME 'a' END\n"),
    ]);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let layer = &loaded.document.unwrap().map.layers[0];
    assert_eq!(layer.span, Some(Span::new("layers/a.inc", 1, 1)));
}

#[test]
fn an_include_outside_the_base_context_is_refused() {
    // Verifies: Q-8
    for target in ["../secret.map", "/etc/passwd", "C:\\secret.map"] {
        let text: &'static str = format!("MAP INCLUDE '{target}' END").leak();
        let loaded = load_files(&[("m.map", text)]);
        assert!(loaded.document.is_none(), "{target}");
        assert_eq!(codes(&loaded), ["include-refused"], "{target}");
    }
}

#[test]
fn include_cycles_and_deep_nesting_are_errors() {
    // Verifies: Q-9
    let cycle = load_files(&[("m.map", "MAP INCLUDE 'a' END"), ("a", "INCLUDE 'm.map'")]);
    assert_eq!(codes(&cycle), ["include-cycle"]);

    let deep = load_files(&[
        ("m.map", "MAP INCLUDE '1' END"),
        ("1", "INCLUDE '2'"),
        ("2", "INCLUDE '3'"),
        ("3", "INCLUDE '4'"),
        ("4", "INCLUDE '5'"),
        ("5", "INCLUDE '6'"),
        ("6", "NAME 'x'"),
    ]);
    assert_eq!(codes(&deep), ["include-too-deep"]);

    let five = load_files(&[
        ("m.map", "MAP INCLUDE '1' END"),
        ("1", "INCLUDE '2'"),
        ("2", "INCLUDE '3'"),
        ("3", "INCLUDE '4'"),
        ("4", "INCLUDE '5'"),
        ("5", "NAME 'x'"),
    ]);
    assert!(five.diagnostics.is_empty(), "{:?}", five.diagnostics);
}

#[test]
fn syntax_errors_fail_the_load_with_a_span() {
    let loaded = load_files(&[("m.map", "MAP\n  NAME \"unterminated\n")]);
    assert!(loaded.document.is_none());
    assert_eq!(
        loaded.diagnostics[0].span,
        Some(Span::new("m.map", 2, 8)),
        "{:?}",
        loaded.diagnostics
    );
}

#[test]
fn an_init_projection_without_a_resolver_is_a_load_error() {
    let loaded = load_files(&[("m.map", "MAP PROJECTION 'init=epsg:4326' END END")]);
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["projection-unresolved"]);
}

#[test]
fn invalid_values_are_errors() {
    let loaded = load_files(&[("m.map", "MAP SIZE 400 END")]);
    assert!(loaded.document.is_none());
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn colours_of_minus_one_mean_no_colour() {
    let loaded = load_files(&[(
        "m.map",
        "MAP LAYER CLASS STYLE COLOR -1 -1 -1 OUTLINECOLOR 1 2 3 END END END END",
    )]);
    let style = &loaded.document.unwrap().map.layers[0].classes[0].styles[0];
    assert_eq!(style.color, None);
    assert_eq!(style.outline_color, Some(Rgba::opaque(1, 2, 3)));
}

#[test]
fn an_include_without_a_string_keeps_the_next_token() {
    let loaded = load_files(&[("m.map", "MAP INCLUDE END")]);
    assert_eq!(codes(&loaded), ["invalid-value"]);
}

#[test]
fn an_unknown_keyword_is_reported_once() {
    let loaded = load_files(&[("m.map", "MAP FROBNICATE 1 END")]);
    assert_eq!(codes(&loaded), ["unknown-keyword"]);
}
