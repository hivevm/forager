//! The conformance suite, msautotest at the pinned MapServer release. G-1: every mapfile loads
//! through the CLI, with the suite's directory as the base context, without a diagnostic of
//! severity error, or, where its only errors are the suite's init file named bare, in-process
//! with a stand-in for that file. G-3: the style IR of every one survives YAML and JSON unchanged.
//! `scripts/check-rust.sh` fetches the suite with `scripts/fetch-msautotest.sh`, which holds the
//! pin, and runs these tests with `FORAGER_MSAUTOTEST` naming it; `cargo test` alone skips them,
//! as they need the network once.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::task::{Context, Poll, Waker};

use forager_adapter_epsg::EpsgTable;
use forager_ports::{ProjectionResolver, ResourceLoader, ResourcePath, SourceError};
use forager_style_ir::StyleDocument;
use serde_json::Value;

/// The comparison of a rendering with its reference image, for the harness.
mod metric;

/// The files of the suite that are not standalone mapfiles, each with its reason.
const SKIPPED: &[(&str, &str)] = &[
    (
        "config/6968.map",
        "malformed on purpose: it tests MapServer's error for BADTOKEN",
    ),
    (
        "misc/data/quoted_text.MAP",
        "binary test data, not a mapfile",
    ),
    (
        "mssql/include/bdry_counpy2_mssql.map",
        "an include fragment",
    ),
    ("mssql/include/mssql_connection.map", "an include fragment"),
    (
        "mssql/include/wfs_ogr_export_metadata.map",
        "an include fragment",
    ),
    (
        "php/maps/include/bdry_counpy2_postgis.map",
        "an include fragment",
    ),
    (
        "php/maps/include/bdry_counpy2_shapefile.map",
        "an include fragment",
    ),
    (
        "php/maps/include/indx_q100kpy4_shapefile.map",
        "an include fragment",
    ),
    ("query/include/bdry_counpy2_ogr.map", "an include fragment"),
    (
        "query/include/bdry_counpy2_postgis.map",
        "an include fragment",
    ),
    (
        "query/include/bdry_counpy2_shapefile.map",
        "an include fragment",
    ),
    ("query/include/indx_q100kpy4_ogr.map", "an include fragment"),
    (
        "query/include/indx_q100kpy4_shapefile.map",
        "an include fragment",
    ),
];

/// The EPSG codes the suite names beyond those built into the resolver (ADR-0023). Their
/// definitions here are stand-ins, so that the test carries no EPSG data: G-1 decides that a
/// mapfile loads; whether a projection is right is G-11's.
const EPSG_CODES: &[u32] = &[
    2157, 3347, 3395, 3978, 21781, 25833, 26711, 26915, 26917, 27700, 102977,
];

/// The suite's own init file, `wxs/data/epsg2`. Named by a path, as in `init=./data/epsg2:42304`,
/// the frontend reads it (ADR-0023); named bare, as `init=epsg2:42304`, PROJ finds it through
/// `PROJ_DATA`, which msautotest's run sets and the CLI has no counterpart of, so the CLI fails
/// that mapfile and the test loads it again with a stand-in for the file.
const INIT_FILE: &str = "epsg2";

/// The suite's EPSG codes, and a stand-in for every code of its init file named bare.
struct StandIn(EpsgTable);

impl ProjectionResolver for StandIn {
    fn resolve(&self, authority: &str, code: &str) -> Option<String> {
        if authority == INIT_FILE {
            return Some("+proj=longlat +datum=WGS84 +no_defs".to_owned());
        }
        self.0.resolve(authority, code)
    }
}

fn stand_in() -> StandIn {
    StandIn(EpsgTable::with_table(&epsg_table()).unwrap())
}

fn mapfiles(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            mapfiles(&path, found);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("map"))
        {
            found.push(path);
        }
    }
}

fn suite() -> PathBuf {
    PathBuf::from(
        std::env::var("FORAGER_MSAUTOTEST")
            .expect("FORAGER_MSAUTOTEST names the suite; scripts/check-rust.sh sets it"),
    )
}

fn epsg_table() -> String {
    EPSG_CODES
        .iter()
        .map(|code| format!("<{code}> +proj=longlat +datum=WGS84 +no_defs <>\n"))
        .collect()
}

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn every_msautotest_mapfile_loads_without_an_error() {
    // Verifies: G-1
    // Verifies: ADR-0023
    let suite = suite();
    let table =
        std::env::temp_dir().join(format!("forager-msautotest-{}.epsg", std::process::id()));
    std::fs::write(&table, epsg_table()).unwrap();

    let resolver = stand_in();
    let loader = Suite(suite.clone());
    let mut found = Vec::new();
    mapfiles(&suite, &mut found);
    let mut skipped = 0;
    let mut loaded = 0;
    let mut with_init_file = 0;
    let mut failures = Vec::new();
    for path in &found {
        let name = path.strip_prefix(&suite).unwrap().to_str().unwrap();
        if SKIPPED.iter().any(|(skip, _)| *skip == name) {
            skipped += 1;
            continue;
        }
        let output = Command::new(env!("CARGO_BIN_EXE_forager"))
            .args([
                "read",
                path.to_str().unwrap(),
                "--base",
                suite.to_str().unwrap(),
            ])
            .args(["--epsg", table.to_str().unwrap()])
            .output()
            .expect("the forager binary runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let errors: Vec<&str> = stderr.lines().filter(|l| l.contains(" error ")).collect();
        let only_init_file = !errors.is_empty()
            && errors.iter().all(|l| {
                l.contains("[projection-unresolved]") && l.contains(&format!("`init={INIT_FILE}:"))
            });
        if output.status.success() {
            loaded += 1;
        } else if only_init_file
            && ready(forager_mapfile::load(
                &ResourcePath::new(name).unwrap(),
                &loader,
                Some(&resolver),
            ))
            .document
            .is_some()
        {
            with_init_file += 1;
        } else {
            failures.push(format!("{name}\n    {}", errors.join("\n    ")));
        }
    }
    std::fs::remove_file(&table).unwrap();
    assert_eq!(
        with_init_file, 1,
        "only the mapfile that names the init file bare needs the stand-in (ADR-0023)"
    );

    assert_eq!(
        skipped,
        SKIPPED.len(),
        "a skipped file is no longer in the suite; the list must name only files that exist"
    );
    assert!(
        failures.is_empty(),
        "{} of {} mapfiles failed to load:\n{}",
        failures.len(),
        found.len() - skipped,
        failures.join("\n")
    );
    eprintln!(
        "{loaded} mapfiles loaded, {with_init_file} more with a stand-in for the init file, \
         {skipped} skipped"
    );
}

/// The suite's files, read inside its directory.
struct Suite(PathBuf);

impl ResourceLoader for Suite {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        std::fs::read(self.0.join(path.as_str())).map_err(|e| SourceError::new(e.to_string()))
    }
}

fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the suite's loader never waits"),
    }
}

/// The JSON Pointer of the first place where `a` and `b` differ.
fn first_difference(a: &Value, b: &Value, at: String) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys = x.keys().chain(y.keys().filter(|k| !x.contains_key(*k)));
            keys.into_iter().find_map(|k| match (x.get(k), y.get(k)) {
                (Some(v), Some(w)) => first_difference(v, w, format!("{at}/{k}")),
                _ => Some(format!("{at}/{k}")),
            })
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (v, w))| first_difference(v, w, format!("{at}/{i}"))),
        _ if a == b => None,
        _ => Some(if at.is_empty() { "/".to_owned() } else { at }),
    }
}

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn every_msautotest_style_ir_reads_back_equal_from_yaml_and_json() {
    // Verifies: G-3
    let suite = suite();
    let resolver = stand_in();
    let loader = Suite(suite.clone());
    let mut found = Vec::new();
    mapfiles(&suite, &mut found);
    let mut checked = 0;
    let mut failures = Vec::new();
    for path in &found {
        let name = path.strip_prefix(&suite).unwrap().to_str().unwrap();
        if SKIPPED.iter().any(|(skip, _)| *skip == name) {
            continue;
        }
        let path = ResourcePath::new(name).unwrap();
        let first = ready(forager_mapfile::load(&path, &loader, Some(&resolver)))
            .document
            .unwrap_or_else(|| panic!("{name} does not load; see the G-1 test"));
        let yaml = first.to_yaml().unwrap();
        let from_yaml = StyleDocument::from_yaml(&yaml).unwrap().document;
        let json = from_yaml.to_json().unwrap();
        let last = StyleDocument::from_json(&json).unwrap().document;
        checked += 1;
        if last != first {
            let (a, b) = (serde_json::to_value(&first), serde_json::to_value(&last));
            let at = first_difference(&a.unwrap(), &b.unwrap(), String::new());
            failures.push(format!(
                "{name} at {}",
                at.as_deref().unwrap_or("an unprintable value")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} style IRs changed through YAML and JSON:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("{checked} style IRs read back equal");
}

#[test]
fn the_first_difference_is_named_by_its_json_pointer() {
    let a = serde_json::json!({"map": {"layers": [{"name": "a", "size": 1.0}], "name": "m"}});
    let b = serde_json::json!({"map": {"layers": [{"name": "a", "size": 1.5}], "name": "m"}});
    assert_eq!(
        first_difference(&a, &b, String::new()).as_deref(),
        Some("/map/layers/0/size")
    );
    let c = serde_json::json!({"map": {"layers": [], "name": "m", "extra": true}});
    assert_eq!(
        first_difference(&a, &c, String::new()).as_deref(),
        Some("/map/layers")
    );
    assert_eq!(first_difference(&a, &a, String::new()), None);
}
