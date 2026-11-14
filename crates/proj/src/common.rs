// The functions of this file follow src/tsfn.cpp, src/phi2.cpp, src/msfn.cpp, and src/adjlon.cpp
// of PROJ 9.4.0, <https://proj.org>.
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

//! The functions the projections share, as PROJ computes them.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// `pj_tsfn`: ts(φ) of Snyder (1987), eq. (7-10), exp(−ψ) of the isometric latitude ψ, for the
/// latitude `phi`, its sine `sinphi`, and the eccentricity `e`.
pub(crate) fn tsfn(phi: f64, sinphi: f64, e: f64) -> f64 {
    let cosphi = libm::cos(phi);
    libm::exp(e * libm::atanh(e * sinphi))
        * if sinphi > 0.0 {
            cosphi / (1.0 + sinphi)
        } else {
            (1.0 - sinphi) / cosphi
        }
}

/// `pj_msfn`: m(φ) of Snyder, eq. (14-15), for the sine and cosine of the latitude and the
/// squared eccentricity `es`.
pub(crate) fn msfn(sinphi: f64, cosphi: f64, es: f64) -> f64 {
    cosphi / (1.0 - es * sinphi * sinphi).sqrt()
}

/// `pj_sinhpsi2tanphi`: tan φ for τ′ = sinh ψ, by Newton's method from GeographicLib's
/// `Math::tauf`; `None` where it does not converge, where PROJ sets an error.
pub(crate) fn sinhpsi2tanphi(taup: f64, e: f64) -> Option<f64> {
    const NUMIT: u32 = 5;
    let rooteps = f64::EPSILON.sqrt();
    let tol = rooteps / 10.0;
    let tmax = 2.0 / rooteps;
    let e2m = 1.0 - e * e;
    let stol = tol * if taup.abs() > 1.0 { taup.abs() } else { 1.0 };
    let mut tau = if taup.abs() > 70.0 {
        taup * libm::exp(e * libm::atanh(e))
    } else {
        taup / e2m
    };
    // Infinities, NaN, and e = 1 return at once.
    if tau.abs().partial_cmp(&tmax) != Some(std::cmp::Ordering::Less) {
        return Some(tau);
    }
    for _ in 0..NUMIT {
        let tau1 = (1.0 + tau * tau).sqrt();
        let sig = libm::sinh(e * libm::atanh(e * tau / tau1));
        let taupa = (1.0 + sig * sig).sqrt() * tau - sig * tau1;
        let dtau = (taup - taupa) * (1.0 + e2m * (tau * tau))
            / (e2m * tau1 * (1.0 + taupa * taupa).sqrt());
        tau += dtau;
        // The test backwards, so that a NaN ends the iteration.
        if dtau
            .abs()
            .partial_cmp(&stol)
            .is_none_or(std::cmp::Ordering::is_lt)
        {
            return Some(tau);
        }
    }
    None
}

/// `pj_phi2`: the latitude whose ts(φ) is `ts0`; `None` where PROJ fails.
pub(crate) fn phi2(ts0: f64, e: f64) -> Option<f64> {
    sinhpsi2tanphi((1.0 / ts0 - ts0) / 2.0, e).map(libm::atan)
}

/// `adjlon`: the longitude reduced to −π…π, slightly overshooting it so that the sign does not
/// switch spuriously at the dateline.
pub(crate) fn adjlon(longitude: f64) -> f64 {
    if longitude.abs() < PI + 1e-12 {
        return longitude;
    }
    let mut longitude = longitude + PI;
    longitude -= TAU * (longitude / TAU).floor();
    longitude - PI
}

/// π/4, PROJ's `M_FORTPI`.
pub(crate) const FRAC_PI_4: f64 = std::f64::consts::FRAC_PI_4;

/// π/2, PROJ's `M_HALFPI`.
pub(crate) const HALF_PI: f64 = FRAC_PI_2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_longitude_past_the_dateline_comes_back_within_it() {
        // Verifies: ADR-draft-reprojection-ported-in-proj
        assert_eq!(adjlon(1.0), 1.0);
        assert!((adjlon(PI + 0.5) - (-PI + 0.5)).abs() < 1e-15);
        assert!((adjlon(-3.0 * PI - 0.25) - (PI - 0.25)).abs() < 1e-12);
    }

    #[test]
    fn phi2_inverts_tsfn() {
        // Verifies: ADR-draft-reprojection-ported-in-proj
        let e = 0.081_819_190_842_621_49;
        for degrees in [-89.0, -45.0, 0.0, 12.5, 49.0, 77.0, 89.9] {
            let phi: f64 = degrees * crate::params::DEG_TO_RAD;
            let back = phi2(tsfn(phi, libm::sin(phi), e), e).unwrap();
            assert!(
                (back - phi).abs() < 1e-14,
                "{degrees}: {back} against {phi}"
            );
        }
    }
}
