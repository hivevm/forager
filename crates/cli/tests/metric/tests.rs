use std::path::PathBuf;

use forager_render::RgbaImage;

use super::{Comparison, Metric, compare, compare_and_keep, decode};

const METRIC: Metric = Metric {
    threshold: 10,
    tolerance: 0.25,
};

/// A 4 by 4 image of one colour.
fn plain(rgba: [u8; 4]) -> RgbaImage {
    RgbaImage {
        width: 4,
        height: 4,
        pixels: rgba.repeat(16),
    }
}

/// `image` with its pixel `i` moved by `delta` in its green channel.
fn nudged(mut image: RgbaImage, i: usize, delta: u8) -> RgbaImage {
    image.pixels[i * 4 + 1] += delta;
    image
}

fn differing(comparison: &Comparison) -> u64 {
    match comparison {
        Comparison::Measured { differing, .. } => *differing,
        Comparison::SizesDiffer { .. } => panic!("the sizes are equal: {comparison}"),
    }
}

fn png_bytes(
    width: u32,
    height: u32,
    color: png::ColorType,
    data: &[u8],
    palette: Option<(&[u8], &[u8])>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    if let Some((palette, trns)) = palette {
        encoder.set_palette(palette.to_vec());
        encoder.set_trns(trns.to_vec());
    }
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(data).unwrap();
    writer.finish().unwrap();
    bytes
}

/// An empty directory of its own for one test, under the target directory.
fn out_dir(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("metric")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn equal_buffers_have_no_share_and_pass() {
    let image = plain([10, 20, 30, 255]);
    let comparison = compare(&image, &image.clone(), &METRIC);
    assert_eq!(comparison.share(), Some(0.0));
    assert!(comparison.passes());
}

#[test]
fn a_channel_just_over_the_threshold_differs_and_one_at_it_does_not() {
    let reference = plain([10, 20, 30, 255]);
    let over = nudged(reference.clone(), 5, METRIC.threshold + 1);
    assert_eq!(differing(&compare(&over, &reference, &METRIC)), 1);
    let at = nudged(reference.clone(), 5, METRIC.threshold);
    assert_eq!(differing(&compare(&at, &reference, &METRIC)), 0);
}

#[test]
fn the_threshold_holds_in_either_direction_and_in_every_channel() {
    let reference = plain([100, 100, 100, 100]);
    for channel in 0..4 {
        let mut rendering = reference.clone();
        rendering.pixels[channel] -= METRIC.threshold + 1;
        assert_eq!(differing(&compare(&rendering, &reference, &METRIC)), 1);
    }
}

#[test]
fn a_share_at_the_tolerance_passes_and_one_pixel_above_it_fails() {
    let reference = plain([10, 20, 30, 255]);
    // A quarter of the 16 pixels is the tolerance.
    let at = (0..4).fold(reference.clone(), |image, i| nudged(image, i, 50));
    let comparison = compare(&at, &reference, &METRIC);
    assert_eq!(comparison.share(), Some(0.25));
    assert!(comparison.passes());
    let above = nudged(at, 4, 50);
    assert!(!compare(&above, &reference, &METRIC).passes());
}

#[test]
fn images_of_different_sizes_fail_naming_both_sizes() {
    let rendering = plain([0, 0, 0, 255]);
    let reference = RgbaImage {
        width: 3,
        height: 5,
        pixels: vec![0; 3 * 5 * 4],
    };
    let comparison = compare(&rendering, &reference, &METRIC);
    assert!(!comparison.passes());
    assert_eq!(comparison.share(), None);
    let message = comparison.to_string();
    assert!(message.contains("4×4"), "{message}");
    assert!(message.contains("3×5"), "{message}");
}

#[test]
fn a_palette_png_decodes_to_the_rgba_image_of_its_pixels() {
    let palette = [255, 0, 0, 0, 0, 255];
    let trns = [255, 128];
    let bytes = png_bytes(
        2,
        1,
        png::ColorType::Indexed,
        &[0, 1],
        Some((&palette, &trns)),
    );
    let expected = RgbaImage {
        width: 2,
        height: 1,
        pixels: vec![255, 0, 0, 255, 0, 0, 255, 128],
    };
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(compare(&decoded, &expected, &METRIC).share(), Some(0.0));
}

#[test]
fn an_rgb_png_decodes_to_the_opaque_rgba_image_of_its_pixels() {
    let bytes = png_bytes(2, 1, png::ColorType::Rgb, &[1, 2, 3, 4, 5, 6], None);
    let expected = RgbaImage {
        width: 2,
        height: 1,
        pixels: vec![1, 2, 3, 255, 4, 5, 6, 255],
    };
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(compare(&decoded, &expected, &METRIC).share(), Some(0.0));
}

#[test]
fn a_broken_png_is_an_error() {
    assert!(decode(b"not a png").is_err());
}

#[test]
fn a_failing_comparison_leaves_the_rendering_and_a_difference_image() {
    let dir = out_dir("failing");
    let reference = plain([10, 20, 30, 255]);
    let rendering = (0..5).fold(reference.clone(), |image, i| nudged(image, i, 50));
    let comparison = compare_and_keep(&rendering, &reference, &METRIC, &dir, "case").unwrap();
    assert!(!comparison.passes());
    let kept = decode(&std::fs::read(dir.join("case.png")).unwrap()).unwrap();
    assert_eq!(kept, rendering);
    let difference = decode(&std::fs::read(dir.join("case.diff.png")).unwrap()).unwrap();
    assert_eq!((difference.width, difference.height), (4, 4));
    let marked = difference
        .pixels
        .chunks(4)
        .filter(|p| *p == [255, 0, 0, 255])
        .count();
    assert_eq!(marked, 5);
}

#[test]
fn images_of_different_sizes_leave_a_difference_image_over_both() {
    let dir = out_dir("sizes");
    let rendering = plain([0, 0, 0, 255]);
    let reference = RgbaImage {
        width: 3,
        height: 5,
        pixels: [0, 0, 0, 255].repeat(15),
    };
    compare_and_keep(&rendering, &reference, &METRIC, &dir, "case").unwrap();
    assert!(dir.join("case.png").is_file());
    let difference = decode(&std::fs::read(dir.join("case.diff.png")).unwrap()).unwrap();
    assert_eq!((difference.width, difference.height), (4, 5));
}

#[test]
fn a_passing_comparison_leaves_nothing() {
    let dir = out_dir("passing");
    let image = plain([10, 20, 30, 255]);
    let comparison = compare_and_keep(&image, &image.clone(), &METRIC, &dir, "case").unwrap();
    assert!(comparison.passes());
    assert!(!dir.exists());
}
