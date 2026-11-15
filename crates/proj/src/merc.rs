// The Mercator projection follows src/projections/merc.cpp of PROJ 9.4.0, <https://proj.org>.
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

//! The Mercator projection, `+proj=merc`, on the sphere and the ellipsoid.

use crate::common::{HALF_PI, msfn, sinhpsi2tanphi};
use crate::params::{Ellipsoid, Params};
use crate::{Error, Plane};

/// The constants of a Mercator projection.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Merc {
    e: f64,
    es: f64,
    k0: f64,
}

impl Merc {
    /// `PJ_PROJECTION(merc)`: the scale `k0` replaced by that of `+lat_ts` where it is given.
    pub(crate) fn new(params: &Params, ellipsoid: &Ellipsoid, k0: f64) -> Result<Self, Error> {
        let mut k0 = k0;
        if let Some(phits) = params.radians("lat_ts")? {
            let phits = phits.abs();
            if phits >= HALF_PI {
                return Err(Error::Invalid(
                    "+proj=merc: |lat_ts| should be <= 90°".to_owned(),
                ));
            }
            k0 = if ellipsoid.es != 0.0 {
                msfn(libm::sin(phits), libm::cos(phits), ellipsoid.es)
            } else {
                libm::cos(phits)
            };
        }
        Ok(Self {
            e: ellipsoid.e,
            es: ellipsoid.es,
            k0,
        })
    }
}

impl Plane for Merc {
    /// `merc_e_forward` and `merc_s_forward`.
    fn forward(&self, lam: f64, phi: f64) -> Option<(f64, f64)> {
        let y = if self.es != 0.0 {
            let sphi = libm::sin(phi);
            let cphi = libm::cos(phi);
            self.k0 * (libm::asinh(sphi / cphi) - self.e * libm::atanh(self.e * sphi))
        } else {
            self.k0 * libm::asinh(libm::tan(phi))
        };
        Some((self.k0 * lam, y))
    }

    /// `merc_e_inverse` and `merc_s_inverse`.
    fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let phi = if self.es != 0.0 {
            libm::atan(sinhpsi2tanphi(libm::sinh(y / self.k0), self.e)?)
        } else {
            libm::atan(libm::sinh(y / self.k0))
        };
        Some((x / self.k0, phi))
    }
}
