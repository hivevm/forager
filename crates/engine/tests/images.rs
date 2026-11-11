//! The images of a render's pixmap symbols, loaded through the host's resource loader and
//! decoded once per render and path (ADR-0029), each held to the bounds of Q-9 on its file and
//! its pixels.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::Rc;

use forager_adapter_geojson::GeoJsonSource;
use forager_engine::{MAX_FONT_OR_IMAGE_FILE_BYTES, Rendered, render_with_files};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::{DisplayItem, Draw, DrawOp, PremultipliedImage, Renderer, RgbaImage};
use forager_style_ir::{
    Bound, Class, Extent, Layer, LayerStatus, Map, RenderMode, Rgba, SourceRef, Style, Symbol,
    SymbolKind, SymbolRef,
};

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the engine waited on a port that never waits")
}

/// Files in memory, recording every path asked for.
#[derive(Default)]
struct Files {
    files: BTreeMap<&'static str, Vec<u8>>,
    asked: RefCell<Vec<String>>,
}

impl Files {
    fn with(mut self, path: &'static str, bytes: Vec<u8>) -> Self {
        self.files.insert(path, bytes);
        self
    }
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

/// A PNG of `width` × `height` pixels of one straight RGBA colour.
fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let pixels = rgba.repeat(width as usize * height as usize);
    forager_encode::png(&RgbaImage {
        width,
        height,
        pixels,
    })
    .unwrap()
}

/// A renderer that keeps the image of every pixmap it is asked to draw, and draws nothing.
#[derive(Clone, Default)]
struct Kept(Rc<RefCell<Vec<Rc<PremultipliedImage>>>>);

impl Draw for Kept {
    fn draw(&mut self, item: &DisplayItem) {
        if let DrawOp::Pixmap { image, .. } = &item.op {
            self.0.borrow_mut().push(Rc::clone(image));
        }
    }
}

impl Renderer for Kept {
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

/// A map of the extent 0..100 on 101 × 101 pixels whose pixmap symbols, one each, show the
/// images `paths`, and of two point layers, `a` and `b`, each drawing every symbol in turn.
fn map(paths: &[&str]) -> Map {
    let symbols = paths.iter().map(|path| Symbol {
        name: Some((*path).to_owned()),
        kind: SymbolKind::Pixmap,
        paths: Vec::new(),
        extent: None,
        filled: false,
        anchor_point: None,
        character: None,
        font: None,
        image: Some((*path).to_owned()),
        transparent: None,
        span: None,
    });
    let styles: Vec<_> = (1..=paths.len() as u32)
        .map(|index| Style {
            symbol: Some(Bound::Value(SymbolRef::Index(index))),
            ..Style::default()
        })
        .collect();
    let layer = |name: &str| Layer {
        name: Some(name.into()),
        render_mode: Some(RenderMode::Point),
        status: Some(LayerStatus::On),
        source: Some(SourceRef {
            kind: "local".into(),
            ..SourceRef::default()
        }),
        classes: vec![Class {
            styles: styles.clone(),
            ..Class::default()
        }],
        ..Layer::default()
    };
    Map {
        extent: Some(Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        }),
        size: Some([101, 101]),
        symbols: symbols.collect(),
        layers: vec![layer("a"), layer("b")],
        ..Map::default()
    }
}

/// Renders `map` with `files`, a point at (50, 50) in each layer: what came of it, and the image
/// of every pixmap drawn.
fn render(map: &Map, files: &Files) -> (Rendered, Vec<Rc<PremultipliedImage>>) {
    let point = r#"{"type": "Point", "coordinates": [50, 50]}"#;
    let sources: BTreeMap<String, GeoJsonSource> = ["a", "b"]
        .into_iter()
        .map(|name| (name.to_owned(), GeoJsonSource::parse(point).unwrap()))
        .collect();
    let kept = Kept::default();
    let rendered = ready(render_with_files(map, &sources, files, kept.clone()));
    let images = kept.0.borrow().clone();
    (rendered, images)
}

#[test]
fn an_image_two_layers_draw_is_loaded_and_decoded_once() {
    // Verifies: ADR-0029
    let files = Files::default().with("icons/pin.png", png(2, 3, [200, 100, 50, 128]));
    let (rendered, images) = render(&map(&["icons/pin.png"]), &files);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(images.len(), 2);
    let first = &images[0];
    assert_eq!((first.width(), first.height()), (2, 3));
    assert_eq!(&first.pixels()[..4], &[100, 50, 25, 128]);
    assert!(Rc::ptr_eq(first, &images[1]));
    assert_eq!(*files.asked.borrow(), ["icons/pin.png"]);
}

/// The messages of the render of `path` among `files` in two layers: one for each layer, each
/// naming the path, and the loader asked for the path at most once.
fn refused(files: &Files, path: &str) -> String {
    let (rendered, images) = render(&map(&[path]), files);
    assert!(images.is_empty());
    let messages: Vec<_> = rendered
        .diagnostics
        .iter()
        .map(|d| {
            assert_eq!(d.code, "pixmap-not-drawn", "{d:?}");
            d.message.as_str()
        })
        .collect();
    assert_eq!(messages.len(), 2, "{messages:?}");
    let why = messages[1].strip_prefix("layer `b`").unwrap();
    assert_eq!(messages[0].strip_prefix("layer `a`").unwrap(), why);
    assert!(
        files.asked.borrow().iter().filter(|p| *p == path).count() <= 1,
        "{:?}",
        files.asked.borrow()
    );
    assert!(why.contains(&format!("the image {path} ")), "{why}");
    why.to_owned()
}

#[test]
fn a_file_past_32_mib_is_refused_by_the_engine_itself() {
    // Verifies: Q-9
    assert_eq!(MAX_FONT_OR_IMAGE_FILE_BYTES, 32 * 1024 * 1024);
    let mut big = png(1, 1, [0, 0, 0, 255]);
    big.resize(MAX_FONT_OR_IMAGE_FILE_BYTES + 1, 0);
    let files = Files::default().with("big.png", big);
    let why = refused(&files, "big.png");
    assert_eq!(*files.asked.borrow(), ["big.png"]);
    assert!(why.contains("exceeds the limit of 33554432 bytes"), "{why}");
}

#[test]
fn a_path_outside_the_base_context_is_refused_unloaded() {
    let files = Files::default();
    let why = refused(&files, "../out.png");
    assert!(why.contains("base context"), "{why}");
    assert!(files.asked.borrow().is_empty());
}

#[test]
fn an_image_that_does_not_load_or_decode_is_refused_with_a_reason_naming_its_path() {
    let whole = png(4, 4, [1, 2, 3, 255]);
    let files = Files::default()
        .with("text.png", b"just some text".to_vec())
        .with("old.gif", b"GIF89a\x01\x00\x01\x00\x80\x00\x00".to_vec())
        .with("cut.png", whole[..whole.len() / 2].to_vec());
    assert!(refused(&files, "missing.png").contains("does not load"));
    assert!(refused(&files, "text.png").contains("unsupported format"));
    assert!(refused(&files, "old.gif").contains("unsupported format"));
    assert!(refused(&files, "cut.png").contains("broken PNG"));
}

#[test]
fn an_image_past_4096_pixels_is_refused_with_the_limit() {
    // Verifies: Q-9
    let mut header = png(1, 1, [0, 0, 0, 255]);
    // Its checksum is left as it was: the size is read before the header is checked.
    header[16..20].copy_from_slice(&4097u32.to_be_bytes());
    let files = Files::default().with("wide.png", header);
    let why = refused(&files, "wide.png");
    assert!(why.contains("past the limit of 4096 × 4096"), "{why}");
}
