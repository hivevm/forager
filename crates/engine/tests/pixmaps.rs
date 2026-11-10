// The tests of a pixmap's anchor point work their values out from MapServer's msAdjustMarkerPos,
// src/maprendering.c, and msGetMarkerSize, src/mapsymbol.c, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota (src/maprendering.c).
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

//! A pixmap symbol drawn at a point, end to end: a style IR, its image through the resource
//! loader, and GeoJSON in, an RGBA image out, compared with a reference image of Forager's own.
//! The positions are read from
//! `agg2RenderPixmapSymbol`, src/mapagg.cpp, at the pin, not from the engine.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::{Rc, Weak};

mod bless;

use bless::assert_matches_reference;
use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{Affine, DisplayItem, Draw, DrawOp, Point, Renderer, RgbaImage};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, GeomTransform, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef,
    Span, Style, Symbol, SymbolKind, SymbolRef,
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

/// A renderer that keeps what it is asked to draw.
#[derive(Clone, Default)]
struct Recorder(Rc<RefCell<Vec<DisplayItem>>>);

impl Draw for Recorder {
    fn draw(&mut self, item: &DisplayItem) {
        self.0.borrow_mut().push(item.clone());
    }
}

impl Renderer for Recorder {
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

/// What the engine asks a renderer to draw of `map` with `files`: each resampled pixmap's point,
/// half-side, and inverse transformation, and the count of pixmaps blended unscaled.
fn recorded(
    map: &Map,
    files: &Files,
    sources: &[(&str, &str)],
) -> (Vec<(Point, i32, Affine)>, usize) {
    let sources: BTreeMap<String, GeoJsonSource> = sources
        .iter()
        .map(|(name, text)| ((*name).to_owned(), GeoJsonSource::parse(text).unwrap()))
        .collect();
    let recorder = Recorder::default();
    let rendered = ready(render_with_files(map, &sources, files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let mut resampled = Vec::new();
    let mut blended = 0;
    for item in recorder.0.borrow().iter() {
        match &item.op {
            DrawOp::ResampledPixmap {
                at, half, inverse, ..
            } => resampled.push((*at, *half, *inverse)),
            DrawOp::Pixmap { .. } => blended += 1,
            other => panic!("{other:?}"),
        }
    }
    (resampled, blended)
}

/// The pin at `SIZE` and `ANGLE` where each is given, and with `bounds`, `MINSIZE` and
/// `MAXSIZE`.
fn pin_map(size: Option<f64>, angle: Option<f64>, bounds: (Option<f64>, Option<f64>)) -> Map {
    let style = Style {
        size: size.map(Bound::Value),
        angle: angle.map(Bound::Value),
        min_size: bounds.0,
        max_size: bounds.1,
        ..symbol_style(1)
    };
    map(
        vec![pixmap("pin", "icons/pin.png")],
        vec![layer("pins", RenderMode::Point, vec![style])],
    )
}

fn affine(c: [f64; 6]) -> Affine {
    let [sx, shy, shx, sy, tx, ty] = c;
    Affine {
        sx,
        shy,
        shx,
        sy,
        tx,
        ty,
    }
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
    // In both: the area's triangle, the map's (30, 20) its pixel (30, 80), in green; the squares
    // at the pixels (50, 50) and (70, 70), the map's (70, 30), in blue; and where the pin would
    // stand about (50, 50), from 34 to 65 (agg2RenderPixmapSymbol, mapagg.cpp), white beside the
    // square, its pixel (40, 50) the pin's half-transparent (6, 16).
    let image = rendered.image.unwrap();
    let drawn = drawn.image.unwrap();
    for image in [&image, &drawn] {
        assert_eq!(rgb(image, 30, 80), GREEN, "the area");
        assert_eq!(rgb(image, 50, 50), BLUE, "the square beside them");
        assert_eq!(rgb(image, 70, 70), BLUE, "the second square");
        assert_eq!(rgb(image, 40, 50), WHITE, "no pin");
    }
    assert!(image == drawn);
}

#[test]
fn a_pixmap_drawn_as_a_stand_in_on_a_line_warns_that_it_is_not_drawn() {
    // MapServer tiles a pixmap along a line without a GAP, which Forager does not: it draws the
    // style's COLOR in its place. The image is never read there, so a file that does not exist
    // would pass unseen; each such symbol warns instead, once.
    let style = |index| Style {
        color: Some(Bound::Value(BLUE)),
        width: Some(Bound::Value(3.0)),
        ..symbol_style(index)
    };
    let twice = map(
        vec![pixmap("brush", "brush.png")],
        vec![layer("lines", RenderMode::Line, vec![style(1), style(1)])],
    );
    let sources = [(
        "lines",
        r#"{"type": "LineString", "coordinates": [[10, 90], [90, 90]]}"#,
    )];
    let rendered = render(&twice, &Files::default(), &sources);
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    for part in ["`lines`", "`brush`", "brush.png", "COLOR"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    let image = rendered.image.unwrap();
    assert_eq!(rgb(&image, 50, 10), BLUE, "the line's stand-in");
    // The first style of its class, drawn as written, strokes in its OUTLINECOLOR without a
    // COLOR, as any line does.
    let outlined = Style {
        outline_color: Some(Bound::Value(GREEN)),
        width: Some(Bound::Value(3.0)),
        ..symbol_style(1)
    };
    let first = map(
        vec![pixmap("brush", "brush.png")],
        vec![layer("lines", RenderMode::Line, vec![outlined])],
    );
    let rendered = render(&first, &Files::default(), &sources);
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    assert!(message.contains("OUTLINECOLOR"), "{message}");
    assert_eq!(
        rgb(&rendered.image.unwrap(), 50, 10),
        GREEN,
        "the outline's stand-in"
    );
    // A later style without a COLOR draws nothing, as any symbol of a later style without one;
    // it warns all the same, as its image is not read.
    let plain = Style {
        color: Some(Bound::Value(BLUE)),
        width: Some(Bound::Value(3.0)),
        ..Style::default()
    };
    let uncolored = map(
        vec![pixmap("brush", "brush.png")],
        vec![layer(
            "lines",
            RenderMode::Line,
            vec![plain, symbol_style(1)],
        )],
    );
    let rendered = render(&uncolored, &Files::default(), &sources);
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn"]);
    assert_eq!(
        rgb(&rendered.image.unwrap(), 50, 10),
        BLUE,
        "the first style alone"
    );
}

/// The polygon of [`tiled_map`]: from 10 to 90 in pixels, about a hole from 40 to 60.
const AREA: &str = r#"{"type": "Polygon", "coordinates": [
    [[10, 10], [90, 10], [90, 90], [10, 90], [10, 10]],
    [[40, 40], [60, 40], [60, 60], [40, 60], [40, 40]]]}"#;

/// A polygon layer of [`AREA`]'s kind in `styles`, drawing the symbols `pin` and `pin2`, two of
/// the test icon.
fn tiled_map(styles: Vec<Style>) -> Map {
    map(
        vec![pixmap("pin", "pin.png"), pixmap("pin2", "pin.png")],
        vec![layer("areas", RenderMode::Polygon, styles)],
    )
}

fn pin_files() -> Files {
    Files([("pin.png", PIN.to_vec())].into())
}

/// A style of the symbol `pin` at a `SIZE`, an `ANGLE`, and a `GAP`.
fn tiled_style(size: Option<f64>, angle: Option<f64>, gap: Option<f64>) -> Style {
    Style {
        size: size.map(Bound::Value),
        angle: angle.map(Bound::Value),
        gap,
        ..symbol_style(1)
    }
}

/// The tiles a renderer is asked to fill with in `map`, and their first ring's first point.
fn tiles_drawn(map: &Map, geometry: &str) -> Vec<Rc<forager_render::Tile>> {
    let sources: BTreeMap<String, GeoJsonSource> = map
        .layers
        .iter()
        .map(|layer| {
            let name = layer.name.clone().unwrap();
            (name, GeoJsonSource::parse(geometry).unwrap())
        })
        .collect();
    let recorder = Recorder::default();
    let rendered = ready(render_with_files(
        map,
        &sources,
        &pin_files(),
        recorder.clone(),
    ));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let items = recorder.0.borrow();
    items
        .iter()
        .filter_map(|item| match &item.op {
            DrawOp::TiledFill { tile, .. } => Some(Rc::clone(tile)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_polygon_moved_off_the_image_builds_no_tile_that_would_refuse_one_on_it() {
    // Verifies: Q-9
    // Four styles move the polygon 9000 pixels off the image, each with a tile near the largest;
    // built, they would hold nearly all the bytes a render may, and refuse the tile of the fifth
    // style, on the image. A fill that draws nothing builds no tile.
    let off = |gap: f64| Style {
        offset: Some([Bound::Value(9000.0), Bound::Value(9000.0)]),
        ..tiled_style(None, None, Some(gap))
    };
    let styles = vec![
        off(4096.0),
        off(4095.0),
        off(4094.0),
        off(4093.0),
        tiled_style(None, None, Some(300.0)),
    ];
    let tiles = tiles_drawn(&tiled_map(styles), AREA);
    let sides: Vec<_> = tiles.iter().map(|tile| tile.width()).collect();
    assert_eq!(sides, [300]);
}

#[test]
fn a_polygon_is_filled_with_its_pixmap_in_tiles_of_the_size_ms_draw_shade_symbol_gives() {
    // msDrawShadeSymbol for the 32 × 32 image: at scale 1 its own; at SIZE 15 ANGLE 45
    // MS_NINT(15); at SIZE 15.5 MS_NINT(15.5), half to even; at SIZE 15.5 GAP 10 the larger of
    // 10 and 15.5, truncated; at scale 1 with GAP 40.7 the larger of 40.7 and 32, truncated.
    for (style, side, reference) in [
        (tiled_style(None, None, None), 32, "polygon_pixmap.png"),
        (
            tiled_style(Some(15.0), Some(45.0), None),
            15,
            "polygon_pixmap_turned.png",
        ),
        (
            tiled_style(Some(15.5), None, None),
            16,
            "polygon_pixmap_size.png",
        ),
        (
            tiled_style(Some(15.5), None, Some(10.0)),
            15,
            "polygon_pixmap_size_gap.png",
        ),
        (
            tiled_style(None, None, Some(40.7)),
            40,
            "polygon_pixmap_gap.png",
        ),
    ] {
        let map = tiled_map(vec![style]);
        let tiles = tiles_drawn(&map, AREA);
        assert_eq!(tiles.len(), 1, "{reference}");
        assert_eq!((tiles[0].width(), tiles[0].height()), (side, side));
        let rendered = render(&map, &pin_files(), &[("areas", AREA)]);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        assert_matches_reference(&rendered.image.unwrap(), reference);
    }
}

#[test]
fn a_tile_repeats_from_the_image_s_origin_and_a_hole_stays_empty() {
    // The test icon's pixel (x, y) is (8x, 8y, 160), opaque in its top half and transparent in the
    // right of its bottom half. At scale 1 the tile is the icon at MS_NINT(16 - 16) = 0, repeated
    // every 32 pixels of the image, not of the polygon, which begins at 10.
    let style = tiled_style(None, None, None);
    let map = tiled_map(vec![style.clone()]);
    let image = render(&map, &pin_files(), &[("areas", AREA)])
        .image
        .unwrap();
    assert_eq!(
        rgb(&image, 12, 3),
        WHITE,
        "outside, its y the icon's top half"
    );
    assert_eq!(rgb(&image, 12, 12), Rgba::opaque(96, 96, 160));
    assert_eq!(
        rgb(&image, 35, 12),
        Rgba::opaque(24, 96, 160),
        "the next tile"
    );
    assert_eq!(
        rgb(&image, 35, 35),
        Rgba::opaque(24, 24, 160),
        "both repeated"
    );
    assert_eq!(rgb(&image, 30, 20), WHITE, "the icon's transparent corner");
    // Even-odd: the hole from 40 to 60, where the tile at (45, 41), the icon's (13, 9), is opaque.
    assert_eq!(rgb(&image, 45, 41), WHITE, "the hole");
    assert_eq!(rgb(&image, 45, 39), Rgba::opaque(104, 56, 160), "beside it");
    // With a gap: tiles of 40, the icon at MS_NINT(20 - 16) = 4 in each, transparent around it.
    let map = tiled_map(vec![tiled_style(None, None, Some(40.7))]);
    let image = render(&map, &pin_files(), &[("areas", AREA)])
        .image
        .unwrap();
    assert_eq!(rgb(&image, 21, 12), Rgba::opaque(136, 64, 160));
    assert_eq!(rgb(&image, 12, 12), Rgba::opaque(64, 64, 160));
    assert_eq!(
        rgb(&image, 83, 83),
        WHITE,
        "the tile's (3, 3), around the icon"
    );
    // No colour is needed, and none is drawn; nor an outline.
    let colored = Style {
        color: Some(Bound::Value(BLUE)),
        outline_color: Some(Bound::Value(GREEN)),
        width: Some(Bound::Value(4.0)),
        ..style.clone()
    };
    let plain = render(&tiled_map(vec![style]), &pin_files(), &[("areas", AREA)]);
    let map = tiled_map(vec![colored]);
    assert!(render(&map, &pin_files(), &[("areas", AREA)]).image == plain.image);
}

#[test]
fn a_polygon_s_pixmap_moves_with_its_offset_and_its_tile_stays_on_the_image() {
    // msDrawShadeSymbol moves the polygon; the tile repeats from the image's origin all the same.
    let style = Style {
        offset: Some([Bound::Value(5.0), Bound::Value(5.0)]),
        ..tiled_style(None, None, None)
    };
    let map = tiled_map(vec![style]);
    let image = render(&map, &pin_files(), &[("areas", AREA)])
        .image
        .unwrap();
    // The icon's (28, 8) at the pixel (92, 40), inside the polygon only as it is moved.
    assert_eq!(rgb(&image, 92, 40), Rgba::opaque(224, 64, 160));
    assert_eq!(rgb(&image, 12, 40), WHITE, "left of it as it is moved");
}

#[test]
fn a_polygon_s_pixmap_that_does_not_load_is_not_drawn_nor_its_colour_and_warns() {
    // MapServer fails the map (preloadSymbol, msDrawShadeSymbol); Forager leaves the style undrawn
    // by the maintainer's decision, as at a point, its COLOR not drawn in its place.
    let style = Style {
        color: Some(Bound::Value(BLUE)),
        ..tiled_style(None, None, None)
    };
    let rendered = render(
        &tiled_map(vec![style]),
        &Files::default(),
        &[("areas", AREA)],
    );
    let codes: Vec<_> = rendered.diagnostics.iter().map(|d| &*d.code).collect();
    assert_eq!(codes, ["pixmap-not-drawn"]);
    let message = &rendered.diagnostics[0].message;
    for part in ["`areas`", "`pin`", "pin.png", "does not load"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    assert_eq!(rgb(&rendered.image.unwrap(), 20, 20), WHITE);
}

#[test]
fn a_thousand_polygons_of_one_pixmap_style_share_one_tile() {
    // Verifies: ADR-0040
    // searchTileCache finds the tile getTile built for the first polygon.
    let features: Vec<_> = (0..1000)
        .map(|i| {
            let x = i % 90;
            format!(
                r#"{{"type": "Feature", "properties": {{}}, "geometry": {{"type": "Polygon",
                "coordinates": [[[{x}, 10], [{}, 10], [{}, 20], [{x}, 10]]]}}}}"#,
                x + 5,
                x + 5
            )
        })
        .collect();
    let geojson = format!(
        r#"{{"type": "FeatureCollection", "features": [{}]}}"#,
        features.join(",")
    );
    let map = tiled_map(vec![tiled_style(Some(15.0), Some(45.0), None)]);
    let tiles = tiles_drawn(&map, &geojson);
    assert_eq!(tiles.len(), 1000);
    assert!(tiles.iter().all(|tile| Rc::ptr_eq(tile, &tiles[0])));
}

/// A renderer that follows the tiles it is handed without keeping them: `built` counts each tile
/// it had not seen, `most` the most of them alive at once.
#[derive(Default)]
struct TilesAlive {
    seen: Vec<Weak<forager_render::Tile>>,
    most: Rc<Cell<usize>>,
    built: Rc<Cell<usize>>,
}

impl Draw for TilesAlive {
    fn draw(&mut self, item: &DisplayItem) {
        let DrawOp::TiledFill { tile, .. } = &item.op else {
            return;
        };
        if !self
            .seen
            .iter()
            .any(|seen| seen.as_ptr() == Rc::as_ptr(tile))
        {
            self.seen.push(Rc::downgrade(tile));
            self.built.set(self.built.get() + 1);
        }
        let alive = self.seen.iter().filter(|seen| seen.strong_count() > 0);
        self.most.set(self.most.get().max(alive.count()));
    }
}

impl Renderer for TilesAlive {
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
fn seven_styles_of_distinct_tiles_hold_at_most_six_at_once() {
    // Verifies: Q-9, ADR-0040
    // addTileCache holds MS_IMAGECACHESIZE, 6, of them. Each new tile drops the one built longest
    // ago, so a second polygon through the same seven styles, in the same order, finds none: 14.
    let styles = (10..17)
        .map(|size| tiled_style(Some(f64::from(size)), None, None))
        .collect();
    let map = tiled_map(styles);
    let draw = |geojson: &str| {
        let sources: BTreeMap<String, GeoJsonSource> =
            [("areas".to_owned(), GeoJsonSource::parse(geojson).unwrap())].into();
        let renderer = TilesAlive::default();
        let (most, built) = (Rc::clone(&renderer.most), Rc::clone(&renderer.built));
        ready(render_with_files(&map, &sources, &pin_files(), renderer));
        (most.get(), built.get())
    };
    let one = r#"{"type": "Polygon", "coordinates": [[[10, 10], [20, 10], [20, 20], [10, 10]]]}"#;
    assert_eq!(draw(one), (6, 7));
    let feature = format!(r#"{{"type": "Feature", "properties": {{}}, "geometry": {one}}}"#);
    let two = format!(r#"{{"type": "FeatureCollection", "features": [{feature}, {feature}]}}"#);
    assert_eq!(draw(&two), (6, 14));
}

#[test]
fn a_polygon_s_pixmap_at_a_size_of_0_has_a_tile_of_a_pixel_and_draws_without_a_fault() {
    // msDrawShadeSymbol: MS_NINT(32 × 0) is 0, then at least 1. The gap computeSymbolStyle gives,
    // 0 × 0 / 0, is no number, so no positive gap. The image drawn into the tile at a scale of 0
    // still covers its one pixel, so the polygon is filled in one colour, the hole left white.
    let map = tiled_map(vec![tiled_style(Some(0.0), None, None)]);
    let tiles = tiles_drawn(&map, AREA);
    assert_eq!((tiles[0].width(), tiles[0].height()), (1, 1));
    let rendered = render(&map, &pin_files(), &[("areas", AREA)]);
    let image = rendered.image.unwrap();
    assert_eq!(rgb(&image, 30, 80), Rgba::opaque(189, 189, 229));
    assert_eq!(rgb(&image, 50, 50), WHITE, "the hole");
}

#[test]
fn a_pixmap_of_a_hatched_class_counts_in_the_clip_buffer_loaded_outside_its_scale_range() {
    // Verifies: ADR-0032
    // msDrawShape loads every pixmap of the class before the clip, whatever its scale range: the
    // style of the 16 × 48 image never draws, yet it counts at its height: MS_NINT(48 + 1) + 2 =
    // 51 pixels around the extent, where the hatch of SIZE 10 alone would give 13.
    let tall = forager_encode::png(&RgbaImage {
        width: 16,
        height: 48,
        pixels: vec![0; 16 * 48 * 4],
    })
    .unwrap();
    let hatch = Symbol {
        kind: SymbolKind::Hatch,
        image: None,
        ..pixmap("hatch", "")
    };
    let hatched = Style {
        color: Some(Bound::Value(BLUE)),
        size: Some(Bound::Value(10.0)),
        ..symbol_style(1)
    };
    let out_of_range = Style {
        min_scale_denom: Some(1e30),
        ..symbol_style(2)
    };
    let map = map(
        vec![hatch, pixmap("tall", "tall.png")],
        vec![layer(
            "areas",
            RenderMode::Polygon,
            vec![hatched, out_of_range],
        )],
    );
    let beyond = r#"{"type": "Polygon", "coordinates":
        [[[-500, -500], [600, -500], [600, 600], [-500, 600], [-500, -500]]]}"#;
    let sources: BTreeMap<String, GeoJsonSource> =
        [("areas".to_owned(), GeoJsonSource::parse(beyond).unwrap())].into();
    let drawn = |files: &Files| {
        let recorder = Recorder::default();
        ready(render_with_files(&map, &sources, files, recorder.clone()));
        let items = recorder.0.borrow();
        let hatches: Vec<f64> = items
            .iter()
            .filter_map(|item| match &item.op {
                DrawOp::Hatch(hatch) => hatch.rings[0].iter().map(|p| p.x).reduce(f64::min),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 1, "the hatch alone");
        hatches[0]
    };
    assert_eq!(drawn(&Files([("tall.png", tall)].into())), -51.0);
    assert_eq!(
        drawn(&Files::default()),
        -13.0,
        "without its image, 1 counts"
    );
}

#[test]
fn a_png_pixmap_scaled_and_turned_is_resampled_into_the_square_about_its_point() {
    // agg2RenderPixmapSymbol, src/mapagg.cpp:656-690, for the 32 × 32 pin. SIZE 55 ANGLE 45 at the
    // pixel (50, 50): the scale 55 / 32 = 1.71875, the half-side MS_NINT(32 × 1.71875 × 1.415) =
    // lrint(77.825) = 78, 78 / 2 + 1 = 40; the inverse of translate(-16, -16), rotate(-45 ×
    // MS_DEG_TO_RAD), scale(1.71875), translate(50, 50), in trans_affine's order of operations
    // (src/renderers/agg/src/agg_trans_affine.cpp, multiply and invert) at the pin.
    // SIZE 16 unturned at the pixel (40.25, 40.25): the scale 16 / 32 = 0.5, MS_NINT(22.64) = 23,
    // 23 / 2 + 1 = 12; the inverse of translate(-16, -16), rotate(-0), scale(0.5), translate(40.25,
    // 40.25).
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let turned = pin_map(Some(55.0), Some(45.0), (None, None));
    let pins = [("pins", r#"{"type": "Point", "coordinates": [50, 50]}"#)];
    assert_eq!(
        recorded(&turned, &files, &pins),
        (
            vec![(
                Point { x: 50.0, y: 50.0 },
                40,
                affine([
                    0.411_407_581_781_264_1,
                    0.411_407_581_781_263_96,
                    -0.411_407_581_781_263_96,
                    0.411_407_581_781_264_1,
                    15.999_999_999_999_995,
                    -25.140_758_178_126_404,
                ]),
            )],
            0
        )
    );
    let rendered = render(&turned, &files, &pins);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // The square's edges lie on whole pixels, 10 and 90: nothing is drawn outside it, and its
    // corners, beyond the turned image, are transparent.
    for (x, y) in [(9, 50), (50, 9), (90, 50), (50, 90), (10, 10), (89, 89)] {
        assert_eq!(rgb(&image, x, y), WHITE, "({x}, {y})");
    }
    assert_matches_reference(&image, "pixmap_scaled_turned.png");

    let scaled = pin_map(Some(16.0), None, (None, None));
    let pins = [(
        "pins",
        r#"{"type": "Point", "coordinates": [40.25, 59.75]}"#,
    )];
    assert_eq!(
        recorded(&scaled, &files, &pins),
        (
            vec![(
                Point { x: 40.25, y: 40.25 },
                12,
                affine([2.0, -0.0, -0.0, 2.0, -64.5, -64.5]),
            )],
            0
        )
    );
    let rendered = render(&scaled, &files, &pins);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    // The pixel (x, y) is sampled at 2 × (x + 0.5) - 64.5, less the filter's half pixel: the pin's
    // pixel (2x - 64, 2y - 64), at a fraction of 0, so wholly. (32, 32) is the pin's (0, 0),
    // (0, 0, 160); (47, 39) its (30, 14), (240, 112, 160); (32, 40) its (0, 16) at alpha 128,
    // premultiplied (0, 64, 80) and blended over white, (255 × 127) >> 8 = 126 plus it; (47, 40)
    // its transparent (30, 16). (31, 32) samples the pin's (-2, 0), past the image, and (48, 32)
    // its (32, 0): the transparent background.
    assert_eq!(rgb(&image, 32, 32), Rgba::opaque(0, 0, 160));
    assert_eq!(rgb(&image, 47, 39), Rgba::opaque(240, 112, 160));
    assert_eq!(rgb(&image, 32, 40), Rgba::opaque(126, 190, 206));
    assert_eq!(rgb(&image, 47, 40), WHITE);
    assert_eq!(rgb(&image, 31, 32), WHITE);
    assert_eq!(rgb(&image, 48, 32), WHITE);
    assert_matches_reference(&image, "pixmap_scaled.png");
}

#[test]
fn a_pixmap_at_angle_360_is_blended_and_one_at_angle_359_resampled() {
    // computeSymbolStyle, maprendering.c:76: 360 × MS_DEG_TO_RAD (.0174532925199432958) is
    // MS_PI × 2 (3.14159265358979323846 × 2) in double, which agg2RenderPixmapSymbol,
    // mapagg.cpp:657, compares the rotation with: blended unscaled, as ticket 03's image holds it.
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let pins = [("pins", r#"{"type": "Point", "coordinates": [50.5, 58.5]}"#)];
    let full = pin_map(None, Some(360.0), (None, None));
    assert_eq!(recorded(&full, &files, &pins), (Vec::new(), 1));
    let rendered = render(&full, &files, &pins);
    assert_matches_reference(&rendered.image.unwrap(), "pixmap_point.png");
    // 359 × MS_DEG_TO_RAD is 6.265732014659643, turned: resampled, the half-side MS_NINT(32 ×
    // 1.415) = lrint(45.28) = 45, 45 / 2 + 1 = 23.
    let turned = pin_map(None, Some(359.0), (None, None));
    let (resampled, blended) = recorded(&turned, &files, &pins);
    assert_eq!(blended, 0);
    assert_eq!(
        resampled,
        [(
            Point { x: 50.5, y: 41.5 },
            23,
            affine([
                0.999_847_695_156_391_3,
                -0.017_452_406_437_283_56,
                0.017_452_406_437_283_56,
                0.999_847_695_156_391_3,
                -35.216_583_472_545_025,
                -24.612_332_823_907_416,
            ]),
        )]
    );
}

#[test]
fn a_pixmap_s_scale_is_bounded_by_minsize_and_maxsize_times_the_resolution_factor() {
    // computeSymbolStyle, maprendering.c:62-65, at RESOLUTION 144 over DEFRESOLUTION 72, a factor
    // of 2: SIZE 100 × 2 = 200 bounded by MAXSIZE 20 × 2 = 40, the scale 40 / 32 = 1.25, the
    // half-side MS_NINT(32 × 1.25 × 1.415) = lrint(56.6) = 57, 57 / 2 + 1 = 29, the inverse 1 /
    // 1.25 = 0.8; SIZE 4 × 2 = 8 raised by MINSIZE 12 × 2 = 24, the scale 0.75, MS_NINT(33.96) =
    // 34, 34 / 2 + 1 = 18, the inverse's scale 1.3333333333333333 by trans_affine's invert.
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let pins = [("pins", r#"{"type": "Point", "coordinates": [50, 50]}"#)];
    let cases = [
        (
            100.0,
            (None, Some(20.0)),
            29,
            [0.8, -0.0, -0.0, 0.8, -24.0, -24.0],
        ),
        (
            4.0,
            (Some(12.0), None),
            18,
            [
                1.333_333_333_333_333_3,
                -0.0,
                -0.0,
                1.333_333_333_333_333_3,
                -50.666_666_666_666_664,
                -50.666_666_666_666_664,
            ],
        ),
    ];
    for (size, bounds, half, inverse) in cases {
        let mut map = pin_map(Some(size), None, bounds);
        map.resolution = Some(144.0);
        map.def_resolution = Some(72.0);
        assert_eq!(
            recorded(&map, &files, &pins),
            (vec![(Point { x: 50.0, y: 50.0 }, half, affine(inverse))], 0),
            "SIZE {size}"
        );
    }
}

#[test]
fn a_square_far_outside_the_image_draws_nothing_and_one_far_around_it_samples_the_middle() {
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let pins = [("pins", r#"{"type": "Point", "coordinates": [50, 50]}"#)];
    // SIZE 1e6 under MAXSIZE 1e6, moved by OFFSET 1e9: the half-side MS_NINT(32 × 31250 × 1.415)
    // / 2 + 1 = 707501 about the pixel (1e9 + 50, 50), far past the image's right edge.
    let mut away = pin_map(Some(1e6), None, (None, Some(1e6)));
    away.layers[0].classes[0].styles[0].offset = Some([Bound::Value(1e9), Bound::Value(0.0)]);
    let rendered = render(&away, &files, &pins);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    assert!(image.pixels.chunks(4).all(|p| p == [255, 255, 255, 255]));
    // SIZE 1e7 under MAXSIZE 1e7 at the pixel (50, 50): the scale 312500, the half-side 7075001,
    // the square's corners millions of pixels past the image on every side. Every pixel samples
    // the pin within a thousandth of a pixel of its centre, (16, 16), so at 4096 / 256, less the
    // filter's half pixel: the pin's pixels (15, 15), (16, 15), (15, 16), and (16, 16), a
    // quarter each, 16384, premultiplied (120, 120, 160, 255), (128, 120, 160, 255), (60, 64, 80,
    // 128), and (0, 0, 0, 0). Each channel (32768 + 16384 × sum) >> 16: (77, 76, 100, 160), the
    // alpha 638 / 4 + 0.5 = 160 (agg-rust's filter truncates to 159). Over white,
    // blender_rgba_pre: (255 × 95) >> 8 = 94 plus the colour, (171, 170, 194).
    let around = pin_map(Some(1e7), None, (None, Some(1e7)));
    let rendered = render(&around, &files, &pins);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let image = rendered.image.unwrap();
    for (x, y) in [(0, 0), (100, 0), (50, 50), (0, 100), (100, 100)] {
        assert_eq!(rgb(&image, x, y), Rgba::opaque(171, 170, 194), "({x}, {y})");
    }
    assert!(image.pixels.chunks(4).all(|p| p == [171, 170, 194, 255]));
}

#[test]
fn a_pixmap_s_anchor_point_turned_with_it_lies_on_its_point() {
    // msAdjustMarkerPos, src/maprendering.c:1013-1050, moves the point by (0.5 - anchor) times
    // the marker's size, turned by sin(-rotation) and cos(-rotation) where the rotation is not 0.
    // msGetMarkerSize, src/mapsymbol.c:720-799, of the 32 × 32 pin at SIZE 10: size = 10, not 1,
    // so the width 10 / 32 × 32 = 10 and the height 10 (772-782). ANGLE 45 is the rotation 45 ×
    // MS_DEG_TO_RAD, sin(-r) = -0.7071067811865475, cos(-r) = 0.7071067811865476.
    // ANCHORPOINT 0 0: (5, 5), turned (7.0710678118654755, 4.440892098500626e-16).
    // ANCHORPOINT 0.2 0.7: (0.5 - 0.2) × 10 = 3 and (0.5 - 0.7) × 10 = -1.9999999999999996,
    // turned (0.7071067811865481, -3.5355339059327373).
    // ANCHORPOINT 1 1: (-5, -5), turned (-7.0710678118654755, -4.440892098500626e-16).
    // Unturned at the pixels (20, 25), (50, 25), (80, 25); at ANGLE 45 at (20, 70), (50, 70), (80,
    // 70). Each is resampled at the scale 10 / 32, the half-side MS_NINT(32 × 0.3125 × 1.415) =
    // lrint(14.15) = 14, 14 / 2 + 1 = 8 (agg2RenderPixmapSymbol, src/mapagg.cpp:656-683).
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let anchored = |name: &str, anchor: [f64; 2]| Symbol {
        anchor_point: Some(anchor),
        ..pixmap(name, "icons/pin.png")
    };
    let symbols = vec![
        anchored("top-left", [0.0, 0.0]),
        anchored("inside", [0.2, 0.7]),
        anchored("bottom-right", [1.0, 1.0]),
    ];
    let names = ["a", "b", "c", "d", "e", "f"];
    let mut layers = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let style = Style {
            size: Some(Bound::Value(10.0)),
            angle: (i >= 3).then_some(Bound::Value(45.0)),
            ..symbol_style(i as u32 % 3 + 1)
        };
        layers.push(layer(name, RenderMode::Point, vec![style]));
    }
    let map = map(symbols, layers);
    // The pixel (x, y) is the map's (x, 100 - y).
    let point = |x: u32, y: u32| format!(r#"{{"type": "Point", "coordinates": [{x}, {y}]}}"#);
    let points = [
        point(20, 75),
        point(50, 75),
        point(80, 75),
        point(20, 30),
        point(50, 30),
        point(80, 30),
    ];
    let sources: Vec<(&str, &str)> = names
        .iter()
        .copied()
        .zip(points.iter().map(String::as_str))
        .collect();
    let (resampled, blended) = recorded(&map, &files, &sources);
    assert_eq!(blended, 0);
    let placed: Vec<(Point, i32)> = resampled.iter().map(|&(at, half, _)| (at, half)).collect();
    let at = |x: f64, y: f64| (Point { x, y }, 8);
    assert_eq!(
        placed,
        [
            at(25.0, 30.0),
            at(53.0, 23.0),
            at(75.0, 20.0),
            at(27.071_067_811_865_476, 70.0),
            at(50.707_106_781_186_546, 66.464_466_094_067_26),
            at(72.928_932_188_134_52, 70.0),
        ]
    );
    let rendered = render(&map, &files, &sources);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_matches_reference(&rendered.image.unwrap(), "pixmap_anchor_point.png");
}

#[test]
fn a_later_transformed_pixmap_style_without_a_color_is_drawn_at_each_vertex() {
    // A line layer of two styles is cached. lineLayerDrawShape hands the second, with
    // GEOMTRANSFORM vertices, to msDrawTransformedShape in the first pass, with no test of its
    // COLOR, and the cache's later passes skip it; msDrawMarkerSymbolInternal draws a pixmap
    // without a colour. So the pin is drawn once at the line's one inner vertex, the pixel
    // (50, 50), blended at MS_NINT(50 - 32 / 2.) = 34 on both axes, after the line's stroke.
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let line = Style {
        color: Some(Bound::Value(GREEN)),
        ..Style::default()
    };
    let pin = Style {
        geom_transform: Some(GeomTransform::Vertices),
        ..symbol_style(1)
    };
    let map = map(
        vec![pixmap("pin", "icons/pin.png")],
        vec![layer("lines", RenderMode::Line, vec![line, pin])],
    );
    let sources: BTreeMap<String, GeoJsonSource> = [(
        "lines".to_owned(),
        GeoJsonSource::parse(
            r#"{"type": "LineString", "coordinates": [[10, 80], [50, 50], [90, 80]]}"#,
        )
        .unwrap(),
    )]
    .into();
    let recorder = Recorder::default();
    let rendered = ready(render_with_files(&map, &sources, &files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    let drawn: Vec<String> = recorder
        .0
        .borrow()
        .iter()
        .map(|item| match &item.op {
            DrawOp::Stroke { color, .. } => format!("stroke {color:?}"),
            DrawOp::Pixmap { left, top, .. } => format!("pixmap at ({left}, {top})"),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        drawn,
        [format!("stroke {GREEN:?}"), "pixmap at (34, 34)".to_owned()]
    );
}

/// What the engine asks a renderer to draw of a pixmap along a line.
#[derive(Debug, PartialEq)]
enum Placed {
    /// A stroke of the line.
    Stroke,
    /// The image blended unscaled with its top-left pixel at the pixel.
    Blended(i32, i32),
    /// The image resampled into the square of the half-side about the point.
    Resampled(Point, i32, Affine),
}

/// What the engine asks a renderer to draw of `map` with `files`, in order, with no diagnostic.
fn placed(map: &Map, files: &Files, sources: &[(&str, &str)]) -> Vec<Placed> {
    let sources: BTreeMap<String, GeoJsonSource> = sources
        .iter()
        .map(|(name, text)| ((*name).to_owned(), GeoJsonSource::parse(text).unwrap()))
        .collect();
    let recorder = Recorder::default();
    let rendered = ready(render_with_files(map, &sources, files, recorder.clone()));
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    recorder
        .0
        .borrow()
        .iter()
        .map(|item| match &item.op {
            DrawOp::Stroke { .. } => Placed::Stroke,
            DrawOp::Pixmap { left, top, .. } => Placed::Blended(*left, *top),
            DrawOp::ResampledPixmap {
                at, half, inverse, ..
            } => Placed::Resampled(*at, *half, *inverse),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// A 201 × 201 image of the extent 0..200, one map unit a pixel, the pixel (x, y) the map's
/// (x, 200 - y).
fn wide_map(symbols: Vec<Symbol>, layers: Vec<Layer>) -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 200.0,
            max_y: 200.0,
        }),
        size: Some([201, 201]),
        ..map(symbols, layers)
    }
}

/// A style that draws the symbol at `index` along a line every `gap` pixels, at `size`.
fn gapped(index: u32, gap: f64, size: Option<f64>) -> Style {
    Style {
        gap: Some(gap),
        size: size.map(Bound::Value),
        ..symbol_style(index)
    }
}

#[test]
fn a_pixmap_along_a_line_lies_where_ms_image_polyline_markers_places_it() {
    // msImagePolylineMarkers, src/maprendering.c:388-598, as msDrawLineSymbol, 679-688, hands it
    // the 32 × 32 pin (its sizex and sizey the image's, src/mapsymbol.c:969-970).
    //
    // GAP 50 without a size, after a black line as line_marker_pixmap draws it, the pin without
    // a COLOR in the cache's later pass (src/mapdraw.c:1489-1494): computeSymbolStyle, 62-66,
    // the scale 32 / 32 = 1, the gap 50 × 32 / 32 = 50, upright (687-688). Along the pixels (10,
    // 40), (90, 40), (190, 40): the first at half the gap, 25 (420), then every 50, the second
    // segment carrying on 125 - 80 = 45 (516): x = 35, 85, then 90 + 45 = 135, 185. Unturned at
    // a scale of 1, each is blended at MS_NINT(x - 32 / 2.), MS_NINT(40 - 32 / 2.)
    // (agg2RenderPixmapSymbol, src/mapagg.cpp:699-700): 19, 69, 119, 169, and 24.
    //
    // GAP -50 SIZE 16: the scale 16 / 32 = 0.5, the gap -50 × 16 / 16 = -50, so 50 apart, each
    // turned with its segment (683-684), its rotation asin(ry), negated where rx ≥ 0 and plus
    // MS_PI where rx < 0 (435-442). Along (10, 100), (90, 100), (90, 180), (10, 180): (35, 100)
    // and (85, 100) at -0; the next segment, ry = 1, at -π/2, 45 along it, (90, 145), then 95 is
    // past its 80; the last, rx = -1, at π, 15 along it, (75, 180), and 65, (25, 180). Each is
    // resampled (657-658): the half-side MS_NINT(32 × 0.5 × 1.415) = lrint(22.64) = 23,
    // 23 / 2 + 1 = 12, and the inverse of translate(-16, -16), rotate(-rotation), scale(0.5),
    // translate(x, y) in trans_affine's order of operations (src/renderers/agg/src/
    // agg_trans_affine.cpp, multiply and invert).
    //
    // GAP 100 on a line of 40 and one of 30: on neither does a marker fall, 50 being past each
    // end; without INITIALGAP, the first, longer than the marker's width 32, takes one at half its
    // length, (140, 100), blended at (124, 84); the second, not longer, none (524-595).
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let black = Style {
        color: Some(Bound::Value(Rgba::opaque(0, 0, 0))),
        ..Style::default()
    };
    let map = wide_map(
        vec![pixmap("pin", "icons/pin.png")],
        vec![
            layer(
                "upright",
                RenderMode::Line,
                vec![black, gapped(1, 50.0, None)],
            ),
            layer(
                "turned",
                RenderMode::Line,
                vec![gapped(1, -50.0, Some(16.0))],
            ),
            layer("short", RenderMode::Line, vec![gapped(1, 100.0, None)]),
        ],
    );
    let sources = [
        (
            "upright",
            r#"{"type": "LineString", "coordinates": [[10, 160], [90, 160], [190, 160]]}"#,
        ),
        (
            "turned",
            r#"{"type": "LineString", "coordinates": [[10, 100], [90, 100], [90, 20], [10, 20]]}"#,
        ),
        (
            "short",
            r#"{"type": "MultiLineString", "coordinates":
                [[[120, 100], [160, 100]], [[120, 50], [150, 50]]]}"#,
        ),
    ];
    let at = |x: f64, y: f64| Point { x, y };
    // Turned by π on the last segment, at (x, 180).
    let leftwards = |x: f64, tx: f64| {
        let shy = 2.449_293_598_294_706_4e-16;
        Placed::Resampled(at(x, 180.0), 12, affine([-2.0, shy, -shy, -2.0, tx, 376.0]))
    };
    assert_eq!(
        placed(&map, &files, &sources),
        [
            Placed::Stroke,
            Placed::Blended(19, 24),
            Placed::Blended(69, 24),
            Placed::Blended(119, 24),
            Placed::Blended(169, 24),
            Placed::Resampled(
                at(35.0, 100.0),
                12,
                affine([2.0, -0.0, -0.0, 2.0, -54.0, -184.0])
            ),
            Placed::Resampled(
                at(85.0, 100.0),
                12,
                affine([2.0, -0.0, -0.0, 2.0, -154.0, -184.0])
            ),
            Placed::Resampled(
                at(90.0, 145.0),
                12,
                affine([
                    1.224_646_799_147_353_2e-16,
                    -2.0,
                    2.0,
                    1.224_646_799_147_353_2e-16,
                    -274.0,
                    195.999_999_999_999_97,
                ])
            ),
            leftwards(75.0, 166.000_000_000_000_06),
            leftwards(25.0, 66.000_000_000_000_04),
            Placed::Blended(124, 84),
        ]
    );
    let rendered = render(&map, &files, &sources);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_matches_reference(&rendered.image.unwrap(), "pixmap_line.png");
}

#[test]
fn a_pixmap_along_a_line_is_anchored_by_the_scaled_image_s_width_and_height() {
    // msImagePolylineMarkers, src/maprendering.c:400-401, sizes the marker as the image times the
    // scale, at least 1: at SIZE 15.5 the scale is 15.5 / 32 = 0.484375 (computeSymbolStyle,
    // 62-65), so 15.5 × 15.5, where at a point msGetMarkerSize cuts the size to 15. ANCHORPOINT
    // 0 0 moves each marker by (0.5 - 0) × 15.5 = 7.75 on both axes, turned by sin(-rotation)
    // and cos(-rotation) where the rotation is not 0 (447-464).
    //
    // GAP 50 along the pixels (10, 30), (90, 30): the points (35, 30) and (85, 30), moved to
    // (42.75, 37.75) and (92.75, 37.75). GAP -50 along (50, 50), (50, 100), ry = 1, the rotation
    // -π/2: the point (50, 75), moved by 7.75 × cos(π/2) - 7.75 × sin(π/2) = -7.749999999999999
    // and 7.75 × sin(π/2) + 7.75 × cos(π/2) = 7.750000000000001, to (42.25, 82.75). GAP 100
    // along (120, 150), (160, 150), on which none falls: at half its length, (140, 150), not
    // moved (555-561). Each is resampled, the half-side MS_NINT(32 × 0.484375 × 1.415) =
    // lrint(21.9325) = 22, 22 / 2 + 1 = 12 (agg2RenderPixmapSymbol, src/mapagg.cpp:679-682).
    let files = Files([("icons/pin.png", PIN.to_vec())].into());
    let symbols = vec![Symbol {
        anchor_point: Some([0.0, 0.0]),
        ..pixmap("top-left", "icons/pin.png")
    }];
    let map = wide_map(
        symbols,
        vec![
            layer(
                "upright",
                RenderMode::Line,
                vec![gapped(1, 50.0, Some(15.5))],
            ),
            layer(
                "turned",
                RenderMode::Line,
                vec![gapped(1, -50.0, Some(15.5))],
            ),
            layer(
                "short",
                RenderMode::Line,
                vec![gapped(1, 100.0, Some(15.5))],
            ),
        ],
    );
    let sources = [
        (
            "upright",
            r#"{"type": "LineString", "coordinates": [[10, 170], [90, 170]]}"#,
        ),
        (
            "turned",
            r#"{"type": "LineString", "coordinates": [[50, 150], [50, 100]]}"#,
        ),
        (
            "short",
            r#"{"type": "LineString", "coordinates": [[120, 50], [160, 50]]}"#,
        ),
    ];
    let at = |x: f64, y: f64| Point { x, y };
    let upright = |x: f64, y: f64, tx: f64, ty: f64| {
        Placed::Resampled(
            at(x, y),
            12,
            affine([
                2.064_516_129_032_258_5,
                -0.0,
                -0.0,
                2.064_516_129_032_258_5,
                tx,
                ty,
            ]),
        )
    };
    assert_eq!(
        placed(&map, &files, &sources),
        [
            upright(42.75, 37.75, -72.258_064_516_129_04, -61.935_483_870_967_75),
            upright(
                92.75,
                37.75,
                -175.483_870_967_741_98,
                -61.935_483_870_967_75
            ),
            Placed::Resampled(
                at(42.25, 82.75),
                12,
                affine([
                    1.264_151_534_603_719_6e-16,
                    -2.064_516_129_032_258_5,
                    2.064_516_129_032_258_5,
                    1.264_151_534_603_719_6e-16,
                    -154.838_709_677_419_4,
                    103.225_806_451_612_91,
                ])
            ),
            upright(
                140.0,
                150.0,
                -273.032_258_064_516_16,
                -293.677_419_354_838_8
            ),
        ]
    );
    let rendered = render(&map, &files, &sources);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_matches_reference(&rendered.image.unwrap(), "pixmap_line_anchor.png");
}
