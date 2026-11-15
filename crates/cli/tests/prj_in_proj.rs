//! The shapefile adapter's `.prj` reader and the core crate `proj` held to one reach
//! (ADR-0039): every proj4 string the reader writes, for each projection it reads, parses in
//! `proj`. The adapter may not depend on `proj` (ADR-0008); the CLI reaches both.

use forager_adapter_shapefile::declared_projection_of_prj;
use forager_ports::DeclaredProjection;
use forager_proj::Projection;

const GCS_WGS84: &str = r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#;
const GCS_NAD83: &str = r#"GEOGCS["GCS_North_American_1983",DATUM["D_North_American_1983",SPHEROID["GRS_1980",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#;

/// A PROJCS of `method` on `geogcs` with `parameters`.
fn projcs(geogcs: &str, method: &str, parameters: &[(&str, f64)]) -> String {
    let parameters: String = parameters
        .iter()
        .map(|(name, value)| format!(r#",PARAMETER["{name}",{value:?}]"#))
        .collect();
    format!(r#"PROJCS["p",{geogcs},PROJECTION["{method}"]{parameters},UNIT["Meter",1.0]]"#)
}

/// The projection names the reader's `method_proj4` matches, read from its source.
fn methods() -> Vec<String> {
    let source = include_str!("../../adapter-shapefile/src/prj.rs");
    let body = source
        .split("fn method_proj4")
        .nth(1)
        .expect("prj.rs has method_proj4");
    let body = body.split("\n}\n").next().unwrap_or_default();
    body.lines()
        .map(str::trim)
        .filter(|l| l.starts_with('"') && l.contains("=>"))
        .flat_map(|l| {
            l.split("=>")
                .next()
                .unwrap_or_default()
                .split('|')
                .map(|name| name.trim().trim_matches('"').to_owned())
        })
        .collect()
}

/// A `.prj` for each branch of the reader's `method_proj4`, named by the projection it matches.
fn fixtures() -> Vec<(&'static str, String)> {
    let utm = [
        ("False_Easting", 500_000.0),
        ("Central_Meridian", 9.0),
        ("Scale_Factor", 0.9996),
    ];
    let conic = [
        ("Central_Meridian", -95.0),
        ("Standard_Parallel_1", 49.0),
        ("Standard_Parallel_2", 77.0),
        ("Latitude_Of_Origin", 49.0),
    ];
    let scaled = [
        ("Central_Meridian", -84.0),
        ("Standard_Parallel_1", 44.0),
        ("Standard_Parallel_2", 45.7),
        ("Scale_Factor", 1.000_038_2),
        ("Latitude_Of_Origin", 43.0),
    ];
    let one = [
        ("Central_Meridian", -95.0),
        ("Scale_Factor", 0.9),
        ("Latitude_Of_Origin", 49.0),
    ];
    let fixtures: Vec<(&str, String)> = vec![
        ("geographic WGS84", GCS_WGS84.to_owned()),
        ("geographic NAD83", GCS_NAD83.to_owned()),
        (
            "transverse_mercator",
            projcs(GCS_WGS84, "Transverse_Mercator", &utm),
        ),
        (
            "transverse_mercator",
            projcs(
                GCS_WGS84,
                "Transverse_Mercator",
                &[("False_Northing", 10_000_000.0), utm[0], utm[1], utm[2]],
            ),
        ),
        (
            "transverse_mercator",
            projcs(
                GCS_NAD83,
                "Transverse_Mercator",
                &[("Central_Meridian", 10.5)],
            ),
        ),
        (
            "lambert_conformal_conic",
            projcs(GCS_NAD83, "Lambert_Conformal_Conic", &conic),
        ),
        (
            "lambert_conformal_conic",
            projcs(GCS_NAD83, "Lambert_Conformal_Conic", &scaled),
        ),
        (
            "lambert_conformal_conic",
            projcs(
                GCS_NAD83,
                "Lambert_Conformal_Conic",
                &[
                    one[0],
                    ("Standard_Parallel_1", 49.0),
                    ("Standard_Parallel_2", 49.0),
                    one[1],
                    one[2],
                ],
            ),
        ),
        (
            "lambert_conformal_conic_2sp",
            projcs(GCS_NAD83, "Lambert_Conformal_Conic_2SP", &conic),
        ),
        (
            "lambert_conformal_conic_1sp",
            projcs(GCS_NAD83, "Lambert_Conformal_Conic_1SP", &one),
        ),
        (
            "mercator",
            projcs(GCS_WGS84, "Mercator", &[("Standard_Parallel_1", 10.0)]),
        ),
        (
            "mercator_2sp",
            projcs(GCS_WGS84, "Mercator_2SP", &[("Standard_Parallel_1", 0.0)]),
        ),
        (
            "mercator_1sp",
            projcs(GCS_WGS84, "Mercator_1SP", &[("Scale_Factor", 1.0)]),
        ),
    ];
    fixtures
}

#[test]
fn every_proj4_string_the_prj_reader_writes_parses_in_proj() {
    // Verifies: ADR-0039
    let fixtures = fixtures();
    for (name, wkt) in &fixtures {
        let DeclaredProjection::Proj4(proj4) = declared_projection_of_prj("a.prj", wkt.as_bytes())
        else {
            panic!("{name}: the reader does not read {wkt}");
        };
        if let Err(error) = Projection::parse(&proj4) {
            panic!("{name}: proj does not read {proj4:?}: {error}");
        }
    }
    // Each projection the reader matches has a fixture here.
    let methods = methods();
    assert!(!methods.is_empty(), "the names of method_proj4 are read");
    for method in methods {
        assert!(
            fixtures.iter().any(|(name, _)| *name == method),
            "no fixture for the projection {method} the .prj reader writes"
        );
    }
}
