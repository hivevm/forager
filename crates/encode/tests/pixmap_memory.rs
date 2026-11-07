//! A pixmap's header is held to Q-9's 4096 × 4096 pixels before anything is allocated for it,
//! and a header within the limit with few bytes behind it allocates nothing for its pixels
//! either, as a reader of untrusted bytes must (docs/CONVENTIONS.md). Measured under a counting
//! allocator; one test, so that no other test's allocations are counted with it.

use forager_encode::{PixmapError, decode_pixmap};
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

/// The signature and header of an 8-bit RGBA PNG of `width` × `height` pixels, and a few bytes
/// of an image data chunk behind them.
fn claimed(width: u32, height: u32) -> Vec<u8> {
    let mut header = b"IHDR".to_vec();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(&header);
    png.extend_from_slice(&crc32(&header).to_be_bytes());
    png.extend_from_slice(&[0, 0, 0, 8, b'I', b'D', b'A', b'T', 0x78, 0x01, 0, 0]);
    png
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The result of decoding `png`, and the peak of the bytes allocated meanwhile.
fn peak(png: &[u8]) -> (Result<(), PixmapError>, usize) {
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let result = decode_pixmap(png).map(|_| ());
    (result, ALLOCATOR.peak_usage() - before)
}

#[test]
fn a_claimed_size_allocates_nothing_for_it() {
    // Verifies: Q-9
    const BOUND: usize = 1024 * 1024;
    for (width, height) in [(4097, 4096), (4096, 4097)] {
        let (result, peak) = peak(&claimed(width, height));
        assert_eq!(result, Err(PixmapError::TooLarge { width, height }));
        assert!(peak < BOUND, "{width} × {height}: a peak of {peak} bytes");
    }
    // Within the limit, 64 MiB of pixels claimed by a few bytes.
    let (result, peak) = peak(&claimed(4096, 4096));
    assert!(matches!(result, Err(PixmapError::Broken(_))), "{result:?}");
    assert!(peak < BOUND, "4096 × 4096: a peak of {peak} bytes");
}
