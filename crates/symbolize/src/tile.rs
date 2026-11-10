// The tile of a polygon's pixmap follows the tile's size in MapServer's msDrawShadeSymbol(),
// getTile(), searchTileCache(), and addTileCache(), src/maprendering.c, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2011 Regents of the University of Minnesota.
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

//! The tile a polygon is filled with: its size, as MapServer computes it, held to Q-9's bound
//! before anything is allocated for it, and the cache of a render's tiles, which builds each once
//! and holds as many as MapServer holds, their bytes counted with the render's decoded images
//! (ADR-0029).

use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use forager_render::{DrawOp, Point, PremultipliedImage, Tile};
use forager_style_ir::c::lrint;

use crate::bounds::ms_max;
use crate::symbol;

/// The most bytes the images one render holds decoded come to, its fill tiles among them (Q-9).
pub const MAX_DECODED_IMAGE_BYTES: usize = 256 * 1024 * 1024;

/// How many tiles a render holds, MapServer's `MS_IMAGECACHESIZE`.
const MAX_TILES_HELD: usize = 6;

/// A tile's side in pixels as msDrawShadeSymbol computes it from the image's `side`, `scale`, and
/// `gap`, before the assignment to `int` narrows it: at a scale other than 1 the side scaled and
/// rounded, or with a positive gap the larger of the gap rounded and the side scaled, not
/// rounded; at a scale of 1 the side, or with a positive gap the larger of the gap and the side.
/// A rounding past a `long` asks for no less than it was given, an infinity, where `MS_NINT`'s
/// `LONG_MIN` would ask for no pixels.
fn asked_side(side: u32, scale: f64, gap: f64) -> f64 {
    let side = f64::from(side);
    let nint = |value: f64| match lrint(value) {
        i64::MIN => f64::INFINITY,
        rounded => rounded as f64,
    };
    match (scale != 1.0, gap > 0.0) {
        (true, true) => ms_max(nint(gap), side * scale),
        (true, false) => nint(side * scale),
        (false, true) => ms_max(gap, side),
        (false, false) => side,
    }
}

/// `asked`, truncated as the assignment to `int` truncates it and at least 1, or `None` past
/// [`Tile::MAX_SIDE`], or where it is no number (Q-9).
fn bounded_side(asked: f64) -> Option<u32> {
    let side = asked.trunc();
    (side <= f64::from(Tile::MAX_SIDE)).then(|| side.max(1.0) as u32)
}

/// A pixmap's tile, by what MapServer's searchTileCache compares of a pixmap's: its symbol, by
/// its index from 1, the tile's size, the scale, and the rotation, bit for bit.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    symbol: usize,
    width: u32,
    height: u32,
    scale: u64,
    rotation: u64,
}

/// The tiles of one render: at most [`MAX_TILES_HELD`], in the order they were built, those let go
/// followed until nothing refers to them, since their bytes count until then.
#[derive(Debug, Default)]
pub struct Tiles {
    held: VecDeque<(Key, Rc<Tile>)>,
    let_go: Vec<Weak<Tile>>,
    /// The bytes of the images the render holds decoded.
    images: usize,
}

impl Tiles {
    /// Sets the bytes of the images the render holds decoded, which the tiles count with.
    pub fn count_images(&mut self, bytes: usize) {
        self.images = bytes;
    }

    /// The bytes of the tiles alive: those held and those let go that something still refers to.
    pub fn bytes(&self) -> usize {
        let held: usize = self.held.iter().map(|(_, tile)| tile.bytes()).sum();
        let let_go: usize = self
            .let_go
            .iter()
            .filter_map(Weak::upgrade)
            .map(|tile| tile.bytes())
            .sum();
        held + let_go
    }

    /// The tile of `key`, held or made new by `mark`. Where six are held, the one built longest
    /// ago is let go first, as addTileCache drops it. One that would bring the render's images
    /// and tiles past [`MAX_DECODED_IMAGE_BYTES`] is refused, and none let go for it.
    fn tile(&mut self, key: Key, mark: DrawOp) -> Result<Rc<Tile>, String> {
        if let Some((_, tile)) = self.held.iter().find(|(held, _)| *held == key) {
            return Ok(Rc::clone(tile));
        }
        // `tile` bounds the sides first, so this is not reached from it; `Tile::new` keeps a bound
        // of its own, so that no caller of it allocates past Q-9's.
        let Some(tile) = Tile::new(key.width, key.height, mark) else {
            return Err(past_side(f64::from(key.width), f64::from(key.height)));
        };
        // What letting go of the oldest frees: nothing while something else refers to it.
        let freed = match self.held.front() {
            Some((_, oldest))
                if self.held.len() == MAX_TILES_HELD && Rc::strong_count(oldest) == 1 =>
            {
                oldest.bytes()
            }
            _ => 0,
        };
        self.let_go.retain(|tile| tile.strong_count() > 0);
        if self.images + self.bytes() - freed + tile.bytes() > MAX_DECODED_IMAGE_BYTES {
            return Err(format!(
                "its tile of {} × {} pixels would bring the images and tiles this render holds \
                 past the limit of {MAX_DECODED_IMAGE_BYTES} bytes",
                key.width, key.height
            ));
        }
        if self.held.len() == MAX_TILES_HELD
            && let Some((_, oldest)) = self.held.pop_front()
        {
            self.let_go.push(Rc::downgrade(&oldest));
        }
        let tile = Rc::new(tile);
        self.held.push_back((key, Rc::clone(&tile)));
        Ok(tile)
    }
}

/// Why a tile of `width` by `height` pixels is refused: each side as a number of pixels where it
/// is one, so that a gap of 1e300 does not print 301 digits.
fn past_side(width: f64, height: f64) -> String {
    let side = Tile::MAX_SIDE;
    let pixels = |asked: f64| {
        if asked.is_finite() && asked.abs() < 1e15 {
            format!("{}", asked.trunc())
        } else {
            format!("{asked:e}")
        }
    };
    format!(
        "its size or gap asks a tile of {} × {} pixels, past the limit of {side} × {side}",
        pixels(width),
        pixels(height)
    )
}

/// The tile of the pixmap symbol at `index` from 1 with the image `image`, as msDrawShadeSymbol
/// asks getTile for it at `scale`, `rotation`, and `gap`: its size held to Q-9's bound first, so
/// that nothing is allocated for one past it; then the tile the cache holds, or a new one, whose
/// mark is the image as agg2RenderPixmapSymbol draws it at the tile's centre, unmoved by the
/// symbol's anchor point. No seamless mode: msDrawShadeSymbol sets it for a vector symbol alone.
pub(crate) fn tile(
    tiles: &mut Tiles,
    index: usize,
    image: Rc<PremultipliedImage>,
    (scale, rotation, gap): (f64, f64, f64),
) -> Result<Rc<Tile>, String> {
    let asked = (
        asked_side(image.width(), scale, gap),
        asked_side(image.height(), scale, gap),
    );
    let (Some(width), Some(height)) = (bounded_side(asked.0), bounded_side(asked.1)) else {
        return Err(past_side(asked.0, asked.1));
    };
    let key = Key {
        symbol: index,
        width,
        height,
        scale: scale.to_bits(),
        rotation: rotation.to_bits(),
    };
    let at = Point {
        x: f64::from(width) / 2.0,
        y: f64::from(height) / 2.0,
    };
    tiles.tile(key, symbol::render_pixmap(at, image, scale, rotation))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The size of the tile of an image of `width` by `height` at `scale` and `gap`.
    fn size(width: u32, height: u32, scale: f64, gap: f64) -> (Option<u32>, Option<u32>) {
        (
            bounded_side(asked_side(width, scale, gap)),
            bounded_side(asked_side(height, scale, gap)),
        )
    }

    fn square(side: u32) -> Option<(Option<u32>, Option<u32>)> {
        Some((Some(side), Some(side)))
    }

    #[test]
    fn a_tile_s_size_is_ms_draw_shade_symbol_s() {
        // 32 × 32 at a scale other than 1 without a gap: MS_NINT(32 × 15 / 32) and, half to even,
        // MS_NINT(15.5) and MS_NINT(16.5).
        assert_eq!(Some(size(32, 32, 15.0 / 32.0, 0.0)), square(15));
        assert_eq!(Some(size(32, 32, 15.5 / 32.0, 0.0)), square(16));
        assert_eq!(Some(size(32, 32, 16.5 / 32.0, 0.0)), square(16));
        // A negative gap counts as none.
        assert_eq!(Some(size(32, 32, 15.5 / 32.0, -10.0)), square(16));
        // A positive gap: the image's term 15.5 truncated, not rounded, and the gap's rounded,
        // half to even: MS_NINT(20.5) is 20.
        assert_eq!(Some(size(32, 32, 15.5 / 32.0, 10.0)), square(15));
        assert_eq!(Some(size(32, 32, 15.5 / 32.0, 20.5)), square(20));
        // MS_NINT(s.gap) rounds where truncation would not (maprendering.c:855-856).
        assert_eq!(Some(size(32, 32, 15.5 / 32.0, 20.6)), square(21));
        // At a scale of 1: the image's own, or the larger of it and the gap, truncated.
        assert_eq!(Some(size(32, 32, 1.0, 0.0)), square(32));
        assert_eq!(Some(size(32, 32, 1.0, 10.0)), square(32));
        assert_eq!(Some(size(32, 32, 1.0, 40.7)), square(40));
        // Each side apart.
        assert_eq!(
            size(32, 16, 2.0, 40.0),
            (Some(64), Some(40)),
            "32 × 2 and the gap over 16 × 2"
        );
        // At least 1: at a scale of 0, where a size of 0 gives a gap that is no number.
        assert_eq!(Some(size(32, 32, 0.0, f64::NAN)), square(1));
    }

    #[test]
    fn a_tile_refused_names_its_sides_in_whole_pixels_or_in_short() {
        let why = past_side(4097.9, 1e300);
        assert!(why.contains("a tile of 4097 × 1e300 pixels"), "{why}");
        let why = past_side(f64::INFINITY, f64::NAN);
        assert!(why.contains("a tile of inf × NaN pixels"), "{why}");
    }

    #[test]
    fn a_tile_past_4096_a_side_has_no_size_whatever_asks_it() {
        // Verifies: Q-9
        assert_eq!(Some(size(32, 32, 4096.0 / 32.0, 0.0)), square(4096));
        assert_eq!(size(32, 32, 4097.0 / 32.0, 0.0), (None, None));
        // Half to even gives 4096 for 4096.5, truncation gives it for 4096.9 with a gap.
        assert_eq!(Some(size(1, 1, 4096.5, 0.0)), square(4096));
        assert_eq!(Some(size(1, 1, 1.0, 4096.9)), square(4096));
        assert_eq!(size(32, 32, 1.0, 4097.0), (None, None));
        assert_eq!(size(32, 32, 2.0, 4097.0), (None, None));
        // Past an int, a long, and any number; none wraps to a small tile as C's narrowing would.
        for huge in [3e9, 4_294_967_312.0, 1e19, 1e300, f64::INFINITY] {
            assert_eq!(size(32, 32, huge / 32.0, 0.0), (None, None), "{huge}");
            assert_eq!(size(32, 32, 1.0, huge), (None, None), "{huge}");
            assert_eq!(size(32, 32, 2.0, huge), (None, None), "{huge}");
        }
        assert_eq!(size(32, 32, f64::NAN, 0.0), (None, None));
    }

    fn pin() -> Rc<PremultipliedImage> {
        Rc::new(PremultipliedImage::new(1, 1, vec![9; 4]).unwrap())
    }

    /// The tile of symbol 1 at a gap of `side`, so `side` pixels a side.
    fn gapped(tiles: &mut Tiles, side: u32) -> Result<Rc<Tile>, String> {
        tile(tiles, 1, pin(), (1.0, 0.0, f64::from(side)))
    }

    #[test]
    fn a_tile_is_kept_by_symbol_size_scale_and_rotation() {
        let mut tiles = Tiles::default();
        let first = tile(&mut tiles, 1, pin(), (2.0, 0.5, 0.0)).unwrap();
        let again = tile(&mut tiles, 1, pin(), (2.0, 0.5, 0.0)).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        for (index, style) in [
            (2, (2.0, 0.5, 0.0)),
            (1, (3.0, 0.5, 0.0)),
            (1, (2.0, 0.25, 0.0)),
            (1, (2.0, 0.5, 9.0)),
        ] {
            let other = tile(&mut tiles, index, pin(), style).unwrap();
            assert!(!Rc::ptr_eq(&first, &other), "{index} {style:?}");
        }
        // SIZE 15.6 and 16.4 of an image of 32 pixels both make tiles of 16 × 16, at scales that
        // differ, so they are kept apart.
        let image = Rc::new(PremultipliedImage::new(32, 32, vec![9; 32 * 32 * 4]).unwrap());
        let mut at =
            |size: f64| tile(&mut tiles, 1, image.clone(), (size / 32.0, 0.0, 0.0)).unwrap();
        let (smaller, larger) = (at(15.6), at(16.4));
        assert_eq!((smaller.width(), larger.width()), (16, 16));
        assert!(!Rc::ptr_eq(&smaller, &larger));
    }

    #[test]
    fn a_seventh_tile_lets_go_of_the_one_built_longest_ago() {
        // addTileCache: of MS_IMAGECACHESIZE tiles the last of the list, the oldest, goes, and a
        // tile found moves nowhere.
        let mut tiles = Tiles::default();
        let built: Vec<_> = (2..8).map(|i| gapped(&mut tiles, i).unwrap()).collect();
        let again = gapped(&mut tiles, 2).unwrap();
        assert!(Rc::ptr_eq(&built[0], &again), "found, not moved");
        let weak: Vec<_> = built.iter().map(Rc::downgrade).collect();
        drop(built);
        drop(again);
        gapped(&mut tiles, 8).unwrap();
        assert_eq!(weak[0].strong_count(), 0, "the tile built longest ago");
        assert!(weak[1..].iter().all(|tile| tile.strong_count() == 1));
        assert_eq!(tiles.held.len(), 6);
    }

    #[test]
    fn a_tile_let_go_counts_until_nothing_refers_to_it() {
        // Verifies: Q-9, ADR-0029
        let mut tiles = Tiles::default();
        let referred = gapped(&mut tiles, 2).unwrap();
        for side in 3..9 {
            gapped(&mut tiles, side).unwrap();
        }
        let held: usize = (3..9).map(|side: usize| side * side * 4).sum();
        assert_eq!(tiles.bytes(), held + 2 * 2 * 4);
        drop(referred);
        assert_eq!(tiles.bytes(), held);
    }

    #[test]
    fn the_tile_that_would_pass_256_mib_with_the_images_is_refused_and_none_let_go_for_it() {
        // Verifies: Q-9, ADR-0029
        let mut tiles = Tiles::default();
        tiles.count_images(MAX_DECODED_IMAGE_BYTES - 16);
        let fits = gapped(&mut tiles, 2).unwrap();
        assert_eq!(tiles.bytes(), 16);
        let why = gapped(&mut tiles, 3).unwrap_err();
        assert!(why.contains("3 × 3"), "{why}");
        assert!(why.contains("past the limit of 268435456 bytes"), "{why}");
        assert_eq!(tiles.bytes(), 16);
        assert!(Rc::ptr_eq(&fits, &gapped(&mut tiles, 2).unwrap()));
        // Six held, the sixth's bytes free the seventh's room when nothing else refers to the
        // oldest; a reference to it keeps them counted, and the seventh is refused.
        let mut tiles = Tiles::default();
        let oldest = gapped(&mut tiles, 2).unwrap();
        for _ in 3..8 {
            let side = 2 + tiles.held.len() as u32;
            gapped(&mut tiles, side).unwrap();
        }
        tiles.count_images(MAX_DECODED_IMAGE_BYTES - tiles.bytes());
        assert!(gapped(&mut tiles, 2).is_ok(), "the tile held");
        let refused = tile(&mut tiles, 2, pin(), (1.0, 0.0, 2.0));
        assert!(refused.is_err(), "the oldest still referred to");
        assert_eq!(tiles.held.len(), 6);
        drop(oldest);
        let taken = tile(&mut tiles, 2, pin(), (1.0, 0.0, 2.0));
        assert!(taken.is_ok(), "its bytes free once nothing refers to it");
    }

    #[test]
    fn a_tile_s_mark_is_the_image_at_its_centre_as_at_a_point() {
        // agg2RenderPixmapSymbol at getTile's (width / 2., height / 2.): unscaled and unturned at
        // MS_NINT(20 - 16) = 4 for a tile of 40 × 40 and a 32 × 32 image.
        let image = Rc::new(PremultipliedImage::new(32, 32, vec![0; 32 * 32 * 4]).unwrap());
        let mut tiles = Tiles::default();
        let gapped = tile(&mut tiles, 1, Rc::clone(&image), (1.0, 0.0, 40.7)).unwrap();
        assert_eq!((gapped.width(), gapped.height()), (40, 40));
        match gapped.mark() {
            DrawOp::Pixmap { left, top, .. } => assert_eq!((*left, *top), (4, 4)),
            other => panic!("{other:?}"),
        }
        // Scaled and turned: resampled about the centre, (7.5, 7.5) of a tile of 15 × 15.
        let turned = tile(&mut tiles, 1, image, (15.0 / 32.0, 1.0, 0.0)).unwrap();
        assert_eq!((turned.width(), turned.height()), (15, 15));
        match turned.mark() {
            DrawOp::ResampledPixmap { at, .. } => assert_eq!(*at, Point { x: 7.5, y: 7.5 }),
            other => panic!("{other:?}"),
        }
    }
}
