//! The metric of the conformance harness: a rendering and its reference image are compared as
//! RGBA buffers of the same size. A pixel differs when one of its channels differs by more than
//! the threshold; a case passes when the share of differing pixels is at most the tolerance. A
//! reference image of another mode, palette or RGB, is expanded to RGBA by the `png` crate here
//! (ADR-0017); the encoder's own decoding stays the strict one of G-12.

use std::fmt;
use std::path::Path;

use forager_render::RgbaImage;

#[cfg(test)]
mod tests;

/// The threshold and the tolerance a category is compared with.
pub struct Metric {
    /// The largest difference of one channel that still counts as equal.
    pub threshold: u8,
    /// The largest share of differing pixels, from 0 to 1, with which a case passes.
    pub tolerance: f64,
}

/// The outcome of comparing a rendering with its reference image.
#[derive(Debug)]
pub enum Comparison {
    /// The images are of one size; `differing` of their `pixels` differ.
    Measured {
        differing: u64,
        pixels: u64,
        passes: bool,
    },
    /// The images are of different sizes, as width and height; such a case fails.
    SizesDiffer {
        rendering: (u32, u32),
        reference: (u32, u32),
    },
}

impl Comparison {
    /// Whether the case passes.
    pub fn passes(&self) -> bool {
        matches!(self, Self::Measured { passes: true, .. })
    }

    /// The share of differing pixels, or `None` when the sizes differ.
    pub fn share(&self) -> Option<f64> {
        match *self {
            Self::Measured {
                differing, pixels, ..
            } => Some(share(differing, pixels)),
            Self::SizesDiffer { .. } => None,
        }
    }
}

impl fmt::Display for Comparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measured {
                differing, pixels, ..
            } => write!(
                f,
                "{differing} of {pixels} pixels differ ({:.4})",
                share(*differing, *pixels)
            ),
            Self::SizesDiffer {
                rendering,
                reference,
            } => write!(
                f,
                "the rendering is {}×{}, the reference {}×{}",
                rendering.0, rendering.1, reference.0, reference.1
            ),
        }
    }
}

/// Compares `rendering` with `reference` by `metric`.
pub fn compare(rendering: &RgbaImage, reference: &RgbaImage, metric: &Metric) -> Comparison {
    let size = (rendering.width, rendering.height);
    let reference_size = (reference.width, reference.height);
    if size != reference_size {
        return Comparison::SizesDiffer {
            rendering: size,
            reference: reference_size,
        };
    }
    let differing = rendering
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(reference.pixels.as_chunks::<4>().0.iter())
        .filter(|(a, b)| differs(a, b, metric.threshold))
        .count() as u64;
    let pixels = u64::from(size.0) * u64::from(size.1);
    Comparison::Measured {
        differing,
        pixels,
        // The share against the tolerance, not the count against the tolerance times the pixels:
        // a decimal tolerance such as 0.29 times 100 rounds below 29, where 29 / 100 rounds to
        // the same double as 0.29 does.
        passes: share(differing, pixels) <= metric.tolerance,
    }
}

/// The share of `differing` among `pixels`; an image without pixels has none differing.
fn share(differing: u64, pixels: u64) -> f64 {
    if pixels == 0 {
        0.0
    } else {
        differing as f64 / pixels as f64
    }
}

/// Compares as [`compare`] does and, when the case fails, writes the rendering as
/// `<name>.png` and a difference image as `<name>.diff.png` into `dir` for the maintainer. An
/// image without pixels has nothing to show and is not written, as PNG holds none.
pub fn compare_and_keep(
    rendering: &RgbaImage,
    reference: &RgbaImage,
    metric: &Metric,
    dir: &Path,
    name: &str,
) -> std::io::Result<Comparison> {
    let comparison = compare(rendering, reference, metric);
    if !comparison.passes() {
        std::fs::create_dir_all(dir)?;
        let difference = difference(rendering, reference, metric.threshold);
        for (file, image) in [
            (format!("{name}.png"), rendering),
            (format!("{name}.diff.png"), &difference),
        ] {
            if image.width == 0 || image.height == 0 {
                continue;
            }
            let bytes = forager_encode::png(image).map_err(std::io::Error::other)?;
            std::fs::write(dir.join(file), bytes)?;
        }
    }
    Ok(comparison)
}

/// Whether `reference` is blank by `metric`: nearly one colour, in that an image of its most
/// common colour passes against it. Of the colours equally common, the least by its channels is
/// taken, so that the answer does not depend on the order of the pixels.
pub fn blank(reference: &RgbaImage, metric: &Metric) -> bool {
    let mut counts = std::collections::BTreeMap::new();
    for pixel in reference.pixels.as_chunks::<4>().0 {
        *counts.entry(*pixel).or_insert(0usize) += 1;
    }
    let Some((&colour, _)) = counts
        .iter()
        .max_by(|(a, m), (b, n)| m.cmp(n).then_with(|| b.cmp(a)))
    else {
        return true;
    };
    let plain = RgbaImage {
        width: reference.width,
        height: reference.height,
        pixels: colour.repeat(reference.pixels.len() / 4),
    };
    compare(&plain, reference, metric).passes()
}

/// Decodes a PNG of any colour type and bit depth into 8-bit RGBA.
pub fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("the PNG is too large")?];
    let frame = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    let samples = frame.line_size * frame.height as usize;
    let buffer = &buffer[..samples];
    let pixels = match frame.color_type {
        png::ColorType::Rgba => buffer.to_vec(),
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return Err("the palette was not expanded".to_owned()),
    };
    Ok(RgbaImage {
        width: frame.width,
        height: frame.height,
        pixels,
    })
}

/// Whether two RGBA pixels differ in one channel by more than `threshold`.
fn differs(a: &[u8; 4], b: &[u8; 4], threshold: u8) -> bool {
    a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > threshold)
}

/// An image over both sizes: a differing pixel, or one only one image has, opaque red; any other
/// the reference's grey, faded, so that the drawing stays recognizable around the differences.
fn difference(rendering: &RgbaImage, reference: &RgbaImage, threshold: u8) -> RgbaImage {
    let width = rendering.width.max(reference.width);
    let height = rendering.height.max(reference.height);
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let pixel = match (rendering.pixel(x, y), reference.pixel(x, y)) {
                (Some(a), Some(b))
                    if !differs(&[a.r, a.g, a.b, a.a], &[b.r, b.g, b.b, b.a], threshold) =>
                {
                    let grey = ((u16::from(b.r) + u16::from(b.g) + u16::from(b.b)) / 3) as u8;
                    [grey, grey, grey, b.a / 4]
                }
                _ => [255, 0, 0, 255],
            };
            pixels.extend_from_slice(&pixel);
        }
    }
    RgbaImage {
        width,
        height,
        pixels,
    }
}
