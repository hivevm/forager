//! The conformance suite, msautotest at the pinned MapServer release. G-1: every mapfile loads
//! through the CLI, with the suite's directory as the base context, without a diagnostic of
//! severity error, or, where its only errors are the suite's init file named bare, in-process
//! with a stand-in for that file. G-3: the style IR of every one survives YAML and JSON unchanged.
//! The conformance harness: every case of `misc` and `renderers` that `conformance/cases.txt`
//! puts in scope is rendered in-process and compared with its reference image by the metric of
//! its category. The noise measurement, in `conformance/noise/`, runs MapServer itself over the
//! same cases where `.github/workflows/noise.yml` builds it, and skips itself elsewhere.
//! `scripts/check-rust.sh` fetches the suite with `scripts/fetch-msautotest.sh`, which holds the
//! pin, and runs these tests with `FORAGER_MSAUTOTEST` naming it; `cargo test` alone skips them,
//! as they need the network once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use forager_adapter_epsg::EpsgTable;
use forager_cli::{FileLoader, MapfileLoader};
use forager_engine::CachedLabel;
use forager_mapfile::Loaded;
use forager_ports::{ProjectionResolver, ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_style_ir::{Map, StyleDocument};
use serde_json::Value;

use crate::conformance::{Case, Category, Entry, Listed, Tally, Verdict, labels, wasm};
use crate::metric::Metric;

/// The cases of the suite and the list that scopes them, for the harness.
mod conformance;
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

/// The EPSG codes the suite names beyond those built into the resolver (ADR-0019). Their
/// definitions here are stand-ins, so that the test carries no EPSG data: G-1 decides that a
/// mapfile loads; whether a projection is right is G-11's.
const EPSG_CODES: &[u32] = &[
    2157, 3347, 3395, 3978, 21781, 25833, 26711, 26915, 26917, 27700, 102977,
];

/// The suite's own init file, `wxs/data/epsg2`. Named by a path, as in `init=./data/epsg2:42304`,
/// the frontend reads it (ADR-0019); named bare, as `init=epsg2:42304`, PROJ finds it through
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

/// Prints a count of a test that passed, G-1's mapfiles or G-3's round trips.
#[expect(
    clippy::print_stderr,
    reason = "the counts of G-1 and G-3, on stderr, shown with --nocapture"
)]
fn count(line: &str) {
    eprintln!("{line}");
}

/// Prints a report of the harness, G-10's labels or the noise measurement.
#[expect(
    clippy::print_stdout,
    reason = "the reports of G-10's labels and of the noise measurement, read with --nocapture \
              and in the noise job's log"
)]
fn report(text: &str) {
    println!("{text}");
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
    // Verifies: ADR-0019
    let suite = suite();
    let table =
        std::env::temp_dir().join(format!("forager-msautotest-{}.epsg", std::process::id()));
    std::fs::write(&table, epsg_table()).unwrap();

    let resolver = stand_in();
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
            && base_context(&suite, name)
                .and_then(|(base, path)| load(&path, &base.for_load(), &resolver))
                .is_ok_and(|loaded| loaded.document.is_some())
        {
            with_init_file += 1;
        } else {
            failures.push(format!("{name}\n    {}", errors.join("\n    ")));
        }
    }
    std::fs::remove_file(&table).unwrap();
    assert_eq!(
        with_init_file, 1,
        "only the mapfile that names the init file bare needs the stand-in (ADR-0019)"
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
    count(&format!(
        "{loaded} mapfiles loaded, {with_init_file} more with a stand-in for the init file, \
         {skipped} skipped"
    ));
}

/// Files the cases of `misc` and `renderers` read, one of each place they lie: a reference image, a
/// shapefile of `renderers/data` and one of `misc/data`, and one each of `query/data` and
/// `wxs/data` that a mapfile of those two directories names by a `..` path
/// (`misc/style_opacity.map`, `renderers/labels-leader.map`). `world_testlines`, which the ticket
/// placed in `misc/data`, lies in `renderers/data` at the pin.
const DATA_AND_REFERENCES: &[&str] = &[
    "renderers/expected/line_simple.png",
    "renderers/data/world_testlines.shp",
    "renderers/data/world_testlines.shx",
    "renderers/data/world_testlines.dbf",
    "misc/data/testlines.shp",
    "query/data/bdry_counpy2.shp",
    "query/data/bdry_counpy2.shx",
    "query/data/bdry_counpy2.dbf",
    "wxs/data/province.shp",
    "wxs/data/province.shx",
    "wxs/data/province.dbf",
];

/// Every file under `dir` whose name ends with `suffix`.
fn files_ending_with(dir: &Path, suffix: &str, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_ending_with(&path, suffix, found);
        } else if path.to_str().is_some_and(|p| p.ends_with(suffix)) {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn the_fetched_suite_holds_the_data_and_the_non_cairo_references() {
    let suite = suite();
    let missing: Vec<_> = DATA_AND_REFERENCES
        .iter()
        .filter(|name| !suite.join(name).is_file())
        .collect();
    assert!(missing.is_empty(), "the fetch did not take {missing:?}");
    let mut cairo = Vec::new();
    files_ending_with(&suite, ".cairo.png", &mut cairo);
    assert!(
        cairo.is_empty(),
        "the fetch took cairo references: {cairo:?}"
    );
}

/// The suite's directory as the base context, and the path of `mapfile` inside it, as
/// `forager render <mapfile> --base <suite>` takes them.
fn base_context(suite: &Path, mapfile: &str) -> Result<(FileLoader, ResourcePath), String> {
    let (base, name) = FileLoader::for_mapfile(&suite.join(mapfile), Some(suite))
        .map_err(|e| format!("its base context: {e}"))?;
    let path = ResourcePath::new(&name).map_err(|e| e.to_string())?;
    Ok((base, path))
}

/// Loads the mapfile at `path` through `loader` in one poll, as the CLI loads one; a read that
/// waited is the error.
fn load(
    path: &ResourcePath,
    loader: &impl ResourceLoader,
    resolver: &StandIn,
) -> Result<Loaded, String> {
    forager_cli::now(forager_mapfile::load(path, loader, Some(resolver))).map_err(|e| e.to_string())
}

/// The files of one load of a mapfile from the base context, with the text of the main mapfile
/// replaced by the one to which `-i` has been applied.
struct MainReplaced<'a> {
    files: &'a MapfileLoader,
    path: &'a ResourcePath,
    main: String,
}

impl ResourceLoader for MainReplaced<'_> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path == self.path {
            Ok(self.main.clone().into_bytes())
        } else {
            self.files.load(path).await
        }
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
    let mut found = Vec::new();
    mapfiles(&suite, &mut found);
    let mut checked = 0;
    let mut failures = Vec::new();
    for path in &found {
        let name = path.strip_prefix(&suite).unwrap().to_str().unwrap();
        if SKIPPED.iter().any(|(skip, _)| *skip == name) {
            continue;
        }
        let first = base_context(&suite, name)
            .and_then(|(base, path)| load(&path, &base.for_load(), &resolver))
            .ok()
            .and_then(|loaded| loaded.document)
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
    count(&format!("{checked} style IRs read back equal"));
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

/// The metric of the category geometry and symbols, as G-6 states it: a pixel differs when one of
/// its channels differs by more than 16, and a case passes when at most 0.1 % of its pixels
/// differ. MapServer at the pin reproduces its own references of the category bit for bit on CI
/// ([`conformance::noise::mapserver_renders_the_cases_in_scope_within_its_noise`]), so the numbers
/// lie above its noise.
const GEOMETRY: Metric = Metric {
    threshold: 16,
    tolerance: 0.001,
};

/// The metric of the category text, as G-13 states it: that of G-6, a pixel differs when one of its
/// channels differs by more than 16, and a case passes when at most 0.1 % of its pixels differ.
/// MapServer at the pin reproduces its own references of the category bit for bit on CI
/// ([`conformance::noise::mapserver_renders_the_cases_in_scope_within_its_noise`]), so the numbers
/// lie above its noise.
const TEXT: Metric = Metric {
    threshold: 16,
    tolerance: 0.001,
};

/// The metric `category` is compared by.
fn metric_of(category: Category) -> &'static Metric {
    match category {
        Category::Geometry => &GEOMETRY,
        Category::Text => &TEXT,
    }
}

/// The list of the cases of `misc` and `renderers`, in scope or out of it.
const CASES: &str = include_str!("conformance/cases.txt");

/// Every record under `conformance/labels/` is the record of a text case in scope of the list, and
/// every such case has its record, read from the list and the directory alone, without the suite:
/// a record left behind by a case that left the list, the scope, or the category text would pass
/// for measured.
#[test]
fn every_label_record_is_that_of_a_text_case_in_scope() {
    let entries = conformance::list(CASES).unwrap();
    let found = labels::records(&labels::records_dir());
    let unmatched = labels::unmatched(&entries, &found);
    assert!(
        unmatched.is_empty(),
        "the records under conformance/labels/ do not line up with the text cases of \
         conformance/cases.txt:\n{}",
        unmatched.join("\n")
    );
}

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn every_msautotest_case_in_scope_holds_its_outcome() {
    // Verifies: G-6
    // Verifies: G-13, for the category text
    // Verifies: G-10, for the text cases the list marks labels-hold
    let suite = suite();
    let cases = conformance::cases(&suite, conformance::DIRECTORIES);
    let entries = conformance::list(CASES).unwrap();
    let mut problems = conformance::unlisted(&cases, &entries);
    problems.extend(blank_markers(&suite, &entries));
    assert!(
        problems.is_empty(),
        "the list conformance/cases.txt does not hold the suite's cases:\n{}",
        problems.join("\n")
    );

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("msautotest");
    let _ = std::fs::remove_dir_all(&out);
    let resolver = stand_in();
    let mut tally = Tally::default();
    let mut failures = Vec::new();
    let mut broken = Vec::new();
    let mut label_report = Vec::new();
    for case in &cases {
        let Some(Listed {
            entry: Entry::InScope { category, passes },
            blank,
            labels_hold,
        }) = entries.get(&case.key())
        else {
            continue;
        };
        let dir = out.join(case.mapfile.trim_end_matches(".map"));
        let name = case.result.trim_end_matches(".png");
        let metric = metric_of(*category);
        let (outcome, labels) =
            match render_and_compare(&suite, case, &resolver, metric, &dir, name) {
                Ok((outcome, labels)) => (Ok(outcome), Some(labels)),
                Err(why) => (Err(why), None),
            };
        // A case marked to hold G-10 whose render fails does not hold it.
        if *labels_hold && labels.is_none() {
            failures.push(format!(
                "{} {}: it renders no labels; the list marks it labels-hold",
                case.mapfile, case.result
            ));
        }
        if *category == Category::Text
            && let Some((labels, uncompared)) = &labels
        {
            let named = format!("{} {}", case.mapfile, case.result);
            label_report.extend(uncompared.iter().map(|layer| format!("{named}: {layer}")));
            match labels_against_record(case, labels) {
                Ok(differences) => {
                    let holds = differences.is_empty();
                    if !holds {
                        label_report.push(format!(
                            "{named}: {} of its labels differ from MapServer's:\n  {}",
                            differences.len(),
                            differences.join("\n  ")
                        ));
                    }
                    if holds != *labels_hold {
                        let (found, marked) = if holds {
                            ("all match MapServer's", "without labels-hold")
                        } else {
                            ("differ from MapServer's", "labels-hold")
                        };
                        failures.push(format!(
                            "{named}: its labels {found}; the list marks it {marked}"
                        ));
                    }
                }
                Err(why) => broken.push(format!("{named}: {why}")),
            }
        }
        tally.count(*blank, matches!(outcome, Ok((true, _))));
        match conformance::verdict(case, *passes, outcome) {
            Some(Verdict::Broken(problem)) => broken.push(problem),
            Some(Verdict::Differs(failure)) => failures.push(failure),
            None => {}
        }
    }
    if !label_report.is_empty() {
        report(&format!("The labels of G-10:\n{}", label_report.join("\n")));
    }
    let in_scope = tally.in_scope;
    // The counts for scripts/check-fidelity.sh, which scripts/check-rust.sh runs after this test.
    if let Some(counts) = std::env::var_os("FORAGER_FIDELITY") {
        std::fs::write(&counts, tally.to_string()).unwrap();
    }
    assert!(
        broken.is_empty(),
        "{} of {in_scope} cases in scope cannot be rendered or compared, whatever outcome the \
         list expects:\n{}",
        broken.len(),
        broken.join("\n")
    );
    assert!(
        failures.is_empty(),
        "{} of {in_scope} cases in scope do not hold their outcome; the renderings and \
         difference images of those that fail are in {}:\n{}",
        failures.len(),
        out.display(),
        failures.join("\n")
    );
}

/// The module the comparison of G-9 runs, which `scripts/check-rust.sh` builds and names.
fn conformance_module() -> PathBuf {
    PathBuf::from(std::env::var("FORAGER_CONFORMANCE_WASM").expect(
        "FORAGER_CONFORMANCE_WASM names the browser bindings' module built with the feature \
         conformance; scripts/check-rust.sh builds it and sets it",
    ))
}

/// G-9: every case in scope, its shapefiles' features written into its map, renders natively and
/// on wasm32 to the same RGBA buffer, byte for byte (Q-2). The module is the browser bindings'
/// wasm32 build, of the same core crates and profile as the bundle, run in wasmi (ADR-0034).
#[test]
#[ignore = "needs the msautotest suite and the conformance module; scripts/check-rust.sh builds \
            them and runs this test"]
fn every_case_in_scope_renders_alike_natively_and_on_wasm32() {
    // Verifies: G-9
    // Verifies: Q-2
    // Verifies: ADR-0034
    let suite = suite();
    let module = std::fs::read(conformance_module()).expect("the conformance module reads");
    let module = wasm::Wasm::new(&module).expect("wasmi compiles the conformance module");
    let cases = conformance::cases(&suite, conformance::DIRECTORIES);
    let entries = conformance::list(CASES).unwrap();
    let resolver = stand_in();
    let (mut in_scope, mut compared, mut differences) = (0, 0, Vec::new());
    for case in &cases {
        let Some(Listed {
            entry: Entry::InScope { .. },
            ..
        }) = entries.get(&case.key())
        else {
            continue;
        };
        in_scope += 1;
        let named = format!("{} {}", case.mapfile, case.result);
        let (base, mut map) = match base_context(&suite, &case.mapfile)
            .and_then(|(base, path)| Ok((map_of(case, &base, &path, &resolver)?, base)))
        {
            Ok((map, base)) => (base, map),
            Err(why) => {
                differences.push(format!("{named}: {why}"));
                continue;
            }
        };
        if let Err(why) = wasm::inline(&mut map, &base.for_load()) {
            differences.push(format!("{named}: {why}"));
            continue;
        }
        let document = StyleDocument::new(map).to_json().unwrap();
        let (native, files) = wasm::native(&document, &base.for_load());
        match module.render(&document, &files) {
            Ok(rendered) => {
                compared += 1;
                if let Some(difference) = wasm::difference(&native, &rendered) {
                    differences.push(format!("{named}: {difference}"));
                }
            }
            Err(why) => differences.push(format!("{named}: the module fails: {why}")),
        }
    }
    assert!(
        differences.is_empty(),
        "{} of the cases in scope render differently on wasm32:\n{}",
        differences.len(),
        differences.join("\n")
    );
    assert!(compared > 0, "no case in scope was compared");
    assert_eq!(compared, in_scope, "every case in scope is compared");
    count(&format!(
        "{compared} of {in_scope} cases in scope render alike natively and on wasm32"
    ));
}

/// The cases whose `blank` marker in the list disagrees with their reference: a case is blank
/// when its reference is a PNG that [`metric::blank`] finds nearly one colour by the metric of its
/// category, or by [`GEOMETRY`] when it is out of scope, an image of its most common colour
/// passing against it. A reference that is no PNG, or that
/// the suite does not hold, is not blank. A list line whose mapfile names no directory is
/// reported, as it can name no reference.
fn blank_markers(suite: &Path, entries: &BTreeMap<(String, String), Listed>) -> Vec<String> {
    let mut problems = Vec::new();
    for ((mapfile, result), listed) in entries {
        let Some((directory, _)) = mapfile.split_once('/') else {
            problems.push(format!(
                "{mapfile} {result}: the mapfile names no directory of the suite"
            ));
            continue;
        };
        let metric = match listed.entry {
            Entry::InScope { category, .. } => metric_of(category),
            Entry::Out(_) => &GEOMETRY,
        };
        let blank = result.ends_with(".png")
            && std::fs::read(suite.join(directory).join("expected").join(result))
                .ok()
                .and_then(|bytes| metric::decode(&bytes).ok())
                .is_some_and(|reference| metric::blank(&reference, metric));
        if blank != listed.blank {
            let is = if blank { "is" } else { "is not" };
            let marks = if listed.blank {
                "marks"
            } else {
                "does not mark"
            };
            problems.push(format!(
                "{mapfile} {result}: its reference {is} blank, but the list {marks} it so"
            ));
        }
    }
    problems
}

/// The labels of a render's label cache, and the layers whose labels are not compared
/// ([`labels::uncompared`]).
type RenderLabels = (Vec<CachedLabel>, Vec<String>);

/// Renders `case` in-process and compares it with its reference by `metric`, its category's,
/// leaving the rendering and a difference image in `dir` as `<name>.png` and `<name>.diff.png`
/// when it fails. The outcome is whether it passes, and the comparison, with the labels of the
/// render; an error is a case that cannot be rendered or compared, which fails.
fn render_and_compare(
    suite: &Path,
    case: &Case,
    resolver: &StandIn,
    metric: &Metric,
    dir: &Path,
    name: &str,
) -> Result<((bool, String), RenderLabels), String> {
    let directory = &case.mapfile[..case.mapfile.find('/').unwrap()];
    let reference = suite.join(directory).join("expected").join(&case.result);
    let reference = std::fs::read(&reference)
        .map_err(|e| format!("its reference {}: {e}", reference.display()))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("its reference: {e}")))?;
    let (rendering, labels) = render(case, suite, resolver)?;
    let comparison = metric::compare_and_keep(&rendering, &reference, metric, dir, name)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(((comparison.passes(), comparison.to_string()), labels))
}

/// What differs between the labels of `case` Forager renders and MapServer's, as the record
/// beside the list holds them (G-10, ADR-0033); an error is a record missing or unreadable.
fn labels_against_record(case: &Case, rendered: &[CachedLabel]) -> Result<Vec<String>, String> {
    let path = labels::record_path(case);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("its label record {}: {e}", path.display()))?;
    let recorded =
        labels::read(&text).map_err(|e| format!("its label record {}: {e}", path.display()))?;
    Ok(labels::compare(rendered, &recorded))
}

/// The rendering of `case`, the labels of its cache, and the layers whose labels are not compared,
/// the options of its `map2img` line applied to its style IR, its sources bound and the fonts of
/// its labels read by the CLI's library from the suite's directory as the base context, as
/// `forager render` binds and reads them. The mapfile loads through the same base context, as
/// `forager render` loads it; unlike there, through the stand-in table of the G-1 test and with
/// `-i` applied, and the binding's diagnostics are reported only with a rendering that gives no
/// image.
fn render(
    case: &Case,
    suite: &Path,
    resolver: &StandIn,
) -> Result<(RgbaImage, RenderLabels), String> {
    let (base, path) = base_context(suite, &case.mapfile)?;
    let map = map_of(case, &base, &path, resolver)?;
    let rendered = forager_cli::render(&map, &[], &base);
    let Some(image) = rendered.image else {
        let errors: Vec<_> = rendered
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect();
        return Err(format!("it renders no image: {}", errors.join("; ")));
    };
    Ok((image, (rendered.labels, labels::uncompared(&map))))
}

/// The style IR of `case`, its mapfile at `path` loaded from `base` through the stand-in table of
/// the G-1 test with `-i` applied, and the other options of its `map2img` line applied to it.
fn map_of(
    case: &Case,
    base: &FileLoader,
    path: &ResourcePath,
    resolver: &StandIn,
) -> Result<Map, String> {
    let options = case.options()?;
    let loaded = match conformance::image_type(&options) {
        Some(image_type) => {
            let text = forager_cli::now(base.for_load().load(path))
                .map_err(|e| e.to_string())?
                .map_err(|e| format!("its mapfile: {e}"))?;
            let text = String::from_utf8(text).map_err(|e| format!("its mapfile: {e}"))?;
            let main = conformance::with_image_type(&text, image_type)?;
            let files = base.for_load();
            let loader = MainReplaced {
                files: &files,
                path,
                main,
            };
            load(path, &loader, resolver)?
        }
        None => load(path, &base.for_load(), resolver)?,
    };
    let document = loaded.document.ok_or_else(|| {
        let errors: Vec<_> = loaded.diagnostics.iter().map(ToString::to_string).collect();
        format!("it does not load: {}", errors.join("; "))
    })?;
    let mut map = document.map;
    conformance::apply(&options, &mut map, &loaded.diagnostics)?;
    Ok(map)
}
