//! The conformance suite, msautotest at the pinned MapServer release. G-1: every mapfile loads
//! through the CLI, with the suite's directory as the base context, without a diagnostic of
//! severity error, or, where its only errors are the suite's init file named bare, in-process
//! with a stand-in for that file. G-3: the style IR of every one survives YAML and JSON unchanged.
//! The conformance harness: every case of `misc` and `renderers` that `conformance/cases.txt`
//! puts in scope is rendered in-process and compared with its reference image by the metric of
//! its category.
//! `scripts/check-rust.sh` fetches the suite with `scripts/fetch-msautotest.sh`, which holds the
//! pin, and runs these tests with `FORAGER_MSAUTOTEST` naming it; `cargo test` alone skips them,
//! as they need the network once.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;

use forager_adapter_epsg::EpsgTable;
use forager_adapter_shapefile::ShapefileSource;
use forager_engine::CachedLabel;
use forager_ports::{ProjectionResolver, ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::StyleDocument;
use serde_json::Value;

use crate::conformance::{Case, Category, Entry, Listed, Tally, Verdict, labels};
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

/// The suite's files, read inside its directory.
struct Suite(PathBuf);

impl ResourceLoader for Suite {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        std::fs::read(self.0.join(path.as_str())).map_err(|e| SourceError::new(e.to_string()))
    }
}

/// The suite with the text of its main mapfile replaced, to which `-i` has been applied.
struct MainReplaced<'a> {
    suite: &'a Suite,
    path: &'a ResourcePath,
    main: String,
}

impl ResourceLoader for MainReplaced<'_> {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        if path == self.path {
            Ok(self.main.clone().into_bytes())
        } else {
            self.suite.load(path).await
        }
    }
}

fn ready<F: Future>(future: F) -> F::Output {
    forager_ports::poll_once(future).expect("the suite's loader never waits")
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

/// The metric of the category geometry and symbols, as G-6 states it: a pixel differs when one of
/// its channels differs by more than 16, and a case passes when at most 0.1 % of its pixels
/// differ. MapServer at the pin reproduces its own references of the category bit for bit on CI
/// ([`mapserver_renders_the_cases_in_scope_within_its_noise`]), so the numbers lie above its noise.
const GEOMETRY: Metric = Metric {
    threshold: 16,
    tolerance: 0.001,
};

/// The metric of the category text, provisional: no criterion states its numbers yet, so the
/// harness compares the category by those of geometry and symbols until the noise measurement over
/// the text cases sets its own.
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

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn every_msautotest_case_in_scope_holds_its_outcome() {
    // Verifies: G-6
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
    let loader = Suite(suite.clone());
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
            match render_and_compare(&suite, case, &loader, &resolver, metric, &dir, name) {
                Ok((outcome, labels)) => (Ok(outcome), Some(labels)),
                Err(why) => (Err(why), None),
            };
        if *category == Category::Text
            && let Some(labels) = &labels
        {
            let named = format!("{} {}", case.mapfile, case.result);
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
        println!("The labels of G-10:\n{}", label_report.join("\n"));
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

/// The thresholds the noise measurement reports the share of differing pixels at.
const NOISE_THRESHOLDS: &[u8] = &[0, 1, 2, 4, 8, 16, 32, 64, 128];

/// The noise of G-6 and G-13: MapServer at the pinned release, which `.github/workflows/noise.yml`
/// builds (ADR-0027), renders every case the list puts in scope, of every category, as
/// msautotest's test library runs it, and each rendering is compared with its reference at each of
/// [`NOISE_THRESHOLDS`]. The report, a table of the share per case and threshold and its maximum
/// over each category, goes to standard output and to the job summary; it names apart each case
/// MapServer does not reproduce: `map2img` fails, writes no PNG, writes one of another size, or
/// differs beyond its category's tolerance at every threshold of the set. The list moves such a
/// case out of scope, so the test fails when one is in scope. For each text case the reader of
/// MapServer's label cache (ADR-0029) renders it beside `map2img`, its image byte for byte the
/// same, and its labels must equal the record beside the list; the test fails on a record that
/// differs or is missing, and the reader's labels stay in `noise/labels/` to take the record from.
/// The renderings stay in `noise/` under the test's target directory.
#[test]
#[ignore = "needs MapServer; .github/workflows/noise.yml builds it and runs this test"]
fn mapserver_renders_the_cases_in_scope_within_its_noise() {
    // Verifies: ADR-0029
    let suite = suite();
    let map2img = PathBuf::from(
        std::env::var("FORAGER_MAP2IMG")
            .expect("FORAGER_MAP2IMG names map2img; .github/workflows/noise.yml sets it"),
    );
    let reader = PathBuf::from(std::env::var("FORAGER_MAPSERVER_LABELS").expect(
        "FORAGER_MAPSERVER_LABELS names the reader of the label cache; .github/workflows/noise.yml \
         sets it",
    ));
    let cases = conformance::cases(&suite, conformance::DIRECTORIES);
    let entries = conformance::list(CASES).unwrap();
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("noise");
    let _ = std::fs::remove_dir_all(&out);
    let mut measured = Vec::new();
    let mut unreproduced = Vec::new();
    let mut records = Vec::new();
    for case in &cases {
        let Some(Entry::InScope { category, .. }) = entries.get(&case.key()).map(|l| &l.entry)
        else {
            continue;
        };
        let name = format!("{} {}", case.mapfile, case.result);
        let rendering = out
            .join(case.mapfile.trim_end_matches(".map"))
            .join(&case.result);
        match measure(&suite, case, metric_of(*category), &map2img, &rendering) {
            Ok(shares) => {
                if *category == Category::Text
                    && let Err(why) = read_labels(&suite, case, &reader, &rendering, &out)
                {
                    records.push(format!("{name}: {why}"));
                }
                measured.push((name, *category, shares));
            }
            Err(reason) => unreproduced.push(format!("{name}: {reason}")),
        }
    }
    let report = noise_report(&measured, &unreproduced);
    println!("{report}");
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary)
            .and_then(|mut file| file.write_all(report.as_bytes()))
            .unwrap();
    }
    assert!(
        unreproduced.is_empty(),
        "{} cases in scope are not reproduced by MapServer itself; the list moves each out of \
         scope with its reason:\n{}",
        unreproduced.len(),
        unreproduced.join("\n")
    );
    assert!(
        records.is_empty(),
        "{} text cases do not hold the record of MapServer's labels beside the list; the reader's \
         labels are in {}:\n{}",
        records.len(),
        out.join("labels").display(),
        records.join("\n")
    );
}

/// Renders `case` with the reader of MapServer's label cache beside `map2img`'s `rendering`,
/// holds the two images byte for byte equal, keeps the reader's labels in `out/labels/`, and
/// holds them equal to the record beside the list; the error says what does not hold.
fn read_labels(
    suite: &Path,
    case: &Case,
    reader: &Path,
    rendering: &Path,
    out: &Path,
) -> Result<(), String> {
    let image = rendering.with_extension("reader.png");
    let mapfile = suite.join(&case.mapfile);
    let output = Command::new(reader)
        .args(case.map2img_arguments(&image)?)
        .current_dir(mapfile.parent().unwrap())
        .output()
        .map_err(|e| format!("the reader does not start: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "the reader fails, {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let ours = std::fs::read(&image).map_err(|e| format!("the reader writes no image: {e}"))?;
    let theirs = std::fs::read(rendering).map_err(|e| e.to_string())?;
    if ours != theirs {
        return Err("the reader's image differs from map2img's".to_owned());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "the reader's labels are not UTF-8".to_owned())?;
    labels::read(&text).map_err(|e| format!("the reader's labels: {e}"))?;
    let record = labels::record_path(case);
    let relative = record
        .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/labels"))
        .unwrap();
    let kept = out.join("labels").join(relative);
    std::fs::create_dir_all(kept.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&kept, &text).map_err(|e| e.to_string())?;
    match std::fs::read_to_string(&record) {
        Ok(recorded) if recorded == text => Ok(()),
        Ok(_) => Err(format!(
            "MapServer's labels differ from {}",
            record.display()
        )),
        Err(e) => Err(format!("{}: {e}", record.display())),
    }
}

/// Runs `map2img` for `case` in its mapfile's directory, writing `rendering`, and compares the
/// rendering with the reference at each of [`NOISE_THRESHOLDS`]. The shares are in their order;
/// the error is why MapServer does not reproduce the case at all, by `metric`.
fn measure(
    suite: &Path,
    case: &Case,
    metric: &Metric,
    map2img: &Path,
    rendering: &Path,
) -> Result<Vec<f64>, String> {
    std::fs::create_dir_all(rendering.parent().unwrap()).map_err(|e| e.to_string())?;
    let mapfile = suite.join(&case.mapfile);
    let output = Command::new(map2img)
        .args(case.map2img_arguments(rendering)?)
        .current_dir(mapfile.parent().unwrap())
        .output()
        .map_err(|e| format!("map2img does not start: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "map2img fails, {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let rendering = std::fs::read(rendering)
        .map_err(|e| format!("map2img writes no image: {e}"))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("map2img's image: {e}")))?;
    let directory = &case.mapfile[..case.mapfile.find('/').unwrap()];
    let reference = suite.join(directory).join("expected").join(&case.result);
    let reference = std::fs::read(&reference)
        .map_err(|e| format!("its reference {}: {e}", reference.display()))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("its reference: {e}")))?;
    let shares = NOISE_THRESHOLDS
        .iter()
        .map(|&threshold| {
            let metric = Metric {
                threshold,
                tolerance: 0.0,
            };
            let comparison = metric::compare(&rendering, &reference, &metric);
            comparison.share().ok_or_else(|| comparison.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    reproduced(shares, metric)
}

/// The shares of a case MapServer reproduces: one at some threshold of the set within the
/// tolerance of `metric`. The shares fall as the threshold rises, so the last decides; the error
/// names a case that differs beyond the tolerance at every threshold.
fn reproduced(shares: Vec<f64>, metric: &Metric) -> Result<Vec<f64>, String> {
    match shares.last() {
        Some(&share) if share > metric.tolerance => Err(format!(
            "{share:.6} of its pixels differ by more than {}, above the tolerance {}",
            NOISE_THRESHOLDS[NOISE_THRESHOLDS.len() - 1],
            metric.tolerance
        )),
        _ => Ok(shares),
    }
}

#[test]
fn a_case_beyond_the_tolerance_at_every_threshold_is_not_reproduced() {
    let loose = Metric {
        threshold: 16,
        tolerance: 0.01,
    };
    for metric in [&GEOMETRY, &TEXT, &loose] {
        let mut shares = vec![0.5; NOISE_THRESHOLDS.len()];
        assert!(
            reproduced(shares.clone(), metric)
                .unwrap_err()
                .contains(&format!("above the tolerance {}", metric.tolerance))
        );
        *shares.last_mut().unwrap() = metric.tolerance;
        assert_eq!(reproduced(shares.clone(), metric), Ok(shares));
    }
    // The tolerance is the case's category's: a share within the loose one is beyond the strict.
    let mut shares = vec![0.5; NOISE_THRESHOLDS.len()];
    *shares.last_mut().unwrap() = 0.005;
    assert!(reproduced(shares.clone(), &loose).is_ok());
    assert!(reproduced(shares, &GEOMETRY).is_err());
}

#[test]
fn the_noise_report_holds_each_share_the_maximum_and_the_cases_not_reproduced() {
    let mut low = vec![0.0; NOISE_THRESHOLDS.len()];
    low[0] = 0.25;
    let mut high = vec![0.0; NOISE_THRESHOLDS.len()];
    high[0] = 0.5;
    high[1] = 0.125;
    let mut text = vec![0.0; NOISE_THRESHOLDS.len()];
    text[0] = 0.75;
    let measured = [
        ("a.map a.png".to_owned(), Category::Geometry, low),
        ("b.map b.png".to_owned(), Category::Geometry, high),
        ("t.map t.png".to_owned(), Category::Text, text),
    ];
    let report = noise_report(&measured, &["c.map c.png: map2img fails".to_owned()]);
    assert!(
        report.contains("| a.map a.png | geometry and symbols | 0.250000 | 0.000000 |"),
        "{report}"
    );
    assert!(
        report.contains("| t.map t.png | text | 0.750000 | 0.000000 |"),
        "{report}"
    );
    // Each category's maximum is over its own cases alone.
    assert!(
        report.contains("| **maximum, geometry and symbols** | | **0.500000** | **0.125000** |"),
        "{report}"
    );
    assert!(
        report.contains("| **maximum, text** | | **0.750000** | **0.000000** |"),
        "{report}"
    );
    assert!(report.contains("3 of 4 cases measured."), "{report}");
    assert!(
        report.contains("### Not reproduced by MapServer\n\n- c.map c.png: map2img fails\n"),
        "{report}"
    );
    let clean = noise_report(&measured, &[]);
    assert!(!clean.contains("Not reproduced"), "{clean}");
    // A category with no case measured has no maximum.
    let geometry = noise_report(&measured[..2], &[]);
    assert!(!geometry.contains("maximum, text"), "{geometry}");
}

/// The report of the noise measurement, as Markdown: a row per measured case with its category and
/// its share of differing pixels at each of [`NOISE_THRESHOLDS`], the maximum of each column over
/// each category that has a case measured, and the cases MapServer does not reproduce, each with
/// its reason.
fn noise_report(measured: &[(String, Category, Vec<f64>)], unreproduced: &[String]) -> String {
    let mut report =
        String::from("## MapServer's noise over the cases in scope\n\n| Case | Category |");
    for threshold in NOISE_THRESHOLDS {
        report.push_str(&format!(" > {threshold} |"));
    }
    report.push_str("\n| --- | --- |");
    report.push_str(&" ---: |".repeat(NOISE_THRESHOLDS.len()));
    report.push('\n');
    let mut maxima = BTreeMap::new();
    for (name, category, shares) in measured {
        report.push_str(&format!("| {name} | {} |", category.name()));
        let maximum = maxima
            .entry(*category)
            .or_insert_with(|| vec![0.0_f64; NOISE_THRESHOLDS.len()]);
        for (max, share) in maximum.iter_mut().zip(shares) {
            *max = max.max(*share);
            report.push_str(&format!(" {share:.6} |"));
        }
        report.push('\n');
    }
    for (category, maximum) in Category::ALL
        .iter()
        .filter_map(|c| maxima.get(c).map(|m| (c, m)))
    {
        report.push_str(&format!("| **maximum, {}** | |", category.name()));
        for max in maximum {
            report.push_str(&format!(" **{max:.6}** |"));
        }
        report.push('\n');
    }
    report.push_str(&format!(
        "\n{} of {} cases measured.\n",
        measured.len(),
        measured.len() + unreproduced.len()
    ));
    if !unreproduced.is_empty() {
        report.push_str("\n### Not reproduced by MapServer\n\n");
        for line in unreproduced {
            report.push_str(&format!("- {line}\n"));
        }
    }
    report
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

/// Renders `case` in-process and compares it with its reference by `metric`, its category's,
/// leaving the rendering and a difference image in `dir` as `<name>.png` and `<name>.diff.png`
/// when it fails. The outcome is whether it passes, and the comparison, with the labels of the
/// render; an error is a case that cannot be rendered or compared, which fails.
fn render_and_compare(
    suite: &Path,
    case: &Case,
    loader: &Suite,
    resolver: &StandIn,
    metric: &Metric,
    dir: &Path,
    name: &str,
) -> Result<((bool, String), Vec<CachedLabel>), String> {
    let directory = &case.mapfile[..case.mapfile.find('/').unwrap()];
    let reference = suite.join(directory).join("expected").join(&case.result);
    let reference = std::fs::read(&reference)
        .map_err(|e| format!("its reference {}: {e}", reference.display()))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("its reference: {e}")))?;
    let (rendering, labels) = render(case, loader, resolver)?;
    let comparison = metric::compare_and_keep(&rendering, &reference, metric, dir, name)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(((comparison.passes(), comparison.to_string()), labels))
}

/// What differs between the labels of `case` Forager renders and MapServer's, as the record
/// beside the list holds them (G-10, ADR-0029); an error is a record missing or unreadable.
fn labels_against_record(case: &Case, rendered: &[CachedLabel]) -> Result<Vec<String>, String> {
    let path = labels::record_path(case);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("its label record {}: {e}", path.display()))?;
    let recorded =
        labels::read(&text).map_err(|e| format!("its label record {}: {e}", path.display()))?;
    Ok(labels::compare(rendered, &recorded))
}

/// The rendering of `case` and the labels of its cache, the options of its `map2img` line applied to its style IR, its sources
/// bound as `forager render` binds them as far as a test can: the CLI's binding lives in its
/// binary, which a test cannot call. As there, each local layer the engine reads is bound to the
/// shapefile its `DATA` names, a layer whose shapefile does not open is left unbound, so that the
/// engine skips it, and the fonts of the labels are read through the loader. Unlike there, the
/// files are read through the suite's loader without the CLI's read limit, each layer opens its
/// shapefile anew instead of sharing it, a shapefile that does not open gives no diagnostic, and
/// the projections resolve through the stand-in table of the G-1 test.
fn render(
    case: &Case,
    loader: &Suite,
    resolver: &StandIn,
) -> Result<(RgbaImage, Vec<CachedLabel>), String> {
    let path = ResourcePath::new(&case.mapfile).map_err(|e| e.to_string())?;
    let options = case.options()?;
    let loaded = match conformance::image_type(&options) {
        Some(image_type) => {
            let text = std::fs::read_to_string(loader.0.join(&case.mapfile))
                .map_err(|e| format!("its mapfile: {e}"))?;
            let main = conformance::with_image_type(&text, image_type)?;
            let loader = MainReplaced {
                suite: loader,
                path: &path,
                main,
            };
            ready(forager_mapfile::load(&path, &loader, Some(resolver)))
        }
        None => ready(forager_mapfile::load(&path, loader, Some(resolver))),
    };
    let document = loaded.document.ok_or_else(|| {
        let errors: Vec<_> = loaded.diagnostics.iter().map(ToString::to_string).collect();
        format!("it does not load: {}", errors.join("; "))
    })?;
    let mut map = document.map;
    conformance::apply(&options, &mut map, &loaded.diagnostics)?;
    let mut sources = BTreeMap::new();
    for layer in &map.layers {
        let (Some(name), Some(source)) = (&layer.name, &layer.source) else {
            continue;
        };
        if forager_engine::reads_bound_source(layer)
            && source.kind == "local"
            && let Some(data) = source.props.get("data")
            && let Ok(data) = ResourcePath::new(data)
            && let Ok(shapefile) = ready(ShapefileSource::open(loader, &data))
        {
            sources.insert(name.clone(), shapefile);
        }
    }
    let rendered = ready(forager_engine::render_with_files(
        &map,
        &sources,
        loader,
        AggRenderer::new(),
    ));
    let Some(image) = rendered.image else {
        let errors: Vec<_> = rendered
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect();
        return Err(format!("it renders no image: {}", errors.join("; ")));
    };
    Ok((image, rendered.labels))
}
