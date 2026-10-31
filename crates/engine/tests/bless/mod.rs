//! The reference images of Forager's own, shared by render.rs and labels.rs: a rendering is
//! compared with its image under `tests/fixtures`, and with FORAGER_BLESS=1 it is written there
//! first, reporting how far it moved the image it replaces, so that the commit can say so
//! (docs/CONVENTIONS.md).

use std::io::Write;
use std::path::{Path, PathBuf};

use forager_render::RgbaImage;

/// Compares `image` with the reference `tests/fixtures/<name>`, blessing it first when
/// FORAGER_BLESS is set.
pub fn assert_matches_reference(image: &RgbaImage, name: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    if std::env::var_os("FORAGER_BLESS").is_some()
        && let Some(report) = bless(&path, image)
    {
        #[expect(
            clippy::explicit_write,
            reason = "eprintln! is captured by the test harness; the bless reports without --nocapture"
        )]
        writeln!(std::io::stderr(), "{report}").unwrap();
    }
    let reference = forager_encode::decode_png(&std::fs::read(&path).unwrap()).unwrap();
    assert!(
        reference == *image,
        "{name} differs from its reference image"
    );
}

/// Writes `image` as the reference at `path` and reports how far it moved the image there, or
/// `None` when it did not.
pub fn bless(path: &Path, image: &RgbaImage) -> Option<String> {
    let name = path.file_name().unwrap().to_string_lossy();
    let before = std::fs::read(path)
        .ok()
        .map(|bytes| forager_encode::decode_png(&bytes).unwrap());
    std::fs::write(path, forager_encode::png(image).unwrap()).unwrap();
    match before {
        None => Some(format!("{name}: new, {}×{}", image.width, image.height)),
        Some(before) => report(&name, &before, image),
    }
}

/// How far `after` moved from `before`: the share of pixels of which a channel differs and the
/// largest difference of each channel, or `None` when no pixel moved.
pub fn report(name: &str, before: &RgbaImage, after: &RgbaImage) -> Option<String> {
    if (before.width, before.height) != (after.width, after.height) {
        return Some(format!(
            "{name}: {}×{} became {}×{}",
            before.width, before.height, after.width, after.height
        ));
    }
    let mut moved = 0_u64;
    let mut largest = [0_u8; 4];
    for (a, b) in before
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.pixels.as_chunks::<4>().0)
    {
        if a != b {
            moved += 1;
            for (channel, (x, y)) in largest.iter_mut().zip(a.iter().zip(b)) {
                *channel = (*channel).max(x.abs_diff(*y));
            }
        }
    }
    if moved == 0 {
        return None;
    }
    let pixels = u64::from(after.width) * u64::from(after.height);
    let [r, g, b, a] = largest;
    Some(format!(
        "{name}: {moved} of {pixels} pixels moved ({:.4}), by at most R {r} G {g} B {b} A {a}",
        moved as f64 / pixels as f64
    ))
}
