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
//! loader, and GeoJSON in, an RGBA image out, compared with a reference image of Forager's own
//! (docs/plan/pixmap-symbols/spec.md, Testing decisions). The positions are read from
//! `agg2RenderPixmapSymbol`, src/mapagg.cpp, at the pin, not from the engine.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::Rc;

mod bless;

use bless::assert_matches_reference;
use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{Affine, DisplayItem, Draw, DrawOp, Point, Renderer, RgbaImage};
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
    let image = rendered.image.unwrap();
    assert_eq!(rgb(&image, 50, 50), BLUE, "the square beside them");
    assert!(image == drawn.image.unwrap());
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
