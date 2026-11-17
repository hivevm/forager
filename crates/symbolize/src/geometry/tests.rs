//! The expected coordinates are GEOS 3.12.1's, the runner's `libgeos-dev` the noise run builds
//! MapServer against: `GEOSSimplify_r` called on the same coordinates, as `msGEOSShape2Geometry`
//! builds them, and printed with 17 significant digits (taken by hand from a local build of the
//! release tarball, 2026-11-15). The lines are those of msautotest's `misc/data/testlines.shp`.

use super::*;

pub(super) fn c(x: f64, y: f64) -> Coord {
    Coord { x, y }
}

pub(super) fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
    points.iter().map(|&(x, y)| c(x, y)).collect()
}

pub(super) fn bits(points: &[Coord]) -> Vec<(u64, u64)> {
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
pub(super) struct Recorder {
    pub(super) visits: Vec<usize>,
    pub(super) added: Vec<usize>,
    pub(super) limit: Option<usize>,
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
pub(super) const TESTLINES: [&[(f64, f64)]; 3] = [
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
pub(super) const TESTLINES_PX: [&[(f64, f64)]; 3] = [
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
pub(super) fn zigzag(n: u32) -> Vec<Coord> {
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

/// The rings of `text`, `;` between rings, of points `x y` with `,` between them.
fn rings_of(text: &str) -> Vec<Vec<Coord>> {
    text.split(';')
        .map(|ring| {
            ring.split(',')
                .map(|point| {
                    let (x, y) = point.split_once(' ').unwrap();
                    c(x.parse().unwrap(), y.parse().unwrap())
                })
                .collect()
        })
        .collect()
}

/// How far the closed ring `ours` lies from `theirs` at worst, vertex by vertex, where it starts
/// at any of its points and runs either way; `None` where their point counts differ.
fn ring_distance(ours: &[Coord], theirs: &[Coord]) -> Option<f64> {
    if ours.len() != theirs.len() || ours.first() != ours.last() || theirs.first() != theirs.last()
    {
        return None;
    }
    let open = &ours[..ours.len() - 1];
    let n = open.len();
    let distance =
        |p: Coord, q: Coord| ((p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y)).sqrt();
    let mut best = f64::INFINITY;
    for start in 0..n {
        for forward in [true, false] {
            let at = |k: usize| {
                if forward {
                    open[(start + k) % n]
                } else {
                    open[(start + n - k % n) % n]
                }
            };
            let worst = (0..n)
                .map(|k| distance(at(k), theirs[k]))
                .fold(0.0, |a: f64, b| if b > a { b } else { a });
            if worst < best {
                best = worst;
            }
        }
    }
    Some(best)
}

#[test]
fn the_case_s_polygons_buffer_to_geos_s_rings_within_a_millionth_of_a_pixel() {
    // Verifies: ADR-0041
    // The polygons of renderers/polygon-as-line-offset.map in its pixels, buffered by 1 and by -1
    // with 30 segments a quadrant: each ring of GEOS's result has a ring of Forager's of its point
    // count, each vertex within 1e-6 px, whatever point the ring starts at and its direction. The
    // rings of the result are as many as GEOS's.
    let fixture = include_str!("geos-buffer.txt");
    let mut lines = fixture.lines().filter(|line| !line.starts_with('#'));
    let mut compared = 0;
    while let Some(shape) = lines.next() {
        let rings = rings_of(shape.strip_prefix("shape ").unwrap());
        let given = [Area {
            shell: rings[0].clone(),
            holes: rings[1..].to_vec(),
        }];
        for _ in 0..2 {
            let (distance, expected) = lines.next().unwrap().split_once(' ').unwrap();
            let distance: f64 = distance.parse().unwrap();
            let theirs: Vec<Vec<Coord>> = expected.split('|').flat_map(rings_of).collect();
            let ours: Vec<Vec<Coord>> =
                buffer(Buffered::Areas(&given), distance, &mut Recorder::default())
                    .unwrap()
                    .into_iter()
                    .flat_map(|area| std::iter::once(area.shell).chain(area.holes))
                    .collect();
            assert_eq!(ours.len(), theirs.len(), "{distance}: {ours:?}");
            let mut unmatched = ours.clone();
            for ring in &theirs {
                let (at, worst) = unmatched
                    .iter()
                    .enumerate()
                    .filter_map(|(i, mine)| ring_distance(mine, ring).map(|d| (i, d)))
                    .fold((usize::MAX, f64::INFINITY), |best, next| {
                        if next.1 < best.1 { next } else { best }
                    });
                assert!(
                    worst <= 1e-6,
                    "{distance}: GEOS's ring of {} points lies {worst} px from Forager's",
                    ring.len()
                );
                unmatched.remove(at);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 10, "4 shells and a hole by 1 and by -1");
}

#[test]
fn a_line_buffers_to_geos_s_ring_and_not_at_all_by_a_distance_not_above_0() {
    // Verifies: ADR-0041
    // GEOSBuffer_r(LINESTRING (0 0, 10 0, 10 10), 1, 30): 155 coordinates, round caps and the
    // round join outside the corner; a line buffered by 0 or less is GEOS's empty polygon.
    let line = [vec![c(0.0, 0.0), c(10.0, 0.0), c(10.0, 10.0)]];
    let buffered = buffer(Buffered::Lines(&line), 1.0, &mut Recorder::default()).unwrap();
    assert_eq!(buffered.len(), 1);
    assert!(buffered[0].holes.is_empty());
    assert_eq!(buffered[0].shell.len(), 155);
    // GEOS's first three coordinates, which lie on the inner corner and the cap's start.
    for (x, y) in [
        (9.0, 1.0),
        (9.0, 10.0),
        (9.001370465245426, 10.052335956242944),
    ] {
        let p = c(x, y);
        assert!(
            buffered[0]
                .shell
                .iter()
                .any(|q| (q.x - p.x).abs() < 1e-8 && (q.y - p.y).abs() < 1e-8),
            "({x}, {y})"
        );
    }
    for distance in [0.0, -1.0] {
        assert!(
            buffer(Buffered::Lines(&line), distance, &mut Recorder::default())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn a_point_buffers_to_a_circle_of_120_segments() {
    // Verifies: ADR-0041
    // GEOSBuffer_r(POINT (0 0), 1, 30): 121 coordinates, from (1, 0) clockwise, each at a multiple
    // of π/60; geo starts elsewhere and runs the other way, over the same vertices.
    let point = [c(0.0, 0.0)];
    let buffered = buffer(Buffered::Points(&point), 1.0, &mut Recorder::default()).unwrap();
    assert_eq!(buffered.len(), 1);
    let shell = &buffered[0].shell;
    assert_eq!(shell.len(), 121);
    for k in 0..120 {
        let angle = -f64::from(k) * PI / 60.0;
        let p = c(libm::cos(angle), libm::sin(angle));
        assert!(
            shell
                .iter()
                .any(|q| (q.x - p.x).abs() < 1e-12 && (q.y - p.y).abs() < 1e-12),
            "the vertex at {k} × π/60"
        );
    }
    assert!(
        buffer(Buffered::Points(&point), 0.0, &mut Recorder::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_buffer_charges_its_offset_curves_before_it_runs_and_its_added_points_after() {
    // Verifies: Q-9, ADR-0041
    // A square of 5 coordinates turns a quarter at each of its 4 corners, 30 steps of π/60 each:
    // 120 points of arcs, charged as added before the buffer runs, and with the 2 ends of each
    // point's offset segments, 130, as visited. The result's points beyond the square's 5 and those
    // 120 are charged after it. A line of 2 points has a half turn of 60 at each end, and is
    // buffered on both sides: 120 added, 128 visited.
    let square = [area(
        &[
            (0.0, 0.0),
            (0.0, 10.0),
            (10.0, 10.0),
            (10.0, 0.0),
            (0.0, 0.0),
        ],
        &[],
    )];
    let mut recorder = Recorder::default();
    let buffered = buffer(Buffered::Areas(&square), 1.0, &mut recorder).unwrap();
    assert_eq!(recorder.visits, [130]);
    assert_eq!(recorder.added[0], 120);
    assert_eq!(recorder.added[1], count(&buffered).saturating_sub(125));
    assert!(
        count(&buffered) <= 125,
        "the arcs bound what the buffer adds"
    );
    let line = [vec![c(0.0, 0.0), c(10.0, 0.0)]];
    let mut recorder = Recorder::default();
    buffer(Buffered::Lines(&line), 1.0, &mut recorder).unwrap();
    assert_eq!((recorder.visits[0], recorder.added[0]), (128, 120));
    // A point's circle, 120 segments.
    let mut recorder = Recorder::default();
    buffer(Buffered::Points(&[c(0.0, 0.0)]), 1.0, &mut recorder).unwrap();
    assert_eq!(recorder.added[0], 120);
    // Past the bound on visits, nothing is buffered and nothing more is charged.
    let mut recorder = Recorder {
        limit: Some(129),
        ..Recorder::default()
    };
    assert!(buffer(Buffered::Areas(&square), 1.0, &mut recorder).is_err());
    assert_eq!(recorder.added, [120]);
}

#[test]
fn a_repeated_point_hides_no_turn_from_the_charge() {
    // Verifies: Q-9, ADR-0041
    // i_overlay drops a point equal to the one before it and still rounds the turn at the next:
    // a zigzag line and a jagged ring with each point doubled are charged the arcs of the same
    // turns as without the repeats.
    let charged = |shape: Buffered<'_>| {
        let mut recorder = Recorder::default();
        buffer(shape, 1.0, &mut recorder).unwrap();
        recorder.added[0]
    };
    let zigzag: Vec<Coord> = (0..20)
        .map(|i| c(f64::from(i), if i % 2 == 0 { 0.0 } else { 10.0 }))
        .collect();
    let doubled: Vec<Coord> = zigzag.iter().flat_map(|&p| [p, p]).collect();
    let once = charged(Buffered::Lines(std::slice::from_ref(&zigzag)));
    assert!(once > 18 * 50, "{once}");
    assert_eq!(charged(Buffered::Lines(&[doubled])), once);
    let mut ring = zigzag.clone();
    ring.extend([c(19.0, -10.0), c(0.0, -10.0), c(0.0, 0.0)]);
    let doubled_ring: Vec<Coord> = ring.iter().flat_map(|&p| [p, p]).collect();
    let jagged = [Area {
        shell: ring,
        holes: vec![],
    }];
    let doubled_jagged = [Area {
        shell: doubled_ring,
        holes: vec![],
    }];
    assert_eq!(
        charged(Buffered::Areas(&doubled_jagged)),
        charged(Buffered::Areas(&jagged))
    );
}

/// `buffered`, the buffer of the points `given` by `distance`, checked at a scale geo's
/// arithmetic does not underflow at: its coordinates are finite, it holds a polygon, each point of
/// `given` lies inside it or within a millionth of the extent of its boundary, and each of its
/// vertices lies within `given`'s bounds grown by `distance`. The result and `given` are scaled
/// alike by the power of two that brings `given`'s extent near 1.
fn assert_covers(buffered: &[Area], given: &[Coord], distance: f64) {
    let rings: Vec<&Vec<Coord>> = buffered
        .iter()
        .flat_map(|area| std::iter::once(&area.shell).chain(&area.holes))
        .collect();
    assert!(!rings.is_empty(), "no polygon");
    assert!(
        rings
            .iter()
            .flat_map(|ring| ring.iter())
            .all(|p| p.x.is_finite() && p.y.is_finite()),
        "{buffered:?}"
    );
    let (min_x, max_x) = given
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (ms_min(lo, p.x), ms_max(hi, p.x))
        });
    let (min_y, max_y) = given
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (ms_min(lo, p.y), ms_max(hi, p.y))
        });
    let (_, e) = libm::frexp(ms_max(max_x - min_x, max_y - min_y));
    let up = |p: &Coord| c(libm::scalbn(p.x, -e), libm::scalbn(p.y, -e));
    let tolerance = 1e-6;
    let scaled: Vec<Vec<Coord>> = rings
        .iter()
        .map(|ring| ring.iter().map(up).collect())
        .collect();
    for p in given.iter().map(up) {
        let mut inside = false;
        let mut near = f64::INFINITY;
        for ring in &scaled {
            for pair in ring.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x)
                {
                    inside = !inside;
                }
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let length = dx * dx + dy * dy;
                let t = if length > 0.0 {
                    (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (qx, qy) = (a.x + t * dx - p.x, a.y + t * dy - p.y);
                near = ms_min(near, (qx * qx + qy * qy).sqrt());
            }
        }
        assert!(
            inside || near <= tolerance,
            "{p:?} lies {near} outside the buffer"
        );
    }
    let grown = libm::scalbn(distance, -e) + tolerance;
    let (low, high) = (up(&c(min_x, min_y)), up(&c(max_x, max_y)));
    for q in scaled.iter().flatten() {
        assert!(
            q.x >= low.x - grown
                && q.x <= high.x + grown
                && q.y >= low.y - grown
                && q.y <= high.y + grown,
            "{q:?} lies beyond the bounds grown by the distance"
        );
    }
}

#[test]
fn a_point_s_tiny_circle_buffers_by_a_tinier_distance_to_a_ring_that_covers_it() {
    // Verifies: Q-3, ADR-0041
    // The render fuzzer's input: the point (3.07847e-319, 0) buffered by 6.6e-163 is a circle
    // some 1.3e-162 across, whose shortest edge is 3.46e-164, so the square of its direction
    // underflows to 0 in i_overlay's offset. That ring buffered by 9.6e-281 neither panics nor
    // vanishes.
    let point = [c(3.07847e-319, 0.0)];
    let circle = buffer(
        Buffered::Points(&point),
        6.607_605_628_962_252e-163,
        &mut Recorder::default(),
    )
    .unwrap();
    assert_covers(&circle, &point, 6.607_605_628_962_252e-163);
    assert_eq!(circle.len(), 1);
    let distance = 9.633_340_286_202_275e-281;
    let buffered = buffer(Buffered::Areas(&circle), distance, &mut Recorder::default()).unwrap();
    assert_covers(&buffered, &circle[0].shell, distance);
}

#[test]
fn a_ring_with_edges_too_short_to_square_buffers_by_its_own_size() {
    // Verifies: Q-3, ADR-0041
    // The fuzzer's circle buffered by 1e-163, a distance about its own size: the squares of its
    // edges underflow as they do by the fuzzer's distance.
    let point = [c(3.07847e-319, 0.0)];
    let circle = buffer(
        Buffered::Points(&point),
        6.607_605_628_962_252e-163,
        &mut Recorder::default(),
    )
    .unwrap();
    let buffered = buffer(Buffered::Areas(&circle), 1e-163, &mut Recorder::default()).unwrap();
    assert_covers(&buffered, &circle[0].shell, 1e-163);
}

#[test]
fn a_line_with_edges_too_short_to_square_buffers_by_its_own_size() {
    // Verifies: Q-3, ADR-0041
    // A line of edges 1.4e-164 long buffered by 1e-164: the squares of its edges underflow too.
    let line = [vec![c(0.0, 0.0), c(1e-164, 1e-164), c(2e-164, 0.0)]];
    let buffered = buffer(Buffered::Lines(&line), 1e-164, &mut Recorder::default()).unwrap();
    assert_covers(&buffered, &line[0], 1e-164);
}
