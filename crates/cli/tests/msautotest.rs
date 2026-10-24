//! G-1 over the conformance suite: every mapfile of msautotest at the pinned MapServer release
//! loads through the CLI, with the suite's directory as the base context, without a diagnostic of
//! severity error. `scripts/check-rust.sh` fetches the suite with `scripts/fetch-msautotest.sh`,
//! which holds the pin, and runs this test with `FORAGER_MSAUTOTEST` naming it; `cargo test`
//! alone skips it, as it needs the network once.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// The EPSG codes the suite names beyond those built into the resolver (ADR-0018). Their
/// definitions here are stand-ins, so that the test carries no EPSG data: G-1 decides that a
/// mapfile loads; whether a projection is right is G-11's.
const EPSG_CODES: &[u32] = &[2157, 3347, 3395, 3978, 21781, 25833, 26711, 26915, 27700];

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

#[test]
#[ignore = "needs the msautotest suite; scripts/check-rust.sh fetches it and runs this test"]
fn every_msautotest_mapfile_loads_without_an_error() {
    // Verifies: G-1
    let suite = PathBuf::from(
        std::env::var("FORAGER_MSAUTOTEST")
            .expect("FORAGER_MSAUTOTEST names the suite; scripts/check-rust.sh sets it"),
    );
    let table =
        std::env::temp_dir().join(format!("forager-msautotest-{}.epsg", std::process::id()));
    let lines: String = EPSG_CODES
        .iter()
        .map(|code| format!("<{code}> +proj=longlat +datum=WGS84 +no_defs <>\n"))
        .collect();
    std::fs::write(&table, lines).unwrap();

    let mut found = Vec::new();
    mapfiles(&suite, &mut found);
    let mut skipped = 0;
    let mut loaded = 0;
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
        if output.status.success() {
            loaded += 1;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let errors: Vec<&str> = stderr.lines().filter(|l| l.contains(" error ")).collect();
            failures.push(format!("{name}\n    {}", errors.join("\n    ")));
        }
    }
    std::fs::remove_file(&table).unwrap();

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
    eprintln!("{loaded} mapfiles loaded, {skipped} skipped");
}
