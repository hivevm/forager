// The transverse Mercator of Poder and Engsager follows src/projections/tmerc.cpp of PROJ 9.4.0,
// <https://proj.org>, its "exact" algorithm, PROJ's default: written by Knud Poder and Karsten
// Engsager, based on the math of R. Koenig and K.H. Weise, and used by PROJ by permission of
// Reference Networks Division, Kort og Matrikelstyrelsen (KMS), Copenhagen, Denmark.
// PROJ's notice, from its file COPYING; tmerc.cpp holds no copyright line of its own:
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

//! The transverse Mercator projection, `+proj=tmerc` and `+proj=utm`, on the ellipsoid, by the
//! Poder/Engsager algorithm PROJ uses by default. On a sphere PROJ takes Evenden and Snyder's
//! approximation, which no case in scope needs, so a sphere is unsupported here.

use crate::params::{Ellipsoid, Params};
use crate::{Error, Plane};

/// `PROJ_ETMERC_ORDER`: the degree of the series.
const ORDER: usize = 6;

/// The largest normalized easting, 150° from the central meridian, past which PROJ refuses a
/// point.
const MAX_CE: f64 = 2.623_395_162_778;

/// The constants of a transverse Mercator.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tmerc {
    /// The meridian quadrant, scaled to the projection.
    qn: f64,
    /// The northing of the origin latitude.
    zb: f64,
    /// Gaussian to geodetic latitude.
    cgb: [f64; ORDER],
    /// Geodetic to Gaussian latitude.
    cbg: [f64; ORDER],
    /// Transverse Mercator to geodetic.
    utg: [f64; ORDER],
    /// Geodetic to transverse Mercator.
    gtu: [f64; ORDER],
}

/// What `utm` sets beside the series: the central meridian, the false easting and northing, and
/// the scale; the origin latitude it sets to 0.
pub(crate) struct Utm {
    pub(crate) lam0: f64,
    pub(crate) x0: f64,
    pub(crate) y0: f64,
    pub(crate) k0: f64,
}

/// `PJ_PROJECTION(utm)`: the zone of `+zone`, or the one nearest `lam0`, `+south` moving the
/// false northing; refused on a sphere.
pub(crate) fn utm(params: &Params, ellipsoid: &Ellipsoid, lam0: f64) -> Result<Utm, Error> {
    if ellipsoid.es == 0.0 {
        return Err(Error::Invalid(
            "+proj=utm: the eccentricity should not be zero".to_owned(),
        ));
    }
    let zone = match params.number("zone")? {
        Some(zone) => {
            if zone.fract() != 0.0 || !(1.0..=60.0).contains(&zone) {
                return Err(Error::Invalid(format!(
                    "+proj=utm: +zone={zone} is not 1 to 60"
                )));
            }
            zone - 1.0
        }
        None => {
            let nearest = ((crate::common::adjlon(lam0) + std::f64::consts::PI) * 30.0
                / std::f64::consts::PI)
                .floor();
            nearest.clamp(0.0, 59.0)
        }
    };
    Ok(Utm {
        lam0: (zone + 0.5) * std::f64::consts::PI / 30.0 - std::f64::consts::PI,
        x0: 500_000.0,
        y0: if params.has("south") {
            10_000_000.0
        } else {
            0.0
        },
        k0: 0.9996,
    })
}

/// `gatg`: the Clenshaw summation between geodetic and Gaussian latitude.
fn gatg(p: &[f64; ORDER], b: f64, cos_2b: f64, sin_2b: f64) -> f64 {
    let two_cos_2b = 2.0 * cos_2b;
    let (mut h, mut h1, mut h2) = (0.0, p[ORDER - 1], 0.0);
    for &c in p[..ORDER - 1].iter().rev() {
        h = -h2 + two_cos_2b * h1 + c;
        h2 = h1;
        h1 = h;
    }
    b + h * sin_2b
}

/// `clenS`: the complex Clenshaw summation; the real and the imaginary part.
fn clen_s(
    a: &[f64; ORDER],
    sin_arg_r: f64,
    cos_arg_r: f64,
    sinh_arg_i: f64,
    cosh_arg_i: f64,
) -> (f64, f64) {
    let r = 2.0 * cos_arg_r * cosh_arg_i;
    let i = -2.0 * sin_arg_r * sinh_arg_i;
    let (mut hr, mut hr1, mut hi, mut hi1) = (a[ORDER - 1], 0.0, 0.0, 0.0);
    for &c in a[..ORDER - 1].iter().rev() {
        let (hr2, hi2) = (hr1, hi1);
        hr1 = hr;
        hi1 = hi;
        hr = -hr2 + r * hr1 - i * hi1 + c;
        hi = -hi2 + i * hr1 + r * hi1;
    }
    let r = sin_arg_r * cosh_arg_i;
    let i = cos_arg_r * sinh_arg_i;
    (r * hr - i * hi, r * hi + i * hr)
}

/// `clens`: the real Clenshaw summation.
fn clens(a: &[f64; ORDER], arg_r: f64) -> f64 {
    let r = 2.0 * libm::cos(arg_r);
    let (mut hr, mut hr1) = (a[ORDER - 1], 0.0);
    for &c in a[..ORDER - 1].iter().rev() {
        let hr2 = hr1;
        hr1 = hr;
        hr = -hr2 + r * hr1 + c;
    }
    libm::sin(arg_r) * hr
}

impl Tmerc {
    /// `setup_exact`, for the origin latitude `phi0` and the scale `k0`; refused on a sphere.
    pub(crate) fn new(ellipsoid: &Ellipsoid, phi0: f64, k0: f64) -> Result<Self, Error> {
        if ellipsoid.es == 0.0 {
            return Err(Error::Unsupported("+proj=tmerc on a sphere".to_owned()));
        }
        // The third flattening, as `pj_calc_ellipsoid_params` computes it.
        let n = libm::pow(libm::tan(libm::asin(ellipsoid.e) / 2.0), 2.0);
        let mut np = n;
        let mut cgb = [0.0; ORDER];
        let mut cbg = [0.0; ORDER];
        let mut utg = [0.0; ORDER];
        let mut gtu = [0.0; ORDER];
        cgb[0] = n
            * (2.0
                + n * (-2.0 / 3.0
                    + n * (-2.0 + n * (116.0 / 45.0 + n * (26.0 / 45.0 + n * (-2854.0 / 675.0))))));
        cbg[0] = n
            * (-2.0
                + n * (2.0 / 3.0
                    + n * (4.0 / 3.0
                        + n * (-82.0 / 45.0 + n * (32.0 / 45.0 + n * (4642.0 / 4725.0))))));
        np *= n;
        cgb[1] = np
            * (7.0 / 3.0
                + n * (-8.0 / 5.0
                    + n * (-227.0 / 45.0 + n * (2704.0 / 315.0 + n * (2323.0 / 945.0)))));
        cbg[1] = np
            * (5.0 / 3.0
                + n * (-16.0 / 15.0
                    + n * (-13.0 / 9.0 + n * (904.0 / 315.0 + n * (-1522.0 / 945.0)))));
        np *= n;
        cgb[2] = np
            * (56.0 / 15.0 + n * (-136.0 / 35.0 + n * (-1262.0 / 105.0 + n * (73814.0 / 2835.0))));
        cbg[2] =
            np * (-26.0 / 15.0 + n * (34.0 / 21.0 + n * (8.0 / 5.0 + n * (-12686.0 / 2835.0))));
        np *= n;
        cgb[3] = np * (4279.0 / 630.0 + n * (-332.0 / 35.0 + n * (-399_572.0 / 14175.0)));
        cbg[3] = np * (1237.0 / 630.0 + n * (-12.0 / 5.0 + n * (-24832.0 / 14175.0)));
        np *= n;
        cgb[4] = np * (4174.0 / 315.0 + n * (-144_838.0 / 6237.0));
        cbg[4] = np * (-734.0 / 315.0 + n * (109_598.0 / 31185.0));
        np *= n;
        cgb[5] = np * (601_676.0 / 22275.0);
        cbg[5] = np * (444_337.0 / 155_925.0);

        np = n * n;
        let qn = k0 / (1.0 + n) * (1.0 + np * (1.0 / 4.0 + np * (1.0 / 64.0 + np / 256.0)));
        utg[0] = n
            * (-0.5
                + n * (2.0 / 3.0
                    + n * (-37.0 / 96.0
                        + n * (1.0 / 360.0 + n * (81.0 / 512.0 + n * (-96199.0 / 604_800.0))))));
        gtu[0] = n
            * (0.5
                + n * (-2.0 / 3.0
                    + n * (5.0 / 16.0
                        + n * (41.0 / 180.0 + n * (-127.0 / 288.0 + n * (7891.0 / 37800.0))))));
        utg[1] = np
            * (-1.0 / 48.0
                + n * (-1.0 / 15.0
                    + n * (437.0 / 1440.0
                        + n * (-46.0 / 105.0 + n * (1_118_711.0 / 3_870_720.0)))));
        gtu[1] = np
            * (13.0 / 48.0
                + n * (-3.0 / 5.0
                    + n * (557.0 / 1440.0
                        + n * (281.0 / 630.0 + n * (-1_983_433.0 / 1_935_360.0)))));
        np *= n;
        utg[2] = np
            * (-17.0 / 480.0 + n * (37.0 / 840.0 + n * (209.0 / 4480.0 + n * (-5569.0 / 90720.0))));
        gtu[2] = np
            * (61.0 / 240.0
                + n * (-103.0 / 140.0 + n * (15061.0 / 26880.0 + n * (167_603.0 / 181_440.0))));
        np *= n;
        utg[3] = np * (-4397.0 / 161_280.0 + n * (11.0 / 504.0 + n * (830_251.0 / 7_257_600.0)));
        gtu[3] =
            np * (49561.0 / 161_280.0 + n * (-179.0 / 168.0 + n * (6_601_661.0 / 7_257_600.0)));
        np *= n;
        utg[4] = np * (-4583.0 / 161_280.0 + n * (108_847.0 / 3_991_680.0));
        gtu[4] = np * (34729.0 / 80640.0 + n * (-3_418_889.0 / 1_995_840.0));
        np *= n;
        utg[5] = np * (-20_648_693.0 / 638_668_800.0);
        gtu[5] = np * (212_378_941.0 / 319_334_400.0);

        let z = gatg(&cbg, phi0, libm::cos(2.0 * phi0), libm::sin(2.0 * phi0));
        let zb = -qn * (z + clens(&gtu, 2.0 * z));
        Ok(Self {
            qn,
            zb,
            cgb,
            cbg,
            utg,
            gtu,
        })
    }
}

impl Plane for Tmerc {
    /// `exact_e_fwd`.
    fn forward(&self, lam: f64, phi: f64) -> Option<(f64, f64)> {
        let mut cn = gatg(&self.cbg, phi, libm::cos(2.0 * phi), libm::sin(2.0 * phi));
        let sin_cn = libm::sin(cn);
        let cos_cn = libm::cos(cn);
        let sin_ce = libm::sin(lam);
        let cos_ce = libm::cos(lam);
        let cos_cn_cos_ce = cos_cn * cos_ce;
        cn = libm::atan2(sin_cn, cos_cn_cos_ce);
        let inv_denom_tan_ce = 1.0 / libm::hypot(sin_cn, cos_cn_cos_ce);
        let tan_ce = sin_ce * cos_cn * inv_denom_tan_ce;
        let mut ce = libm::asinh(tan_ce);
        let two_inv_denom_tan_ce = 2.0 * inv_denom_tan_ce;
        let two_inv_denom_tan_ce_square = two_inv_denom_tan_ce * inv_denom_tan_ce;
        let tmp_r = cos_cn_cos_ce * two_inv_denom_tan_ce_square;
        let sin_arg_r = sin_cn * tmp_r;
        let cos_arg_r = cos_cn_cos_ce * tmp_r - 1.0;
        let sinh_arg_i = tan_ce * two_inv_denom_tan_ce;
        let cosh_arg_i = two_inv_denom_tan_ce_square - 1.0;
        let (dcn, dce) = clen_s(&self.gtu, sin_arg_r, cos_arg_r, sinh_arg_i, cosh_arg_i);
        cn += dcn;
        ce += dce;
        (ce.abs() <= MAX_CE).then_some((self.qn * ce, self.qn * cn + self.zb))
    }

    /// `exact_e_inv`.
    fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let mut cn = (y - self.zb) / self.qn;
        let mut ce = x / self.qn;
        if ce.abs() > MAX_CE {
            return None;
        }
        let sin_arg_r = libm::sin(2.0 * cn);
        let cos_arg_r = libm::cos(2.0 * cn);
        let exp_2_ce = libm::exp(2.0 * ce);
        let half_inv_exp_2_ce = 0.5 / exp_2_ce;
        let sinh_arg_i = 0.5 * exp_2_ce - half_inv_exp_2_ce;
        let cosh_arg_i = 0.5 * exp_2_ce + half_inv_exp_2_ce;
        let (dcn, dce) = clen_s(&self.utg, sin_arg_r, cos_arg_r, sinh_arg_i, cosh_arg_i);
        cn += dcn;
        ce += dce;
        let sin_cn = libm::sin(cn);
        let cos_cn = libm::cos(cn);
        let sinh_ce = libm::sinh(ce);
        ce = libm::atan2(sinh_ce, cos_cn);
        let modulus_ce = libm::hypot(sinh_ce, cos_cn);
        cn = libm::atan2(sin_cn, modulus_ce);
        let tmp = 2.0 * modulus_ce / (sinh_ce * sinh_ce + 1.0);
        let sin_2_cn = sin_cn * tmp;
        let cos_2_cn = tmp * modulus_ce - 1.0;
        Some((ce, gatg(&self.cgb, cn, cos_2_cn, sin_2_cn)))
    }
}
