// The resampling follows span_image_filter_rgba_bilinear_clip
// (src/renderers/agg/include/agg_span_image_filter_rgba.h), span_image_filter
// (agg_span_image_filter.h), span_interpolator_linear (agg_span_interpolator_linear.h),
// dda2_line_interpolator (agg_dda_line.h), and iround (agg_basics.h) of the AGG that MapServer
// bundles, at the release pinned in scripts/fetch-msautotest.sh.
//
// Anti-Grain Geometry - Version 2.4
// Copyright (C) 2002-2005 Maxim Shemanarev (http://www.antigrain.com)
// Permission to copy, use, modify, sell and distribute this software is granted provided this
// copyright notice appears in all copies. This software is provided "as is" without express or
// implied warranty, and with no claim as to its suitability for any purpose.

//! A pixmap resampled as MapServer's AGG 2.4 resamples one it draws turned or scaled
//! (ADR-0041): `span_image_filter_rgba_bilinear_clip` over `span_interpolator_linear`, `iround`
//! being `lrint` narrowed to an `int`, as MapServer's build defines it under `HAVE_LRINT`.
//!
//! agg-rust has both, as AGG 2.6 has them: its filter truncates where AGG 2.4 rounds, and its
//! `iround` rounds a half away from zero where `lrint` rounds it to even. So they are ported here.

use agg_rust::color::Rgba8;
use forager_render::{Affine, PremultipliedImage};
use forager_style_ir::c::lrint;

/// `image_subpixel_shift`: a position in 256ths of a pixel.
const SUBPIXEL_SHIFT: u32 = 8;
const SUBPIXEL_SCALE: i32 = 1 << SUBPIXEL_SHIFT;
const SUBPIXEL_MASK: i32 = SUBPIXEL_SCALE - 1;

/// AGG's `iround` under `HAVE_LRINT`: `lrint`, its `long` narrowed to the `int` it returns.
fn iround(v: f64) -> i32 {
    lrint(v) as i32
}

/// AGG's `dda2_line_interpolator`, forward-adjusted: `count` steps from `y1` to `y2` in whole
/// numbers. C's `int` arithmetic, whose overflow C leaves undefined, wraps here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Dda2 {
    cnt: i32,
    lft: i32,
    rem: i32,
    modulo: i32,
    y: i32,
}

impl Dda2 {
    fn new(y1: i32, y2: i32, count: i32) -> Self {
        let cnt = if count <= 0 { 1 } else { count };
        let delta = y2.wrapping_sub(y1);
        let mut lft = delta.wrapping_div(cnt);
        let mut rem = delta.wrapping_rem(cnt);
        let mut modulo = rem;
        if modulo <= 0 {
            modulo = modulo.wrapping_add(count);
            rem = rem.wrapping_add(count);
            lft = lft.wrapping_sub(1);
        }
        modulo = modulo.wrapping_sub(count);
        Self {
            cnt,
            lft,
            rem,
            modulo,
            y: y1,
        }
    }

    /// `operator++`.
    fn step(&mut self) {
        self.modulo = self.modulo.wrapping_add(self.rem);
        self.y = self.y.wrapping_add(self.lft);
        if self.modulo > 0 {
            self.modulo = self.modulo.wrapping_sub(self.cnt);
            self.y = self.y.wrapping_add(1);
        }
    }
}

/// AGG's `span_interpolator_linear<trans_affine, 8>`: the positions in the source of the pixels
/// of a span, its two ends transformed and rounded to 256ths of a pixel, those between stepped
/// linearly.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Interpolator {
    trans: Affine,
    x: Dda2,
    y: Dda2,
}

impl Interpolator {
    pub(crate) fn new(trans: Affine) -> Self {
        Self {
            trans,
            x: Dda2::default(),
            y: Dda2::default(),
        }
    }

    /// `trans_affine::transform`.
    fn transform(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.trans;
        (x * m.sx + y * m.shx + m.tx, x * m.shy + y * m.sy + m.ty)
    }

    /// `begin`: a span of `len` pixels from (`x`, `y`).
    pub(crate) fn begin(&mut self, x: f64, y: f64, len: u32) {
        let scale = f64::from(SUBPIXEL_SCALE);
        let (tx, ty) = self.transform(x, y);
        let (x1, y1) = (iround(tx * scale), iround(ty * scale));
        let (tx, ty) = self.transform(x + f64::from(len), y);
        let (x2, y2) = (iround(tx * scale), iround(ty * scale));
        // C passes the unsigned length as an `int`.
        let count = len as i32;
        self.x = Dda2::new(x1, x2, count);
        self.y = Dda2::new(y1, y2, count);
    }

    /// `coordinates`: the position of the current pixel, in 256ths of a pixel.
    pub(crate) fn coordinates(&self) -> (i32, i32) {
        (self.x.y, self.y.y)
    }

    /// `operator++`: on to the next pixel.
    pub(crate) fn step(&mut self) {
        self.x.step();
        self.y.step();
    }
}

/// `span_image_filter_rgba_bilinear_clip::generate` over `source` with a transparent background,
/// as agg2RenderPixmapSymbol sets it up: the colours of the pixels of `span` from (`x`, `y`), each
/// sampled at its centre, half a pixel in (`filter_dx_dbl`), and taken back by the half pixel
/// (`filter_dx_int`, 128) to the four pixels about it, weighted by the fraction of the position,
/// their sum started at a half and shifted down by 16 bits, so rounded. A pixel of the four
/// beyond the image adds the background, nothing; where all four lie beyond it, the colour is the
/// background, which the same sum gives. So AGG's three branches, all four inside, at the edge,
/// and wholly outside, compute this one sum.
pub(crate) fn generate(
    span: &mut [Rgba8],
    x: i32,
    y: i32,
    source: &PremultipliedImage,
    interpolator: &mut Interpolator,
) {
    const HALF: u32 = (SUBPIXEL_SCALE * SUBPIXEL_SCALE / 2) as u32;
    let width = i64::from(source.width());
    let height = i64::from(source.height());
    let pixels = source.pixels();
    let pixel = |x: i32, y: i32| -> [u8; 4] {
        let (x, y) = (i64::from(x), i64::from(y));
        if x < 0 || y < 0 || x >= width || y >= height {
            return [0; 4];
        }
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    interpolator.begin(f64::from(x) + 0.5, f64::from(y) + 0.5, span.len() as u32);
    for color in span {
        let (x_hr, y_hr) = interpolator.coordinates();
        let x_hr = x_hr.wrapping_sub(SUBPIXEL_SCALE / 2);
        let y_hr = y_hr.wrapping_sub(SUBPIXEL_SCALE / 2);
        let (x_lr, y_lr) = (x_hr >> SUBPIXEL_SHIFT, y_hr >> SUBPIXEL_SHIFT);
        let (fx, fy) = ((x_hr & SUBPIXEL_MASK) as u32, (y_hr & SUBPIXEL_MASK) as u32);
        let scale = SUBPIXEL_SCALE as u32;
        let mut fg = [HALF; 4];
        for (weight, p) in [
            ((scale - fx) * (scale - fy), pixel(x_lr, y_lr)),
            (fx * (scale - fy), pixel(x_lr.wrapping_add(1), y_lr)),
            ((scale - fx) * fy, pixel(x_lr, y_lr.wrapping_add(1))),
            (fx * fy, pixel(x_lr.wrapping_add(1), y_lr.wrapping_add(1))),
        ] {
            for (sum, c) in fg.iter_mut().zip(p) {
                *sum += weight * u32::from(c);
            }
        }
        let [r, g, b, a] = fg.map(|sum| sum >> (2 * SUBPIXEL_SHIFT));
        *color = Rgba8::new(r, g, b, a);
        interpolator.step();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: Affine = Affine {
        sx: 1.0,
        shy: 0.0,
        shx: 0.0,
        sy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// A 2 × 2 image: its top-left pixel all 0, its top-right all 1, the bottom row all 0.
    fn two_by_two() -> PremultipliedImage {
        #[rustfmt::skip]
        let pixels = vec![
            0, 0, 0, 0,  1, 1, 1, 1,
            0, 0, 0, 0,  0, 0, 0, 0,
        ];
        PremultipliedImage::new(2, 2, pixels).unwrap()
    }

    fn generated(source: &PremultipliedImage, trans: Affine, x: i32, len: usize) -> Vec<[u8; 4]> {
        let mut span = vec![Rgba8::new(9, 9, 9, 9); len];
        generate(&mut span, x, 0, source, &mut Interpolator::new(trans));
        span.iter().map(|c| [c.r, c.g, c.b, c.a]).collect()
    }

    #[test]
    fn a_pixel_weighted_half_and_half_between_0_and_1_rounds_to_1_inside_the_image() {
        // Verifies: ADR-0041
        // Through the identity moved half a pixel right and half a pixel down, the pixel (0, 0)
        // is sampled at its centre (0.5, 0.5) plus (0.5, 0.5): x_hr = iround(256) = 256, less
        // filter_dx_int 128, is 128, so x_lr 0 at the fraction 128; y_hr likewise 128 less 128,
        // y_lr 0 at the fraction 0. All four pixels lie inside the 2 × 2 image (0 < maxx 1 and
        // 0 < maxy 1). The weights: (256 - 128) × 256 = 32768 on (0, 0), 128 × 256 = 32768 on
        // (1, 0), 0 on the bottom row; each channel (32768 + 32768 × 0 + 32768 × 1) >> 16 = 1.
        // agg-rust's filter starts the sum at 0 and gives 32768 >> 16 = 0.
        let trans = Affine {
            tx: 0.5,
            ..IDENTITY
        };
        assert_eq!(generated(&two_by_two(), trans, 0, 1), [[1, 1, 1, 1]]);
    }

    #[test]
    fn a_pixel_weighted_half_and_half_between_0_and_1_rounds_to_1_at_the_edge() {
        // Verifies: ADR-0041
        // Through the identity moved a pixel and a half right, the pixel (0, 0) is sampled at
        // x_hr = iround(2 × 256) - 128 = 384: x_lr 1, past maxx - 1, at the edge, at the fraction
        // 128; y_lr 0 at 0. The weight 32768 falls on (1, 0), of channels 1, and 32768 on (2, 0),
        // beyond the image, which adds the transparent background, 0: (32768 + 32768 × 1) >> 16
        // = 1 in each channel. agg-rust's filter gives 0.
        let trans = Affine {
            tx: 1.5,
            ..IDENTITY
        };
        assert_eq!(generated(&two_by_two(), trans, 0, 1), [[1, 1, 1, 1]]);
        // Wholly beyond the image, two pixels further: the background.
        let trans = Affine {
            tx: 3.5,
            ..IDENTITY
        };
        assert_eq!(generated(&two_by_two(), trans, 0, 1), [[0, 0, 0, 0]]);
    }

    #[test]
    fn a_resampled_pixel_sums_its_four_weighted_neighbours() {
        // Verifies: ADR-0041
        // A 2 × 2 image of channels 0, 100 above and 200, 40 below, through the identity moved
        // by (0.3, 0.1): the centre (0.5, 0.5) of the pixel (0, 0) goes to (0.8, 0.6), x_hr =
        // iround(204.8) = 205 less 128 = 77, y_hr = iround(153.6) = 154 less 128 = 26, all four
        // pixels inside. Weights: (256 - 77) × (256 - 26) = 41170 on 0, 77 × 230 = 17710 on 100,
        // 179 × 26 = 4654 on 200, 77 × 26 = 2002 on 40: (32768 + 1771000 + 930800 + 80080) >> 16
        // = 2814648 >> 16 = 42.
        #[rustfmt::skip]
        let pixels = vec![
            0, 0, 0, 0,  100, 100, 100, 100,
            200, 200, 200, 200,  40, 40, 40, 40,
        ];
        let source = PremultipliedImage::new(2, 2, pixels).unwrap();
        let trans = Affine {
            tx: 0.3,
            ty: 0.1,
            ..IDENTITY
        };
        assert_eq!(generated(&source, trans, 0, 1), [[42, 42, 42, 42]]);
    }

    #[test]
    fn an_interpolator_span_at_a_scale_of_0_5_rounds_its_ends_half_to_even() {
        // Verifies: ADR-0041
        // At a scale of 0.5, a span of 2 pixels from x = 1/256 has its ends at 0.5 × 1/256 × 256
        // = 0.5 and 0.5 × (1/256 + 2) × 256 = 256.5: lrint gives 0 and 256, half to even, where
        // agg-rust's iround gives 1 and 257. Its y, 5/256, at 2.5: 2, where agg-rust gives 3.
        // Between the ends, dda2_line_interpolator steps (256 - 0) / 2 = 128.
        let mut interpolator = Interpolator::new(Affine {
            sx: 0.5,
            sy: 0.5,
            ..IDENTITY
        });
        interpolator.begin(1.0 / 256.0, 5.0 / 256.0, 2);
        assert_eq!(interpolator.coordinates(), (0, 2));
        interpolator.step();
        assert_eq!(interpolator.coordinates(), (128, 2));
        interpolator.step();
        assert_eq!(interpolator.coordinates(), (256, 2));
    }

    #[test]
    fn positions_past_an_int_wrap_as_c_narrows_them_and_nothing_panics() {
        // lrint of a position past a long is LONG_MIN, whose low 32 bits are 0; past an int but
        // within a long, the low 32 bits: 2^32 + 5 gives 5.
        assert_eq!(iround(1e300), 0);
        assert_eq!(iround(f64::NAN), 0);
        assert_eq!(iround(4_294_967_301.0), 5);
        let mut interpolator = Interpolator::new(Affine {
            sx: 1e300,
            tx: -1e9,
            ..IDENTITY
        });
        interpolator.begin(0.5, 0.5, 3);
        for _ in 0..3 {
            interpolator.step();
        }
        let wide = Affine {
            sx: 8e6,
            sy: 8e6,
            ..IDENTITY
        };
        let span = generated(&two_by_two(), wide, i32::MAX - 4, 4);
        assert_eq!(span.len(), 4);
    }
}
