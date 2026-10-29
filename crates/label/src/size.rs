// The sizes follow msComputeTextPath, src/maplabel.c, and msGetGlyphByIndex, src/fontcache.c, of
// MapServer at the release pinned in scripts/fetch-msautotest.sh; the pixel size is held as
// FreeType 2.13.3's FT_Set_Pixel_Sizes, src/base/ftobjs.c, holds it.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2013 Regents of the University of Minnesota.
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

/// The size a label's glyphs are drawn at, in points, as `msComputeTextPath` computes it: the
/// label's size times the scale factor, held at least at its minimum and at most at its maximum
/// size times the resolution factor, rounded half to even as `MS_NINT` (`lrint`) rounds.
pub fn glyph_size(
    size: f64,
    scale_factor: f64,
    min_size: f64,
    max_size: f64,
    resolution_factor: f64,
) -> f64 {
    let scaled = size * scale_factor;
    let least = min_size * resolution_factor;
    // MS_MAX and MS_MIN, which keep their second argument where a comparison fails.
    let held = if scaled > least { scaled } else { least };
    let most = max_size * resolution_factor;
    let held = if held < most { held } else { most };
    held.round_ties_even()
}

/// The pixel size, FreeType's ppem, of a glyph size: `MS_NINT(size * 96 / 72.0)` of the glyph size
/// taken as an `unsigned int`, held between 1 and 65535 as `FT_Set_Pixel_Sizes` holds it.
pub fn pixel_size(glyph_size: f64) -> u16 {
    let pixels = (glyph_size.trunc() * 96.0 / 72.0).round_ties_even();
    if pixels >= 65535.0 {
        u16::MAX
    } else if pixels >= 1.0 {
        // In 1..65535 and integral, so the conversion is exact.
        pixels as u16
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glyph_size_is_held_between_its_bounds_and_rounded_half_to_even() {
        // Verifies: ADR-0028
        assert_eq!(glyph_size(10.0, 1.0, 4.0, 256.0, 1.0), 10.0);
        assert_eq!(glyph_size(10.5, 1.0, 4.0, 256.0, 1.0), 10.0);
        assert_eq!(glyph_size(11.5, 1.0, 4.0, 256.0, 1.0), 12.0);
        assert_eq!(glyph_size(5.0, 2.5, 4.0, 256.0, 1.0), 12.0);
        assert_eq!(glyph_size(2.0, 1.0, 4.0, 256.0, 1.0), 4.0);
        assert_eq!(glyph_size(2.0, 1.0, 4.0, 256.0, 1.5), 6.0);
        assert_eq!(glyph_size(300.0, 1.0, 4.0, 256.0, 1.0), 256.0);
    }

    #[test]
    fn the_pixel_size_is_rounded_half_to_even_and_held_as_freetype_holds_it() {
        // Verifies: ADR-0028
        assert_eq!(pixel_size(10.0), 13); // 13.33
        assert_eq!(pixel_size(9.0), 12); // 12.0
        assert_eq!(pixel_size(15.0), 20); // 20.0
        assert_eq!(pixel_size(1.5), 1); // the glyph size as an unsigned int: 1, so 1.33
        assert_eq!(pixel_size(3.0), 4); // 4.0
        assert_eq!(pixel_size(0.0), 1);
        assert_eq!(pixel_size(1e12), 65535);
        assert_eq!(pixel_size(f64::NAN), 1);
    }
}
