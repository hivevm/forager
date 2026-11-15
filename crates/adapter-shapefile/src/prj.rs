//! The reader of a shapefile's `.prj` (ADR-draft-declared-projection): the first 2047 bytes, as
//! MapServer's `msSHPLayerOpen` reads them, parsed as the ESRI WKT a `.prj` holds and written as
//! the proj4 string GDAL's `OSRExportToProj4` writes for it, for what the core crate `proj`
//! implements: longitude and latitude, the transverse Mercator with UTM, the Lambert conformal
//! conic, the Mercator, and the orthographic projection, on WGS84 or NAD83, in metres and degrees.
//! Anything else does not read, and says why. An `AUTHORITY` is not followed: MapServer would load
//! `init=epsg:<code>`, which needs the projection resolver the render does not have.

use forager_ports::DeclaredProjection;

/// The bytes of a `.prj` MapServer reads: `char szPRJ[2048]`, its last one the terminator.
pub(crate) const MAX_PRJ_BYTES: usize = 2047;

/// The deepest nesting of brackets the reader follows; a `.prj` holds four.
const MAX_DEPTH: usize = 16;

/// A node of WKT: its keyword and its values.
#[derive(Debug, PartialEq)]
struct Node {
    keyword: String,
    values: Vec<Value>,
}

#[derive(Debug, PartialEq)]
enum Value {
    Text(String),
    Number(f64),
    Node(Node),
}

impl Node {
    /// The first child node of `keyword`, in any case.
    fn child(&self, keyword: &str) -> Option<&Node> {
        self.values.iter().find_map(|v| match v {
            Value::Node(n) if n.keyword.eq_ignore_ascii_case(keyword) => Some(n),
            _ => None,
        })
    }

    /// The text value at `index`.
    fn text(&self, index: usize) -> Option<&str> {
        match self.values.get(index) {
            Some(Value::Text(t)) => Some(t),
            _ => None,
        }
    }

    /// The number value at `index`.
    fn number(&self, index: usize) -> Option<f64> {
        match self.values.get(index) {
            Some(Value::Number(n)) => Some(*n),
            _ => None,
        }
    }
}

/// The projection the `.prj` named `file` declares, from its `bytes`, of which only the first
/// [`MAX_PRJ_BYTES`] are read.
pub(crate) fn declared(file: &str, bytes: &[u8]) -> DeclaredProjection {
    let bytes = &bytes[..bytes.len().min(MAX_PRJ_BYTES)];
    // The C string ends at its first NUL.
    let bytes = bytes.split(|&b| b == 0).next().unwrap_or_default();
    let unreadable = |why: String| DeclaredProjection::Unreadable {
        file: file.to_owned(),
        why,
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return unreadable("it is not UTF-8 text".to_owned());
    };
    let node = match parse(text) {
        Ok(node) => node,
        Err(why) => return unreadable(why),
    };
    match proj4(&node) {
        Ok(Some(proj4)) => DeclaredProjection::Proj4(proj4),
        Ok(None) => DeclaredProjection::NoProjection,
        Err(why) => unreadable(why),
    }
}

/// Parses `text` as one WKT node.
fn parse(text: &str) -> Result<Node, String> {
    let mut parser = Parser {
        text: text.as_bytes(),
        at: 0,
    };
    let node = parser.node(0)?;
    parser.space();
    if parser.at < parser.text.len() {
        return Err(format!("text follows the WKT at byte {}", parser.at));
    }
    Ok(node)
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.space();
        self.text.get(self.at).copied()
    }

    fn node(&mut self, depth: usize) -> Result<Node, String> {
        if depth > MAX_DEPTH {
            return Err(format!("the WKT nests deeper than {MAX_DEPTH} levels"));
        }
        self.space();
        let start = self.at;
        while self
            .text
            .get(self.at)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.at += 1;
        }
        if self.at == start {
            return Err(format!("a keyword is expected at byte {start}"));
        }
        let keyword = String::from_utf8_lossy(&self.text[start..self.at]).into_owned();
        let close = match self.peek() {
            Some(b'[') => b']',
            Some(b'(') => b')',
            _ => return Err(format!("{keyword} opens no bracket")),
        };
        self.at += 1;
        let mut values = Vec::new();
        loop {
            match self.peek() {
                Some(b) if b == close && values.is_empty() => {
                    self.at += 1;
                    break;
                }
                Some(b'"') => values.push(Value::Text(self.quoted()?)),
                Some(b) if b == b'-' || b == b'+' || b == b'.' || b.is_ascii_digit() => {
                    values.push(Value::Number(self.number()?));
                }
                Some(b) if b.is_ascii_alphabetic() => {
                    values.push(Value::Node(self.node(depth + 1)?));
                }
                _ => return Err(format!("{keyword} holds no value at byte {}", self.at)),
            }
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b) if b == close => {
                    self.at += 1;
                    break;
                }
                _ => return Err(format!("{keyword} is not closed at byte {}", self.at)),
            }
        }
        Ok(Node { keyword, values })
    }

    fn quoted(&mut self) -> Result<String, String> {
        self.at += 1;
        let start = self.at;
        while let Some(&b) = self.text.get(self.at) {
            if b == b'"' {
                let text = String::from_utf8_lossy(&self.text[start..self.at]).into_owned();
                self.at += 1;
                return Ok(text);
            }
            self.at += 1;
        }
        Err(format!(
            "a quote opened at byte {} is not closed",
            start - 1
        ))
    }

    fn number(&mut self) -> Result<f64, String> {
        let start = self.at;
        while self
            .text
            .get(self.at)
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.text[start..self.at]).unwrap_or_default();
        text.parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("{text:?} at byte {start} is no number"))
    }
}

/// The proj4 string of a `GEOGCS` or `PROJCS`, `None` for a `LOCAL_CS`, which declares no
/// projection (`OSRIsLocal`).
fn proj4(node: &Node) -> Result<Option<String>, String> {
    match node.keyword.to_ascii_uppercase().as_str() {
        "LOCAL_CS" => Ok(None),
        "GEOGCS" => Ok(Some(format!("+proj=longlat {} +no_defs", datum(node)?))),
        "PROJCS" => projected(node).map(Some),
        other => Err(format!("{other} is no coordinate system a .prj holds")),
    }
}

/// The datum of a `GEOGCS`, as `+datum=`, after checking its prime meridian and angular unit.
fn datum(geogcs: &Node) -> Result<&'static str, String> {
    if let Some(primem) = geogcs.child("PRIMEM")
        && primem.number(1).is_some_and(|p| p != 0.0)
    {
        return Err("a prime meridian other than Greenwich is not supported".to_owned());
    }
    if let Some(unit) = geogcs.child("UNIT")
        && unit
            .number(1)
            .is_some_and(|f| (f - 0.017_453_292_519_943_3).abs() > 1e-15)
    {
        return Err("an angular unit other than the degree is not supported".to_owned());
    }
    let name = geogcs
        .child("DATUM")
        .and_then(|d| d.text(0))
        .ok_or("the GEOGCS names no DATUM")?;
    // OSRMorphFromESRI's names and the WKT ones, as the cases a .prj in scope holds.
    let normal: String = name
        .trim_start_matches("D_")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase();
    match normal.as_str() {
        "WGS1984" | "WGS84" => Ok("+datum=WGS84"),
        "NORTHAMERICAN1983" | "NORTHAMERICANDATUM1983" | "NAD83" => Ok("+datum=NAD83"),
        _ => Err(format!("the datum {name} is not supported")),
    }
}

/// The proj4 string of a `PROJCS`.
fn projected(projcs: &Node) -> Result<String, String> {
    let geogcs = projcs.child("GEOGCS").ok_or("the PROJCS holds no GEOGCS")?;
    let datum = datum(geogcs)?;
    if let Some(unit) = projcs.child("UNIT")
        && unit.number(1).is_some_and(|f| f != 1.0)
    {
        return Err("a linear unit other than the metre is not supported".to_owned());
    }
    let method = projcs
        .child("PROJECTION")
        .and_then(|p| p.text(0))
        .ok_or("the PROJCS names no PROJECTION")?;
    let parameter = |names: &[&str], default: f64| -> f64 {
        projcs
            .values
            .iter()
            .find_map(|v| match v {
                Value::Node(n)
                    if n.keyword.eq_ignore_ascii_case("PARAMETER")
                        && n.text(0).is_some_and(|t| {
                            names.iter().any(|name| t.eq_ignore_ascii_case(name))
                        }) =>
                {
                    n.number(1)
                }
                _ => None,
            })
            .unwrap_or(default)
    };
    let x0 = parameter(&["False_Easting"], 0.0);
    let y0 = parameter(&["False_Northing"], 0.0);
    let lon0 = parameter(
        &[
            "Central_Meridian",
            "Longitude_Of_Origin",
            "Longitude_Of_Center",
        ],
        0.0,
    );
    let lat0 = parameter(&["Latitude_Of_Origin", "Latitude_Of_Center"], 0.0);
    let k = parameter(&["Scale_Factor"], 1.0);
    let p = Parameters {
        x0,
        y0,
        lon0,
        lat0,
        k,
        lat1: parameter(&["Standard_Parallel_1"], lat0),
        lat2: parameter(
            &["Standard_Parallel_2"],
            parameter(&["Standard_Parallel_1"], lat0),
        ),
        lat_ts: parameter(&["Standard_Parallel_1"], 0.0),
    };
    method_proj4(method, &p, datum)
}

/// The parameters of a `PROJCS`, in degrees and metres.
#[derive(Clone, Copy)]
struct Parameters {
    x0: f64,
    y0: f64,
    lon0: f64,
    lat0: f64,
    k: f64,
    lat1: f64,
    lat2: f64,
    lat_ts: f64,
}

/// The proj4 string `OSRExportToProj4` writes for the projection `method` with `p` and `datum`.
fn method_proj4(method: &str, p: &Parameters, datum: &str) -> Result<String, String> {
    let Parameters {
        x0,
        y0,
        lon0,
        lat0,
        k,
        lat1,
        lat2,
        lat_ts,
    } = *p;
    let tail = format!("{datum} +units=m +no_defs");
    let n = |v: f64| format!("{v}");
    let proj4 = match method.to_ascii_lowercase().as_str() {
        "transverse_mercator" => {
            if let Some(zone) = utm_zone(lat0, lon0, k, x0, y0) {
                let south = if y0 == 10_000_000.0 { " +south" } else { "" };
                format!("+proj=utm +zone={zone}{south} {tail}")
            } else {
                format!(
                    "+proj=tmerc +lat_0={} +lon_0={} +k={} +x_0={} +y_0={} {tail}",
                    n(lat0),
                    n(lon0),
                    n(k),
                    n(x0),
                    n(y0)
                )
            }
        }
        "lambert_conformal_conic" | "lambert_conformal_conic_2sp" => {
            if lat1 == lat2 && k != 1.0 {
                format!(
                    "+proj=lcc +lat_1={} +lat_0={} +lon_0={} +k_0={} +x_0={} +y_0={} {tail}",
                    n(lat1),
                    n(lat0),
                    n(lon0),
                    n(k),
                    n(x0),
                    n(y0)
                )
            } else {
                format!(
                    "+proj=lcc +lat_0={} +lon_0={} +lat_1={} +lat_2={} +x_0={} +y_0={} {tail}",
                    n(lat0),
                    n(lon0),
                    n(lat1),
                    n(lat2),
                    n(x0),
                    n(y0)
                )
            }
        }
        "lambert_conformal_conic_1sp" => format!(
            "+proj=lcc +lat_1={} +lat_0={} +lon_0={} +k_0={} +x_0={} +y_0={} {tail}",
            n(lat0),
            n(lat0),
            n(lon0),
            n(k),
            n(x0),
            n(y0)
        ),
        "mercator" | "mercator_2sp" => format!(
            "+proj=merc +lon_0={} +lat_ts={} +x_0={} +y_0={} {tail}",
            n(lon0),
            n(lat_ts),
            n(x0),
            n(y0)
        ),
        "mercator_1sp" => format!(
            "+proj=merc +lon_0={} +k={} +x_0={} +y_0={} {tail}",
            n(lon0),
            n(k),
            n(x0),
            n(y0)
        ),
        "orthographic" => format!(
            "+proj=ortho +lat_0={} +lon_0={} +x_0={} +y_0={} {tail}",
            n(lat0),
            n(lon0),
            n(x0),
            n(y0)
        ),
        _ => return Err(format!("the projection {method} is not supported")),
    };
    Ok(proj4)
}

/// The UTM zone a transverse Mercator's parameters are, as PROJ's export recognizes one.
fn utm_zone(lat0: f64, lon0: f64, k: f64, x0: f64, y0: f64) -> Option<i32> {
    if lat0 != 0.0 || k != 0.9996 || x0 != 500_000.0 || !(y0 == 0.0 || y0 == 10_000_000.0) {
        return None;
    }
    let zone = (lon0 + 183.0) / 6.0;
    // A zone's central meridian, exactly.
    #[expect(clippy::cast_possible_truncation, reason = "a whole zone of 1 to 60")]
    (zone.fract() == 0.0 && (1.0..=60.0).contains(&zone)).then_some(zone as i32)
}

#[cfg(test)]
mod tests;
