// The source follows MapServer's readPNG and msLoadMSRasterBufferFromFile, src/mapimageio.c, of
// the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! Decodes a pixmap symbol's image as MapServer's `readPNG` decodes it (`src/mapimageio.c`,
//! 629–733, at the pin), within the size Q-9 allows, premultiplied (ADR-0017, ADR-0040).

use std::fmt;

use forager_render::PremultipliedImage;

/// The largest width or height of a pixmap symbol's image (Q-9).
pub const MAX_PIXMAP_SIDE: u32 = 4096;

/// Why an image is not decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PixmapError {
    /// It is shorter than a signature, so its format cannot be told.
    TooShort,
    /// Its first bytes are not PNG's signature.
    Unsupported,
    /// Its header states more than [`MAX_PIXMAP_SIDE`] pixels across or down.
    TooLarge { width: u32, height: u32 },
    /// It starts as a PNG but does not decode.
    Broken(String),
}

impl fmt::Display for PixmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "is too short to tell its format"),
            Self::Unsupported => write!(f, "is of an unsupported format: only PNG is read"),
            Self::TooLarge { width, height } => write!(
                f,
                "is {width} × {height} pixels, past the limit of \
                 {MAX_PIXMAP_SIDE} × {MAX_PIXMAP_SIDE}"
            ),
            Self::Broken(why) => write!(f, "is a broken PNG: {why}"),
        }
    }
}

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// The most bytes the decoder may take for everything but the pixels, together: the chunks it
/// buffers and its line buffer. The text and ICC profile chunks, which `readPNG` reads neither,
/// are skipped and take none of it. Its number is libpng's `PNG_USER_CHUNK_MALLOC_MAX` in the
/// `pnglibconf.h` it builds with, which bounds each chunk apart.
const MAX_DECODER_BYTES: usize = 8_000_000;

/// The most bytes deflate makes of one, 1032 to 1, its best case: a match of 258 bytes coded in
/// two bits.
const MAX_DEFLATE_RATIO: u64 = 1032;

/// The width and height a PNG's header states, read from its first bytes, allocating nothing;
/// an image past [`MAX_PIXMAP_SIDE`] is refused. The format is told by the signature, as
/// `msLoadMSRasterBufferFromFile` (`src/mapimageio.c`, 1140–1174) tells it.
pub fn pixmap_size(bytes: &[u8]) -> Result<(u32, u32), PixmapError> {
    // `msLoadMSRasterBufferFromFile` fails a file it cannot read eight bytes of with "Unable to
    // read header" (1150–1155), before it tells the format.
    let Some(signature) = bytes.get(..8) else {
        return Err(PixmapError::TooShort);
    };
    if signature != SIGNATURE {
        return Err(PixmapError::Unsupported);
    }
    let header = bytes
        .get(8..24)
        .filter(|header| header[..8] == *b"\x00\x00\x00\x0dIHDR")
        .ok_or_else(|| PixmapError::Broken("it has no image header".to_owned()))?;
    let word = |at: usize| {
        u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let (width, height) = (word(8), word(12));
    if width > MAX_PIXMAP_SIDE || height > MAX_PIXMAP_SIDE {
        return Err(PixmapError::TooLarge { width, height });
    }
    Ok((width, height))
}

/// Decodes a PNG as `readPNG` decodes it: a palette, grey below 8 bits, and a `tRNS` chunk
/// expanded, 16 bits cut to their high byte, grey turned to RGB, an opaque alpha added where
/// there is none, the image whole where it is interlaced, and its end read; then each pixel
/// whose alpha is below 255 premultiplied as `(c * a + 255) >> 8`, a transparent one to 0
/// (714–729). Nothing is allocated for the pixels before the size is held to
/// [`MAX_PIXMAP_SIDE`] and the bytes present could hold them.
///
/// Two steps are libpng's, read in libpng 1.6.43 as Ubuntu 24.04 ships it (`1.6.43-5ubuntu0.7`),
/// which MapServer links in the noise build: `png_set_strip_16` keeps a sample's high byte
/// (`png_do_chop`, `pngrtran.c`, 2491–2516), not the rounded value of `png_set_scale_16`; and
/// `png_read_image`, where `png_set_interlace_handling` was not called, warns and turns it on
/// itself (`pngread.c`, 722–741), so that an interlaced image comes out whole.
///
/// Two malformed files decode otherwise than libpng decodes them: a grey or RGB `tRNS` chunk of
/// the wrong length, which libpng ignores (`png_handle_tRNS`, `pngrutil.c`, 1843, 1859) and the
/// `png` crate uses; and a grey key past the range of an image below 8 bits, which libpng masks
/// to the depth (`png_do_expand`, `pngrtran.c`, 4449 and on) and the `png` crate does not.
pub fn decode_pixmap(bytes: &[u8]) -> Result<PremultipliedImage, PixmapError> {
    let (width, height) = pixmap_size(bytes)?;
    let broken = |e: png::DecodingError| PixmapError::Broken(e.to_string());
    // The bytes of the scanlines not interlaced, fewer than interlaced, each its filter byte
    // and its samples; the colour type and depth follow the size in the header.
    let samples = match bytes.get(25) {
        Some(2) => 3,
        Some(4) => 2,
        Some(6) => 4,
        _ => 1,
    };
    let bits = u64::from(bytes.get(24).copied().unwrap_or(1)) * samples;
    let scanlines = u64::from(height) * (1 + (u64::from(width) * bits).div_ceil(8));
    if scanlines > bytes.len() as u64 * MAX_DEFLATE_RATIO {
        return Err(PixmapError::Broken(
            "it holds too few bytes for the pixels its header states".to_owned(),
        ));
    }

    let limits = png::Limits {
        bytes: MAX_DECODER_BYTES,
    };
    let mut decoder = png::Decoder::new_with_limits(std::io::Cursor::new(bytes), limits);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(broken)?;
    let channels = match reader.output_color_type() {
        (png::ColorType::Grayscale, png::BitDepth::Eight) => 1,
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => 2,
        (png::ColorType::Rgb, png::BitDepth::Eight) => 3,
        (png::ColorType::Rgba, png::BitDepth::Eight) => 4,
        (color, depth) => {
            return Err(PixmapError::Broken(format!(
                "it decodes to {color:?} of {depth:?} bits, which is not expanded"
            )));
        }
    };
    let count = width as usize * height as usize;
    let decoded = count * channels;
    if reader.output_buffer_size() != Some(decoded) {
        return Err(PixmapError::Broken(
            "its rows are not of the size its header states".to_owned(),
        ));
    }
    // Decoded into the front of the image's own bytes, and widened in place from the back.
    let mut pixels = vec![0; count * 4];
    reader.next_frame(&mut pixels[..decoded]).map_err(broken)?;
    reader.finish().map_err(broken)?;
    for i in (0..count).rev() {
        let at = i * channels;
        let rgba = match channels {
            1 => [pixels[at], pixels[at], pixels[at], 255],
            2 => [pixels[at], pixels[at], pixels[at], pixels[at + 1]],
            3 => [pixels[at], pixels[at + 1], pixels[at + 2], 255],
            _ => [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]],
        };
        pixels[i * 4..i * 4 + 4].copy_from_slice(&premultiplied(rgba));
    }
    Ok(PremultipliedImage::new(width, height, pixels).expect("four bytes for each pixel"))
}

/// A pixel premultiplied as `readPNG` premultiplies it (714–729).
fn premultiplied([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    match a {
        255 => [r, g, b, a],
        0 => [0, 0, 0, 0],
        _ => {
            let times = |c: u8| ((u32::from(c) * u32::from(a) + 255) >> 8) as u8;
            [times(r), times(g), times(b), a]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // PNG's colour types.
    const GREY: u8 = 0;
    const RGB: u8 = 2;
    const PALETTE: u8 = 3;
    const GREY_ALPHA: u8 = 4;
    const RGBA: u8 = 6;

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

    fn chunk(file: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        file.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = file.len();
        file.extend_from_slice(kind);
        file.extend_from_slice(data);
        let crc = crc32(&file[start..]);
        file.extend_from_slice(&crc.to_be_bytes());
    }

    /// `data` as a zlib stream of stored deflate blocks.
    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = data.chunks(65_535).collect();
        let blocks = if blocks.is_empty() {
            vec![&[][..]]
        } else {
            blocks
        };
        for (i, block) in blocks.iter().enumerate() {
            out.push(u8::from(i + 1 == blocks.len()));
            let len = block.len() as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(block);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + u32::from(byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        out.extend_from_slice(&((b << 16) | a).to_be_bytes());
        out
    }

    /// A PNG of `width` × `height` pixels of colour type `color` and bit depth `depth`, with
    /// the chunks `extra` before its data, whose scanlines are `rows`, each packed as PNG packs
    /// it without its filter byte. Interlaced, its depth is a whole number of bytes per pixel.
    fn file(
        (width, height): (u32, u32),
        color: u8,
        depth: u8,
        interlaced: bool,
        extra: &[(&[u8; 4], &[u8])],
        rows: &[&[u8]],
    ) -> Vec<u8> {
        let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[depth, color, 0, 0, u8::from(interlaced)]);
        chunk(&mut file, b"IHDR", &header);
        for (kind, data) in extra {
            chunk(&mut file, kind, data);
        }
        let mut raw = Vec::new();
        if interlaced {
            // Adam7: each pass's start and step across and down.
            const PASSES: [(usize, usize, usize, usize); 7] = [
                (0, 0, 8, 8),
                (4, 0, 8, 8),
                (0, 4, 4, 8),
                (2, 0, 4, 4),
                (0, 2, 2, 4),
                (1, 0, 2, 2),
                (0, 1, 1, 2),
            ];
            let size = rows[0].len() / width as usize;
            for (x0, y0, dx, dy) in PASSES {
                for y in (y0..height as usize).step_by(dy) {
                    let pixels: Vec<u8> = (x0..width as usize)
                        .step_by(dx)
                        .flat_map(|x| rows[y][x * size..(x + 1) * size].to_vec())
                        .collect();
                    if !pixels.is_empty() {
                        raw.push(0);
                        raw.extend_from_slice(&pixels);
                    }
                }
            }
        } else {
            for row in rows {
                raw.push(0);
                raw.extend_from_slice(row);
            }
        }
        chunk(&mut file, b"IDAT", &zlib(&raw));
        chunk(&mut file, b"IEND", &[]);
        file
    }

    fn decoded(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let image = decode_pixmap(bytes).unwrap();
        (image.width(), image.height(), image.pixels().to_vec())
    }

    #[test]
    fn a_palette_is_expanded_opaque() {
        // Verifies: ADR-0017
        let png = file(
            (3, 1),
            PALETTE,
            8,
            false,
            &[(b"PLTE", &[10, 20, 30, 200, 100, 50, 255, 255, 255])],
            &[&[0, 1, 2]],
        );
        let expected = vec![10, 20, 30, 255, 200, 100, 50, 255, 255, 255, 255, 255];
        assert_eq!(decoded(&png), (3, 1, expected));
    }

    #[test]
    fn a_palette_with_trns_takes_its_alphas_premultiplied() {
        // Verifies: ADR-0040
        // Two alphas for three entries: the third is opaque, as libpng reads it.
        let png = file(
            (3, 1),
            PALETTE,
            8,
            false,
            &[
                (b"PLTE", &[10, 20, 30, 200, 100, 50, 255, 255, 255]),
                (b"tRNS", &[0, 128]),
            ],
            &[&[0, 1, 2]],
        );
        let expected = vec![0, 0, 0, 0, 100, 50, 25, 128, 255, 255, 255, 255];
        assert_eq!(decoded(&png), (3, 1, expected));
    }

    fn grey(levels: &[u8]) -> Vec<u8> {
        levels.iter().flat_map(|&v| [v, v, v, 255]).collect()
    }

    #[test]
    fn grey_below_8_bits_is_scaled_as_png_do_expand_scales_it() {
        // Verifies: ADR-0017
        // 1 bit: 0 or 255, ten pixels across two bytes.
        let png = file((10, 1), GREY, 1, false, &[], &[&[0b1011_0001, 0b1000_0000]]);
        let expected = grey(&[255, 0, 255, 255, 0, 0, 0, 255, 255, 0]);
        assert_eq!(decoded(&png), (10, 1, expected));
        // 2 bits: each level times 0x55.
        let png = file((4, 1), GREY, 2, false, &[], &[&[0b0001_1011]]);
        assert_eq!(decoded(&png), (4, 1, grey(&[0, 85, 170, 255])));
        // 4 bits: each level times 0x11.
        let png = file((3, 1), GREY, 4, false, &[], &[&[0x05, 0xF0]]);
        assert_eq!(decoded(&png), (3, 1, grey(&[0, 85, 255])));
    }

    #[test]
    fn grey_of_8_bits_turns_rgb_and_opaque() {
        // Verifies: ADR-0017
        let png = file((3, 1), GREY, 8, false, &[], &[&[0, 77, 255]]);
        assert_eq!(decoded(&png), (3, 1, grey(&[0, 77, 255])));
    }

    #[test]
    fn grey_with_alpha_turns_rgb_premultiplied() {
        // Verifies: ADR-0040
        let png = file(
            (4, 1),
            GREY_ALPHA,
            8,
            false,
            &[],
            &[&[100, 255, 100, 128, 100, 0, 255, 1]],
        );
        let expected = vec![100, 100, 100, 255, 50, 50, 50, 128, 0, 0, 0, 0, 1, 1, 1, 1];
        assert_eq!(decoded(&png), (4, 1, expected));
    }

    #[test]
    fn rgb_gains_an_opaque_alpha() {
        // Verifies: ADR-0017
        let png = file((2, 1), RGB, 8, false, &[], &[&[1, 2, 3, 250, 251, 252]]);
        let expected = vec![1, 2, 3, 255, 250, 251, 252, 255];
        assert_eq!(decoded(&png), (2, 1, expected));
    }

    /// Nine RGBA pixels, three by three, and what `readPNG` makes of them: an opaque pixel is
    /// kept, a transparent one is zeroed, and every other is premultiplied as
    /// `(c * a + 255) >> 8`.
    const RGBA_ROWS: [[u8; 12]; 3] = [
        [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0],
        [10, 20, 30, 254, 200, 150, 100, 1, 255, 255, 255, 64],
        [99, 0, 7, 200, 1, 2, 3, 255, 128, 64, 32, 32],
    ];
    const RGBA_PREMULTIPLIED: [u8; 36] = [
        255, 0, 0, 255, 0, 128, 0, 128, 0, 0, 0, 0, //
        10, 20, 30, 254, 1, 1, 1, 1, 64, 64, 64, 64, //
        78, 0, 6, 200, 1, 2, 3, 255, 16, 8, 4, 32,
    ];

    #[test]
    fn rgba_is_premultiplied_where_its_alpha_is_below_255() {
        // Verifies: ADR-0040
        let rows: Vec<&[u8]> = RGBA_ROWS.iter().map(|r| &r[..]).collect();
        let png = file((3, 3), RGBA, 8, false, &[], &rows);
        assert_eq!(decoded(&png), (3, 3, RGBA_PREMULTIPLIED.to_vec()));
    }

    #[test]
    fn an_interlaced_image_yields_the_bytes_of_the_same_image_not_interlaced() {
        // Verifies: ADR-0017
        // libpng's png_read_image turns the interlace handling on itself (`decode_pixmap`).
        let rows: Vec<&[u8]> = RGBA_ROWS.iter().map(|r| &r[..]).collect();
        let png = file((3, 3), RGBA, 8, true, &[], &rows);
        assert_eq!(decoded(&png), (3, 3, RGBA_PREMULTIPLIED.to_vec()));
    }

    #[test]
    fn sixteen_bits_keep_the_high_byte_as_png_set_strip_16_does() {
        // Verifies: ADR-0017
        let row: &[u8] = &[
            0x12, 0x34, 0xAB, 0xCD, 0xFF, 0x00, 0x80, 0xFF, // alpha 0x80: premultiplied
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0xFF, 0x00, // alpha 0xFF: opaque
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFF, // alpha 0x00: zeroed
        ];
        let png = file((3, 1), RGBA, 16, false, &[], &[row]);
        let expected = vec![9, 86, 128, 128, 1, 3, 5, 255, 0, 0, 0, 0];
        assert_eq!(decoded(&png), (3, 1, expected));
    }

    #[test]
    fn an_rgb_trns_key_makes_its_colour_transparent() {
        // Verifies: ADR-0017
        // Of an 8-bit image, the key's low bytes are compared (png_do_expand).
        let png = file(
            (2, 1),
            RGB,
            8,
            false,
            &[(b"tRNS", &[0, 10, 0, 20, 0, 30])],
            &[&[10, 20, 30, 10, 20, 31]],
        );
        let expected = vec![0, 0, 0, 0, 10, 20, 31, 255];
        assert_eq!(decoded(&png), (2, 1, expected));
    }

    #[test]
    fn a_16_bit_trns_key_is_matched_on_all_16_bits_before_the_chop() {
        // Verifies: ADR-0017
        let key: &[u8] = &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let row: &[u8] = &[
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, // the key: transparent
            0x12, 0x35, 0x56, 0x78, 0x9A, 0xBC, // the same high bytes: opaque
        ];
        let png = file((2, 1), RGB, 16, false, &[(b"tRNS", key)], &[row]);
        let expected = vec![0, 0, 0, 0, 0x12, 0x56, 0x9A, 0xFF];
        assert_eq!(decoded(&png), (2, 1, expected));
    }

    #[test]
    fn a_format_other_than_png_is_unsupported() {
        assert_eq!(
            decode_pixmap(b"GIF89a\x01\x00\x01\x00\x00\x00\x00"),
            Err(PixmapError::Unsupported)
        );
        assert_eq!(
            decode_pixmap(b"not an image at all"),
            Err(PixmapError::Unsupported)
        );
    }

    #[test]
    fn a_truncated_png_is_broken_and_a_short_file_too_short() {
        let png = file((3, 1), RGB, 8, false, &[], &[&[1, 2, 3, 4, 5, 6, 7, 8, 9]]);
        let truncated = &png[..png.len() - 20];
        assert!(matches!(
            decode_pixmap(truncated),
            Err(PixmapError::Broken(_))
        ));
        // The image's data complete, its end missing: libpng's png_read_end fails it.
        let without_end = &png[..png.len() - 12];
        assert!(matches!(
            decode_pixmap(without_end),
            Err(PixmapError::Broken(_))
        ));
        assert_eq!(decode_pixmap(&png[..6]), Err(PixmapError::TooShort));
    }

    #[test]
    fn a_header_past_4096_pixels_is_refused_and_4096_is_not() {
        // Verifies: Q-9
        let header = |size| file(size, RGBA, 8, false, &[], &[])[..33].to_vec();
        for (width, height) in [(4097, 4096), (4096, 4097)] {
            let png = header((width, height));
            let refused = PixmapError::TooLarge { width, height };
            assert_eq!(pixmap_size(&png), Err(refused.clone()));
            assert_eq!(decode_pixmap(&png), Err(refused));
        }
        assert_eq!(pixmap_size(&header((4096, 4096))), Ok((4096, 4096)));
    }
}
