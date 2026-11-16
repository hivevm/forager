//! The expected coordinates marked GEOS's are GEOS 3.12.1's, the runner's `libgeos-dev` the noise
//! run builds MapServer against: `GEOSTopologyPreserveSimplify_r` called on the same coordinates,
//! as `msGEOSShape2Geometry` builds them, and printed with 17 significant digits (taken from a
//! local build of the release tarball, 2026-11-16). Where the port departs from GEOS 3.12.1, the
//! expected coordinates are those JTS's `master` decides, and GEOS's are named beside them.

use super::super::tests::{Recorder, TESTLINES_PX, bits, c, coords, zigzag};
use super::*;

/// `parts`, each a line or a ring, simplified together by `tolerance`.
fn simplified(parts: &[(&[(f64, f64)], bool)], tolerance: f64) -> Vec<Vec<Coord>> {
    let points: Vec<Vec<Coord>> = parts.iter().map(|(p, _)| coords(p)).collect();
    let lines: Vec<Line<'_>> = points
        .iter()
        .zip(parts)
        .map(|(points, &(_, ring))| Line { points, ring })
        .collect();
    simplify_preserving(&lines, tolerance, &mut Recorder::default()).unwrap()
}

fn assert_bits(ours: &[Vec<Coord>], expected: &[&[(f64, f64)]]) {
    assert_eq!(ours.len(), expected.len());
    for (ours, expected) in ours.iter().zip(expected) {
        assert_eq!(bits(ours), bits(&coords(expected)));
    }
}

#[test]
fn the_case_s_lines_keep_geos_s_points_in_pixels() {
    // Verifies: ADR-0047
    // The style simplifypt([shape],50) of renderers/geomtransform-vertex.map, each line a shape of
    // its own.
    let expected: [&[(f64, f64)]; 3] = [
        &[
            (203.07637315317842, 201.22759266068576),
            (34.76373324366195, 186.70315165326554),
            (5.714851228821556, 291.7917542363646),
            (216.74643527780918, 279.8304498773127),
            (209.05702533270437, 214.8976547853165),
        ],
        &[
            (287.65988254933137, 226.00458026157898),
            (409.83606278821895, 315.71436295446847),
            (491.85643553600363, 271.28666104941846),
        ],
        &[
            (51.74486834760787, 285.4357146573406),
            (183.37456642413727, 222.44727017900146),
        ],
    ];
    for (input, expected) in TESTLINES_PX.iter().zip(expected) {
        assert_bits(&simplified(&[(input, false)], 50.0), &[expected]);
    }
}

#[test]
fn the_polygons_of_world_testpoly_keep_geos_s_points_in_map_units() {
    // Verifies: ADR-0047
    // msautotest's misc/data/world_testpoly.shp at 20: the first polygon's ring loses its first
    // point; the fourth is a shell with a hole, simplified together.
    let polygons: [&[&[(f64, f64)]]; 4] = [
        &[&[
            (-9.67836147605436, -3.94253391242653),
            (-13.634936139536089, -14.681807999019796),
            (-5.1565618606466685, -19.768832566353446),
            (3.887037370168713, -8.46433352783422),
            (-9.67836147605436, -3.94253391242653),
        ]],
        &[&[
            (65.4965571300985, 15.840339404982117),
            (33.84395982224466, -13.551358095167872),
            (101.67095405336002, -12.42090819131595),
            (54.19205809157927, -3.94253391242653),
            (96.01870453410041, 8.49241502994462),
            (65.4965571300985, 15.840339404982117),
        ]],
        &[&[
            (-106.89705320731971, -11.855683239389988),
            (-91.63597950531876, -37.290806076058246),
            (-23.24376032227743, 1.1444906549071223),
            (-106.89705320731971, -11.855683239389988),
        ]],
        &[
            &[
                (-38.50483402427839, -34.46468131642844),
                (-125.54947662087643, -53.117104729985165),
                (-82.59238027450337, -69.50862833583804),
                (5.582712225946597, -48.030080162651515),
                (-38.50483402427839, -34.46468131642844),
            ],
            &[
                (-48.678883158945695, -44.638730451095746),
                (-43.59185859161204, -52.5518797780592),
                (-99.54912883228221, -55.94322948961497),
                (-48.678883158945695, -44.638730451095746),
            ],
        ],
    ];
    let expected: [&[&[(f64, f64)]]; 4] = [
        &[&[
            (3.887037370168713, -8.46433352783422),
            (-13.634936139536089, -14.681807999019796),
            (-5.1565618606466685, -19.768832566353446),
            (3.887037370168713, -8.46433352783422),
        ]],
        &[&[
            (65.4965571300985, 15.840339404982117),
            (33.84395982224466, -13.551358095167872),
            (101.67095405336002, -12.42090819131595),
            (54.19205809157927, -3.94253391242653),
            (96.01870453410041, 8.49241502994462),
            (65.4965571300985, 15.840339404982117),
        ]],
        &[&[
            (-106.89705320731971, -11.855683239389988),
            (-91.63597950531876, -37.290806076058246),
            (-23.24376032227743, 1.1444906549071223),
            (-106.89705320731971, -11.855683239389988),
        ]],
        &[
            &[
                (-38.50483402427839, -34.46468131642844),
                (-125.54947662087643, -53.117104729985165),
                (-82.59238027450337, -69.50862833583804),
                (5.582712225946597, -48.030080162651515),
                (-38.50483402427839, -34.46468131642844),
            ],
            &[
                (-48.678883158945695, -44.638730451095746),
                (-43.59185859161204, -52.5518797780592),
                (-99.54912883228221, -55.94322948961497),
                (-48.678883158945695, -44.638730451095746),
            ],
        ],
    ];
    for (rings, expected) in polygons.iter().zip(expected) {
        let parts: Vec<(&[(f64, f64)], bool)> = rings.iter().map(|ring| (*ring, true)).collect();
        assert_bits(&simplified(&parts, 20.0), expected);
    }
}

#[test]
fn a_flattening_that_would_cross_another_line_of_the_shape_is_refused() {
    // Verifies: ADR-0047
    // The whole first line lies within 20 of (0 0, 20 0), which would cross the second line: it
    // is split at (10 10), and each half flattens. GEOS: MULTILINESTRING ((0 0, 10 10, 20 0),
    // (10 5, 10 -5)).
    let first: &[(f64, f64)] = &[
        (0.0, 0.0),
        (5.0, 6.0),
        (10.0, 10.0),
        (15.0, 6.0),
        (20.0, 0.0),
    ];
    let second: &[(f64, f64)] = &[(10.0, 5.0), (10.0, -5.0)];
    assert_bits(
        &simplified(&[(first, false), (second, false)], 20.0),
        &[&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)], second],
    );
}

#[test]
fn a_closed_line_keeps_its_first_point_and_4_coordinates() {
    // Verifies: ADR-0047
    // A closed line keeps 4 coordinates however large the tolerance. GEOS: LINESTRING (0 0, 20 0,
    // 10 20, 0 0) at 50.
    let closed: &[(f64, f64)] = &[
        (0.0, 0.0),
        (10.0, 0.1),
        (20.0, 0.0),
        (10.0, 20.0),
        (0.0, 0.0),
    ];
    assert_bits(
        &simplified(&[(closed, false)], 50.0),
        &[&[(0.0, 0.0), (20.0, 0.0), (10.0, 20.0), (0.0, 0.0)]],
    );
    // A closed line keeps a first point that lies within the tolerance of its neighbours' segment;
    // a ring of a polygon loses it. GEOS: LINESTRING (10 0.1, 20 0, 10 20, 0 0, 10 0.1) and
    // POLYGON ((0 0, 20 0, 10 20, 0 0)) at 1.
    let first: &[(f64, f64)] = &[
        (10.0, 0.1),
        (20.0, 0.0),
        (10.0, 20.0),
        (0.0, 0.0),
        (10.0, 0.1),
    ];
    assert_bits(&simplified(&[(first, false)], 1.0), &[first]);
    assert_bits(
        &simplified(&[(first, true)], 1.0),
        &[&[(0.0, 0.0), (20.0, 0.0), (10.0, 20.0), (0.0, 0.0)]],
    );
}

#[test]
fn a_flattening_that_jumps_another_line_keeps_its_bend_as_jts_decides() {
    // Verifies: ADR-0047
    // Flattening the bend to (0 0, 20 0) would leave the short line on its other side, which
    // ComponentJumpChecker refuses. GEOS 3.12.1 lacks that test: MULTILINESTRING ((0 0, 20 0),
    // (9 4, 11 4)) at 20.
    let bend: &[(f64, f64)] = &[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)];
    let short: &[(f64, f64)] = &[(9.0, 4.0), (11.0, 4.0)];
    assert_bits(
        &simplified(&[(bend, false), (short, false)], 20.0),
        &[bend, short],
    );
}

#[test]
fn a_shell_keeps_the_first_point_whose_drop_would_leave_its_hole_outside() {
    // Verifies: ADR-0047
    // Dropping the shell's first point, (10 -2), would make (0 0, 20 0) its edge, and the hole
    // below it would lie outside the shell. GEOS 3.12.1 drops it without the jump test:
    // POLYGON ((0 0, 20 0, 20 20, 0 20, 0 0), (9 -1, 11 -1, 10 -0.5, 9 -1)) at 5.
    let shell: &[(f64, f64)] = &[
        (10.0, -2.0),
        (20.0, 0.0),
        (20.0, 20.0),
        (0.0, 20.0),
        (0.0, 0.0),
        (10.0, -2.0),
    ];
    let hole: &[(f64, f64)] = &[(9.0, -1.0), (11.0, -1.0), (10.0, -0.5), (9.0, -1.0)];
    assert_bits(
        &simplified(&[(shell, true), (hole, true)], 5.0),
        &[shell, hole],
    );
    // Without the hole, the same shell loses its first point.
    assert_bits(
        &simplified(&[(shell, true)], 5.0),
        &[&[
            (0.0, 0.0),
            (20.0, 0.0),
            (20.0, 20.0),
            (0.0, 20.0),
            (0.0, 0.0),
        ]],
    );
}

#[test]
fn a_section_whose_distances_are_no_number_is_kept_whole_without_a_hang() {
    // Verifies: ADR-0047
    // The inner point is no number, so the section's split point is its first end; the second
    // line crosses its flattening, so JTS and GEOS would split it at that end again without end.
    // The port keeps it whole.
    let nan: &[(f64, f64)] = &[(0.0, 0.0), (f64::NAN, f64::NAN), (10.0, 0.0)];
    let across: &[(f64, f64)] = &[(5.0, -1.0), (5.0, 1.0)];
    assert_bits(
        &simplified(&[(nan, false), (across, false)], 1.0),
        &[nan, across],
    );
    // A line of one point, which GEOS cannot build, is kept as it is, and no other line's jump
    // test compares it.
    let one: &[(f64, f64)] = &[(3.0, 3.0)];
    let bend: &[(f64, f64)] = &[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)];
    assert_bits(
        &simplified(&[(bend, false), (one, false)], 20.0),
        &[&[(0.0, 0.0), (20.0, 0.0)], one],
    );
    // A line of no numbers alone is flattened: no test refuses it.
    let none: &[(f64, f64)] = &[(f64::NAN, 0.0), (f64::NAN, f64::NAN), (0.0, f64::NAN)];
    assert_bits(
        &simplified(&[(none, false)], 1.0),
        &[&[(f64::NAN, 0.0), (0.0, f64::NAN)]],
    );
}

#[test]
fn the_scanned_points_examined_segments_and_compared_lines_are_charged_before_each_runs() {
    // Verifies: Q-9, ADR-0047
    // The bend (0 0, 5 1, 10 0) lies within 2 of its flattening: its 1 inner point is scanned,
    // the output index's 0 segments and the input index's 3 are examined, the 1 other line is
    // compared, and its point (6 0.5), inside the section's envelope, has its crossings counted
    // with the section's 2 segments and the flattening. The flattening jumps it and is refused;
    // each half is one segment, kept without a test.
    let bend: &[(f64, f64)] = &[(0.0, 0.0), (5.0, 1.0), (10.0, 0.0)];
    let short: &[(f64, f64)] = &[(4.0, 0.5), (6.0, 0.5)];
    let points = [coords(bend), coords(short)];
    let lines = [
        Line {
            points: &points[0],
            ring: false,
        },
        Line {
            points: &points[1],
            ring: false,
        },
    ];
    let mut recorder = Recorder::default();
    assert_eq!(
        simplify_preserving(&lines, 2.0, &mut recorder),
        Ok(points.to_vec())
    );
    assert_eq!(recorder.visits, [1, 0, 3, 1, 3]);
    assert!(recorder.added.is_empty());
    // Alone, the bend flattens: no other line to compare.
    let mut alone = Recorder::default();
    assert_eq!(
        simplify_preserving(&lines[..1], 2.0, &mut alone),
        Ok(vec![vec![c(0.0, 0.0), c(10.0, 0.0)]])
    );
    assert_eq!(alone.visits, [1, 0, 2]);
    // A budget that refuses the input index's segments stops before they are examined.
    let mut refusing = Recorder {
        limit: Some(3),
        ..Recorder::default()
    };
    assert_eq!(simplify_preserving(&lines, 2.0, &mut refusing), Err(()));
    assert_eq!(refusing.visits, [1, 0, 3]);
}

#[test]
fn a_zigzag_that_keeps_every_point_needs_no_recursion() {
    // Verifies: Q-9, ADR-0047
    // Every section lies farther than the tolerance, so none is tested against the indexes: the
    // 10,000 points scan 49,985,001 inner points in all, as the Douglas-Peucker of `simplify`.
    let input = zigzag(10_000);
    let mut recorder = Recorder::default();
    let lines = [Line {
        points: &input,
        ring: false,
    }];
    assert_eq!(
        simplify_preserving(&lines, 0.5, &mut recorder),
        Ok(vec![input.clone()])
    );
    assert_eq!(recorder.visits.iter().sum::<usize>(), 49_985_001);
}
