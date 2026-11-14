//! G-11: the core's reprojection agrees with PROJ, as MapServer at the pinned build calls it, over
//! the reference points of `tests/reference/points.tsv`. The record `tests/reference/mapserver.tsv`
//! is the output of the reader `crates/cli/tests/conformance/mapserver_proj.c` in the noise run,
//! taken from its artifact (ADR-draft-proj-references-from-mapserver); this test needs neither PROJ
//! nor MapServer.

use forager_proj::{Projection, Reprojection};
use forager_style_ir::Coord;

const POINTS: &str = include_str!("reference/points.tsv");
const RECORD_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/mapserver.tsv");

/// The tolerance of G-11, in metres, as the specification states it: 1 mm, set by the maintainer
/// from the spread this test reports (1.9 nm over the reference points at the time).
const TOLERANCE_METRES: f64 = 1e-3;

/// WGS84's semimajor axis and squared eccentricity, by which a difference in degrees is measured
/// in metres.
const A: f64 = 6_378_137.0;
const ES: f64 = 0.006_694_379_990_141_3;

/// Whether `proj4` is longitude and latitude.
fn geographic(proj4: &str) -> bool {
    proj4.split_whitespace().any(|p| {
        matches!(
            p,
            "+proj=longlat" | "+proj=latlong" | "+proj=lonlat" | "+proj=latlon"
        )
    })
}

/// The distance in metres between `a` and `b`, in a projection in metres, or in degrees of
/// longitude and latitude, measured by the radii of curvature of WGS84 at their latitude.
fn metres(a: Coord, b: Coord, in_degrees: bool) -> f64 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    if !in_degrees {
        return (dx * dx + dy * dy).sqrt();
    }
    let phi = a.y.to_radians();
    let w = (1.0 - ES * phi.sin() * phi.sin()).sqrt();
    let meridian = A * (1.0 - ES) / (w * w * w);
    let normal = A / w;
    let north = dy.to_radians() * meridian;
    let east = dx.to_radians() * normal * phi.cos();
    (north * north + east * east).sqrt()
}

/// The points of the list: the line, its source and target, and the point.
fn points() -> Vec<(String, String, String, Coord)> {
    POINTS
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(
                f.len(),
                4,
                "a line of points.tsv holds four fields: {line:?}"
            );
            let at = Coord {
                x: f[2].parse().unwrap(),
                y: f[3].parse().unwrap(),
            };
            (line.to_owned(), f[0].to_owned(), f[1].to_owned(), at)
        })
        .collect()
}

#[expect(
    clippy::print_stdout,
    reason = "the measured spread of G-11, which the maintainer sets the tolerance from"
)]
fn report(text: &str) {
    println!("{text}");
}

#[test]
fn the_core_s_reprojection_agrees_with_mapserver_s_call_of_proj_within_g_11() {
    // Verifies: G-11
    // Verifies: ADR-draft-proj-references-from-mapserver
    let record = std::fs::read_to_string(RECORD_PATH).unwrap_or_else(|e| {
        panic!(
            "the record {RECORD_PATH} ({e}) is taken from the noise run's artifact, noise/proj/ \
             (ADR-draft-proj-references-from-mapserver)"
        )
    });
    let mut lines = record.lines();
    let proj = lines.next().unwrap_or_default();
    let mapserver = lines.next().unwrap_or_default();
    assert!(
        proj.starts_with("# PROJ ") && mapserver.starts_with("# MapServer "),
        "the record names the PROJ release and the MapServer build first: {proj:?} {mapserver:?}"
    );
    let recorded: Vec<&str> = lines.collect();
    let points = points();
    assert_eq!(
        recorded.len(),
        points.len(),
        "the record holds a line for each point of the list"
    );
    let mut outside = Vec::new();
    let mut spread: f64 = 0.0;
    for ((line, source, target, at), recorded) in points.iter().zip(&recorded) {
        let Some(result) = recorded
            .strip_prefix(line.as_str())
            .and_then(|r| r.strip_prefix('\t'))
        else {
            panic!("the record's line {recorded:?} is not that of the point {line:?}");
        };
        let way = Reprojection::new(
            Projection::parse(source).unwrap(),
            Projection::parse(target).unwrap(),
        );
        let ours = way.point(*at);
        match (result, ours) {
            ("fail", Err(_)) => {}
            ("fail", Ok(p)) => outside.push(format!("{line}: MapServer fails, proj gives {p:?}")),
            (xy, ours) => {
                let (x, y) = xy.split_once('\t').expect("x and y");
                let theirs = Coord {
                    x: x.parse().unwrap(),
                    y: y.parse().unwrap(),
                };
                match ours {
                    Err(_) => {
                        outside.push(format!("{line}: proj fails, MapServer gives {theirs:?}"))
                    }
                    Ok(p) => {
                        let d = metres(theirs, p, geographic(target));
                        spread = spread.max(d);
                        if d.is_nan() || d > TOLERANCE_METRES {
                            outside.push(format!("{line}: {d} m from MapServer's {theirs:?}"));
                        }
                    }
                }
            }
        }
    }
    report(&format!(
        "G-11: the largest distance of {} points from MapServer's PROJ ({}, {}) is {spread:e} m",
        points.len(),
        proj.trim_start_matches("# "),
        mapserver.trim_start_matches("# ")
    ));
    assert!(
        outside.is_empty(),
        "{} of {} points lie outside G-11's tolerance of {TOLERANCE_METRES} m:\n{}",
        outside.len(),
        points.len(),
        outside.join("\n")
    );
}
