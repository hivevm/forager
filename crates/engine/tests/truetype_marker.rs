//! A TrueType symbol: its glyph read from the render's fonts through the port the symbolizer asks,
//! drawn as MapServer's drawGlyphMarker draws it at a point, anchored, along a line, and tiled over
//! a polygon, and what is reported where the font or the character is missing or the fill's tile
//! is refused (ADR-0044).

use std::path::PathBuf;

use std::collections::BTreeMap;
use std::future::Future;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Expr, Extent, Label, Layer, LayerStatus, Map, Position, RenderMode, Rgba,
    SourceRef, Span, Style, Symbol, SymbolKind, SymbolRef,
};

/// This crate's directory: the run time's `CARGO_MANIFEST_DIR`, else the compiled-in path.
fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

struct Files(BTreeMap<&'static str, Vec<u8>>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

fn files() -> Files {
    let path = manifest_dir().join("../label/tests/fixtures/dejavu-sans-subset.ttf");
    Files([("sans.ttf", std::fs::read(path).unwrap())].into())
}

/// A 41 × 41 image of the extent 0..40 with a point layer at (20, 20) drawing the TrueType symbol
/// of `font` and `character` in black at `SIZE 20`; the font set names `sans`.
fn map(font: &str, character: &str) -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 40.0,
            max_y: 40.0,
        }),
        size: Some([41, 41]),
        image_color: Some(WHITE),
        fonts: [("sans".to_owned(), "sans.ttf".to_owned())]
            .into_iter()
            .collect(),
        symbols: vec![Symbol {
            name: Some("ttf".to_owned()),
            kind: SymbolKind::Truetype,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: Some(character.to_owned()),
            font: Some(font.to_owned()),
            image: None,
            transparent: None,
            span: None,
        }],
        layers: vec![Layer {
            name: Some("points".into()),
            render_mode: Some(RenderMode::Point),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                styles: vec![Style {
                    symbol: Some(Bound::Value(SymbolRef::Index(1))),
                    size: Some(Bound::Value(20.0)),
                    color: Some(Bound::Value(BLACK)),
                    ..Style::default()
                }],
                ..Class::default()
            }],
            span: Some(Span::new("m.map", 3, 1)),
            ..Layer::default()
        }],
        ..Map::default()
    }
}

fn render(map: &Map) -> Rendered {
    render_geometry(map, r#"{"type": "Point", "coordinates": [20, 20]}"#)
}

/// `map` rendered with its layer's one feature of the GeoJSON geometry `geometry`.
fn render_geometry(map: &Map, geometry: &str) -> Rendered {
    let sources = BTreeMap::from([("points".to_owned(), GeoJsonSource::parse(geometry).unwrap())]);
    ready(render_with_files(
        map,
        &sources,
        &files(),
        AggRenderer::new(),
    ))
}

/// How many pixels are not the image colour.
fn drawn(rendered: &Rendered) -> usize {
    let image = rendered.image.as_ref().unwrap();
    image
        .pixels
        .chunks(4)
        .filter(|p| *p != [255, 255, 255, 255])
        .count()
}

fn codes(rendered: &Rendered) -> Vec<&str> {
    rendered.diagnostics.iter().map(|d| &*d.code).collect()
}

#[test]
fn a_truetype_symbol_draws_its_glyph_centred_on_its_point() {
    let rendered = render(&map("sans", "A"));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert!(drawn(&rendered) > 20, "{} pixels drawn", drawn(&rendered));
    // Centred on (20, 20) by its control box: the corners stay white.
    let image = rendered.image.as_ref().unwrap();
    for (x, y) in [(1, 1), (39, 1), (1, 39), (39, 39)] {
        assert_eq!(image.pixel(x, y), Some(WHITE), "({x}, {y})");
    }
}

#[test]
fn a_character_the_font_lacks_draws_glyph_0_and_warns() {
    // Verifies: ADR-0044
    let rendered = render(&map("sans", "☃"));
    assert_eq!(codes(&rendered), ["symbol-glyph-missing"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.contains("symbol `ttf`")
            && message.contains("U+2603")
            && message.contains("glyph 0"),
        "{message}"
    );
    assert!(drawn(&rendered) > 0, "glyph 0 is drawn");
}

#[test]
fn a_font_the_font_set_lacks_draws_nothing_and_warns() {
    // Verifies: ADR-0044
    let rendered = render(&map("serif", "A"));
    assert_eq!(codes(&rendered), ["symbol-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.contains("symbol `ttf`") && message.contains("no font `serif`"),
        "{message}"
    );
    assert_eq!(drawn(&rendered), 0);
}

/// The pixels that are not the image colour within `[x0, x1) × [y0, y1)`.
fn drawn_in(rendered: &Rendered, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> usize {
    let image = rendered.image.as_ref().unwrap();
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .filter(|&(x, y)| image.pixel(x, y) != Some(WHITE))
        .count()
}

#[test]
fn a_truetype_symbol_is_moved_by_its_anchor_point_by_its_character_s_size() {
    // An ANCHORPOINT of 0 0 puts the glyph's control box, the marker's size, with its top left
    // corner on the point (msAdjustMarkerPos): nothing is drawn above or left of it.
    let mut map = map("sans", "A");
    map.symbols[0].anchor_point = Some([0.0, 0.0]);
    let rendered = render(&map);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert!(drawn(&rendered) > 20, "{} pixels drawn", drawn(&rendered));
    assert_eq!(drawn_in(&rendered, (0, 41), (0, 19)), 0, "above the point");
    assert_eq!(
        drawn_in(&rendered, (0, 19), (0, 41)),
        0,
        "left of the point"
    );
}

/// `map` with a label `x` up and right of its point, rendered: the label cache sizes the marker
/// of the class's first style, the TrueType symbol, which the label moves away from. It asserts
/// that nothing is reported and that the label is cached.
fn assert_labelled(mut map: Map) {
    map.layers[0].classes[0].labels = vec![Label {
        text: Some(Expr::String("x".to_owned())),
        font: Some(Bound::Value("sans".to_owned())),
        size: Some(Bound::Value(8.0)),
        color: Some(Bound::Value(BLACK)),
        position: Some(Bound::Value(Position::Ur)),
        span: Some(Span::new("m.map", 9, 7)),
        ..Label::default()
    }];
    let rendered = render(&map);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(rendered.labels.len(), 1, "{:?}", rendered.labels);
}

#[test]
fn the_label_cache_sizes_a_truetype_marker_by_its_character() {
    // A label beside a TrueType marker moves away from the marker's size, which is now computed:
    // the label is drawn, and nothing reports the size as unknown.
    assert_labelled(map("sans", "A"));
}

#[test]
fn the_label_cache_sizes_a_truetype_marker_outside_its_scale_range() {
    // msAddLabel sizes the class's first style by msGetMarkerSize whatever its scale range, so its
    // font is loaded although the style draws nothing at this scale: the label is cached.
    let mut map = map("sans", "A");
    map.layers[0].classes[0].styles[0].max_scale_denom = Some(1.0);
    assert_labelled(map);
}

#[test]
fn a_truetype_symbol_with_a_gap_is_drawn_along_a_line() {
    let mut map = map("sans", "A");
    map.layers[0].render_mode = Some(RenderMode::Line);
    let style = &mut map.layers[0].classes[0].styles[0];
    style.size = Some(Bound::Value(8.0));
    style.gap = Some(-12.0);
    let rendered = render_geometry(
        &map,
        r#"{"type": "LineString", "coordinates": [[0, 20], [40, 20]]}"#,
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    // Glyphs every 12 pixels from 6 along the line, at 20 down: none far above or below it.
    assert!(drawn(&rendered) > 20, "{} pixels drawn", drawn(&rendered));
    assert_eq!(drawn_in(&rendered, (0, 41), (0, 12)), 0, "above the line");
    assert_eq!(drawn_in(&rendered, (0, 41), (29, 41)), 0, "below the line");
}

/// `map` of a polygon layer whose TrueType style has `gap`, over a square of 4..36 a side.
fn render_fill(gap: f64) -> Rendered {
    let mut map = map("sans", "A");
    map.layers[0].render_mode = Some(RenderMode::Polygon);
    let style = &mut map.layers[0].classes[0].styles[0];
    style.size = Some(Bound::Value(8.0));
    style.gap = Some(gap);
    render_geometry(
        &map,
        r#"{"type": "Polygon", "coordinates": [[[4, 4], [36, 4], [36, 36], [4, 36], [4, 4]]]}"#,
    )
}

#[test]
fn a_truetype_symbol_fills_a_polygon_with_its_glyph_tiled() {
    let rendered = render_fill(0.0);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert!(drawn(&rendered) > 50, "{} pixels drawn", drawn(&rendered));
    assert_eq!(
        drawn_in(&rendered, (0, 41), (0, 3)),
        0,
        "outside the polygon"
    );
}

#[test]
fn a_truetype_fill_whose_tile_passes_the_bound_warns_and_draws_no_fill() {
    // Verifies: Q-9
    // A GAP of 5000 asks a tile of 5000 × 5000 pixels, past 4096 a side: nothing is allocated
    // for it, and the polygon is not filled.
    let rendered = render_fill(5000.0);
    assert_eq!(codes(&rendered), ["symbol-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(
        message.contains("symbol `ttf`") && message.contains("5000 × 5000"),
        "{message}"
    );
    assert_eq!(drawn(&rendered), 0);
}

#[test]
fn each_layer_whose_fill_lacks_its_character_warns_though_the_tile_is_held() {
    // Two polygon layers fill with the same TrueType symbol, whose font lacks the character: the
    // second finds the first's tile held, and warns as the first does.
    let mut map = map("sans", "☃");
    map.layers[0].render_mode = Some(RenderMode::Polygon);
    map.layers[0].classes[0].styles[0].size = Some(Bound::Value(8.0));
    let mut second = map.layers[0].clone();
    second.name = Some("again".into());
    map.layers.push(second);
    let square =
        r#"{"type": "Polygon", "coordinates": [[[4, 4], [36, 4], [36, 36], [4, 36], [4, 4]]]}"#;
    let sources = BTreeMap::from(
        ["points", "again"].map(|name| (name.to_owned(), GeoJsonSource::parse(square).unwrap())),
    );
    let rendered = ready(render_with_files(
        &map,
        &sources,
        &files(),
        AggRenderer::new(),
    ));
    assert_eq!(
        codes(&rendered),
        ["symbol-glyph-missing", "symbol-glyph-missing"]
    );
    assert!(drawn(&rendered) > 0, "glyph 0 is tiled");
}

#[test]
fn a_truetype_symbol_with_an_outline_wider_than_any_stroke_renders_without_a_panic() {
    // Verifies: ADR-0025
    // The glyph's outline is bounded as a stroke's width is, so that its contour stays within
    // agg-rust's fixed-point coordinates. As computeSymbolStyle, a marker's outline is as wide as
    // the style's `WIDTH`, not its `OUTLINEWIDTH`, and bounded by its `MAXWIDTH`, 32 by default;
    // both are as wide here, so that the width reaches the renderer.
    let mut map = map("sans", "A");
    let style = &mut map.layers[0].classes[0].styles[0];
    style.outline_color = Some(Bound::Value(Rgba::opaque(200, 30, 30)));
    style.width = Some(Bound::Value(1e12));
    style.max_width = Some(1e12);
    let rendered = render(&map);
    assert!(rendered.image.is_some(), "{:?}", rendered.diagnostics);
}

#[test]
fn a_truetype_marker_whose_glyph_passes_the_reach_is_not_drawn_and_warns_once() {
    // Verifies: ADR-0027, ADR-0049, Q-3
    // Moved 4,194,400 pixels right by its `OFFSET`, the `A` of 20 points lies past
    // `MAX_GLYPH_REACH` of the image, 4,194,304 pixels, so the renderer draws none of it. Two
    // such markers of the layer warn once.
    let mut map = map("sans", "A");
    let style = &mut map.layers[0].classes[0].styles[0];
    style.offset = Some([Bound::Value(4_194_400.0), Bound::Value(0.0)]);
    let two = r#"{"type": "MultiPoint", "coordinates": [[20, 20], [10, 10]]}"#;
    for geometry in [r#"{"type": "Point", "coordinates": [20, 20]}"#, two] {
        let rendered = render_geometry(&map, geometry);
        assert_eq!(
            codes(&rendered),
            ["symbol-not-drawn"],
            "{:?}",
            rendered.diagnostics
        );
        let warning = &rendered.diagnostics[0];
        assert!(warning.message.contains("`points`"), "{}", warning.message);
        assert_eq!(warning.span, Some(Span::new("m.map", 3, 1)));
        assert_eq!(drawn(&rendered), 0);
    }
}
