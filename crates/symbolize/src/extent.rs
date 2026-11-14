// The fitting of the extent to the image follows MapServer's msAdjustExtent(), src/maputil.c of
// the release pinned in scripts/fetch-msautotest.sh, and its MS_CELLSIZE and MS_MAP2IMAGE
// macros, src/mapserver.h; the pixel of a mark follows msTransformPoint, src/maputil.c, the
// pixels of a line or a polygon msTransformShapeSimplify, src/mapprimitive.cpp, the transform mode
// msImageStartLayer, src/mapdraw.c, keeps for a layer without APPROXIMATION_SCALE from the AGG
// renderer's default, src/mapagg.cpp, and whether a mark is drawn msPointInRect, src/mapsearch.c.
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

use forager_render::Point;
use forager_style_ir::Coord;
use forager_style_ir::Extent;
use forager_style_ir::c::ms_max;

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
        let cellsize = ms_max(
            (extent.max_x - extent.min_x) / w,
            (extent.max_y - extent.min_y) / h,
        );
        if !(cellsize.is_finite() && cellsize > 0.0) {
            return None;
        }
        let ox = ms_max((w - (extent.max_x - extent.min_x) / cellsize) / 2.0, 0.0);
        let oy = ms_max((h - (extent.max_y - extent.min_y) / cellsize) / 2.0, 0.0);
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

    /// The pixel of a point drawn as a mark, as msTransformPoint in `src/maputil.c` computes it:
    /// by the inverse of the cellsize.
    pub fn point_to_pixel(&self, c: Coord) -> Point {
        let inverse = 1.0 / self.cellsize;
        Point {
            x: (c.x - self.extent.min_x) * inverse,
            y: (self.extent.max_y - c.y) * inverse,
        }
    }

    /// A line in pixels as msTransformShapeSimplify in `src/mapprimitive.cpp` gives it, each
    /// vertex by the inverse of the cellsize: its first vertex, each further one but the last more
    /// than a pixel from the one kept before it, and its last unless that equals the one kept
    /// before; nothing for a line left with fewer than two vertices.
    pub(crate) fn line_to_pixels(&self, line: &[Coord]) -> Vec<Point> {
        if line.len() < 2 {
            return Vec::new();
        }
        let mut kept = vec![self.point_to_pixel(line[0])];
        for &c in &line[1..line.len() - 1] {
            let p = self.point_to_pixel(c);
            if far(kept[kept.len() - 1], p) {
                kept.push(p);
            }
        }
        let last = self.point_to_pixel(line[line.len() - 1]);
        if last != kept[kept.len() - 1] {
            kept.push(last);
        }
        if kept.len() < 2 { Vec::new() } else { kept }
    }

    /// A polygon's ring in pixels as msTransformShapeSimplify in `src/mapprimitive.cpp` gives
    /// it, each vertex by the inverse of the cellsize: its first two vertices, each further one
    /// but the last two more than a pixel from the one kept before it, and its last two, the
    /// closing one among them; nothing for a ring of fewer than four vertices.
    pub(crate) fn ring_to_pixels(&self, ring: &[Coord]) -> Vec<Point> {
        if ring.len() < 4 {
            return Vec::new();
        }
        let mut kept = vec![self.point_to_pixel(ring[0]), self.point_to_pixel(ring[1])];
        let before_last = ring.len() - 2;
        for &c in &ring[2..before_last] {
            let p = self.point_to_pixel(c);
            if far(kept[kept.len() - 1], p) {
                kept.push(p);
            }
        }
        kept.push(self.point_to_pixel(ring[before_last]));
        kept.push(self.point_to_pixel(ring[before_last + 1]));
        kept
    }

    /// Whether `c` lies in the extent, its edges included, as msPointInRect in
    /// `src/mapsearch.c` decides it.
    pub fn contains(&self, c: Coord) -> bool {
        !(c.x < self.extent.min_x
            || c.x > self.extent.max_x
            || c.y < self.extent.min_y
            || c.y > self.extent.max_y)
    }
}

/// Whether msTransformShapeSimplify keeps `p` after `kept`: when it lies more than a pixel away.
fn far(kept: Point, p: Point) -> bool {
    let (dx, dy) = (p.x - kept.x, p.y - kept.y);
    dx * dx + dy * dy > 1.0
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
            view.point_to_pixel(Coord { x: -50.0, y: 100.0 }),
            Point { x: 0.0, y: 0.0 }
        );
        assert_eq!(
            view.point_to_pixel(Coord { x: 150.0, y: 0.0 }),
            Point { x: 200.0, y: 100.0 }
        );
    }

    #[test]
    fn a_mark_is_placed_by_the_inverse_of_the_cellsize() {
        // msTransformPoint multiplies by 1 / cellsize: 0.3 × 10 is 3, where 0.3 / 0.1 is
        // 2.9999999999999996.
        let view = View::for_image(extent(0.0, 0.0, 10.0, 10.0), 101, 101).unwrap();
        assert_eq!(view.cellsize, 0.1);
        let c = Coord { x: 0.3, y: 9.7 };
        assert_eq!(view.point_to_pixel(c).x, 3.0);
    }

    #[test]
    fn the_extent_contains_its_edges_and_nothing_past_them() {
        // msPointInRect.
        let view = View::for_image(extent(0.0, 0.0, 100.0, 100.0), 101, 101).unwrap();
        for (x, y) in [(0.0, 0.0), (100.0, 100.0), (0.0, 100.0), (50.0, 50.0)] {
            assert!(view.contains(Coord { x, y }), "{x} {y}");
        }
        let past = 1e-9;
        for (x, y) in [
            (-past, 50.0),
            (100.0 + past, 50.0),
            (50.0, -past),
            (50.0, 100.0 + past),
        ] {
            assert!(!view.contains(Coord { x, y }), "{x} {y}");
        }
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
