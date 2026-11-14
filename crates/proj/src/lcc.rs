// The Lambert conformal conic follows src/projections/lcc.cpp of PROJ 9.4.0, <https://proj.org>.
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

//! The Lambert conformal conic projection, `+proj=lcc`, on the sphere and the ellipsoid.

use crate::common::{FRAC_PI_4, HALF_PI, msfn, phi2, tsfn};
use crate::params::{Ellipsoid, Params};
use crate::{Error, Plane};

const EPS10: f64 = 1e-10;

/// The constants of a Lambert conformal conic.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lcc {
    n: f64,
    rho0: f64,
    c: f64,
    es: f64,
    e: f64,
    k0: f64,
}

/// What `lcc` sets the projection's origin latitude to, beside its constants: `lat_1` where
/// neither `lat_2` nor `lat_0` is given.
pub(crate) fn origin_latitude(params: &Params) -> Result<Option<f64>, Error> {
    if params.has("lat_2") || params.has("lat_0") {
        return Ok(None);
    }
    Ok(Some(params.radians("lat_1")?.unwrap_or(0.0)))
}

impl Lcc {
    /// `PJ_PROJECTION(lcc)`, for the origin latitude `phi0` and the scale `k0`.
    pub(crate) fn new(
        params: &Params,
        ellipsoid: &Ellipsoid,
        phi0: f64,
        k0: f64,
    ) -> Result<Self, Error> {
        let phi1 = params.radians("lat_1")?.unwrap_or(0.0);
        let phi2 = params.radians("lat_2")?.unwrap_or(phi1);
        let invalid = |why: &str| Err(Error::Invalid(format!("+proj=lcc: {why}")));
        if (phi1 + phi2).abs() < EPS10 {
            return invalid("|lat_1 + lat_2| should be > 0");
        }
        let mut sinphi = libm::sin(phi1);
        let mut n = sinphi;
        let cosphi = libm::cos(phi1);
        if cosphi.abs() < EPS10 || phi1.abs() >= HALF_PI {
            return invalid("|lat_1| should be < 90°");
        }
        if libm::cos(phi2).abs() < EPS10 || phi2.abs() >= HALF_PI {
            return invalid("|lat_2| should be < 90°");
        }
        let secant = (phi1 - phi2).abs() >= EPS10;
        let (es, e) = (ellipsoid.es, ellipsoid.e);
        let (rho0, c);
        if es != 0.0 {
            let m1 = msfn(sinphi, cosphi, es);
            let ml1 = tsfn(phi1, sinphi, e);
            if secant {
                sinphi = libm::sin(phi2);
                n = libm::log(m1 / msfn(sinphi, libm::cos(phi2), es));
                if n == 0.0 {
                    return invalid("the eccentricity gives no cone");
                }
                let ml2 = tsfn(phi2, sinphi, e);
                let denom = libm::log(ml1 / ml2);
                if denom == 0.0 {
                    return invalid("the eccentricity gives no cone");
                }
                n /= denom;
            }
            c = m1 * libm::pow(ml1, -n) / n;
            rho0 = c * if (phi0.abs() - HALF_PI).abs() < EPS10 {
                0.0
            } else {
                libm::pow(tsfn(phi0, libm::sin(phi0), e), n)
            };
        } else {
            if secant {
                n = libm::log(cosphi / libm::cos(phi2))
                    / libm::log(
                        libm::tan(FRAC_PI_4 + 0.5 * phi2) / libm::tan(FRAC_PI_4 + 0.5 * phi1),
                    );
            }
            if n == 0.0 {
                return invalid("|lat_1 + lat_2| should be > 0");
            }
            c = cosphi * libm::pow(libm::tan(FRAC_PI_4 + 0.5 * phi1), n) / n;
            rho0 = if (phi0.abs() - HALF_PI).abs() < EPS10 {
                0.0
            } else {
                c * libm::pow(libm::tan(FRAC_PI_4 + 0.5 * phi0), -n)
            };
        }
        Ok(Self {
            n,
            rho0,
            c,
            es,
            e,
            k0,
        })
    }
}

impl Plane for Lcc {
    /// `lcc_e_forward`.
    fn forward(&self, lam: f64, phi: f64) -> Option<(f64, f64)> {
        let rho = if (phi.abs() - HALF_PI).abs() < EPS10 {
            if phi * self.n <= 0.0 {
                return None;
            }
            0.0
        } else {
            self.c
                * if self.es != 0.0 {
                    libm::pow(tsfn(phi, libm::sin(phi), self.e), self.n)
                } else {
                    libm::pow(libm::tan(FRAC_PI_4 + 0.5 * phi), -self.n)
                }
        };
        let lam = lam * self.n;
        Some((
            self.k0 * (rho * libm::sin(lam)),
            self.k0 * (self.rho0 - rho * libm::cos(lam)),
        ))
    }

    /// `lcc_e_inverse`.
    fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let mut x = x / self.k0;
        let mut y = self.rho0 - y / self.k0;
        let mut rho = (x * x + y * y).sqrt();
        if rho == 0.0 {
            return Some((0.0, if self.n > 0.0 { HALF_PI } else { -HALF_PI }));
        }
        if self.n < 0.0 {
            rho = -rho;
            x = -x;
            y = -y;
        }
        let phi = if self.es != 0.0 {
            phi2(libm::pow(rho / self.c, 1.0 / self.n), self.e)?
        } else {
            2.0 * libm::atan(libm::pow(self.c / rho, 1.0 / self.n)) - HALF_PI
        };
        Some((libm::atan2(x, y) / self.n, phi))
    }
}
