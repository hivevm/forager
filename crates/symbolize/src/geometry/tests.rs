//! The expected coordinates are GEOS 3.12.1's, the runner's `libgeos-dev` the noise run builds
//! MapServer against: `GEOSSimplify_r` called on the same coordinates, as `msGEOSShape2Geometry`
//! builds them, and printed with 17 significant digits (taken by hand from a local build of the
//! release tarball, 2026-11-15). The lines are those of msautotest's `misc/data/testlines.shp`.

use super::*;

fn c(x: f64, y: f64) -> Coord {
    Coord { x, y }
}

fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
    points.iter().map(|&(x, y)| c(x, y)).collect()
}

fn bits(points: &[Coord]) -> Vec<(u64, u64)> {
    points
        .iter()
        .map(|p| (p.x.to_bits(), p.y.to_bits()))
        .collect()
}

fn area(shell: &[(f64, f64)], holes: &[&[(f64, f64)]]) -> Area {
    Area {
        shell: coords(shell),
        holes: holes.iter().map(|h| coords(h)).collect(),
    }
}

/// A budget that records each charge, and refuses the first past `limit` visits.
#[derive(Default)]
struct Recorder {
    visits: Vec<usize>,
    added: Vec<usize>,
    limit: Option<usize>,
}

impl Charge for Recorder {
    type Error = ();

    fn visit(&mut self, points: usize) -> Result<(), ()> {
        self.visits.push(points);
        match self.limit {
            Some(limit) if self.visits.iter().sum::<usize>() > limit => Err(()),
            _ => Ok(()),
        }
    }

    fn add(&mut self, points: usize) -> Result<(), ()> {
        self.added.push(points);
        Ok(())
    }
}

fn line(points: &[(f64, f64)], tolerance: f64) -> Vec<Coord> {
    simplify_line(&coords(points), tolerance, &mut Recorder::default()).unwrap()
}

fn areas(areas: &[Area], tolerance: f64) -> Vec<Area> {
    simplify_areas(areas, tolerance, &mut Recorder::default()).unwrap()
}

/// The three lines of `testlines.shp`, in map units.
const TESTLINES: [&[(f64, f64)]; 3] = [
    &[
        (-28.106284293591287, 32.95354302414644),
        (-48.43848871542007, 53.88375345838195),
        (-145.91346873771687, 43.11964523506083),
        (-166.24567315954565, -30.435094290966816),
        (-113.02313805534679, -53.15932276242251),
        (-18.538188095083626, -22.06301011727261),
        (-23.920242206744188, 23.38544682563878),
    ],
    &[
        (31.096310934674868, 15.611368664351305),
        (56.81056946816421, 1.2592243665898124),
        (68.77068971629879, -14.288931955985138),
        (82.52482800165355, -29.83708827856009),
        (100.46500837385541, -33.42512435300046),
        (113.62114064680345, -44.78723858872831),
        (116.6111707088371, -47.179262638355226),
        (159.06959758971485, -55.55134681204943),
        (162.05962765174849, -34.02313036540719),
        (174.01974789988307, -16.082949993205325),
    ],
    &[
        (-134.02785089976587, -25.98630703753902),
        (-58.852932293612994, 28.275288347353268),
        (-41.89618373583416, 18.10123921268596),
    ],
];

/// The same lines in the pixels of `renderers/geomtransform-vertex.map`, 500 by 500 over its
/// extent as msAdjustExtent widens it: x from -170.245673, y down from 173.79891349999997, by a
/// cellsize of 0.6999307034068136.
const TESTLINES_PX: [&[(f64, f64)]; 3] = [
    &[
        (203.07637315317842, 201.22759266068576),
        (174.02749113833798, 171.32433176305588),
        (34.76373324366195, 186.70315165326554),
        (5.714851228821556, 291.7917542363646),
        (81.75457179708025, 324.25815178236275),
        (216.74643527780918, 279.8304498773127),
        (209.05702533270437, 214.8976547853165),
    ],
    &[
        (287.65988254933137, 226.00458026157898),
        (324.39817450927654, 246.50967344852518),
        (341.48575216506504, 268.7235244010502),
        (361.1364664692218, 290.9373753535752),
        (386.7678329529045, 296.06364865031173),
        (405.5641683742718, 312.2968474233108),
        (409.83606278821895, 315.71436295446847),
        (470.49696346626797, 327.6756673135204),
        (474.76885788021514, 296.91802753310117),
        (491.85643553600363, 271.28666104941846),
    ],
    &[
        (51.74486834760787, 285.4357146573406),
        (159.14824162477603, 207.9114752993847),
        (183.37456642413727, 222.44727017900146),
    ],
];

#[test]
fn the_case_s_lines_keep_geos_s_points_in_map_units() {
    // Verifies: ADR-0041
    // The layer vertex2 of renderers/geomtransform-vertex.map: simplify([shape],25).
    let expected: [&[(f64, f64)]; 3] = [
        &[
            (-28.106284293591287, 32.95354302414644),
            (-145.91346873771687, 43.11964523506083),
            (-166.24567315954565, -30.435094290966816),
            (-113.02313805534679, -53.15932276242251),
            (-18.538188095083626, -22.06301011727261),
            (-23.920242206744188, 23.38544682563878),
        ],
        &[
            (31.096310934674868, 15.611368664351305),
            (116.6111707088371, -47.179262638355226),
            (159.06959758971485, -55.55134681204943),
            (174.01974789988307, -16.082949993205325),
        ],
        &[
            (-134.02785089976587, -25.98630703753902),
            (-41.89618373583416, 18.10123921268596),
        ],
    ];
    for (input, expected) in TESTLINES.iter().zip(expected) {
        assert_eq!(bits(&line(input, 25.0)), bits(&coords(expected)));
    }
}

#[test]
fn the_case_s_lines_keep_geos_s_points_in_pixels() {
    // Verifies: ADR-0041
    // The style simplify([shape],10) of renderers/geomtransform-vertex.map, on the lines in pixels.
    let expected: [&[(f64, f64)]; 3] = [
        &[
            (203.07637315317842, 201.22759266068576),
            (174.02749113833798, 171.32433176305588),
            (34.76373324366195, 186.70315165326554),
            (5.714851228821556, 291.7917542363646),
            (81.75457179708025, 324.25815178236275),
            (216.74643527780918, 279.8304498773127),
            (209.05702533270437, 214.8976547853165),
        ],
        &[
            (287.65988254933137, 226.00458026157898),
            (409.83606278821895, 315.71436295446847),
            (470.49696346626797, 327.6756673135204),
            (491.85643553600363, 271.28666104941846),
        ],
        &[
            (51.74486834760787, 285.4357146573406),
            (159.14824162477603, 207.9114752993847),
            (183.37456642413727, 222.44727017900146),
        ],
    ];
    for (input, expected) in TESTLINES_PX.iter().zip(expected) {
        assert_eq!(bits(&line(input, 10.0)), bits(&coords(expected)));
    }
}

#[test]
fn a_ring_loses_its_first_point_where_it_lies_within_the_tolerance_of_its_neighbours() {
    // Verifies: ADR-0041
    // GEOS: POLYGON ((0 0, 0 10.5, 10.25 10.75, 10.5 0.25, 0 0)) at 2.
    let input = area(
        &[
            (5.25, 0.5),
            (0.0, 0.0),
            (0.0, 10.5),
            (10.25, 10.75),
            (10.5, 0.25),
            (5.25, 0.5),
        ],
        &[],
    );
    let expected = area(
        &[
            (0.0, 0.0),
            (0.0, 10.5),
            (10.25, 10.75),
            (10.5, 0.25),
            (0.0, 0.0),
        ],
        &[],
    );
    assert_eq!(areas(&[input], 2.0), [expected]);
}

#[test]
fn a_hole_that_collapses_is_removed() {
    // Verifies: ADR-0041
    // GEOS: POLYGON ((0 0, 0 100, 100 100, 100 0, 0 0)) at 2.
    let input = area(
        &[
            (0.0, 0.0),
            (0.0, 100.0),
            (100.0, 100.0),
            (100.0, 0.0),
            (0.0, 0.0),
        ],
        &[&[(40.5, 40.0), (41.25, 40.5), (41.0, 41.5), (40.5, 40.0)]],
    );
    let expected = area(
        &[
            (0.0, 0.0),
            (0.0, 100.0),
            (100.0, 100.0),
            (100.0, 0.0),
            (0.0, 0.0),
        ],
        &[],
    );
    assert_eq!(areas(&[input], 2.0), [expected]);
}

#[test]
fn a_shell_that_collapses_leaves_no_polygon() {
    // Verifies: ADR-0041
    // GEOS: POLYGON EMPTY at 2, the hole removed with its shell.
    let input = area(
        &[
            (0.0, 0.0),
            (50.0, 1.5),
            (100.0, 0.0),
            (50.0, -1.25),
            (0.0, 0.0),
        ],
        &[&[(20.0, 0.25), (30.0, 0.5), (40.0, 0.25), (20.0, 0.25)]],
    );
    assert_eq!(areas(&[input], 2.0), []);
}

#[test]
fn a_valid_result_is_kept_as_it_is() {
    // Verifies: ADR-0041
    // GEOS: POLYGON ((0 0, 0 100, 100 99.75, 100 0, 0 0), (20 20, 40 20, 40 40, 20 40, 20 20)) at
    // 2: the shell's and the hole's slight bends are dropped, and nothing is repaired.
    let input = area(
        &[
            (0.0, 0.0),
            (0.0, 100.0),
            (30.0, 100.5),
            (60.0, 100.0),
            (100.0, 99.75),
            (100.0, 0.0),
            (0.0, 0.0),
        ],
        &[&[
            (20.0, 20.0),
            (40.0, 20.0),
            (40.0, 40.0),
            (30.0, 40.5),
            (20.0, 40.0),
            (20.0, 20.0),
        ]],
    );
    let expected = area(
        &[
            (0.0, 0.0),
            (0.0, 100.0),
            (100.0, 99.75),
            (100.0, 0.0),
            (0.0, 0.0),
        ],
        &[&[
            (20.0, 20.0),
            (40.0, 20.0),
            (40.0, 40.0),
            (20.0, 40.0),
            (20.0, 20.0),
        ]],
    );
    assert_eq!(areas(&[input], 2.0), [expected]);
}

#[test]
fn an_invalid_result_is_repaired_within_a_millionth_of_geos_s() {
    // Verifies: ADR-0041
    // At 10 the ring keeps 51.46 30.09, 26.46 46.85, 73.13 65.88, and 59.31 62.82, whose last
    // edge crosses its first: GEOS's buffer of 0 gives POLYGON ((51.460000000000001 30.09,
    // 26.460000000000001 46.850000000000001, 58.625411520418055 59.965658479399089,
    // 51.460000000000001 30.09)), the crossing for a vertex.
    let input = area(
        &[
            (51.46, 30.09),
            (38.76, 34.05),
            (26.46, 46.85),
            (53.93, 64.53),
            (73.13, 65.88),
            (59.31, 62.82),
            (51.46, 30.09),
        ],
        &[],
    );
    let geos = coords(&[
        (51.46, 30.09),
        (26.46, 46.85),
        (58.625411520418055, 59.96565847939909),
    ]);
    let repaired = areas(&[input], 10.0);
    let [Area { shell, holes }] = repaired.as_slice() else {
        panic!("one polygon: {repaired:?}");
    };
    assert!(holes.is_empty());
    assert_eq!(shell.len(), geos.len() + 1, "{shell:?}");
    assert_eq!(shell.first(), shell.last());
    for vertex in &shell[1..] {
        let nearest = geos
            .iter()
            .map(|g| ((g.x - vertex.x).powi(2) + (g.y - vertex.y).powi(2)).sqrt())
            .fold(f64::INFINITY, |a, b| if b < a { b } else { a });
        assert!(nearest < 1e-6, "{vertex:?} is {nearest} from GEOS's");
    }
}

#[test]
fn a_tolerance_of_zero_drops_only_points_on_their_segment() {
    // GEOS's Douglas-Peucker drops a point whose distance is not above 0, where geo's keeps the
    // line as it is: the collinear (5, 0) goes, (15, 1) stays. GEOS: LINESTRING (0 0, 10 0, 15 1,
    // 20 0).
    let input = [
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (15.0, 1.0),
        (20.0, 0.0),
    ];
    assert_eq!(
        line(&input, 0.0),
        coords(&[(0.0, 0.0), (10.0, 0.0), (15.0, 1.0), (20.0, 0.0)])
    );
}

#[test]
fn a_tolerance_that_is_no_number_keeps_every_point() {
    // GEOS compares each distance with the tolerance, which is false for a NaN, so it splits every
    // section and keeps each point, where geo's would keep the end points alone. GEOS: LINESTRING
    // (0 0, 5 0, 10 0).
    let input = [(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)];
    assert_eq!(line(&input, f64::NAN), coords(&input));
}

#[test]
fn each_section_is_charged_before_it_is_scanned() {
    // Verifies: Q-9, ADR-0041
    // A line of 5 points at 0.5: the whole line's 3 inner points, split at (1, 1), the first of
    // two farthest; then (1, 1) to (4, 0), whose 2 inner points lie equally far, split at the
    // first; then (2, 0) to (4, 0), its 1 inner point. Every point is kept.
    let input = coords(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0), (4.0, 0.0)]);
    let mut recorder = Recorder::default();
    assert_eq!(simplify_line(&input, 0.5, &mut recorder), Ok(input.clone()));
    assert_eq!(recorder.visits, [3, 2, 1]);
    // A budget that refuses the first section stops the scan before it.
    let mut refusing = Recorder {
        limit: Some(2),
        ..Recorder::default()
    };
    assert_eq!(simplify_line(&input, 0.5, &mut refusing), Err(()));
    assert_eq!(refusing.visits, [3]);
}

#[test]
fn equally_far_points_split_at_the_first_as_geos_splits() {
    // Verifies: ADR-0041
    // (1, 1) and (2, 1) lie 1 from the segment between the ends: GEOS splits at the first and
    // keeps it, LINESTRING (0 0, 1 1, 3 0) at 0.5, where geo's Simplify keeps (2, 1).
    let input = [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0)];
    assert_eq!(
        line(&input, 0.5),
        coords(&[(0.0, 0.0), (1.0, 1.0), (3.0, 0.0)])
    );
    // A ring on an integer grid: GEOS gives POLYGON ((9 3, 2 2, 3 7, 5 9, 9 3)) at 2, where the
    // rule of geo's Simplify drops (3, 7).
    let ring = area(
        &[
            (9.0, 3.0),
            (7.0, 2.0),
            (2.0, 2.0),
            (3.0, 7.0),
            (5.0, 9.0),
            (9.0, 3.0),
        ],
        &[],
    );
    let expected = area(
        &[(9.0, 3.0), (2.0, 2.0), (3.0, 7.0), (5.0, 9.0), (9.0, 3.0)],
        &[],
    );
    assert_eq!(areas(&[ring], 2.0), [expected]);
}

/// A zigzag of `n` points whose amplitude shrinks towards its end, so that each section's farthest
/// point lies beside its first end, and a Douglas-Peucker keeps every point, one section deeper
/// for each.
fn zigzag(n: u32) -> Vec<Coord> {
    (0..n)
        .map(|i| {
            let amplitude = f64::from(n - i);
            c(
                f64::from(i),
                if i % 2 == 0 { amplitude } else { -amplitude },
            )
        })
        .collect()
}

#[test]
fn a_zigzag_that_keeps_every_point_needs_no_recursion() {
    // Verifies: ADR-0041
    // geo's recursive Simplify overflowed a stack of 1 MiB at 4,500 such points. The sections
    // shrink by one point each, so 10,000 points visit 49,985,001 inner points in all.
    let input = zigzag(10_000);
    let mut recorder = Recorder::default();
    assert_eq!(simplify_line(&input, 0.5, &mut recorder), Ok(input));
    assert_eq!(recorder.visits.iter().sum::<usize>(), 49_985_001);
}

#[test]
fn the_points_a_repair_adds_are_charged_as_added() {
    // Verifies: Q-9, ADR-0041
    // Two bars that cross, a multipolygon GEOS finds invalid: the union's outline is a cross of 12
    // corners and its closing point, 13 points from the bars' 10, so 3 are added.
    let rectangle = |x0: f64, y0: f64, x1: f64, y1: f64| {
        area(&[(x0, y0), (x0, y1), (x1, y1), (x1, y0), (x0, y0)], &[])
    };
    let bars = [
        rectangle(0.0, 4.0, 20.0, 6.0),
        rectangle(9.0, 0.0, 11.0, 10.0),
    ];
    let mut recorder = Recorder::default();
    let repaired = simplify_areas(&bars, 0.1, &mut recorder).unwrap();
    assert_eq!(repaired.len(), 1, "{repaired:?}");
    assert_eq!(repaired[0].shell.len(), 13, "{repaired:?}");
    assert_eq!(recorder.added, [3]);
}
