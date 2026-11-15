//! G-11: the core's reprojection agrees with PROJ, as MapServer at the pinned build calls it, over
//! the reference points of `tests/reference/points.tsv`. The record `tests/reference/mapserver.tsv`
//! is the output of the reader `crates/cli/tests/conformance/mapserver_proj.c` in the noise run,
//! taken from its artifact (ADR-0038); this test needs neither PROJ
//! nor MapServer. Beside it, the rectangles of `tests/reference/rects.tsv` and their record
//! `tests/reference/mapserver-rects.tsv`, by the same reader through `msProjectRect`, hold the area
//! asked of a source to MapServer's within the same tolerance.

use forager_proj::{Projection, Reprojection};
use forager_style_ir::{Coord, Extent};

const POINTS: &str = include_str!("reference/points.tsv");
const RECORD_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/mapserver.tsv");
const RECTS: &str = include_str!("reference/rects.tsv");
const RECTS_RECORD_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/reference/mapserver-rects.tsv"
);

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

/// The lines of a list, `list`, each split into its source, its target, and its `numbers`
/// numbers.
fn lines(list: &str, numbers: usize) -> Vec<(String, String, String, Vec<f64>)> {
    list.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(
                f.len(),
                2 + numbers,
                "a line of the list holds {} fields: {line:?}",
                2 + numbers
            );
            let numbers = f[2..].iter().map(|n| n.parse().unwrap()).collect();
            (line.to_owned(), f[0].to_owned(), f[1].to_owned(), numbers)
        })
        .collect()
}

/// The points of the list: the line, its source and target, and the point.
fn points() -> Vec<(String, String, String, Coord)> {
    lines(POINTS, 2)
        .into_iter()
        .map(|(line, source, target, n)| (line, source, target, Coord { x: n[0], y: n[1] }))
        .collect()
}

/// A record, read from `path`: its two header lines, and its lines after them.
fn record(path: &str) -> (String, String, Vec<String>) {
    let record = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "the record {path} ({e}) is taken from the noise run's artifact, noise/proj/ \
             (ADR-0038)"
        )
    });
    let mut lines = record.lines().map(str::to_owned);
    let proj = lines.next().unwrap_or_default();
    let mapserver = lines.next().unwrap_or_default();
    assert!(
        proj.starts_with("# PROJ ") && mapserver.starts_with("# MapServer "),
        "the record names the PROJ release and the MapServer build first: {proj:?} {mapserver:?}"
    );
    (proj, mapserver, lines.collect())
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
    // Verifies: ADR-0038
    let (proj, mapserver, recorded) = record(RECORD_PATH);
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

#[test]
fn the_area_asked_of_a_source_is_mapserver_s_rectangle_within_g_11() {
    // Verifies: G-11
    // Verifies: ADR-0038
    let (proj, mapserver, recorded) = record(RECTS_RECORD_PATH);
    let rects = lines(RECTS, 4);
    assert_eq!(
        recorded.len(),
        rects.len(),
        "the record holds a line for each rectangle of the list"
    );
    let mut outside = Vec::new();
    let mut spread: f64 = 0.0;
    for ((line, source, target, n), recorded) in rects.iter().zip(&recorded) {
        let Some(result) = recorded
            .strip_prefix(line.as_str())
            .and_then(|r| r.strip_prefix('\t'))
        else {
            panic!("the record's line {recorded:?} is not that of the rectangle {line:?}");
        };
        let way = Reprojection::new(
            Projection::parse(source).unwrap(),
            Projection::parse(target).unwrap(),
        );
        let ours = way.extent(&Extent {
            min_x: n[0],
            min_y: n[1],
            max_x: n[2],
            max_y: n[3],
        });
        match (result, ours) {
            ("fail", None) => {}
            ("fail", Some(r)) => outside.push(format!("{line}: MapServer fails, proj gives {r:?}")),
            (numbers, ours) => {
                let t: Vec<f64> = numbers.split('\t').map(|v| v.parse().unwrap()).collect();
                let Some(r) = ours else {
                    outside.push(format!("{line}: proj fails, MapServer gives {t:?}"));
                    continue;
                };
                let in_degrees = geographic(target);
                let corners = [
                    (
                        Coord { x: t[0], y: t[1] },
                        Coord {
                            x: r.min_x,
                            y: r.min_y,
                        },
                    ),
                    (
                        Coord { x: t[2], y: t[3] },
                        Coord {
                            x: r.max_x,
                            y: r.max_y,
                        },
                    ),
                ];
                for (theirs, ours) in corners {
                    let d = metres(theirs, ours, in_degrees);
                    spread = spread.max(d);
                    if d.is_nan() || d > TOLERANCE_METRES {
                        outside.push(format!(
                            "{line}: {ours:?} lies {d} m from MapServer's {theirs:?}"
                        ));
                    }
                }
            }
        }
    }
    report(&format!(
        "G-11: the largest distance of the corners of {} rectangles from MapServer's ({}, {}) is \
         {spread:e} m",
        rects.len(),
        proj.trim_start_matches("# "),
        mapserver.trim_start_matches("# ")
    ));
    assert!(
        outside.is_empty(),
        "{} corners of {} rectangles lie outside G-11's tolerance of {TOLERANCE_METRES} m:\n{}",
        outside.len(),
        rects.len(),
        outside.join("\n")
    );
}

const SHAPES: &str = include_str!("reference/shapes.tsv");
const SHAPES_RECORD_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/reference/mapserver-shapes.tsv"
);

/// The points of `text`, `x,y x,y ...`.
fn shape_points(text: &str) -> Vec<Coord> {
    text.split(' ')
        .map(|xy| {
            let (x, y) = xy.split_once(',').expect("x,y");
            Coord {
                x: x.parse().unwrap(),
                y: y.parse().unwrap(),
            }
        })
        .collect()
}

/// The lines a shape of `kind` is left with, as `msProjectShapeEx` leaves them: `None` where it
/// fails.
fn reprojected_shape(
    way: &Reprojection,
    kind: &str,
    lines: &[Vec<Coord>],
) -> Option<Vec<Vec<Coord>>> {
    if kind == "point" {
        let kept: Vec<Vec<Coord>> = lines
            .iter()
            .filter_map(|line| {
                let mut line = line.clone();
                way.point_line(&mut line).ok().map(|_| line)
            })
            .collect();
        return (!kept.is_empty()).then_some(kept);
    }
    Some(way.shape(lines, kind == "polygon").lines)
}

#[test]
fn the_core_cuts_and_wraps_shapes_as_mapserver_reprojects_them_within_g_11() {
    // Verifies: G-11
    // Verifies: ADR-0038
    let (proj, mapserver, recorded) = record(SHAPES_RECORD_PATH);
    let shapes: Vec<&str> = SHAPES
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(
        recorded.len(),
        shapes.len(),
        "the record holds a line for each shape of the list"
    );
    let mut outside = Vec::new();
    let mut spread: f64 = 0.0;
    for (line, recorded) in shapes.iter().zip(&recorded) {
        let Some(result) = recorded
            .strip_prefix(line)
            .and_then(|r| r.strip_prefix('\t'))
        else {
            panic!("the record's line {recorded:?} is not that of the shape {line:?}");
        };
        let f: Vec<&str> = line.split('\t').collect();
        let way = Reprojection::new(
            Projection::parse(f[0]).unwrap(),
            Projection::parse(f[1]).unwrap(),
        );
        let lines: Vec<Vec<Coord>> = f[3].split('|').map(shape_points).collect();
        let ours = reprojected_shape(&way, f[2], &lines);
        let theirs: Option<Vec<Vec<Coord>>> = (result != "fail").then(|| {
            result
                .split('\t')
                .map(|l| {
                    if l == "-" {
                        Vec::new()
                    } else {
                        shape_points(l)
                    }
                })
                .collect()
        });
        match (theirs, ours) {
            (None, None) => {}
            (theirs, ours) => {
                let (Some(theirs), Some(ours)) = (&theirs, &ours) else {
                    outside.push(format!("{line}: MapServer {theirs:?}, proj {ours:?}"));
                    continue;
                };
                let shapes_of = |s: &Vec<Vec<Coord>>| s.iter().map(Vec::len).collect::<Vec<_>>();
                if shapes_of(theirs) != shapes_of(ours) {
                    outside.push(format!(
                        "{line}: MapServer's lines hold {:?} points, proj's {:?}",
                        shapes_of(theirs),
                        shapes_of(ours)
                    ));
                    continue;
                }
                for (a, b) in theirs.iter().flatten().zip(ours.iter().flatten()) {
                    let d = metres(*a, *b, geographic(f[1]));
                    spread = spread.max(d);
                    if d.is_nan() || d > TOLERANCE_METRES {
                        outside.push(format!("{line}: {b:?} lies {d} m from MapServer's {a:?}"));
                    }
                }
            }
        }
    }
    report(&format!(
        "G-11: the largest distance of the points of {} shapes from MapServer's ({}, {}) is \
         {spread:e} m",
        shapes.len(),
        proj.trim_start_matches("# "),
        mapserver.trim_start_matches("# ")
    ));
    assert!(
        outside.is_empty(),
        "{} differences from MapServer's shapes:\n{}",
        outside.len(),
        outside.join("\n")
    );
}
