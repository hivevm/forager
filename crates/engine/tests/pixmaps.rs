//! A pixmap symbol drawn at a point, end to end: a style IR, its image through the resource
//! loader, and GeoJSON in, an RGBA image out, compared with a reference image of Forager's own
//! (docs/plan/pixmap-symbols/spec.md, Testing decisions). The positions are read from
//! `agg2RenderPixmapSymbol`, src/mapagg.cpp, at the pin, not from the engine.

use std::collections::BTreeMap;
use std::future::Future;

mod bless;

use bless::assert_matches_reference;
use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Span, Style,
    Symbol, SymbolKind, SymbolRef,
};

const WHITE: Rgba = Rgba::opaque(255, 255, 255);
const GREEN: Rgba = Rgba::opaque(200, 220, 180);
const BLUE: Rgba = Rgba::opaque(30, 60, 200);

/// The test icon, 32 × 32 RGBA: pixel (x, y) is (8x, 8y, 160), opaque in its top half, of alpha
/// 128 in the left of its bottom half, and transparent in the right.
const PIN: &[u8] = include_bytes!("../../cli/tests/fixtures/pin.png");

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// Files in memory.
#[derive(Default)]
struct Files(BTreeMap<&'static str, Vec<u8>>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

/// A pixmap symbol named `name` whose image is `path`.
fn pixmap(name: &str, path: &str) -> Symbol {
    Symbol {
        name: Some(name.to_owned()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: Some(path.to_owned()),
        transparent: None,
        span: None,
    }
}

/// A style that draws the symbol at `index`.
fn symbol_style(index: u32) -> Style {
    Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        ..Style::default()
    }
}

fn layer(name: &str, mode: RenderMode, styles: Vec<Style>) -> Layer {
    Layer {
        name: Some(name.into()),
        render_mode: Some(mode),
        status: Some(LayerStatus::On),
        source: Some(SourceRef {
            kind: "local".into(),
            ..SourceRef::default()
        }),
        classes: vec![Class {
            styles,
            ..Class::default()
        }],
        span: Some(Span::new("m.map", 2, 1)),
        ..Layer::default()
    }
}

/// A 101 × 101 image of the extent 0..100, one map unit a pixel, with `symbols` and `layers`.
fn map(symbols: Vec<Symbol>, layers: Vec<Layer>) -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        }),
        size: Some([101, 101]),
        image_color: Some(WHITE),
        symbols,
        layers,
        ..Map::default()
    }
}

/// Renders `map` with `files`, each layer's GeoJSON by its name.
fn render(map: &Map, files: &Files, sources: &[(&str, &str)]) -> Rendered {
    let sources: BTreeMap<String, GeoJsonSource> = sources
        .iter()
        .map(|(name, text)| ((*name).to_owned(), GeoJsonSource::parse(text).unwrap()))
        .collect();
    ready(render_with_files(map, &sources, files, AggRenderer::new()))
}

fn rgb(image: &RgbaImage, x: u32, y: u32) -> Rgba {
    image.pixel(x, y).unwrap()
}

#[test]
fn a_png_pixmap_is_blended_unturned_at_its_own_size_at_the_nearest_whole_pixel() {
    // The point (50.5, 58.5) is the pixel (50.5, 41.5). agg2RenderPixmapSymbol blends the image
    // at MS_NINT(50.5 - 32 / 2.) = MS_NINT(34.5) = 34 and MS_NINT(41.5 - 32 / 2.) =
    // MS_NINT(25.5) = 26, lrint rounding half to even.
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let map = map(
        vec![pixmap("pin", "icons/pin.png")],
        vec![layer("pins", RenderMode::Point, vec![symbol_style(1)])],
    );
    let rendered = render(
        &map,
        &files,
        &[("pins", r#"{"type": "Point", "coordinates": [50.5, 58.5]}"#)],
    );
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // The icon's top-left pixel, (0, 0, 160) opaque, lies at (34, 26), and nothing left of it or
    // above it is drawn.
    assert_eq!(rgb(&image, 34, 26), Rgba::opaque(0, 0, 160));
    assert_eq!(rgb(&image, 33, 26), WHITE);
    assert_eq!(rgb(&image, 34, 25), WHITE);
    // Its top-right pixel, (248, 0, 160), at (65, 26); nothing right of it.
    assert_eq!(rgb(&image, 65, 26), Rgba::opaque(248, 0, 160));
    assert_eq!(rgb(&image, 66, 26), WHITE);
    // Its bottom-left pixel, (0, 248, 160) at alpha 128, premultiplied as readPNG premultiplies,
    // (c × 128 + 255) >> 8 = (0, 124, 80), and blended over white as blender_rgba_pre blends it,
    // (255 × 127) >> 8 = 126 plus the colour: (126, 250, 206).
    assert_eq!(rgb(&image, 34, 57), Rgba::opaque(126, 250, 206));
    assert_eq!(rgb(&image, 34, 58), WHITE);
    // Its transparent bottom-right pixel leaves the map as it was.
    assert_eq!(rgb(&image, 65, 57), WHITE);
    // A style's colour and opacity do not reach the image (computeSymbolStyle).
    let mut colored = map.clone();
    colored.layers[0].classes[0].styles[0].color = Some(Bound::Value(BLUE));
    colored.layers[0].classes[0].styles[0].opacity = Some(Bound::Value(30.0));
    let pins = [("pins", r#"{"type": "Point", "coordinates": [50.5, 58.5]}"#)];
    assert!(render(&colored, &files, &pins).image.unwrap() == image);
    assert_matches_reference(&image, "pixmap_point.png");
}

#[test]
fn a_pixmap_that_does_not_load_leaves_its_style_undrawn_and_the_map_drawn() {
    // Its file missing, or its image past 4096 × 4096, read from its header: the style is not
    // drawn, the layer warns naming the symbol and the path, and every other layer and style
    // draws.
    let mut wide = forager_encode::png(&RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![0, 0, 0, 255],
    })
    .unwrap();
    wide[16..20].copy_from_slice(&4097u32.to_be_bytes());
    let files = Files([("wide.png", wide)].into());
    let square = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(9.0)),
        ..Style::default()
    };
    let area = Style {
        color: Some(Bound::Value(GREEN)),
        ..Style::default()
    };
    let symbols = vec![pixmap("gone", "icons/gone.png"), pixmap("wide", "wide.png")];
    let with = map(
        symbols.clone(),
        vec![
            layer("areas", RenderMode::Polygon, vec![area.clone()]),
            layer(
                "pins",
                RenderMode::Point,
                vec![symbol_style(1), square.clone(), symbol_style(2)],
            ),
        ],
    );
    let without = map(
        symbols,
        vec![
            layer("areas", RenderMode::Polygon, vec![area]),
            layer("pins", RenderMode::Point, vec![square]),
        ],
    );
    let sources = [
        (
            "areas",
            r#"{"type": "Polygon", "coordinates": [[[10, 10], [40, 10], [40, 40], [10, 10]]]}"#,
        ),
        (
            "pins",
            r#"{"type": "MultiPoint", "coordinates": [[50, 50], [70, 30]]}"#,
        ),
    ];
    let rendered = render(&with, &files, &sources);
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn", "pixmap-not-drawn"]);
    let gone = &rendered.diagnostics[0].message;
    assert!(gone.contains("`pins`"), "{gone}");
    assert!(gone.contains("`gone`"), "{gone}");
    assert!(gone.contains("icons/gone.png"), "{gone}");
    assert!(gone.contains("does not load"), "{gone}");
    let wide = &rendered.diagnostics[1].message;
    assert!(wide.contains("`wide`"), "{wide}");
    assert!(wide.contains("wide.png"), "{wide}");
    assert!(wide.contains("4096"), "{wide}");
    let drawn = render(&without, &files, &sources);
    assert!(drawn.diagnostics.is_empty(), "{:?}", drawn.diagnostics);
    let image = rendered.image.unwrap();
    assert_eq!(rgb(&image, 50, 50), BLUE, "the square beside them");
    assert!(image == drawn.image.unwrap());
}
