//! Resolves EPSG codes to proj4 strings (ADR-0023). Built in are only EPSG:4326, EPSG:3857, and
//! the WGS84 UTM zones, derived from the zone number; every other code comes from a table the host
//! supplies in the format of PROJ's former `epsg` init file. The repository holds no EPSG data.

use std::collections::BTreeMap;
use std::fmt;

use forager_ports::ProjectionResolver;

/// How many unusable lines of a table are reported (Q-9).
pub const MAX_TABLE_ERRORS: usize = 100;

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
    ///
    /// At most [`MAX_TABLE_ERRORS`] errors are reported; a last one counts the rest (Q-9).
    pub fn with_table(text: &str) -> Result<Self, Vec<TableError>> {
        let (entries, errors) = parse(text);
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

/// A table's definitions and its errors, read as the mapfile frontend reads an init file and as
/// PROJ 9.5.0 reads one (`get_init_string` in its `src/init.cpp`): `<code>` at the start of a
/// line, the code compared as written, and the parameters up to the next `<`, across lines, `#`
/// starting a comment to the end of its line; the first definition of a code holds. What follows
/// a `<>` on its line is ignored, as PROJ ignores it. Stricter than PROJ, which only skips them,
/// a `<` without its `>` on its line, a `<code>` inside a line, a code that is not an EPSG
/// number as written, and text on a later line outside a definition are errors. Each error names
/// the line where it starts; at most [`MAX_TABLE_ERRORS`] are listed, and a last one counts the
/// rest (Q-9).
fn parse(text: &str) -> (BTreeMap<u32, String>, Vec<TableError>) {
    let mut entries = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut errors = Vec::new();
    let mut dropped = 0usize;
    let mut report = |line: usize, message: String| {
        if errors.len() < MAX_TABLE_ERRORS {
            errors.push(TableError { line, message });
        } else {
            dropped += 1;
        }
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect();
    let joined = lines.join("\n");
    let starts: Vec<usize> = std::iter::once(0)
        .chain(joined.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_of = |at: usize| starts.partition_point(|&start| start <= at);
    // What the text up to the next `<` is: before the first code or after a `<>`, nothing; after
    // a code, its definition; after a tag that is none, skipped.
    enum Body {
        Outside { at_start: bool },
        Definition { code: u32, line: usize },
        Skipped,
    }
    let mut body = Body::Outside { at_start: true };
    let mut pos = 0;
    loop {
        let open = joined[pos..].find('<').map(|i| pos + i);
        let end = open.unwrap_or(joined.len());
        let text = &joined[pos..end];
        match body {
            Body::Outside { at_start } => {
                // Text on the line of a `<>` is ignored; on a later line it is no definition.
                let skip = if at_start {
                    0
                } else {
                    text.find('\n').map_or(text.len(), |i| i + 1)
                };
                let mut at = pos + skip;
                for stray in text[skip..].split('\n') {
                    if !stray.trim().is_empty() {
                        report(line_of(at), "a definition starts with <code>".to_owned());
                    }
                    at += stray.len() + 1;
                }
            }
            Body::Definition { code, line } => {
                let definition = text.split_whitespace().collect::<Vec<_>>().join(" ");
                // The first definition holds, as in PROJ, a refused one too.
                if seen.insert(code) {
                    match usable(code, &definition) {
                        Ok(definition) => {
                            entries.insert(code, definition);
                        }
                        Err(message) => report(line, message),
                    }
                }
            }
            Body::Skipped => {}
        }
        let Some(open) = open else { break };
        let line = line_of(open);
        let line_end = joined[open..].find('\n').map_or(joined.len(), |i| open + i);
        let Some(close) = joined[open..line_end].find('>').map(|i| open + i) else {
            report(line, "the code is not closed by > on its line".to_owned());
            body = Body::Skipped;
            pos = line_end;
            continue;
        };
        let code = &joined[open + 1..close];
        let line_start = starts[line - 1];
        body = if code.is_empty() {
            Body::Outside { at_start: false }
        } else if !joined[line_start..open].trim().is_empty() {
            report(line, format!("`<{code}>` does not start its line"));
            Body::Skipped
        } else if let Some(number) = Some(code)
            .filter(|c| {
                c.bytes().all(|b| b.is_ascii_digit()) && !(c.len() > 1 && c.starts_with('0'))
            })
            .and_then(|c| c.parse().ok())
        {
            Body::Definition { code: number, line }
        } else {
            report(line, format!("`{code}` is not an EPSG code"));
            Body::Skipped
        };
        pos = close + 1;
    }
    if dropped > 0 {
        errors.push(TableError {
            line: lines.len(),
            message: format!("{dropped} more unusable lines are not shown"),
        });
    }
    (entries, errors)
}

/// The definition of `code`, unless the core cannot use it: one that is empty, refers on with
/// `init=`, or names a grid file.
fn usable(code: u32, definition: &str) -> Result<String, String> {
    if definition.is_empty() {
        return Err(format!("EPSG:{code} has no definition"));
    }
    let params = || {
        definition
            .split_whitespace()
            .map(|p| p.trim_start_matches('+'))
    };
    if params().any(|p| p.starts_with("init=")) {
        return Err(format!(
            "EPSG:{code} refers to another definition with init=, which the core cannot read"
        ));
    }
    let names_a_grid = params().any(|param| {
        let grids = [
            "nadgrids=",
            "geoidgrids=",
            "grids=",
            "xy_grids=",
            "z_grids=",
        ]
        .iter()
        .find_map(|key| param.strip_prefix(key));
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
    Ok(definition.split_whitespace().collect::<Vec<_>>().join(" "))
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
        // Verifies: ADR-0023
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
        // Verifies: ADR-0023
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
    fn a_definition_ends_at_its_terminator_and_a_bom_is_ignored() {
        let table = EpsgTable::with_table("\u{feff}<1> +proj=longlat +no_defs <> +junk\n").unwrap();
        assert_eq!(
            table.resolve("epsg", "1").as_deref(),
            Some("+proj=longlat +no_defs")
        );
    }

    #[test]
    fn definitions_that_need_files_or_other_definitions_are_refused() {
        // Verifies: ADR-0023
        let errors = EpsgTable::with_table(
            "<1> +init=epsg:2 <>\n\
             <2> +proj=pipeline +step +proj=hgridshift +grids=foo.tif <>\n\
             <3> +proj=longlat +xy_grids=a.tif <>\n\
             <+4> +proj=longlat <>\n",
        )
        .unwrap_err();
        assert_eq!(
            errors.iter().map(|e| e.line).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert!(errors[0].message.contains("init="), "{}", errors[0]);
    }

    #[test]
    fn the_errors_of_a_table_are_bounded() {
        // Verifies: Q-9
        let errors = EpsgTable::with_table(&"x\n".repeat(100_000)).unwrap_err();
        assert_eq!(errors.len(), MAX_TABLE_ERRORS + 1);
        assert!(
            errors.last().unwrap().message.starts_with("99900 more"),
            "{}",
            errors.last().unwrap()
        );
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

    /// The cases of the init file format both readers of it run (revision ticket 19).
    const FORMAT: &str = include_str!("../tests/fixtures/init-file-format.txt");

    #[test]
    fn the_table_is_read_as_proj_reads_an_init_file() {
        // Verifies: ADR-0023
        let (entries, errors) = parse(FORMAT);
        for line in FORMAT.lines() {
            let Some(expect) = line.strip_prefix("# expect ") else {
                continue;
            };
            let (code, outcome) = expect.split_once(' ').unwrap();
            let code: u32 = code.parse().unwrap();
            if let Some(definition) = outcome.strip_prefix("= ") {
                assert_eq!(
                    entries.get(&code).map(String::as_str),
                    Some(definition),
                    "{code}"
                );
            } else {
                let reason = outcome.strip_prefix("refused ").unwrap();
                assert!(!entries.contains_key(&code), "{code}");
                assert!(
                    errors
                        .iter()
                        .any(|e| e.message.contains(&format!("EPSG:{code} "))
                            && e.message.contains(reason)),
                    "{code}: {errors:?}"
                );
            }
        }
        // A refused definition is reported at the line where it starts.
        let lines: Vec<_> = errors.iter().map(|e| e.line).collect();
        assert_eq!(lines, [14, 16, 19]);
    }

    #[test]
    fn a_tag_that_is_no_code_is_an_error_on_its_own_line() {
        // Verifies: ADR-0023
        let errors = EpsgTable::with_table(
            "<1 +proj=longlat\n\
             <2> +proj=merc <>\n\
             <04326> +proj=merc <>\n\
             < 3 > +proj=merc <>\n\
             <4> +proj=longlat <5> +proj=merc <>\n",
        )
        .unwrap_err();
        let found: Vec<_> = errors
            .iter()
            .map(|e| (e.line, e.message.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                (1, "the code is not closed by > on its line"),
                (3, "`04326` is not an EPSG code"),
                (4, "` 3 ` is not an EPSG code"),
                (5, "`<5>` does not start its line"),
            ]
        );
        assert!(errors.iter().all(|e| !e.message.contains('\n')));
    }
}
