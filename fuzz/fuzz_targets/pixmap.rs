//! The pixmap decoder on an arbitrary image file: the input is a pixmap symbol's image, read for
//! its size and decoded, as the engine reads a symbol's file (Q-3, Q-9). An image that decodes is
//! of the size its header states, within 4096 × 4096, four bytes to each pixel; the memory a
//! decode takes past that bound fails the run at `scripts/fuzz.sh`'s limit.
// Verifies: Q-3
#![no_main]

use forager_encode::{MAX_PIXMAP_SIDE, decode_pixmap, pixmap_size};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let size = pixmap_size(data);
    if let Ok(image) = decode_pixmap(data) {
        let (width, height) = (image.width(), image.height());
        assert_eq!(
            size,
            Ok((width, height)),
            "the image is not of its header's size"
        );
        assert!(width <= MAX_PIXMAP_SIDE && height <= MAX_PIXMAP_SIDE);
        assert_eq!(image.pixels().len(), width as usize * height as usize * 4);
    }
});
