//! A polygon fill's tile within its bounds (Q-9, ADR-0040), measured under a counting allocator:
//! a tile past 4096 × 4096 pixels allocates nothing, and the tile that would bring a render's
//! decoded images and tiles past 256 MiB is not built. One test, so that no other test's
//! allocations are counted with it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::Rc;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::images::MAX_DECODED_IMAGE_BYTES;
use forager_engine::{Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{DisplayItem, Draw, DrawOp, Renderer, RgbaImage};
use forager_render_agg::AggRenderer;
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Style, Symbol,
    SymbolKind, SymbolRef,
};
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

const MIB: usize = 1024 * 1024;
const WHITE: Rgba = Rgba::opaque(255, 255, 255);
/// The test icon, 32 × 32 RGBA: pixel (x, y) is (8x, 8y, 160), opaque in its top half.
const PIN: &[u8] = include_bytes!("../../cli/tests/fixtures/pin.png");
const AREA: &str = r#"{"type": "Polygon", "coordinates":
    [[[10, 10], [90, 10], [90, 90], [10, 90], [10, 10]]]}"#;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// Files in memory.
struct Files(BTreeMap<&'static str, Vec<u8>>);

impl ResourceLoader for Files {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new("no such file"))
    }
}

/// An [`AggRenderer`] that also tells the size of each tile it fills with, in `sizes`.
struct Told {
    renderer: AggRenderer,
    sizes: Rc<RefCell<Vec<(u32, u32)>>>,
}

impl Draw for Told {
    fn draw(&mut self, item: &DisplayItem) {
        if let DrawOp::TiledFill { tile, .. } = &item.op {
            self.sizes.borrow_mut().push((tile.width(), tile.height()));
        }
        self.renderer.draw(item);
    }
}

impl Renderer for Told {
    type Layer<'a> = &'a mut Self;

    fn begin(&mut self, width: u32, height: u32, background: Rgba) {
        self.renderer.begin(width, height, background);
    }

    fn layer(&mut self, _: u8) -> &mut Self {
        self
    }

    fn finish(self) -> RgbaImage {
        self.renderer.finish()
    }
}

fn pixmap(path: &str) -> Symbol {
    Symbol {
        name: Some(path.to_owned()),
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

fn layer(mode: RenderMode, styles: Vec<Style>) -> Layer {
    Layer {
        name: Some("areas".into()),
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
        ..Layer::default()
    }
}

/// A 101 × 101 image of the extent 0..100, one map unit a pixel, its polygon layer in `styles`.
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

fn of_symbol(index: u32) -> Style {
    Style {
        symbol: Some(Bound::Value(SymbolRef::Index(index))),
        ..Style::default()
    }
}

/// The symbol at `index` at a `SIZE` past MapServer's default bound of 500.
fn sized(index: u32, size: f64) -> Style {
    Style {
        size: Some(Bound::Value(size)),
        max_size: Some(8192.0),
        ..of_symbol(index)
    }
}

fn gapped(index: u32, gap: f64) -> Style {
    Style {
        gap: Some(gap),
        ..of_symbol(index)
    }
}

/// Renders `map`: what came of it, the tiles it filled with, and the most bytes it held at once
/// beyond what was held before it.
fn measured(map: &Map, files: &Files) -> (Rendered, Vec<(u32, u32)>, usize) {
    let sources: BTreeMap<String, GeoJsonSource> =
        [("areas".to_owned(), GeoJsonSource::parse(AREA).unwrap())].into();
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let renderer = Told {
        renderer: AggRenderer::new(),
        sizes: Rc::clone(&sizes),
    };
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let rendered = ready(render_with_files(map, &sources, files, renderer));
    let peak = ALLOCATOR.peak_usage() - before;
    let sizes = sizes.borrow().clone();
    (rendered, sizes, peak)
}

fn messages(rendered: &Rendered) -> Vec<&str> {
    for diagnostic in &rendered.diagnostics {
        assert_eq!(diagnostic.code, "pixmap-not-drawn", "{diagnostic:?}");
    }
    let all = rendered.diagnostics.iter();
    all.map(|d| d.message.as_str()).collect()
}

/// msDrawShadeSymbol would ask getTile for 4097 × 4097 of the first two, 64 MiB and more each;
/// the styles are not drawn, and nothing is allocated for them. The third fits.
fn a_tile_past_4096_a_side_allocates_nothing(files: &Files) {
    let symbols = vec![pixmap("pin.png"), pixmap("pin.png"), pixmap("pin.png")];
    let past = vec![sized(1, 4097.0), gapped(2, 4097.0)];
    let past = map(symbols.clone(), vec![layer(RenderMode::Polygon, past)]);
    let (rendered, sizes, peak) = measured(&past, files);
    assert!(sizes.is_empty(), "{sizes:?}");
    assert!(peak < MIB, "a peak of {peak} bytes for the tiles refused");
    let messages = messages(&rendered);
    assert_eq!(messages.len(), 2, "{messages:?}");
    for message in messages {
        assert!(message.contains("`pin.png`"), "{message}");
        assert!(message.contains("4097 × 4097"), "{message}");
        assert!(
            message.contains("past the limit of 4096 × 4096"),
            "{message}"
        );
    }
    let image = rendered.image.unwrap();
    assert_eq!(image.pixel(50, 50), Some(WHITE));

    // MS_NINT(32 × 4096 / 32): 4096 × 4096, 64 MiB, and no more. The icon, 128 times its size, is
    // opaque at the pixel (70, 70), its (0, 0) and beside it, whose blue is 160.
    let fits = map(
        symbols,
        vec![layer(RenderMode::Polygon, vec![sized(3, 4096.0)])],
    );
    let (rendered, sizes, peak) = measured(&fits, files);
    assert_eq!(sizes, [(4096, 4096)]);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert!(peak < 64 * MIB + 4 * MIB, "a peak of {peak} bytes");
    let pixel = rendered.image.unwrap().pixel(70, 70).unwrap();
    assert_eq!((pixel.b, pixel.a), (160, 255));
}

/// One image of 64 MiB and one of 4096 bytes, and the tiles: the large image's 4096 × 4096 at
/// scale 1, the small one's at a GAP of 4096 and 4095. Those come to 268 406 788 bytes of the
/// 268 435 456, so the tile at a GAP of 4090, 66 912 400 bytes, would pass them; one of 33 × 33
/// does not.
fn the_tile_that_would_pass_256_mib_is_not_built(files: &Files) {
    let held = 2 * (4096 * 4096 * 4) + 4096 * 4096 * 4 + 4095 * 4095 * 4 + 32 * 32 * 4;
    assert_eq!(held, 268_406_788);
    assert!(held + 4090 * 4090 * 4 > MAX_DECODED_IMAGE_BYTES);
    let styles = vec![
        Style {
            max_size: Some(4096.0),
            ..of_symbol(1)
        },
        gapped(2, 4096.0),
        gapped(2, 4095.0),
        gapped(2, 4090.0),
        gapped(2, 33.0),
    ];
    let symbols = vec![pixmap("big.png"), pixmap("pin.png"), pixmap("later.png")];
    // A later layer's image, which would pass the bound with the tiles, is refused too.
    let map = map(
        symbols,
        vec![
            layer(RenderMode::Polygon, styles),
            layer(RenderMode::Point, vec![of_symbol(3)]),
        ],
    );
    let (rendered, sizes, peak) = measured(&map, files);
    assert_eq!(
        sizes,
        [(4096, 4096), (4096, 4096), (4095, 4095), (33, 33)],
        "the styles but the one refused"
    );
    assert!(
        peak < MAX_DECODED_IMAGE_BYTES + 4 * MIB,
        "a peak of {peak} bytes"
    );
    let messages = messages(&rendered);
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(messages[0].contains("`pin.png`"), "{}", messages[0]);
    assert!(messages[0].contains("4090 × 4090"), "{}", messages[0]);
    assert!(
        messages[0].contains("past the limit of 268435456 bytes"),
        "{}",
        messages[0]
    );
    assert!(messages[1].contains("later.png"), "{}", messages[1]);
    assert!(
        messages[1].contains("past the limit of 268435456 bytes"),
        "{}",
        messages[1]
    );
    drop(rendered);
}

#[test]
fn a_tile_is_held_to_its_side_and_to_the_bytes_of_the_render() {
    // Verifies: Q-9, ADR-0040
    let side = 4096;
    let big = forager_encode::png(&RgbaImage {
        width: side,
        height: side,
        pixels: vec![0; side as usize * side as usize * 4],
    })
    .unwrap();
    let files = Files(
        [
            ("pin.png", PIN.to_vec()),
            ("big.png", big.clone()),
            ("later.png", big),
        ]
        .into(),
    );
    a_tile_past_4096_a_side_allocates_nothing(&files);
    the_tile_that_would_pass_256_mib_is_not_built(&files);
}
