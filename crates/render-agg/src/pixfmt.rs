// The pixel format follows the pixfmt_alpha_blend_rgba and blender_rgba_pre of the AGG that
// MapServer bundles, and the colours of aggColor(), src/mapagg.cpp, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2007 Regents of the University of Minnesota (src/mapagg.cpp).
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
//
// Anti-Grain Geometry - Version 2.4
// Copyright (C) 2002-2005 Maxim Shemanarev (http://www.antigrain.com)
// Permission to copy, use, modify, sell and distribute this software is granted provided this
// copyright notice appears in all copies. This software is provided "as is" without express or
// implied warranty, and with no claim as to its suitability for any purpose.

//! MapServer's pixel format: `pixfmt_alpha_blend_rgba` over `blender_rgba_pre` of the AGG 2.4
//! that MapServer bundles (src/renderers/agg/include/agg_pixfmt_rgba.h at the pin), which
//! src/mapagg.cpp draws every image with. Its colours are premultiplied by their alpha, as
//! `aggColor` gives them through `rgba8_pre`.
//!
//! agg-rust's own pixel formats round as AGG 2.6 does; over a transparent pixel, where the alpha
//! is divided out again when the image is written, a difference of one in the alpha changes the
//! colour by far more, so the image is blended here as MapServer blends it.

use agg_rust::basics::CoverType;
use agg_rust::color::Rgba8;
use agg_rust::pixfmt_rgba::PixelFormat;

/// A colour premultiplied by its alpha, as AGG 2.4's `rgba8::premultiply` does it: each channel
/// times the alpha, shifted down by 8 bits.
pub(crate) fn premultiplied(r: u8, g: u8, b: u8, a: u8) -> Rgba8 {
    let times = |c: u8| (u32::from(c) * u32::from(a)) >> 8;
    match a {
        255 => Rgba8::new(r.into(), g.into(), b.into(), 255),
        0 => Rgba8::new(0, 0, 0, 0),
        _ => Rgba8::new(times(r), times(g), times(b), a.into()),
    }
}

/// `blender_rgba_pre::blend_pix` with a cover: `alpha` is the colour's alpha scaled by it.
fn blend_pix_cover(p: &mut [u8], c: &Rgba8, alpha: u32, cover: u32) {
    let inverse = 255 - alpha;
    let cover = cover + 1;
    for (p, c) in p[..3].iter_mut().zip([c.r, c.g, c.b]) {
        *p = ((u32::from(*p) * inverse + u32::from(c) * cover) >> 8) as u8;
    }
    p[3] = (255 - ((inverse * (255 - u32::from(p[3]))) >> 8)) as u8;
}

/// `blender_rgba_pre::blend_pix` without a cover.
fn blend_pix(p: &mut [u8], c: &Rgba8, alpha: u32) {
    let inverse = 255 - alpha;
    for (p, c) in p[..3].iter_mut().zip([c.r, c.g, c.b]) {
        *p = (((u32::from(*p) * inverse) >> 8) + u32::from(c)) as u8;
    }
    p[3] = (255 - ((inverse * (255 - u32::from(p[3]))) >> 8)) as u8;
}

fn copy(p: &mut [u8], c: &Rgba8) {
    p.copy_from_slice(&[c.r, c.g, c.b, c.a]);
}

/// `copy_or_blend_pix` with a cover.
pub(crate) fn copy_or_blend_pix(p: &mut [u8], c: &Rgba8, cover: CoverType) {
    let a = u32::from(c.a);
    if a == 0 {
        return;
    }
    if cover == 255 {
        if a == 255 {
            copy(p, &Rgba8::new(c.r.into(), c.g.into(), c.b.into(), 255));
        } else {
            blend_pix(p, c, a);
        }
        return;
    }
    let alpha = (a * (u32::from(cover) + 1)) >> 8;
    if alpha == 255 {
        copy(p, &Rgba8::new(c.r.into(), c.g.into(), c.b.into(), 255));
    } else {
        blend_pix_cover(p, c, alpha, cover.into());
    }
}

/// The span's alpha and how its pixels blend, as `blend_hline` decides it once for the span.
fn blend_span(pixels: &mut [u8], c: &Rgba8, cover: CoverType) {
    if c.a == 0 {
        return;
    }
    let alpha = (u32::from(c.a) * (u32::from(cover) + 1)) >> 8;
    for p in pixels.as_chunks_mut::<4>().0.iter_mut() {
        if alpha == 255 {
            copy(p, c);
        } else if cover == 255 {
            blend_pix(p, c, alpha);
        } else {
            blend_pix_cover(p, c, alpha, cover.into());
        }
    }
}

/// An RGBA image of premultiplied colours, rows top first, without padding.
pub(crate) struct PixfmtMapserver<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
}

impl<'a> PixfmtMapserver<'a> {
    /// The pixel format over `pixels`, which must hold `width` × `height` pixels.
    pub(crate) fn new(pixels: &'a mut [u8], width: u32, height: u32) -> Self {
        debug_assert_eq!(pixels.len(), width as usize * height as usize * 4);
        Self {
            pixels,
            width,
            height,
        }
    }

    /// The bytes of `len` pixels from (x, y); the renderer clips every call to the image.
    fn span(&mut self, x: i32, y: i32, len: u32) -> &mut [u8] {
        let start = (y as usize * self.width as usize + x as usize) * 4;
        &mut self.pixels[start..start + len as usize * 4]
    }
}

impl PixelFormat for PixfmtMapserver<'_> {
    type ColorType = Rgba8;

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn blend_pixel(&mut self, x: i32, y: i32, c: &Rgba8, cover: CoverType) {
        copy_or_blend_pix(self.span(x, y, 1), c, cover);
    }

    fn blend_hline(&mut self, x: i32, y: i32, len: u32, c: &Rgba8, cover: CoverType) {
        blend_span(self.span(x, y, len), c, cover);
    }

    fn blend_solid_hspan(&mut self, x: i32, y: i32, len: u32, c: &Rgba8, covers: &[CoverType]) {
        if c.a == 0 {
            return;
        }
        for (p, &cover) in self
            .span(x, y, len)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(covers)
        {
            let alpha = (u32::from(c.a) * (u32::from(cover) + 1)) >> 8;
            if alpha == 255 {
                copy(p, &Rgba8::new(c.r.into(), c.g.into(), c.b.into(), 255));
            } else {
                blend_pix_cover(p, c, alpha, cover.into());
            }
        }
    }

    fn copy_hline(&mut self, x: i32, y: i32, len: u32, c: &Rgba8) {
        for p in self.span(x, y, len).as_chunks_mut::<4>().0.iter_mut() {
            copy(p, c);
        }
    }

    fn copy_pixel(&mut self, x: i32, y: i32, c: &Rgba8) {
        copy(self.span(x, y, 1), c);
    }

    fn blend_color_hspan(
        &mut self,
        x: i32,
        y: i32,
        len: u32,
        colors: &[Rgba8],
        covers: &[CoverType],
        cover: CoverType,
    ) {
        let pixels = self
            .span(x, y, len)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(colors);
        if covers.is_empty() {
            for (p, c) in pixels {
                copy_or_blend_pix(p, c, cover);
            }
        } else {
            for ((p, c), &cover) in pixels.zip(covers) {
                copy_or_blend_pix(p, c, cover);
            }
        }
    }

    fn pixel(&self, x: i32, y: i32) -> Rgba8 {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = &self.pixels[i..i + 4];
        Rgba8::new(p[0].into(), p[1].into(), p[2].into(), p[3].into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_premultiplied_by_truncation() {
        let c = premultiplied(128, 255, 3, 6);
        assert_eq!((c.r, c.g, c.b, c.a), (3, 5, 0, 6));
        let c = premultiplied(9, 9, 9, 0);
        assert_eq!((c.r, c.g, c.b, c.a), (0, 0, 0, 0));
    }

    #[test]
    fn a_partial_cover_blends_as_agg_2_4_blends_premultiplied_colours() {
        // Grey 128 at a cover of 5 over no colour: alpha (255 × 6) >> 8 = 5, the colour
        // (128 × 6) >> 8 = 3, the alpha 255 - ((250 × 255) >> 8) = 6.
        let mut p = [0u8; 4];
        copy_or_blend_pix(&mut p, &premultiplied(128, 128, 128, 255), 5);
        assert_eq!(p, [3, 3, 3, 6]);
        // A translucent colour at full cover over white: 255 × 127 >> 8 plus the colour.
        let mut p = [255u8; 4];
        copy_or_blend_pix(&mut p, &premultiplied(255, 0, 0, 128), 255);
        assert_eq!(p, [126 + 127, 126, 126, 255]);
    }
}
