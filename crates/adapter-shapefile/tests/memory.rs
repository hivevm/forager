//! The memory the shapefile source takes follows the files' size (ADR-0024): each case of the
//! ADR's *Enforcement* is opened and queried under a counting allocator, and its peak may not
//! pass four times the files' size plus one mebibyte. One test, so that no other test's
//! allocations are counted with it.

mod common;

use common::*;
use peak_alloc::PeakAlloc;

// Counts the bytes allocated, and their peak, around the system allocator.
#[global_allocator]
static ALLOCATOR: PeakAlloc = PeakAlloc;

const MIB: usize = 1024 * 1024;

/// Opens and queries `files` with the whole plane and with an extent across its bounds, and
/// returns the peak of the bytes allocated meanwhile, the copies the loader hands over included,
/// with the number of features of the whole plane.
fn peak(files: &Files) -> (usize, usize) {
    let before = ALLOCATOR.current_usage();
    ALLOCATOR.reset_peak_usage();
    let count = {
        let source = open(&files.loader()).expect("the shapefile opens");
        let all = query(&source, EVERYWHERE).len();
        let _ = query(&source, [0.5, 0.5, 2.0, 2.0]);
        all
    };
    (ALLOCATOR.peak_usage() - before, count)
}

fn bounded(case: &str, files: &Files) -> usize {
    let (peak, count) = peak(files);
    let bound = 4 * files.size() + MIB;
    assert!(
        peak < bound,
        "{case}: a peak of {peak} bytes for {} bytes of files",
        files.size()
    );
    count
}

/// An arc of `points` points in one part.
fn long_arc(points: usize) -> Vec<u8> {
    let line: Vec<(f64, f64)> = (0..points).map(|i| (i as f64 % 10.0, 1.0)).collect();
    lines(ARC, &[&line])
}

#[test]
fn memory_follows_the_files_size() {
    // Verifies: ADR-0024
    const BOUNDS: [f64; 4] = [0.0, 0.0, 10.0, 10.0];

    // A record with counts under the guards but past its size.
    let mut capped = lines(ARC, &[&[(0.0, 0.0), (1.0, 1.0)]]);
    capped[36..40].copy_from_slice(&10_000_000i32.to_le_bytes());
    capped[40..44].copy_from_slice(&50_000_000i32.to_le_bytes());
    let files = Files::new(ARC, BOUNDS, &vec![capped; 1000]);
    assert_eq!(bounded("counts past the size", &files), 0);

    // A record whose offset or size lies outside the .shp.
    let mut files = Files::new(ARC, BOUNDS, &[long_arc(2)]);
    files.set_shx_raw(
        ARC,
        BOUNDS,
        &[
            (50, i32::MAX / 2 - 1),
            (i32::MAX / 2 - 1, 30),
            (50, 1 << 29),
        ],
    );
    assert_eq!(bounded("outside the .shp", &files), 0);

    // A .shx header whose record count exceeds the entries the file holds.
    let mut files = Files::new(POINT, BOUNDS, &vec![point(1.0, 1.0); 3]);
    files.shx[24..28].copy_from_slice(&(50i32 + 4 * 256_000_000).to_be_bytes());
    assert_eq!(bounded("a record count past the entries", &files), 0);

    // A .shx that points many entries at one large record, read until the byte total is spent.
    let mut files = Files::new(ARC, BOUNDS, &[long_arc(50_000)]);
    let size = files.shp.len() - 108;
    files.set_shx(ARC, BOUNDS, &vec![(100, size); 2_000]);
    assert_eq!(bounded("many entries at one record", &files), 1);

    // Many small records, each read as a feature, and many parts of one point each.
    let points: Vec<Vec<u8>> = (0..100_000)
        .map(|i| point(f64::from(i % 10), 1.0))
        .collect();
    let files = Files::new(POINT, BOUNDS, &points);
    assert_eq!(bounded("many points", &files), 100_000);
    let one: &[(f64, f64)] = &[(1.0, 1.0)];
    let files = Files::new(ARC, BOUNDS, &[lines(ARC, &vec![one; 50_000])]);
    assert_eq!(bounded("many parts", &files), 1);
}
