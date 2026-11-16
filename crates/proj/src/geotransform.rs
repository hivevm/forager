// The geotransform of a rotated map follows msMapComputeGeotransformEx, msMapComputeGeotransform,
// and msMapSetFakedExtent, src/mapobject.c, and InvGeoTransform, src/mapresample.c, of MapServer
// at the release pinned in scripts/fetch-msautotest.sh.
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

//! The geotransform of a rotated map: the affine step MapServer applies on the map's side of each
//! layer's reprojection, between the map's coordinates and the frame of its image, `0, 0, width,
//! height` with y up (ADR-0045).

use forager_style_ir::c::ms_pi_over_180;
use forager_style_ir::{Coord, Extent};

/// The geotransform of a rotated map, as `msMapSetFakedExtent` copies it onto the map's
/// projection: from the frame of the image, y up, into the map's coordinates, and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geotransform {
    /// The six terms, `x = gt[0] + gt[1]·col + gt[2]·row`, `y = gt[3] + gt[4]·col + gt[5]·row`.
    gt: [f64; 6],
    /// Their inverse, by `InvGeoTransform`.
    inverse: [f64; 6],
}

impl Geotransform {
    /// The geotransform of a map rotated by `angle` degrees, its `extent` the one grown to the
    /// image of `width` × `height` pixels: `msMapComputeGeotransform` of it, flipped as
    /// `msMapSetFakedExtent` flips it so that y runs up in the frame of the image. `None` where
    /// MapServer computes none, for an image one pixel wide or high or an extent of no width,
    /// where `InvGeoTransform` finds no inverse, or where a term is no finite number.
    pub fn rotated_map(extent: &Extent, width: u32, height: u32, angle: f64) -> Option<Self> {
        let map = map_geotransform(extent, width, height, angle)?;
        let mut gt = map;
        let h = f64::from(height);
        gt[0] += h * map[2];
        gt[3] += h * map[5];
        gt[2] *= -1.0;
        gt[5] *= -1.0;
        let inverse = inverse(&gt)?;
        // An angle or an extent that is no finite number gives none, where MapServer would draw
        // nothing in place.
        if !gt.iter().chain(&inverse).all(|v| v.is_finite()) {
            return None;
        }
        Some(Self { gt, inverse })
    }

    /// `point`, in the frame of the image, in the map's coordinates: `msProjectPointEx`'s step on
    /// the side reprojected from.
    pub fn forward(&self, point: Coord) -> Coord {
        let gt = &self.gt;
        Coord {
            x: gt[0] + gt[1] * point.x + gt[2] * point.y,
            y: gt[3] + gt[4] * point.x + gt[5] * point.y,
        }
    }

    /// `point`, in the map's coordinates, in the frame of the image: `msProjectPointEx`'s step on
    /// the side reprojected into.
    pub fn inverse(&self, point: Coord) -> Coord {
        let inv = &self.inverse;
        Coord {
            x: inv[0] + inv[1] * point.x + inv[2] * point.y,
            y: inv[3] + inv[4] * point.x + inv[5] * point.y,
        }
    }

    /// Its term `i`, as MapServer's tests of the reprojection read `gt.geotransform[i]`.
    pub(crate) fn term(&self, i: usize) -> f64 {
        self.gt[i]
    }

    /// Its inverse's term `i`, as MapServer reads `gt.invgeotransform[i]`.
    pub(crate) fn inverse_term(&self, i: usize) -> f64 {
        self.inverse[i]
    }
}

/// `msMapComputeGeotransform`: the geotransform of the map from pixel and line into its
/// coordinates, centred on the image's width and height over two (bug 1916), its resolutions those
/// of `extent` over `width - 1` and `height - 1`; `None` where MapServer computes none.
pub(crate) fn map_geotransform(
    extent: &Extent,
    width: u32,
    height: u32,
    angle: f64,
) -> Option<[f64; 6]> {
    if extent.min_x == extent.max_x || width <= 1 || height <= 1 {
        return None;
    }
    let geo_width = extent.max_x - extent.min_x;
    let geo_height = extent.max_y - extent.min_y;
    let resolution_x = geo_width / f64::from(width - 1);
    let resolution_y = geo_height / f64::from(height - 1);
    let rot_angle = ms_pi_over_180(angle);
    let center_x = extent.min_x + geo_width * 0.5;
    let center_y = extent.min_y + geo_height * 0.5;
    let (w, h) = (f64::from(width), f64::from(height));
    let mut gt = [0.0; 6];
    gt[1] = libm::cos(rot_angle) * resolution_x;
    gt[2] = libm::sin(rot_angle) * resolution_y;
    gt[0] = center_x - (w * 0.5) * gt[1] - (h * 0.5) * gt[2];
    gt[4] = libm::sin(rot_angle) * resolution_x;
    gt[5] = -libm::cos(rot_angle) * resolution_y;
    gt[3] = center_y - (w * 0.5) * gt[4] - (h * 0.5) * gt[5];
    Some(gt)
}

/// `InvGeoTransform`: the inverse of `gt`, with an implicit last row of `1 0 0`; `None` where its
/// determinant is below 1e-15.
fn inverse(gt: &[f64; 6]) -> Option<[f64; 6]> {
    let det = gt[1] * gt[5] - gt[2] * gt[4];
    if det.abs() < 0.000_000_000_000_001 {
        return None;
    }
    let inv_det = 1.0 / det;
    let mut out = [0.0; 6];
    out[1] = gt[5] * inv_det;
    out[4] = -gt[4] * inv_det;
    out[2] = -gt[2] * inv_det;
    out[5] = gt[1] * inv_det;
    out[0] = (gt[2] * gt[3] - gt[0] * gt[5]) * inv_det;
    out[3] = (-gt[1] * gt[3] + gt[0] * gt[4]) * inv_det;
    Some(out)
}
