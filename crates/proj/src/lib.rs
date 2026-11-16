// A point's way from one projection into another follows src/fwd.cpp and src/inv.cpp of PROJ 9.4.0,
// <https://proj.org>; whether two projections differ follows msProjectionsDiffer, src/mapproject.c,
// of MapServer at the release pinned in scripts/fetch-msautotest.sh.
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
// Copyright (c) 2000, Frank Warmerdam; Copyright (c) 2018, Thomas Knudsen / SDFE (src/fwd.cpp,
// src/inv.cpp)
// Each file grants the MIT licence below; PROJ's notice, from its file COPYING:
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

//! The reprojection of the core: its own port of PROJ's formulas, every transcendental function
//! taken from `libm`, so that a coordinate reprojects to the same bits on every target
//! (ADR-0037, Q-2). It reads a projection as the proj4 string of the style IR and implements only
//! what a case in scope or a ticket needs: the projections longitude and latitude, `lcc`, `merc`,
//! `tmerc` with `utm`, and `ortho`, the parameter `+over`, and a datum's shift by `+towgs84`.
//! Anything else is an [`Error`] naming it, and a point that does not transform is
//! [`NotTransformed`], never a panic (Q-3). On a rotated map, the map's side of a reprojection
//! holds the map's [`Geotransform`], MapServer's step between the map's coordinates and the frame
//! of its image (ADR-0045).

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod common;
mod datum;
mod geotransform;
mod lcc;
mod merc;
mod ortho;
mod params;
mod rect;
mod shape;
mod tmerc;

use std::fmt;

use forager_style_ir::Coord;

use crate::common::{HALF_PI, adjlon};
pub use crate::geotransform::Geotransform;
use crate::params::{DEG_TO_RAD, Datum, Ellipsoid, Params, RAD_TO_DEG};
pub use crate::shape::Shape;

/// Why a projection cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A projection, a parameter, or a value this crate does not implement, named.
    Unsupported(String),
    /// A parameter whose value PROJ refuses too, with why.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(what) => write!(f, "{what} is not supported"),
            Error::Invalid(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

/// A point that does not transform: outside a projection's domain, or past what its formulas
/// converge on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotTransformed;

/// A projection's plane: the forward and inverse formulas, on an ellipsoid of semimajor axis 1,
/// the longitude counted from the central meridian, in radians; `None` where a point does not
/// transform.
trait Plane {
    fn forward(&self, lam: f64, phi: f64) -> Option<(f64, f64)>;
    fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)>;
}

/// What a projection's coordinates are.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// Longitude and latitude, in degrees.
    LongLat,
    Lcc(lcc::Lcc),
    Merc(merc::Merc),
    Tmerc(tmerc::Tmerc),
    Ortho(ortho::Ortho),
}

/// The projections this crate implements, by the names PROJ gives them.
const PROJECTIONS: &[&str] = &[
    "longlat", "latlong", "lonlat", "latlon", "lcc", "merc", "tmerc", "utm", "ortho",
];

/// The parameters this crate reads; any other is unsupported. `no_defs`, `wktext`, and
/// `type=crs` change nothing PROJ computes here.
const KNOWN: &[&str] = &[
    "proj", "datum", "ellps", "R", "a", "b", "rf", "f", "es", "e", "towgs84", "nadgrids", "lat_0",
    "lon_0", "lat_1", "lat_2", "lat_ts", "k", "k_0", "x_0", "y_0", "units", "to_meter", "no_defs",
    "wktext", "type", "zone", "south", "over",
];

/// A projection read from a proj4 string.
#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    kind: Kind,
    ellipsoid: Ellipsoid,
    datum: Datum,
    /// The central meridian, in radians.
    lam0: f64,
    /// The false easting and northing, in metres.
    x0: f64,
    y0: f64,
    /// The metres of one unit of its coordinates.
    to_meter: f64,
    /// `+over`: a longitude is not brought back within ±180° on the way in or out.
    over: bool,
}

impl Projection {
    /// Reads `proj4`, a proj4 string as the style IR holds it, as PROJ's `pj_init` reads it; an
    /// error names a projection or a parameter this crate does not implement, or a value PROJ
    /// refuses.
    pub fn parse(proj4: &str) -> Result<Self, Error> {
        let params = Params::read(proj4);
        // The projection is named first, so that one not implemented is named rather than a
        // parameter only it reads.
        let name = params
            .text("proj")?
            .ok_or_else(|| Error::Invalid("the projection names no +proj".to_owned()))?;
        if !PROJECTIONS.contains(&name) {
            return Err(Error::Unsupported(format!("+proj={name}")));
        }
        if let Some(unknown) = params.keys().find(|key| !KNOWN.contains(key)) {
            return Err(Error::Unsupported(format!("+{unknown}")));
        }
        if let Some(kind) = params.text("type")?
            && kind != "crs"
        {
            return Err(Error::Unsupported(format!("+type={kind}")));
        }
        let (ellipsoid, datum) = params::ellipsoid_and_datum(&params)?;
        let mut k0 = match params.number("k_0")? {
            Some(k0) => k0,
            None => params.number("k")?.unwrap_or(1.0),
        };
        if k0 <= 0.0 {
            return Err(Error::Invalid(format!(
                "the scale factor {k0} is not positive"
            )));
        }
        let mut phi0 = params.radians("lat_0")?.unwrap_or(0.0);
        let mut lam0 = params.radians("lon_0")?.unwrap_or(0.0);
        let mut x0 = params.number("x_0")?.unwrap_or(0.0);
        let mut y0 = params.number("y_0")?.unwrap_or(0.0);
        let kind = match name {
            "longlat" | "latlong" | "lonlat" | "latlon" => {
                if params.has("lon_0") {
                    return Err(Error::Unsupported(format!("+lon_0 of +proj={name}")));
                }
                Kind::LongLat
            }
            "lcc" => {
                if let Some(origin) = lcc::origin_latitude(&params)? {
                    phi0 = origin;
                }
                Kind::Lcc(lcc::Lcc::new(&params, &ellipsoid, phi0, k0)?)
            }
            "merc" => Kind::Merc(merc::Merc::new(&params, &ellipsoid, k0)?),
            "ortho" => Kind::Ortho(ortho::Ortho::new(&ellipsoid, phi0)),
            "tmerc" => Kind::Tmerc(tmerc::Tmerc::new(&ellipsoid, phi0, k0)?),
            "utm" => {
                let utm = tmerc::utm(&params, &ellipsoid, lam0)?;
                (lam0, x0, y0, k0, phi0) = (utm.lam0, utm.x0, utm.y0, utm.k0, 0.0);
                Kind::Tmerc(tmerc::Tmerc::new(&ellipsoid, phi0, k0)?)
            }
            other => return Err(Error::Unsupported(format!("+proj={other}"))),
        };
        if phi0.abs() > HALF_PI {
            return Err(Error::Invalid("|lat_0| should be <= 90°".to_owned()));
        }
        Ok(Self {
            kind,
            ellipsoid,
            datum,
            lam0,
            x0,
            y0,
            to_meter: params::to_meter(&params)?,
            over: params.has("over"),
        })
    }

    /// Its base geographic CRS, as `getBaseGeographicCRS` of MapServer's `src/mapproject.c` takes
    /// it from PROJ: longitude and latitude in degrees, on its ellipsoid and in its datum.
    pub(crate) fn base_geographic(&self) -> Self {
        Self {
            kind: Kind::LongLat,
            ellipsoid: self.ellipsoid,
            datum: self.datum,
            lam0: 0.0,
            x0: 0.0,
            y0: 0.0,
            to_meter: 1.0,
            over: false,
        }
    }

    /// Longitude and latitude on WGS84: the projections a way that runs no formula holds, which
    /// nothing reads.
    fn stand_in() -> Self {
        Self {
            kind: Kind::LongLat,
            ellipsoid: params::Ellipsoid::wgs84(),
            datum: Datum::Wgs84,
            lam0: 0.0,
            x0: 0.0,
            y0: 0.0,
            to_meter: 1.0,
            over: false,
        }
    }

    /// The same projection with `+over`, as `msProjectRect` copies a projection with it.
    pub(crate) fn with_over(&self) -> Self {
        Self {
            over: true,
            ..self.clone()
        }
    }

    /// `adjlon` of `lam`, unless `+over` keeps it as it is (`inv_finalize`, `fwd_prepare`).
    fn wrap(&self, lam: f64) -> f64 {
        if self.over { lam } else { adjlon(lam) }
    }

    /// Whether its coordinates are longitude and latitude.
    pub fn is_geographic(&self) -> bool {
        self.kind == Kind::LongLat
    }

    /// The plane of a projected kind.
    fn plane(&self) -> Option<&dyn Plane> {
        match &self.kind {
            Kind::LongLat => None,
            Kind::Lcc(lcc) => Some(lcc),
            Kind::Merc(merc) => Some(merc),
            Kind::Tmerc(tmerc) => Some(tmerc),
            Kind::Ortho(ortho) => Some(ortho),
        }
    }

    /// The WGS84 longitude and latitude, in radians, of `point` in this projection, as `pj_inv`
    /// and `inv_finalize` compute them.
    fn to_wgs84(&self, point: Coord) -> Option<(f64, f64)> {
        let (lam, phi) = match self.plane() {
            None => (self.wrap(point.x * DEG_TO_RAD), point.y * DEG_TO_RAD),
            Some(plane) => {
                let ra = 1.0 / self.ellipsoid.a;
                let x = (point.x * self.to_meter - self.x0) * ra;
                let y = (point.y * self.to_meter - self.y0) * ra;
                let (lam, phi) = plane.inverse(x, y)?;
                (self.wrap(lam + self.lam0), phi)
            }
        };
        if !(lam.is_finite() && phi.is_finite()) {
            return None;
        }
        let (lam, phi) = datum::to_wgs84(lam, phi, &self.datum, &self.ellipsoid);
        (lam.is_finite() && phi.is_finite()).then_some((lam, phi))
    }

    /// The point of this projection at the WGS84 longitude `lam` and latitude `phi`, in radians,
    /// as `fwd_prepare`, `pj_fwd`, and `fwd_finalize` compute it.
    fn at_wgs84(&self, lam: f64, phi: f64) -> Option<Coord> {
        // A latitude past a pole, or a longitude past ±10 radians, is refused; one a hair past a
        // pole is clamped to it.
        if phi.abs() - HALF_PI > 1e-12 || !(-10.0..=10.0).contains(&lam) {
            return None;
        }
        let phi = phi.clamp(-HALF_PI, HALF_PI);
        let (lam, phi) = datum::from_wgs84(self.wrap(lam), phi, &self.datum, &self.ellipsoid);
        let lam = self.wrap(lam - self.lam0);
        let point = match self.plane() {
            None => Coord {
                x: lam * RAD_TO_DEG,
                y: phi * RAD_TO_DEG,
            },
            Some(plane) => {
                let (x, y) = plane.forward(lam, phi)?;
                let fr_meter = 1.0 / self.to_meter;
                Coord {
                    x: fr_meter * (self.ellipsoid.a * x + self.x0),
                    y: fr_meter * (self.ellipsoid.a * y + self.y0),
                }
            }
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }
}

/// Whether a layer in the projection `layer` is reprojected into a map in `map`, as
/// `msProjectionsDiffer` finds it: not where either has no parameters, and otherwise where their
/// parameters differ in number or in any one, compared in order as text.
pub fn differ(layer: &str, map: &str) -> bool {
    let layer: Vec<&str> = layer.split_whitespace().collect();
    let map: Vec<&str> = map.split_whitespace().collect();
    !layer.is_empty() && !map.is_empty() && layer != map
}

/// The way from one projection into another.
#[derive(Clone, Debug, PartialEq)]
pub struct Reprojection {
    from: Projection,
    to: Projection,
    /// Whether each side is longitude and latitude as `msProjIsGeographicCRS` finds it: never a
    /// map's `AUTO`, which has no PROJ object.
    from_geographic: bool,
    to_geographic: bool,
    /// Whether PROJ's formulas run between the two: not where MapServer creates no PROJ object,
    /// or one from a CRS into the same CRS.
    formulas: bool,
    /// The geotransform of a rotated map on the side reprojected from, applied before the
    /// formulas, and on the side reprojected into, its inverse applied after them.
    from_gt: Option<Geotransform>,
    to_gt: Option<Geotransform>,
}

impl Reprojection {
    /// The way from `from` into `to`.
    pub fn new(from: Projection, to: Projection) -> Self {
        Self {
            from_geographic: from.is_geographic(),
            to_geographic: to.is_geographic(),
            from,
            to,
            formulas: true,
            from_gt: None,
            to_gt: None,
        }
    }

    /// The way that runs no formula, where MapServer creates no PROJ object or one from a CRS into
    /// the same CRS, each side longitude and latitude to MapServer's tests as `from_geographic` and
    /// `to_geographic` say. Only a geotransform moves a point on it
    /// ([`Self::onto_rotated_map`]).
    pub fn identity(from_geographic: bool, to_geographic: bool) -> Self {
        Self {
            from: Projection::stand_in(),
            to: Projection::stand_in(),
            from_geographic,
            to_geographic,
            formulas: false,
            from_gt: None,
            to_gt: None,
        }
    }

    /// The same way into a rotated map, the map's `geotransform` on the side reprojected into, as
    /// `msMapSetFakedExtent` puts it on the map's projection: a point lands in the frame of the
    /// image, y up. Its [`Self::reversed`] holds it on the side reprojected from.
    pub fn onto_rotated_map(self, geotransform: Geotransform) -> Self {
        Self {
            to_gt: Some(geotransform),
            ..self
        }
    }

    /// The way from `from` into a map's `AUTO`, which has no PROJ object: into `from`'s base
    /// geographic CRS, as `msProjectCreateReprojector` reprojects it, the map's side never
    /// longitude and latitude to MapServer's tests.
    pub fn into_auto(from: Projection) -> Self {
        let to = from.base_geographic();
        Self {
            from_geographic: from.is_geographic(),
            to_geographic: false,
            from,
            to,
            formulas: true,
            from_gt: None,
            to_gt: None,
        }
    }

    /// The way back, from `to` into `from`.
    pub fn reversed(&self) -> Self {
        Self {
            from: self.to.clone(),
            to: self.from.clone(),
            from_geographic: self.to_geographic,
            to_geographic: self.from_geographic,
            formulas: self.formulas,
            from_gt: self.to_gt,
            to_gt: self.from_gt,
        }
    }

    /// Whether either side holds a geotransform, as MapServer's tests read
    /// `gt.need_geotransform`.
    pub(crate) fn rotated(&self) -> bool {
        self.from_gt.is_some() || self.to_gt.is_some()
    }

    /// `point`, in the projection reprojected from, in the projection reprojected into, its
    /// longitude and latitude passing through WGS84, as `msProjectPointEx` moves it: by the
    /// geotransform of the side reprojected from, the formulas, and the inverse geotransform of
    /// the side reprojected into.
    pub fn point(&self, point: Coord) -> Result<Coord, NotTransformed> {
        let point = match &self.from_gt {
            Some(gt) => gt.forward(point),
            None => point,
        };
        // PROJ refuses a coordinate of HUGE_VAL; here, any that is no finite number.
        if !(point.x.is_finite() && point.y.is_finite()) {
            return Err(NotTransformed);
        }
        let point = if self.formulas {
            let (lam, phi) = self.from.to_wgs84(point).ok_or(NotTransformed)?;
            self.to.at_wgs84(lam, phi).ok_or(NotTransformed)?
        } else {
            point
        };
        Ok(match &self.to_gt {
            Some(gt) => gt.inverse(point),
            None => point,
        })
    }
}

#[cfg(test)]
mod tests;
