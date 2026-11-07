//! One pixmap placed at many points is held once per render (ADR-0040): every item the renderer
//! is handed for it refers to the same decoded image, the image is loaded once, and the memory of
//! the render is released when it ends. Measured under the counting allocator of
//! `label_memory.rs`; one test, so that no other test's allocations are counted with it.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::{Rc, Weak};

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::render_with_files;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{DisplayItem, Draw, DrawOp, PremultipliedImage, Renderer, RgbaImage};
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Style, Symbol,
    SymbolKind, SymbolRef,
};
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

const POINTS: usize = 10_000;
const SIDE: u32 = 512;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// The one PNG under every path, each load counted.
struct Files {
    png: Vec<u8>,
    loads: Cell<usize>,
}

impl ResourceLoader for Files {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.loads.set(self.loads.get() + 1);
        Ok(self.png.clone())
    }
}

/// A renderer that keeps no item it is handed, only where the image of each pixmap lies and a
/// weak reference to the first, into `first`, so that it holds no strong reference to it past the
/// item and the test can tell whether anything still holds the image once the render has ended.
struct Pointers<'a> {
    first: &'a RefCell<Option<Weak<PremultipliedImage>>>,
    same: usize,
    other: usize,
}

impl Draw for Pointers<'_> {
    fn draw(&mut self, item: &DisplayItem) {
        if let DrawOp::Pixmap { image, .. } = &item.op {
            let mut first = self.first.borrow_mut();
            let first = first.get_or_insert_with(|| Rc::downgrade(image));
            if std::ptr::eq(first.as_ptr(), Rc::as_ptr(image)) {
                self.same += 1;
            } else {
                self.other += 1;
            }
        }
    }
}

impl Renderer for Pointers<'_> {
    type Layer<'a>
        = &'a mut Self
    where
        Self: 'a;

    fn begin(&mut self, _: u32, _: u32, _: Rgba) {}
    fn layer(&mut self, _: u8) -> &mut Self {
        self
    }
    fn finish(self) -> RgbaImage {
        let counts = [self.same as u64, self.other as u64];
        RgbaImage {
            width: 0,
            height: 0,
            pixels: counts.iter().flat_map(|c| c.to_le_bytes()).collect(),
        }
    }
}

fn map() -> Map {
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1000.0,
            max_y: 1000.0,
        }),
        size: Some([1000, 1000]),
        symbols: vec![Symbol {
            name: Some("big".into()),
            kind: SymbolKind::Pixmap,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: Some("big.png".into()),
            transparent: None,
            span: None,
        }],
        layers: vec![Layer {
            name: Some("places".into()),
            render_mode: Some(RenderMode::Point),
            status: Some(LayerStatus::On),
            source: Some(SourceRef {
                kind: "local".into(),
                ..SourceRef::default()
            }),
            classes: vec![Class {
                // MAXSIZE at the image's height: by MapServer's default of 500, computeSymbolStyle
                // would scale the image of 512 pixels to 500.
                styles: vec![Style {
                    symbol: Some(Bound::Value(SymbolRef::Index(1))),
                    max_size: Some(f64::from(SIDE)),
                    ..Style::default()
                }],
                ..Class::default()
            }],
            ..Layer::default()
        }],
        ..Map::default()
    }
}

#[test]
fn one_pixmap_at_ten_thousand_points_is_one_image_released_when_the_render_ends() {
    // Verifies: ADR-0040
    let png = forager_encode::png(&RgbaImage {
        width: SIDE,
        height: SIDE,
        pixels: [10, 20, 30, 255].repeat(SIDE as usize * SIDE as usize),
    })
    .unwrap();
    let points: Vec<String> = (0..POINTS)
        .map(|i| format!("[{}, {}]", i % 100 * 10, i / 100 * 10))
        .collect();
    let geojson = format!(
        r#"{{"type": "MultiPoint", "coordinates": [{}]}}"#,
        points.join(",")
    );
    let sources: BTreeMap<String, GeoJsonSource> =
        [("places".to_owned(), GeoJsonSource::parse(&geojson).unwrap())].into();
    let map = map();
    let render = |files: &Files, first: &RefCell<Option<Weak<PremultipliedImage>>>| {
        let pointers = Pointers {
            first,
            same: 0,
            other: 0,
        };
        let rendered = ready(render_with_files(&map, &sources, files, pointers));
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        let pixels = rendered.image.unwrap().pixels;
        let count = |i: usize| u64::from_le_bytes(pixels[i * 8..i * 8 + 8].try_into().unwrap());
        (count(0) as usize, count(1) as usize)
    };
    // A first render, so that what the process sets up once is not counted.
    let warm = Files {
        png: png.clone(),
        loads: Cell::new(0),
    };
    render(&warm, &RefCell::new(None));

    let files = Files {
        png,
        loads: Cell::new(0),
    };
    // Held before the render, so that only the weak reference it comes to hold is counted in it.
    let first = RefCell::new(None);
    let before = ALLOCATOR.current_usage();
    let (same, other) = render(&files, &first);
    // Once the render has returned, nothing holds the image: the store went with the render.
    let weak = first.take().expect("an item drew the image");
    assert!(
        weak.upgrade().is_none(),
        "the image outlives the render that decoded it"
    );
    // The weak reference keeps the image's allocation, which is freed with it.
    drop(weak);
    let after = ALLOCATOR.current_usage();
    assert_eq!(same, POINTS, "an item per point, each the one image");
    assert_eq!(other, 0, "no item refers to another image");
    assert_eq!(files.loads.get(), 1, "the image is loaded once");
    assert_eq!(
        after, before,
        "the render's memory is released when it ends"
    );
}
