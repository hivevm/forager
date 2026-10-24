//! Resolves EPSG codes to proj4 strings (ADR-0018). Built in are only EPSG:4326, EPSG:3857, and
//! the WGS84 UTM zones, derived from the zone number; every other code comes from a table the host
//! supplies in the format of PROJ's former `epsg` init file. The repository holds no EPSG data.

use std::collections::BTreeMap;
use std::fmt;

use forager_ports::ProjectionResolver;

/// The definitions of EPSG codes: the built-in ones, and those of a host's table.
#[derive(Clone, Debug, Default)]
pub struct EpsgTable {
    entries: BTreeMap<u32, String>,
}

/// A line of a table file that cannot be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl EpsgTable {
    /// Only the built-in definitions.
    pub fn new() -> Self {
        Self::default()
    }

    /// The built-in definitions and those of `text`, a table of lines `<code> +proj=… <>`.
    /// Comments start with `#`; a definition that names a grid file is refused, since the core
    /// reads no files.
    pub fn with_table(text: &str) -> Result<Self, Vec<TableError>> {
        let mut entries = BTreeMap::new();
        let mut errors = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            match parse_line(content) {
                Ok((code, definition)) => {
                    entries.insert(code, definition);
                }
                Err(message) => errors.push(TableError { line, message }),
            }
        }
        if errors.is_empty() {
            Ok(Self { entries })
        } else {
            Err(errors)
        }
    }

    fn definition(&self, code: u32) -> Option<String> {
        self.entries.get(&code).cloned().or_else(|| built_in(code))
    }
}

impl ProjectionResolver for EpsgTable {
    fn resolve(&self, authority: &str, code: &str) -> Option<String> {
        if !authority.eq_ignore_ascii_case("epsg") {
            return None;
        }
        self.definition(code.trim().parse().ok()?)
    }
}

fn parse_line(content: &str) -> Result<(u32, String), String> {
    let rest = content
        .strip_prefix('<')
        .ok_or("a definition starts with <code>")?;
    let (code, rest) = rest.split_once('>').ok_or("the code is not closed by >")?;
    let code: u32 = code
        .trim()
        .parse()
        .map_err(|_| format!("`{}` is not an EPSG code", code.trim()))?;
    let definition = rest.trim().strip_suffix("<>").unwrap_or(rest).trim();
    if definition.is_empty() {
        return Err(format!("EPSG:{code} has no definition"));
    }
    let names_a_grid = definition.split_whitespace().any(|param| {
        let param = param.trim_start_matches('+');
        let grids = param
            .strip_prefix("nadgrids=")
            .or_else(|| param.strip_prefix("geoidgrids="));
        grids.is_some_and(|value| {
            value
                .split(',')
                .any(|g| g.trim_start_matches('@') != "null")
        })
    });
    if names_a_grid {
        return Err(format!(
            "EPSG:{code} names a grid file, which the core cannot read"
        ));
    }
    Ok((code, definition.to_owned()))
}

/// EPSG:4326, EPSG:3857, and the WGS84 UTM zones 1–60, north (326zz) and south (327zz).
fn built_in(code: u32) -> Option<String> {
    match code {
        4326 => Some("+proj=longlat +datum=WGS84 +no_defs".to_owned()),
        3857 => Some(
            "+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 +units=m \
             +nadgrids=@null +wktext +no_defs"
                .to_owned(),
        ),
        32601..=32660 => Some(format!(
            "+proj=utm +zone={} +datum=WGS84 +units=m +no_defs",
            code - 32600
        )),
        32701..=32760 => Some(format!(
            "+proj=utm +zone={} +south +datum=WGS84 +units=m +no_defs",
            code - 32700
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_codes_resolve() {
        // Verifies: ADR-0018
        let table = EpsgTable::new();
        assert_eq!(
            table.resolve("EPSG", "4326").as_deref(),
            Some("+proj=longlat +datum=WGS84 +no_defs")
        );
        assert!(
            table
                .resolve("epsg", "3857")
                .unwrap()
                .starts_with("+proj=merc")
        );
        assert_eq!(
            table.resolve("epsg", "32611").as_deref(),
            Some("+proj=utm +zone=11 +datum=WGS84 +units=m +no_defs")
        );
        assert_eq!(
            table.resolve("epsg", "32760").as_deref(),
            Some("+proj=utm +zone=60 +south +datum=WGS84 +units=m +no_defs")
        );
        for unknown in ["27700", "32600", "32661", "x", ""] {
            assert_eq!(table.resolve("epsg", unknown), None, "{unknown}");
        }
        assert_eq!(table.resolve("esri", "4326"), None);
    }

    #[test]
    fn a_host_table_adds_and_overrides_codes() {
        // Verifies: ADR-0018
        let table = EpsgTable::with_table(
            "# British National Grid\n\
             <27700> +proj=tmerc +lat_0=49 +lon_0=-2 +k=0.9996012717 +x_0=400000 +y_0=-100000 +ellps=airy +units=m +no_defs <>\n\
             \n\
             <4326> +proj=longlat +ellps=WGS84 +no_defs <> # overridden\n",
        )
        .unwrap();
        assert!(
            table
                .resolve("epsg", "27700")
                .unwrap()
                .starts_with("+proj=tmerc")
        );
        assert_eq!(
            table.resolve("epsg", "4326").as_deref(),
            Some("+proj=longlat +ellps=WGS84 +no_defs")
        );
        assert!(table.resolve("epsg", "32632").is_some(), "built-ins remain");
    }

    #[test]
    fn unusable_lines_are_reported_with_their_number() {
        let errors = EpsgTable::with_table(
            "<1> +proj=longlat <>\n\
             27700 +proj=tmerc\n\
             <x> +proj=longlat <>\n\
             <2> <>\n\
             <3> +proj=tmerc +nadgrids=uk_os_OSTN15_NTv2_OSGBtoETRS.tif <>\n\
             <4> +proj=merc +nadgrids=@null <>\n",
        )
        .unwrap_err();
        assert_eq!(
            errors.iter().map(|e| e.line).collect::<Vec<_>>(),
            [2, 3, 4, 5]
        );
        assert!(errors[3].message.contains("grid file"), "{}", errors[3]);
    }
}
