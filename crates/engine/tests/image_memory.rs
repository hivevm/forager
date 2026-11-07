//! The images one render holds decoded come to at most 256 MiB (Q-9): images of 4096 × 4096
//! pixels, 64 MiB each decoded, are taken by the store in turn; four are held, and the fifth,
//! which would pass the bound, is refused before it is decoded. Measured under a counting
//! allocator; one test, so that no other test's allocations are counted with it.

use std::future::Future;
use std::rc::Rc;

use forager_engine::images::{Images, MAX_DECODED_IMAGE_BYTES};
use forager_ports::{ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

const MIB: usize = 1024 * 1024;

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the store waited on a port that never waits")
}

/// The same PNG under every path.
struct Same(Vec<u8>);

impl ResourceLoader for Same {
    async fn load(&self, _: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        Ok(self.0.clone())
    }
}

#[test]
fn the_image_that_would_pass_256_mib_is_refused_before_it_is_decoded() {
    // Verifies: Q-9, ADR-0040
    let side = 4096;
    let file = forager_encode::png(&RgbaImage {
        width: side,
        height: side,
        pixels: vec![0; side as usize * side as usize * 4],
    })
    .unwrap();
    let files = Same(file);
    let decoded = side as usize * side as usize * 4;
    assert_eq!(4 * decoded, MAX_DECODED_IMAGE_BYTES);

    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let mut images = Images::default();
    let held: Vec<_> = (0..4)
        .map(|i| ready(images.image(&format!("{i}.png"), &files)).unwrap())
        .collect();
    let peak = ALLOCATOR.peak_usage() - before;
    assert!(
        peak < MAX_DECODED_IMAGE_BYTES + 4 * MIB,
        "a peak of {peak} bytes for four images"
    );

    let level = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let why = ready(images.image("4.png", &files)).unwrap_err();
    assert!(why.contains("4.png"), "{why}");
    assert!(
        why.contains(&format!(
            "past the limit of {MAX_DECODED_IMAGE_BYTES} bytes"
        )),
        "{why}"
    );
    let peak = ALLOCATOR.peak_usage() - level;
    assert!(peak < MIB, "a peak of {peak} bytes for the image refused");

    // The others are still held.
    for (i, image) in held.iter().enumerate() {
        let again = ready(images.image(&format!("{i}.png"), &files)).unwrap();
        assert!(Rc::ptr_eq(image, &again));
    }
}
