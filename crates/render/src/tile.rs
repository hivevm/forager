//! The tile a polygon is filled with or a line is brushed with: its size within Q-9's bound, the
//! drawing of the image that makes it, and the image, built when it is first drawn with and once
//! only.

use std::cell::OnceCell;

use crate::{DrawOp, PremultipliedImage};

/// A fill tile or a brush tile, a display item's counted reference to one image however many
/// polygons it fills or lines it brushes (ADR-0029): `width` by `height` pixels, transparent but
/// for the drawings `marks` in them, drawn in turn. Whoever keeps count of the bytes of a render's
/// images counts [`Tile::bytes`] from the moment the tile exists, so that one past the bound is
/// refused before anything is allocated for it; the renderer builds the pixels from the marks when
/// it first draws with the tile.
#[derive(Debug, PartialEq)]
pub struct Tile {
    width: u32,
    height: u32,
    marks: Vec<DrawOp>,
    image: OnceCell<PremultipliedImage>,
}

impl Tile {
    /// The largest width and height of a tile (Q-9).
    pub const MAX_SIDE: u32 = 4096;

    /// The tile of `width` by `height` pixels that `marks` draw, or `None` for a side of no
    /// pixels or past [`Tile::MAX_SIDE`].
    pub fn new(width: u32, height: u32, marks: Vec<DrawOp>) -> Option<Self> {
        let sides = 1..=Self::MAX_SIDE;
        (sides.contains(&width) && sides.contains(&height)).then_some(Self {
            width,
            height,
            marks,
            image: OnceCell::new(),
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// What the tile holds decoded once it is built, counted from its creation.
    pub fn bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// The bytes of the copy a renderer makes of a tile of `width` by `height` pixels to brush a
    /// line with it, while it draws the line: AGG's `line_image_pattern` copies the tile grown by
    /// 2 pixels on each side, its bilinear filter's dilation and 1, 4 bytes a pixel (Q-9).
    pub fn brush_bytes(width: u32, height: u32) -> usize {
        (width as usize + 4) * (height as usize + 4) * 4
    }

    /// The drawings that make the tile, in the tile's own pixels, in the order they are drawn.
    pub fn marks(&self) -> &[DrawOp] {
        &self.marks
    }

    /// The tile's image, which `build` makes the first time it is asked for and never again.
    pub fn image(&self, build: impl FnOnce(&Self) -> PremultipliedImage) -> &PremultipliedImage {
        self.image.get_or_init(|| build(self))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    fn mark() -> DrawOp {
        let image = PremultipliedImage::new(1, 1, vec![0; 4]).unwrap();
        DrawOp::Pixmap {
            image: Rc::new(image),
            left: 0,
            top: 0,
        }
    }

    #[test]
    fn a_tile_past_4096_pixels_a_side_or_of_none_cannot_be_made() {
        // Verifies: Q-9
        assert!(Tile::new(4096, 4096, vec![mark()]).is_some());
        assert_eq!(
            Tile::new(4096, 4096, vec![mark()]).unwrap().bytes(),
            67_108_864
        );
        assert!(Tile::new(4097, 1, vec![mark()]).is_none());
        assert!(Tile::new(1, 4097, vec![mark()]).is_none());
        assert!(Tile::new(0, 1, vec![mark()]).is_none());
        assert!(Tile::new(1, 0, vec![mark()]).is_none());
    }

    #[test]
    fn a_brush_copies_the_tile_grown_by_two_pixels_a_side() {
        // Verifies: Q-9
        assert_eq!(Tile::brush_bytes(1, 1), 5 * 5 * 4);
        assert_eq!(Tile::brush_bytes(4096, 4096), 4100 * 4100 * 4);
    }

    #[test]
    fn a_tile_is_built_once_however_often_it_is_asked_for() {
        let tile = Tile::new(1, 1, vec![mark()]).unwrap();
        let built = Cell::new(0);
        let build = |_: &Tile| {
            built.set(built.get() + 1);
            PremultipliedImage::new(1, 1, vec![7; 4]).unwrap()
        };
        assert_eq!(tile.image(build).pixels(), [7; 4]);
        assert_eq!(tile.image(build).pixels(), [7; 4]);
        assert_eq!(built.get(), 1);
    }
}
