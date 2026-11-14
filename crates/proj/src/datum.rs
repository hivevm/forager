// The geocentric coordinates and the Helmert transform follow src/conversions/cart.cpp and
// src/transformations/helmert.cpp of PROJ 9.4.0, <https://proj.org>, and their chaining the cs2cs
// emulation of its src/4D_api.cpp and src/fwd.cpp.
// PROJ's notice, from its file COPYING; the files of PROJ this one follows hold no other:
// All source, data files and other contents of the PROJ package are available under the following
// terms. Note that the PROJ 4.3 and earlier was "public domain" as is common with US government
// work, but apparently this is not a well defined legal term in many countries. Frank Warmerdam
// placed everything under the following MIT style license because he believed it is effectively
// the same as public domain, allowing anyone to use the code as they wish, including making
// proprietary derivatives. Initial PROJ 4.3 public domain code was put as Frank Warmerdam as
// copyright holder, but he didn't mean to imply he did the work. Essentially all work was done by
// Gerald Evenden. Copyright information can be found in source files.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
// NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The shift of a longitude and latitude from one datum to WGS84 and back, through geocentric
//! coordinates, as PROJ's cs2cs emulation chains `cart` and an exact `helmert` of the position
//! vector convention.

use crate::params::{Datum, Ellipsoid};

/// PROJ's `ARCSEC_TO_RAD`.
const ARCSEC_TO_RAD: f64 = crate::params::DEG_TO_RAD / 3600.0;

/// A point in geocentric coordinates, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cartesian {
    x: f64,
    y: f64,
    z: f64,
}

/// `cartesian`: the geocentric coordinates of the longitude `lam` and the latitude `phi`, in
/// radians, on the ellipsoid, at height 0.
fn cartesian(lam: f64, phi: f64, ellipsoid: &Ellipsoid) -> Cartesian {
    let cosphi = libm::cos(phi);
    let sinphi = libm::sin(phi);
    let n = if ellipsoid.es == 0.0 {
        ellipsoid.a
    } else {
        ellipsoid.a / (1.0 - ellipsoid.es * sinphi * sinphi).sqrt()
    };
    Cartesian {
        x: n * cosphi * libm::cos(lam),
        y: n * cosphi * libm::sin(lam),
        z: n * (1.0 - ellipsoid.es) * sinphi,
    }
}

/// `hypot` as IEEE's square root of the sum of squares, which rounds alike on every target (Q-2).
fn hypot(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

/// `geodetic`: the longitude and latitude, in radians, of the geocentric `point` on the
/// ellipsoid, by Bowring's formula; the height is dropped.
fn geodetic(point: Cartesian, ellipsoid: &Ellipsoid) -> (f64, f64) {
    let (a, b) = (ellipsoid.a, ellipsoid.b());
    let p = hypot(point.x, point.y);
    let y_theta = point.z * a;
    let x_theta = p * b;
    let norm = hypot(y_theta, x_theta);
    let c = if norm == 0.0 { 1.0 } else { x_theta / norm };
    let s = if norm == 0.0 { 0.0 } else { y_theta / norm };
    let y_phi = point.z + ellipsoid.e2s() * b * s * s * s;
    let x_phi = p - ellipsoid.es * a * c * c * c;
    let phi = if x_phi <= 0.0 {
        if point.z >= 0.0 {
            crate::common::HALF_PI
        } else {
            -crate::common::HALF_PI
        }
    } else {
        libm::atan(y_phi / x_phi)
    };
    (libm::atan2(point.y, point.x), phi)
}

/// The exact rotation matrix of a Helmert transform of the rotations `rx`, `ry`, `rz`, in
/// radians, transposed for the position vector convention, as `build_rot_matrix` builds it.
fn rotation(rx: f64, ry: f64, rz: f64) -> [[f64; 3]; 3] {
    let (cf, sf) = (libm::cos(rx), libm::sin(rx));
    let (ct, st) = (libm::cos(ry), libm::sin(ry));
    let (cp, sp) = (libm::cos(rz), libm::sin(rz));
    let frame = [
        [ct * cp, cf * sp + sf * st * cp, sf * sp - cf * st * cp],
        [-ct * sp, cf * cp - sf * st * sp, sf * cp + cf * st * sp],
        [st, -sf * ct, cf * ct],
    ];
    let mut transposed = [[0.0; 3]; 3];
    for (i, row) in frame.iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            transposed[j][i] = *value;
        }
    }
    transposed
}

/// A Helmert transform of the seven parameters of a `towgs84`, from the datum into WGS84.
struct Helmert {
    translation: [f64; 3],
    /// The rotation, where it has one or a scale; a translation alone is added as it is.
    rotation: Option<[[f64; 3]; 3]>,
    scale: f64,
}

impl Helmert {
    fn new(shift: &[f64; 7]) -> Self {
        let [x, y, z, rx, ry, rz, s] = *shift;
        let no_rotation = rx == 0.0 && ry == 0.0 && rz == 0.0;
        Self {
            translation: [x, y, z],
            rotation: (!no_rotation || s != 0.0)
                .then(|| rotation(rx * ARCSEC_TO_RAD, ry * ARCSEC_TO_RAD, rz * ARCSEC_TO_RAD)),
            scale: 1.0 + s * 1e-6,
        }
    }

    /// `helmert_forward_3d`.
    fn forward(&self, p: Cartesian) -> Cartesian {
        let [tx, ty, tz] = self.translation;
        let Some(r) = self.rotation else {
            return Cartesian {
                x: p.x + tx,
                y: p.y + ty,
                z: p.z + tz,
            };
        };
        Cartesian {
            x: self.scale * (r[0][0] * p.x + r[0][1] * p.y + r[0][2] * p.z) + tx,
            y: self.scale * (r[1][0] * p.x + r[1][1] * p.y + r[1][2] * p.z) + ty,
            z: self.scale * (r[2][0] * p.x + r[2][1] * p.y + r[2][2] * p.z) + tz,
        }
    }

    /// `helmert_reverse_3d`: through the transposed matrix.
    fn reverse(&self, p: Cartesian) -> Cartesian {
        let [tx, ty, tz] = self.translation;
        let Some(r) = self.rotation else {
            return Cartesian {
                x: p.x - tx,
                y: p.y - ty,
                z: p.z - tz,
            };
        };
        let x = (p.x - tx) / self.scale;
        let y = (p.y - ty) / self.scale;
        let z = (p.z - tz) / self.scale;
        Cartesian {
            x: r[0][0] * x + r[1][0] * y + r[2][0] * z,
            y: r[0][1] * x + r[1][1] * y + r[2][1] * z,
            z: r[0][2] * x + r[1][2] * y + r[2][2] * z,
        }
    }
}

/// The longitude and latitude `(lam, phi)`, in radians, in `datum` on `ellipsoid`, shifted into
/// WGS84, as `inv_finalize` shifts them: `cart`, `helmert` forward, `cart` of WGS84 inverse.
pub(crate) fn to_wgs84(lam: f64, phi: f64, datum: &Datum, ellipsoid: &Ellipsoid) -> (f64, f64) {
    let Datum::Geocentric(shift) = datum else {
        return (lam, phi);
    };
    let mut point = cartesian(lam, phi, ellipsoid);
    if let Some(shift) = shift {
        point = Helmert::new(shift).forward(point);
    }
    geodetic(point, &Ellipsoid::wgs84())
}

/// The WGS84 longitude and latitude `(lam, phi)` shifted into `datum` on `ellipsoid`, as
/// `fwd_prepare` shifts them: `cart` of WGS84, `helmert` inverse, `cart` inverse.
pub(crate) fn from_wgs84(lam: f64, phi: f64, datum: &Datum, ellipsoid: &Ellipsoid) -> (f64, f64) {
    let Datum::Geocentric(shift) = datum else {
        return (lam, phi);
    };
    let mut point = cartesian(lam, phi, &Ellipsoid::wgs84());
    if let Some(shift) = shift {
        point = Helmert::new(shift).reverse(point);
    }
    geodetic(point, ellipsoid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geocentric_coordinates_read_back() {
        // Verifies: ADR-draft-reprojection-ported-in-proj
        let wgs84 = Ellipsoid::wgs84();
        let at = cartesian(0.0, 0.0, &wgs84);
        assert_eq!((at.x, at.y, at.z), (6_378_137.0, 0.0, 0.0));
        for (lam, phi) in [(0.3, 0.8), (-2.0, -1.2), (3.0, 1.5)] {
            let (l, p) = geodetic(cartesian(lam, phi, &wgs84), &wgs84);
            assert!(
                (l - lam).abs() < 1e-14 && (p - phi).abs() < 1e-14,
                "{lam} {phi}"
            );
        }
    }

    #[test]
    fn a_helmert_shift_reverses() {
        // Verifies: ADR-draft-reprojection-ported-in-proj
        let datum = Datum::Geocentric(Some([
            446.448, -125.157, 542.06, 0.1502, 0.247, 0.8421, -20.4894,
        ]));
        let airy = Ellipsoid {
            a: 6_377_563.396,
            es: 0.006_670_539_999_985_363,
            e: 0.006_670_539_999_985_363_f64.sqrt(),
        };
        let (lam, phi) = (-0.02, 0.9);
        let (l, p) = to_wgs84(lam, phi, &datum, &airy);
        assert!(
            (l - lam).abs() > 1e-7,
            "OSGB36 shifts the longitude by some metres"
        );
        // The height the shift gives, which the way back does not take, keeps it within some
        // millimetres, as PROJ's two-dimensional transform does.
        let (back_l, back_p) = from_wgs84(l, p, &datum, &airy);
        assert!((back_l - lam).abs() < 1e-9 && (back_p - phi).abs() < 1e-9);
    }
}
