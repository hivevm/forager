//! The conformance suite, msautotest at the pinned MapServer release. G-1: every mapfile loads
//! through the CLI, with the suite's directory as the base context, without a diagnostic of
//! severity error, or, where its only errors are the suite's init file named bare, in-process
//! with a stand-in for that file. G-3: the style IR of every one survives YAML and JSON unchanged.
//! The conformance harness: every case of `misc` and `renderers` that `conformance/cases.txt`
//! puts in scope is rendered in-process and compared with its reference image.
//! `scripts/check-rust.sh` fetches the suite with `scripts/fetch-msautotest.sh`, which holds the
//! pin, and runs these tests with `FORAGER_MSAUTOTEST` naming it; `cargo test` alone skips them,
//! as they need the network once.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;

use forager_adapter_epsg::EpsgTable;
use forager_adapter_shapefile::ShapefileSource;
use forager_ports::{ProjectionResolver, ResourceLoader, ResourcePath, SourceError};
use forager_render::RgbaImage;
use forager_render_agg::AggRenderer;
use forager_style_ir::StyleDocument;
use serde_json::Value;

use crate::conformance::{Case, Entry, Listed, Verdict};
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

/// The list of the cases of `misc` and `renderers`, in scope or out of it.
const CASES: &str = include_str!("conformance/cases.txt");

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn every_msautotest_case_in_scope_holds_its_outcome() {
    // Verifies: G-6
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
    let mut in_scope = 0;
    let mut failures = Vec::new();
    let mut broken = Vec::new();
    for case in &cases {
        let Some(Entry::InScope { passes, .. }) = entries.get(&case.key()).map(|l| &l.entry) else {
            continue;
        };
        in_scope += 1;
        let dir = out.join(case.mapfile.trim_end_matches(".map"));
        let name = case.result.trim_end_matches(".png");
        let outcome = render_and_compare(&suite, case, &loader, &resolver, &dir, name);
        match conformance::verdict(case, *passes, outcome) {
            Some(Verdict::Broken(problem)) => broken.push(problem),
            Some(Verdict::Differs(failure)) => failures.push(failure),
            None => {}
        }
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

/// The noise of G-6: MapServer at the pinned release, which `.github/workflows/noise.yml` builds
/// (ADR-0027), renders every case the list puts in scope as msautotest's test library runs it, and
/// each rendering is compared with its reference at each of [`NOISE_THRESHOLDS`]. The report, a
/// table of the share per case and threshold and its maximum over the category, goes to standard
/// output and to the job summary; it names apart each case MapServer does not reproduce at all:
/// `map2img` fails, writes no PNG, or writes one of another size. The renderings stay in
/// `noise/` under the test's target directory. A sensor: it fails only when no case is measured.
#[test]
#[ignore = "needs MapServer; .github/workflows/noise.yml builds it and runs this test"]
fn mapserver_renders_the_cases_in_scope_within_its_noise() {
    let suite = suite();
    let map2img = PathBuf::from(
        std::env::var("FORAGER_MAP2IMG")
            .expect("FORAGER_MAP2IMG names map2img; .github/workflows/noise.yml sets it"),
    );
    let cases = conformance::cases(&suite, conformance::DIRECTORIES);
    let entries = conformance::list(CASES).unwrap();
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("noise");
    let _ = std::fs::remove_dir_all(&out);
    let mut measured = Vec::new();
    let mut unreproduced = Vec::new();
    for case in &cases {
        let Some(Entry::InScope { .. }) = entries.get(&case.key()).map(|l| &l.entry) else {
            continue;
        };
        let name = format!("{} {}", case.mapfile, case.result);
        let rendering = out
            .join(case.mapfile.trim_end_matches(".map"))
            .join(&case.result);
        match measure(&suite, case, &map2img, &rendering) {
            Ok(shares) => measured.push((name, shares)),
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
        !measured.is_empty(),
        "MapServer reproduces none of the cases in scope:\n{}",
        unreproduced.join("\n")
    );
}

/// Runs `map2img` for `case` in its mapfile's directory, writing `rendering`, and compares the
/// rendering with the reference at each of [`NOISE_THRESHOLDS`]. The shares are in their order;
/// the error is why MapServer does not reproduce the case at all.
fn measure(
    suite: &Path,
    case: &Case,
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
    NOISE_THRESHOLDS
        .iter()
        .map(|&threshold| {
            let metric = Metric {
                threshold,
                tolerance: 0.0,
            };
            let comparison = metric::compare(&rendering, &reference, &metric);
            comparison.share().ok_or_else(|| comparison.to_string())
        })
        .collect()
}

/// The report of the noise measurement, as Markdown: a row per measured case with its share of
/// differing pixels at each of [`NOISE_THRESHOLDS`], the maximum of each column over the category,
/// and the cases MapServer does not reproduce, each with its reason.
fn noise_report(measured: &[(String, Vec<f64>)], unreproduced: &[String]) -> String {
    let mut report = String::from("## MapServer's noise over the cases in scope\n\n| Case |");
    for threshold in NOISE_THRESHOLDS {
        report.push_str(&format!(" > {threshold} |"));
    }
    report.push_str("\n| --- |");
    report.push_str(&" ---: |".repeat(NOISE_THRESHOLDS.len()));
    report.push('\n');
    let mut maximum = vec![0.0_f64; NOISE_THRESHOLDS.len()];
    for (name, shares) in measured {
        report.push_str(&format!("| {name} |"));
        for (max, share) in maximum.iter_mut().zip(shares) {
            *max = max.max(*share);
            report.push_str(&format!(" {share:.6} |"));
        }
        report.push('\n');
    }
    report.push_str("| **maximum, geometry and symbols** |");
    for max in &maximum {
        report.push_str(&format!(" **{max:.6}** |"));
    }
    report.push_str(&format!(
        "\n\n{} of {} cases measured.\n",
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
/// when its reference is a PNG that [`metric::blank`] finds nearly one colour by [`GEOMETRY`],
/// an image of its most common colour passing against it. A reference that is no PNG, or that
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
        let blank = result.ends_with(".png")
            && std::fs::read(suite.join(directory).join("expected").join(result))
                .ok()
                .and_then(|bytes| metric::decode(&bytes).ok())
                .is_some_and(|reference| metric::blank(&reference, &GEOMETRY));
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

/// Renders `case` in-process and compares it with its reference by [`GEOMETRY`], leaving the
/// rendering and a difference image in `dir` as `<name>.png` and `<name>.diff.png` when it
/// fails. The outcome is whether it passes, and the comparison; an error is a case that cannot
/// be rendered or compared, which fails.
fn render_and_compare(
    suite: &Path,
    case: &Case,
    loader: &Suite,
    resolver: &StandIn,
    dir: &Path,
    name: &str,
) -> Result<(bool, String), String> {
    let directory = &case.mapfile[..case.mapfile.find('/').unwrap()];
    let reference = suite.join(directory).join("expected").join(&case.result);
    let reference = std::fs::read(&reference)
        .map_err(|e| format!("its reference {}: {e}", reference.display()))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("its reference: {e}")))?;
    let rendering = render(case, loader, resolver)?;
    let comparison = metric::compare_and_keep(&rendering, &reference, &GEOMETRY, dir, name)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok((comparison.passes(), comparison.to_string()))
}

/// The rendering of `case`, the options of its `map2img` line applied to its style IR, its sources
/// bound as `forager render` binds them as far as a test can: the CLI's binding lives in its
/// binary, which a test cannot call. As there, each local layer the engine reads is bound to the
/// shapefile its `DATA` names, and a layer whose shapefile does not open is left unbound, so that
/// the engine skips it. Unlike there, the files are read through the suite's loader without the
/// CLI's limit on a data file, each layer opens its shapefile anew instead of sharing it, a
/// shapefile that does not open gives no diagnostic, and the projections resolve through the
/// stand-in table of the G-1 test.
fn render(case: &Case, loader: &Suite, resolver: &StandIn) -> Result<RgbaImage, String> {
    let path = ResourcePath::new(&case.mapfile).map_err(|e| e.to_string())?;
    let loaded = ready(forager_mapfile::load(&path, loader, Some(resolver)));
    let document = loaded.document.ok_or_else(|| {
        let errors: Vec<_> = loaded.diagnostics.iter().map(ToString::to_string).collect();
        format!("it does not load: {}", errors.join("; "))
    })?;
    let mut map = document.map;
    conformance::apply(&case.options()?, &mut map, &loaded.diagnostics)?;
    let mut sources = BTreeMap::new();
    for layer in &map.layers {
        let (Some(name), Some(source)) = (&layer.name, &layer.source) else {
            continue;
        };
        if forager_engine::reads_features(layer)
            && source.kind == "local"
            && let Some(data) = source.props.get("data")
            && let Ok(data) = ResourcePath::new(data)
            && let Ok(shapefile) = ready(ShapefileSource::open(loader, &data))
        {
            sources.insert(name.clone(), shapefile);
        }
    }
    let rendered = ready(forager_engine::render(&map, &sources, AggRenderer::new()));
    rendered.image.ok_or_else(|| {
        let errors: Vec<_> = rendered
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect();
        format!("it renders no image: {}", errors.join("; "))
    })
}
