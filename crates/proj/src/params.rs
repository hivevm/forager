// The tables of ellipsoids, datums, and units, and the set-up of a projection from its parameters,
// follow src/ellps.cpp, src/datums.cpp, src/units.cpp, src/ell_set.cpp, src/datum_set.cpp, and
// src/init.cpp of PROJ 9.4.0, <https://proj.org>.
// Copyright (c) 2000, Frank Warmerdam (src/datums.cpp, src/datum_set.cpp)
// Copyright (c) 1995, Gerald Evenden; Copyright (c) 2002, Frank Warmerdam (src/init.cpp)
// src/ellps.cpp, src/units.cpp, and src/ell_set.cpp hold no copyright line of their own. Each file
// grants the MIT licence below; PROJ's notice, from its file COPYING:
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

//! A proj4 string read into the parameters of a projection, as PROJ's `pj_init` reads it.

use crate::Error;

/// The parameters of a proj4 string, each `+key=value` or `+key`, in their order; the first of a
/// key holds, as `pj_param` finds it.
pub(crate) struct Params<'a> {
    list: Vec<(&'a str, Option<&'a str>)>,
}

impl<'a> Params<'a> {
    /// Reads `text`, its parameters separated by white space, each with or without its leading `+`.
    pub(crate) fn read(text: &'a str) -> Self {
        let list = text
            .split_whitespace()
            .map(|token| token.strip_prefix('+').unwrap_or(token))
            .filter(|token| !token.is_empty())
            .map(|token| match token.split_once('=') {
                Some((key, value)) => (key, Some(value)),
                None => (token, None),
            })
            .collect();
        Self { list }
    }

    /// Each key, once, in the order of its first occurrence.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.list
            .iter()
            .enumerate()
            .filter(|(at, (key, _))| self.list[..*at].iter().all(|(k, _)| k != key))
            .map(|(_, (key, _))| *key)
    }

    /// Whether `key` is given.
    pub(crate) fn has(&self, key: &str) -> bool {
        self.list.iter().any(|(k, _)| *k == key)
    }

    /// The text of `key`'s first value; an error where it is given without one.
    pub(crate) fn text(&self, key: &str) -> Result<Option<&'a str>, Error> {
        match self.list.iter().find(|(k, _)| *k == key) {
            None => Ok(None),
            Some((_, Some(value))) => Ok(Some(value)),
            Some((_, None)) => Err(Error::Invalid(format!("+{key} has no value"))),
        }
    }

    /// The number of `key`, as `pj_atof` reads a plain decimal; an error where it is no finite
    /// number.
    pub(crate) fn number(&self, key: &str) -> Result<Option<f64>, Error> {
        self.text(key)?.map(|text| number(key, text)).transpose()
    }

    /// The angle of `key` in degrees, in radians, as `pj_param`'s `r` reads it; only a plain
    /// decimal of degrees is read.
    pub(crate) fn radians(&self, key: &str) -> Result<Option<f64>, Error> {
        Ok(self.number(key)?.map(|degrees| degrees * DEG_TO_RAD))
    }
}

/// `text`, the value of `key`, as a finite number.
fn number(key: &str, text: &str) -> Result<f64, Error> {
    text.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| Error::Invalid(format!("+{key}={text} is no number")))
}

/// PROJ's `DEG_TO_RAD`, `.017453292519943296`, the same double as MapServer's `MS_DEG_TO_RAD`.
pub(crate) use forager_style_ir::c::DEG_TO_RAD;

/// PROJ's `RAD_TO_DEG`.
pub(crate) const RAD_TO_DEG: f64 = 57.295_779_513_082_32;

/// The size and shape of an ellipsoid, as `pj_ellipsoid` sets them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ellipsoid {
    /// The semimajor axis, in metres.
    pub(crate) a: f64,
    /// The squared eccentricity.
    pub(crate) es: f64,
    /// The eccentricity.
    pub(crate) e: f64,
}

impl Ellipsoid {
    fn new(a: f64, es: f64) -> Self {
        Self {
            a,
            es,
            e: es.sqrt(),
        }
    }

    /// The semiminor axis.
    pub(crate) fn b(&self) -> f64 {
        self.a * (1.0 - self.es).sqrt()
    }

    /// The second eccentricity squared, e′².
    pub(crate) fn e2s(&self) -> f64 {
        self.es / (1.0 - self.es)
    }

    /// WGS84, the frame every datum shift passes through.
    pub(crate) fn wgs84() -> Self {
        let f = 1.0 / 298.257_223_563;
        Self::new(6_378_137.0, 2.0 * f - f * f)
    }
}

/// The ellipsoids of PROJ's table, by their semimajor axis and reverse flattening, that the
/// datums below and the cases in scope name; another `+ellps` is unsupported until a case or a
/// ticket needs it (ADR-draft-reprojection-ported-in-proj).
const ELLIPSOIDS: &[(&str, f64, f64)] = &[
    ("WGS84", 6_378_137.0, 298.257_223_563),
    ("GRS80", 6_378_137.0, 298.257_222_101),
];

/// The datums of PROJ's table that the cases in scope name, by their `towgs84` and ellipsoid.
const DATUMS: &[(&str, &str, &str)] = &[("WGS84", "0,0,0", "WGS84"), ("NAD83", "0,0,0", "GRS80")];

/// The linear units of PROJ's table that the cases in scope name, in metres; `+to_meter` gives
/// any other.
const UNITS: &[(&str, f64)] = &[("m", 1.0)];

/// The datum a projection's coordinates are in, by how PROJ's `pj_init` shifts it to WGS84
/// (`src/4D_api.cpp`, its cs2cs emulation): by `+nadgrids=@null`, not at all; by a Helmert
/// transform of `+towgs84`, through geocentric coordinates; by a change of ellipsoid alone, where
/// `+towgs84` is all zeros and the ellipsoid is not WGS84's; or not at all, where none is given.
/// MapServer at the pin creates its transformation by `proj_create_crs_to_crs_from_pj`, in its
/// `createNormalizedPJ`, whose operation for these definitions is not verified to be this chain;
/// G-11's measurement against PROJ is the guard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Datum {
    /// Its longitude and latitude are WGS84's.
    Wgs84,
    /// Through geocentric coordinates on its ellipsoid, shifted by the Helmert transform of its
    /// `towgs84` where that is not all zeros.
    Geocentric(Option<crate::datum::Helmert>),
}

/// The ellipsoid and the datum of `params`, as `pj_datum_set` and `pj_ellipsoid` read them, the
/// ellipsoid GRS80 where none is given, as `pj_init` defaults it.
pub(crate) fn ellipsoid_and_datum(params: &Params) -> Result<(Ellipsoid, Datum), Error> {
    let datum = params.text("datum")?;
    let datum_row = datum
        .map(|name| {
            DATUMS
                .iter()
                .find(|(id, _, _)| *id == name)
                .ok_or_else(|| Error::Unsupported(format!("+datum={name}")))
        })
        .transpose()?;
    let ellipsoid = ellipsoid(params, datum_row.map(|(_, _, ellps)| *ellps))?;
    let grids = params.text("nadgrids")?;
    let towgs84 = params.text("towgs84")?.or(datum_row.map(|(_, t, _)| *t));
    let datum = match (grids, towgs84) {
        (Some("@null"), _) => Datum::Wgs84,
        (Some(other), _) => return Err(Error::Unsupported(format!("+nadgrids={other}"))),
        (None, None) => Datum::Wgs84,
        (None, Some(text)) => {
            let shift = towgs84_params(text)?;
            if shift.iter().all(|&p| p == 0.0) {
                let wgs84 = (ellipsoid.a - 6_378_137.0).abs() < 1e-8
                    && (ellipsoid.es - 0.006_694_379_990_141_3).abs() < 1e-15;
                if wgs84 {
                    Datum::Wgs84
                } else {
                    Datum::Geocentric(None)
                }
            } else {
                Datum::Geocentric(Some(crate::datum::Helmert::new(&shift)))
            }
        }
    };
    Ok((ellipsoid, datum))
}

/// The up to seven numbers of a `towgs84`, the missing ones zero.
fn towgs84_params(text: &str) -> Result<[f64; 7], Error> {
    let mut shift = [0.0; 7];
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() > 7 {
        return Err(Error::Invalid(format!(
            "+towgs84={text} has more than 7 numbers"
        )));
    }
    for (slot, part) in shift.iter_mut().zip(parts) {
        *slot = number("towgs84", part)?;
    }
    Ok(shift)
}

/// The ellipsoid of `params`: a sphere of `+R`; else the `+ellps` named, or `datum_ellps` of the
/// datum, or GRS80, its size replaced by `+a` and its shape by the first of `+rf`, `+f`, `+es`,
/// `+e`, and `+b`.
fn ellipsoid(params: &Params, datum_ellps: Option<&str>) -> Result<Ellipsoid, Error> {
    if let Some(r) = params.number("R")? {
        if r <= 0.0 {
            return Err(Error::Invalid(format!("+R={r} is not positive")));
        }
        return Ok(Ellipsoid::new(r, 0.0));
    }
    let given = params.text("ellps")?;
    let name = given.or(datum_ellps).unwrap_or("GRS80");
    let (_, table_a, table_rf) = ELLIPSOIDS
        .iter()
        .find(|(id, _, _)| *id == name)
        .ok_or_else(|| Error::Unsupported(format!("+ellps={name}")))?;
    let a = params.number("a")?.unwrap_or(*table_a);
    if a <= 0.0 {
        return Err(Error::Invalid(format!("+a={a} is not positive")));
    }
    let from_f = |f: f64| 2.0 * f - f * f;
    let es = if let Some(rf) = params.number("rf")? {
        if rf <= 0.0 {
            return Err(Error::Invalid(format!("+rf={rf} is not positive")));
        }
        from_f(1.0 / rf)
    } else if let Some(f) = params.number("f")? {
        from_f(f)
    } else if let Some(es) = params.number("es")? {
        es
    } else if let Some(e) = params.number("e")? {
        e * e
    } else if let Some(b) = params.number("b")? {
        if b <= 0.0 {
            return Err(Error::Invalid(format!("+b={b} is not positive")));
        }
        from_f((a - b) / a)
    } else {
        from_f(1.0 / table_rf)
    };
    if !(0.0..1.0).contains(&es) {
        return Err(Error::Invalid(format!(
            "the ellipsoid's squared eccentricity {es} lies outside 0 to 1"
        )));
    }
    Ok(Ellipsoid::new(a, es))
}

/// The metres of one unit of the projection's coordinates: `+to_meter`, else `+units`, else 1.
pub(crate) fn to_meter(params: &Params) -> Result<f64, Error> {
    let factor = if let Some(text) = params.text("to_meter")? {
        match text.split_once('/') {
            Some((numerator, denominator)) => {
                number("to_meter", numerator)? / number("to_meter", denominator)?
            }
            None => number("to_meter", text)?,
        }
    } else if let Some(name) = params.text("units")? {
        UNITS
            .iter()
            .find(|(id, _)| *id == name)
            .map(|(_, metres)| *metres)
            .ok_or_else(|| Error::Unsupported(format!("+units={name}")))?
    } else {
        1.0
    };
    if factor > 0.0 && factor.is_finite() {
        Ok(factor)
    } else {
        Err(Error::Invalid(format!("a unit of {factor} metres")))
    }
}
