//! The images of a render's pixmap symbols, loaded through the host's resource loader and
//! decoded once per render and path (ADR-0040).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::rc::Rc;

use forager_engine::images::Images;
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the store waited on a port that never waits")
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

#[test]
fn an_image_is_loaded_and_decoded_once_however_often_it_is_asked_for() {
    // Verifies: ADR-0040
    let files = Files::default().with("icons/pin.png", png(2, 3, [200, 100, 50, 128]));
    let mut images = Images::default();
    let first = ready(images.image("icons/pin.png", &files)).unwrap();
    assert_eq!((first.width(), first.height()), (2, 3));
    assert_eq!(&first.pixels()[..4], &[100, 50, 25, 128]);
    for _ in 1..1000 {
        let again = ready(images.image("icons/pin.png", &files)).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
    }
    assert_eq!(*files.asked.borrow(), ["icons/pin.png"]);
}

/// The reason the store gives for `path` among `files`, asked twice: the loader is called once.
fn refused(files: &Files, path: &str) -> String {
    let mut images = Images::default();
    let why = ready(images.image(path, files)).unwrap_err();
    assert_eq!(ready(images.image(path, files)).unwrap_err(), why);
    assert_eq!(
        files.asked.borrow().iter().filter(|p| *p == path).count(),
        1
    );
    assert!(why.contains(path), "{why}");
    why
}

#[test]
fn a_file_past_32_mib_is_refused_by_the_store_itself() {
    // Verifies: Q-9
    let mut big = png(1, 1, [0, 0, 0, 255]);
    big.resize(32 * 1024 * 1024 + 1, 0);
    let files = Files::default().with("big.png", big);
    let why = refused(&files, "big.png");
    assert!(why.contains("exceeds the limit of 33554432 bytes"), "{why}");
}

#[test]
fn a_path_outside_the_base_context_is_refused_unloaded() {
    let files = Files::default();
    let mut images = Images::default();
    let why = ready(images.image("../out.png", &files)).unwrap_err();
    assert_eq!(ready(images.image("../out.png", &files)).unwrap_err(), why);
    assert!(why.contains("../out.png"), "{why}");
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
