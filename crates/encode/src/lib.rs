//! Encodes an RGBA image as PNG, and decodes one back (ADR-0017).

use forager_render::RgbaImage;

/// Encodes `image` as an 8-bit RGBA PNG.
pub fn png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&image.pixels)
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Decodes an 8-bit RGBA PNG, as [`png`] writes it.
pub fn decode_png(bytes: &[u8]) -> Result<RgbaImage, String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("only 8-bit RGBA PNG is read".to_owned());
    }
    let (width, height) = (info.width, info.height);
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("the PNG is too large")?];
    reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    Ok(RgbaImage {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_survives_png_unchanged() {
        // Verifies: G-12, ADR-0017
        let pixels: Vec<u8> = (0..7 * 5 * 4).map(|i| (i * 37 % 256) as u8).collect();
        let image = RgbaImage {
            width: 7,
            height: 5,
            pixels,
        };
        assert_eq!(decode_png(&png(&image).unwrap()).unwrap(), image);
    }

    #[test]
    fn a_broken_png_is_an_error() {
        assert!(decode_png(b"not a png").is_err());
    }
}
