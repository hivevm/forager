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
//! (ADR-0037, Q-2). It reads a projection as the proj4 string of the
//! style IR and implements only what a case in scope or a ticket needs: longitude and latitude,
//! `lcc`, and `merc`, and a datum's shift by `+towgs84`. Anything else is an [`Error`] naming it,
//! and a point that does not transform is [`NotTransformed`], never a panic (Q-3).

#![warn(clippy::disallowed_types, clippy::disallowed_methods)]

mod common;
mod datum;
mod lcc;
mod merc;
mod params;

use std::fmt;

use forager_style_ir::{Coord, Extent};

use crate::common::{HALF_PI, adjlon};
use crate::params::{DEG_TO_RAD, Datum, Ellipsoid, Params, RAD_TO_DEG};

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
}

/// The projections this crate implements, by the names PROJ gives them.
const PROJECTIONS: &[&str] = &["longlat", "latlong", "lonlat", "latlon", "lcc", "merc"];

/// The parameters this crate reads; any other is unsupported. `no_defs`, `wktext`, and
/// `type=crs` change nothing PROJ computes here.
const KNOWN: &[&str] = &[
    "proj", "datum", "ellps", "R", "a", "b", "rf", "f", "es", "e", "towgs84", "nadgrids", "lat_0",
    "lon_0", "lat_1", "lat_2", "lat_ts", "k", "k_0", "x_0", "y_0", "units", "to_meter", "no_defs",
    "wktext", "type",
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
        let k0 = match params.number("k_0")? {
            Some(k0) => k0,
            None => params.number("k")?.unwrap_or(1.0),
        };
        if k0 <= 0.0 {
            return Err(Error::Invalid(format!(
                "the scale factor {k0} is not positive"
            )));
        }
        let mut phi0 = params.radians("lat_0")?.unwrap_or(0.0);
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
            other => return Err(Error::Unsupported(format!("+proj={other}"))),
        };
        if phi0.abs() > HALF_PI {
            return Err(Error::Invalid("|lat_0| should be <= 90°".to_owned()));
        }
        Ok(Self {
            kind,
            ellipsoid,
            datum,
            lam0: params.radians("lon_0")?.unwrap_or(0.0),
            x0: params.number("x_0")?.unwrap_or(0.0),
            y0: params.number("y_0")?.unwrap_or(0.0),
            to_meter: params::to_meter(&params)?,
        })
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
        }
    }

    /// The WGS84 longitude and latitude, in radians, of `point` in this projection, as `pj_inv`
    /// and `inv_finalize` compute them.
    fn to_wgs84(&self, point: Coord) -> Option<(f64, f64)> {
        let (lam, phi) = match self.plane() {
            None => (adjlon(point.x * DEG_TO_RAD), point.y * DEG_TO_RAD),
            Some(plane) => {
                let ra = 1.0 / self.ellipsoid.a;
                let x = (point.x * self.to_meter - self.x0) * ra;
                let y = (point.y * self.to_meter - self.y0) * ra;
                let (lam, phi) = plane.inverse(x, y)?;
                (adjlon(lam + self.lam0), phi)
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
        let (lam, phi) = datum::from_wgs84(adjlon(lam), phi, &self.datum, &self.ellipsoid);
        let lam = adjlon(lam - self.lam0);
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
}

/// The points along each side of a rectangle that [`Reprojection::extent`] reprojects, as
/// `msProjectRect` samples one: `NUMBER_OF_SAMPLE_POINTS`.
const SAMPLES_PER_SIDE: u32 = 100;

impl Reprojection {
    /// The way from `from` into `to`.
    pub fn new(from: Projection, to: Projection) -> Self {
        Self { from, to }
    }

    /// The way back, from `to` into `from`.
    pub fn reversed(&self) -> Self {
        Self {
            from: self.to.clone(),
            to: self.from.clone(),
        }
    }

    /// `point`, in the projection reprojected from, in the projection reprojected into, its
    /// longitude and latitude passing through WGS84.
    pub fn point(&self, point: Coord) -> Result<Coord, NotTransformed> {
        // PROJ refuses a coordinate of HUGE_VAL; here, any that is no finite number.
        if !(point.x.is_finite() && point.y.is_finite()) {
            return Err(NotTransformed);
        }
        let (lam, phi) = self.from.to_wgs84(point).ok_or(NotTransformed)?;
        self.to.at_wgs84(lam, phi).ok_or(NotTransformed)
    }

    /// The rectangle that holds the reprojected points of `extent`'s sides, each side sampled at
    /// a hundred points, its corners included, as `msProjectRect` samples it; `None` where none
    /// reprojects.
    pub fn extent(&self, extent: &Extent) -> Option<Extent> {
        let mut bounds: Option<Extent> = None;
        let width = extent.max_x - extent.min_x;
        let height = extent.max_y - extent.min_y;
        for i in 0..=SAMPLES_PER_SIDE {
            let t = f64::from(i) / f64::from(SAMPLES_PER_SIDE);
            let along = [
                Coord {
                    x: extent.min_x + t * width,
                    y: extent.min_y,
                },
                Coord {
                    x: extent.min_x + t * width,
                    y: extent.max_y,
                },
                Coord {
                    x: extent.min_x,
                    y: extent.min_y + t * height,
                },
                Coord {
                    x: extent.max_x,
                    y: extent.min_y + t * height,
                },
            ];
            for point in along.into_iter().filter_map(|p| self.point(p).ok()) {
                bounds = Some(match bounds {
                    None => Extent {
                        min_x: point.x,
                        min_y: point.y,
                        max_x: point.x,
                        max_y: point.y,
                    },
                    Some(b) => Extent {
                        min_x: if point.x < b.min_x { point.x } else { b.min_x },
                        min_y: if point.y < b.min_y { point.y } else { b.min_y },
                        max_x: if point.x > b.max_x { point.x } else { b.max_x },
                        max_y: if point.y > b.max_y { point.y } else { b.max_y },
                    },
                });
            }
        }
        bounds
    }
}

#[cfg(test)]
mod tests;
