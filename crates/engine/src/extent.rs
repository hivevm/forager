// The fitting of the extent to the image follows MapServer's msAdjustExtent(), src/maputil.c of
// the release pinned in scripts/fetch-msautotest.sh, and its MS_CELLSIZE and MS_MAP2IMAGE
// macros, src/mapserver.h.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.

use forager_ports::Coord;
use forager_render::Point;
use forager_style_ir::Extent;

/// The mapping from map coordinates to pixels of one image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The extent, grown so that its aspect matches the image's.
    pub extent: Extent,
    /// Map units per pixel.
    pub cellsize: f64,
}

impl View {
    /// Fits `extent` to an image of `width` × `height` pixels as MapServer does: the extent runs
    /// from the centre of the first pixel to the centre of the last, and grows on the shorter
    /// axis, centred. `None` when no cellsize follows, for an image one pixel wide or an empty
    /// extent.
    pub fn for_image(extent: Extent, width: u32, height: u32) -> Option<Self> {
        let corners = [extent.min_x, extent.min_y, extent.max_x, extent.max_y];
        if width <= 1 || height <= 1 || !corners.iter().all(|v| v.is_finite()) {
            return None;
        }
        let (w, h) = (f64::from(width - 1), f64::from(height - 1));
        let cellsize = ((extent.max_x - extent.min_x) / w).max((extent.max_y - extent.min_y) / h);
        if !(cellsize.is_finite() && cellsize > 0.0) {
            return None;
        }
        let ox = ((w - (extent.max_x - extent.min_x) / cellsize) / 2.0).max(0.0);
        let oy = ((h - (extent.max_y - extent.min_y) / cellsize) / 2.0).max(0.0);
        Some(Self {
            extent: Extent {
                min_x: extent.min_x - ox * cellsize,
                min_y: extent.min_y - oy * cellsize,
                max_x: extent.max_x + ox * cellsize,
                max_y: extent.max_y + oy * cellsize,
            },
            cellsize,
        })
    }

    pub fn to_pixel(&self, c: Coord) -> Point {
        Point {
            x: (c.x - self.extent.min_x) / self.cellsize,
            y: (self.extent.max_y - c.y) / self.cellsize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Extent {
        Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    #[test]
    fn the_extent_grows_on_the_shorter_axis_and_maps_corners_to_pixel_centres() {
        let view = View::for_image(extent(0.0, 0.0, 100.0, 100.0), 201, 101).unwrap();
        assert_eq!(view.cellsize, 1.0);
        assert_eq!(view.extent, extent(-50.0, 0.0, 150.0, 100.0));
        assert_eq!(
            view.to_pixel(Coord { x: -50.0, y: 100.0 }),
            Point { x: 0.0, y: 0.0 }
        );
        assert_eq!(
            view.to_pixel(Coord { x: 150.0, y: 0.0 }),
            Point { x: 200.0, y: 100.0 }
        );
    }

    #[test]
    fn degenerate_images_and_extents_have_no_view() {
        assert_eq!(View::for_image(extent(0.0, 0.0, 1.0, 1.0), 1, 100), None);
        assert_eq!(View::for_image(extent(0.0, 0.0, 0.0, 0.0), 100, 100), None);
        assert_eq!(
            View::for_image(extent(0.0, 0.0, f64::NAN, 1.0), 100, 100),
            None
        );
    }
}
