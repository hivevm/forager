// The orthographic projection follows src/projections/ortho.cpp of PROJ 9.4.0, <https://proj.org>.
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

//! The orthographic projection, `+proj=ortho`, on the sphere and the ellipsoid: a point on the far
//! hemisphere does not transform.

use crate::Plane;
use crate::common::{HALF_PI, adjlon};
use crate::params::Ellipsoid;

const EPS10: f64 = 1e-10;

/// Where the projection is centred.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    NPole,
    SPole,
    Equit,
    Obliq,
}

/// The constants of an orthographic projection.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ortho {
    phi0: f64,
    sinph0: f64,
    cosph0: f64,
    nu0: f64,
    y_shift: f64,
    y_scale: f64,
    mode: Mode,
    es: f64,
    /// `P->a / P->b`.
    a_over_b: f64,
}

impl Ortho {
    /// `PJ_PROJECTION(ortho)`.
    pub(crate) fn new(ellipsoid: &Ellipsoid, phi0: f64) -> Self {
        let sinph0 = libm::sin(phi0);
        let cosph0 = libm::cos(phi0);
        let mode = if (phi0.abs() - HALF_PI).abs() <= EPS10 {
            if phi0 < 0.0 { Mode::SPole } else { Mode::NPole }
        } else if phi0.abs() > EPS10 {
            Mode::Obliq
        } else {
            Mode::Equit
        };
        let es = ellipsoid.es;
        let (mut nu0, mut y_shift, mut y_scale) = (0.0, 0.0, 0.0);
        if es != 0.0 {
            nu0 = 1.0 / libm::sqrt(1.0 - es * sinph0 * sinph0);
            y_shift = es * nu0 * sinph0 * cosph0;
            y_scale = 1.0 / libm::sqrt(1.0 - es * cosph0 * cosph0);
        }
        Self {
            phi0,
            sinph0,
            cosph0,
            nu0,
            y_shift,
            y_scale,
            mode,
            es,
            a_over_b: 1.0 / libm::sqrt(1.0 - es),
        }
    }

    /// `ortho_s_forward`.
    fn s_forward(&self, lam: f64, phi: f64) -> Option<(f64, f64)> {
        let cosphi = libm::cos(phi);
        let mut coslam = libm::cos(lam);
        let y = match self.mode {
            Mode::Equit => {
                if cosphi * coslam < -EPS10 {
                    return None;
                }
                libm::sin(phi)
            }
            Mode::Obliq => {
                let sinphi = libm::sin(phi);
                if self.sinph0 * sinphi + self.cosph0 * cosphi * coslam < -EPS10 {
                    return None;
                }
                self.cosph0 * sinphi - self.sinph0 * cosphi * coslam
            }
            Mode::NPole | Mode::SPole => {
                if self.mode == Mode::NPole {
                    coslam = -coslam;
                }
                if (phi - self.phi0).abs() - EPS10 > HALF_PI {
                    return None;
                }
                cosphi * coslam
            }
        };
        Some((cosphi * libm::sin(lam), y))
    }

    /// `ortho_s_inverse`.
    fn s_inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (mut x, mut y) = (x, y);
        let rh = libm::hypot(x, y);
        let mut sinc = rh;
        if sinc > 1.0 {
            if sinc - 1.0 > EPS10 {
                return None;
            }
            sinc = 1.0;
        }
        let cosc = libm::sqrt(1.0 - sinc * sinc);
        if rh.abs() <= EPS10 {
            return Some((0.0, self.phi0));
        }
        let mut phi;
        match self.mode {
            Mode::NPole => {
                y = -y;
                phi = libm::acos(sinc);
            }
            Mode::SPole => phi = -libm::acos(sinc),
            Mode::Equit | Mode::Obliq => {
                if self.mode == Mode::Equit {
                    phi = y * sinc / rh;
                    x *= sinc;
                    y = cosc * rh;
                } else {
                    phi = cosc * self.sinph0 + y * sinc * self.cosph0 / rh;
                    y = (cosc - self.sinph0 * phi) * rh;
                    x *= sinc * self.cosph0;
                }
                phi = if phi.abs() >= 1.0 {
                    if phi < 0.0 { -HALF_PI } else { HALF_PI }
                } else {
                    libm::asin(phi)
                };
            }
        }
        let lam = if y == 0.0 && matches!(self.mode, Mode::Obliq | Mode::Equit) {
            if x == 0.0 {
                0.0
            } else if x < 0.0 {
                -HALF_PI
            } else {
                HALF_PI
            }
        } else {
            libm::atan2(x, y)
        };
        Some((lam, phi))
    }

    /// `ortho_e_inverse`.
    fn e_inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let sq = |v: f64| v * v;
        let es = self.es;
        if matches!(self.mode, Mode::NPole | Mode::SPole) {
            let rh2 = sq(x) + sq(y);
            let phi = if rh2 >= 1.0 - 1e-15 {
                if rh2 - 1.0 > EPS10 {
                    return None;
                }
                0.0
            } else {
                let sign = if self.mode == Mode::NPole { 1.0 } else { -1.0 };
                libm::acos(libm::sqrt(rh2 * (1.0 - es) / (1.0 - es * rh2))) * sign
            };
            let flip = if self.mode == Mode::NPole { -1.0 } else { 1.0 };
            return Some((libm::atan2(x, y * flip), phi));
        }
        if self.mode == Mode::Equit {
            if sq(x) + sq(y * self.a_over_b) > 1.0 + 1e-11 {
                return None;
            }
            let sinphi2 = if y == 0.0 {
                0.0
            } else {
                1.0 / (sq((1.0 - es) / y) + es)
            };
            let sign = |v: f64| if v > 0.0 { 1.0 } else { -1.0 };
            if sinphi2 > 1.0 - 1e-11 {
                return Some((0.0, HALF_PI * sign(y)));
            }
            let phi = libm::asin(libm::sqrt(sinphi2)) * sign(y);
            let sinlam = x * libm::sqrt((1.0 - es * sinphi2) / (1.0 - sinphi2));
            let lam = if sinlam.abs() - 1.0 > -1e-15 {
                HALF_PI * sign(x)
            } else {
                libm::asin(sinlam)
            };
            return Some((lam, phi));
        }
        let recentered_y = (y - self.y_shift) / self.y_scale;
        if sq(x) + sq(recentered_y) > 1.0 + 1e-11 {
            return None;
        }
        // The spherical inverse is a better first guess than the centre near the poles; a point it
        // does not transform takes PROJ's HUGE_VAL, as the iteration would.
        let (mut lam, mut phi) = self.s_inverse(x, recentered_y)?;
        for _ in 0..20 {
            let cosphi = libm::cos(phi);
            let sinphi = libm::sin(phi);
            let coslam = libm::cos(lam);
            let sinlam = libm::sin(lam);
            let one_minus_es_sinphi2 = 1.0 - es * sinphi * sinphi;
            let nu = 1.0 / libm::sqrt(one_minus_es_sinphi2);
            let x_new = nu * cosphi * sinlam;
            let y_new = nu * (sinphi * self.cosph0 - cosphi * self.sinph0 * coslam)
                + es * (self.nu0 * self.sinph0 - nu * sinphi) * self.cosph0;
            let rho = (1.0 - es) * nu / one_minus_es_sinphi2;
            let j11 = -rho * sinphi * sinlam;
            let j12 = nu * cosphi * coslam;
            let j21 = rho * (cosphi * self.cosph0 + sinphi * self.sinph0 * coslam);
            let j22 = nu * self.sinph0 * cosphi * sinlam;
            let d = j11 * j22 - j12 * j21;
            let dx = x - x_new;
            let dy = y - y_new;
            let dphi = (j22 * dx - j12 * dy) / d;
            let dlam = (-j21 * dx + j11 * dy) / d;
            phi += dphi;
            if phi > HALF_PI {
                phi = HALF_PI - (phi - HALF_PI);
                lam = adjlon(lam + std::f64::consts::PI);
            } else if phi < -HALF_PI {
                phi = -HALF_PI + (-HALF_PI - phi);
                lam = adjlon(lam + std::f64::consts::PI);
            }
            lam += dlam;
            if dphi.abs() < 1e-12 && dlam.abs() < 1e-12 {
                return Some((lam, phi));
            }
        }
        None
    }
}

impl Plane for Ortho {
    /// `ortho_s_forward` and `ortho_e_forward`.
    fn forward(&self, lam: f64, phi: f64) -> Option<(f64, f64)> {
        if self.es == 0.0 {
            return self.s_forward(lam, phi);
        }
        let cosphi = libm::cos(phi);
        let sinphi = libm::sin(phi);
        let coslam = libm::cos(lam);
        let sinlam = libm::sin(lam);
        if self.sinph0 * sinphi + self.cosph0 * cosphi * coslam < -EPS10 {
            return None;
        }
        let nu = 1.0 / libm::sqrt(1.0 - self.es * sinphi * sinphi);
        Some((
            nu * cosphi * sinlam,
            nu * (sinphi * self.cosph0 - cosphi * self.sinph0 * coslam)
                + self.es * (self.nu0 * self.sinph0 - nu * sinphi) * self.cosph0,
        ))
    }

    /// `ortho_s_inverse` and `ortho_e_inverse`.
    fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if self.es == 0.0 {
            self.s_inverse(x, y)
        } else {
            self.e_inverse(x, y)
        }
    }
}
