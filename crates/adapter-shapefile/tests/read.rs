//! The shapefile source at the feature source port: which records it reads, as which geometry,
//! and which files it refuses. Each test holds a rule of `docs/semantics/shapefile.md`.

mod common;

use common::*;
use forager_ports::{Coord, Feature, Geometry, Stored, Value};

const SQUARE: &[(f64, f64)] = &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 0.0)];
const HOLE: &[(f64, f64)] = &[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 1.0)];
const LINE: &[(f64, f64)] = &[(0.0, 0.0), (5.0, 5.0)];
const OTHER: &[(f64, f64)] = &[(6.0, 6.0), (7.0, 8.0)];
const BOUNDS: [f64; 4] = [0.0, 0.0, 10.0, 10.0];

fn c(x: f64, y: f64) -> Coord {
    Coord { x, y }
}

fn coords(points: &[(f64, f64)]) -> Vec<Coord> {
    points.iter().map(|&(x, y)| c(x, y)).collect()
}

/// `content` read as a file of `shape_type`, a null record after it and a second copy after that.
fn read_as(shape_type: i32, content: &[u8]) -> Vec<forager_ports::Feature> {
    Files::new(
        shape_type,
        BOUNDS,
        &[content.to_vec(), null(), content.to_vec()],
    )
    .query(EVERYWHERE)
}

/// The content with the height or measure a shape type of Z or M carries: their range and a
/// value per point, or one value for a point.
fn measured(base: i32, content: Vec<u8>, points: usize) -> Vec<u8> {
    let mut r = content;
    r.extend(if base == POINT {
        vec![0; 8]
    } else {
        vec![0; 16 + 8 * points]
    });
    r
}

#[test]
fn every_shape_type_mapserver_reads_is_read_as_its_geometry() {
    // Verifies: ADR-0024
    for offset in [0, 10, 20] {
        let point = read_as(POINT + offset, &measured(POINT, point(1.0, 2.0), 1));
        assert_eq!(ids(&point), [0, 2], "type {}", POINT + offset);
        assert_eq!(point[0].geometry, Geometry::Point(c(1.0, 2.0)));

        let multi = read_as(
            MULTIPOINT + offset,
            &measured(MULTIPOINT, multipoint(LINE), 2),
        );
        assert_eq!(ids(&multi), [0, 2], "type {}", MULTIPOINT + offset);
        assert_eq!(multi[0].geometry, Geometry::MultiPoint(coords(LINE)));

        let arc = read_as(ARC + offset, &measured(ARC, lines(ARC, &[LINE]), 2));
        assert_eq!(arc[0].geometry, Geometry::LineString(coords(LINE)));
        let arcs = read_as(ARC + offset, &measured(ARC, lines(ARC, &[LINE, OTHER]), 4));
        assert_eq!(
            arcs[0].geometry,
            Geometry::MultiLineString(vec![coords(LINE), coords(OTHER)])
        );

        let polygon = read_as(
            POLYGON + offset,
            &measured(POLYGON, lines(POLYGON, &[SQUARE, HOLE]), 8),
        );
        assert_eq!(ids(&polygon), [0, 2], "type {}", POLYGON + offset);
        assert_eq!(
            polygon[0].geometry,
            Geometry::Polygon(vec![coords(SQUARE), coords(HOLE)])
        );
    }
}

#[test]
fn a_shape_type_mapserver_does_not_read_yields_no_feature() {
    // Verifies: ADR-0024
    for shape_type in [0, 2, 31, 255] {
        let files = Files::new(shape_type, BOUNDS, &[lines(POLYGON, &[SQUARE])]);
        assert_eq!(files.ids(), [] as [u64; 0], "type {shape_type}");
    }
}

#[test]
fn the_shape_type_is_the_one_byte_of_the_shx_header() {
    // Verifies: ADR-0024
    // The .shp says polygon, the records say polygon; the .shx's byte says point.
    let mut files = Files::new(POLYGON, BOUNDS, &[lines(POLYGON, &[SQUARE])]);
    files.shx[32] = POINT as u8;
    // The byte after it is not read.
    files.shx[33] = 1;
    let features = files.query(EVERYWHERE);
    assert_eq!(features[0].geometry, Geometry::Point(c(0.0, 0.0)));
}

#[test]
fn the_record_header_and_type_are_not_read() {
    // Verifies: ADR-0024
    let mut record = point(1.0, 2.0);
    record[..4].copy_from_slice(&MULTIPOINT.to_le_bytes());
    let mut files = Files::new(POINT, BOUNDS, &[record]);
    // The record number and content length of the record header.
    files.shp[100..108].fill(0xff);
    assert_eq!(
        files.query(EVERYWHERE)[0].geometry,
        Geometry::Point(c(1.0, 2.0))
    );
}

#[test]
fn an_arc_or_polygon_of_no_parts_is_a_shape_without_lines() {
    // Verifies: ADR-0024
    let arc = read_as(ARC, &raw_lines(ARC, BOUNDS, &[], &[(1.0, 1.0)]));
    assert_eq!(arc[0].geometry, Geometry::MultiLineString(vec![]));
    let polygon = read_as(POLYGON, &raw_lines(POLYGON, BOUNDS, &[], &[]));
    assert_eq!(polygon[0].geometry, Geometry::Polygon(vec![]));
    let multi = read_as(MULTIPOINT, &multipoint(&[]));
    assert_eq!(multi[0].geometry, Geometry::MultiPoint(vec![]));
}

#[test]
fn points_are_taken_in_sequence_from_the_first_whatever_a_part_starts_at() {
    // Verifies: ADR-0024
    let points = [(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
    // Parts at 1 and 2: the first holds one point, the second two; the points read are 0, 1, 2.
    let features = read_as(ARC, &raw_lines(ARC, BOUNDS, &[1, 2], &points));
    assert_eq!(
        features[0].geometry,
        Geometry::MultiLineString(vec![vec![c(0.0, 0.0)], vec![c(1.0, 1.0), c(2.0, 2.0)]])
    );
}

// --- Selection by extent -------------------------------------------------------------------

/// Three points: one at the origin, one at (5, 5), one at (10, 10).
fn three_points() -> Files {
    Files::new(
        POINT,
        BOUNDS,
        &[point(0.0, 0.0), point(5.0, 5.0), point(10.0, 10.0)],
    )
}

#[test]
fn an_extent_outside_the_file_bounds_selects_nothing() {
    // Verifies: ADR-0024
    let files = three_points();
    assert!(files.query([11.0, 11.0, 20.0, 20.0]).is_empty());
    assert!(files.query([-5.0, 0.0, -0.5, 10.0]).is_empty());
    // A record outside the .shx bounds but inside the extent is not selected either: the file's
    // bounds decide before any record's box is read.
    let stray = Files::new(POINT, BOUNDS, &[point(5.0, 5.0), point(50.0, 50.0)]);
    assert!(stray.query([40.0, 40.0, 60.0, 60.0]).is_empty());
}

#[test]
fn an_extent_that_contains_the_file_bounds_selects_every_record() {
    // Verifies: ADR-0024
    // The bounds of the .shx, not of the .shp, decide; the records' own boxes are not read.
    let mut files = Files::new(
        POLYGON,
        [0.0, 0.0, 1.0, 1.0],
        &[lines(POLYGON, &[SQUARE]), lines(POLYGON, &[OTHER])],
    );
    files.shp[36..68].fill(0);
    assert_eq!(ids(&files.query([0.0, 0.0, 1.0, 1.0])), [0, 1]);
}

#[test]
fn an_extent_across_the_file_bounds_selects_by_each_records_box() {
    // Verifies: ADR-0024
    // Boxes [0, 0, 4, 4] and [6, 6, 7, 8]; edges that touch overlap.
    let files = Files::new(
        POLYGON,
        BOUNDS,
        &[lines(POLYGON, &[SQUARE]), lines(ARC, &[OTHER])],
    );
    assert_eq!(ids(&files.query([4.0, 4.0, 5.0, 5.0])), [0]);
    assert_eq!(ids(&files.query([5.0, 5.0, 6.0, 20.0])), [1]);
    assert_eq!(ids(&files.query([4.5, 4.5, 5.5, 5.5])), [] as [u64; 0]);
    // The box the record stores decides, not its points.
    let mut boxed = lines(POLYGON, &[SQUARE]);
    boxed[4..36].copy_from_slice(
        &[20.0f64, 20.0, 30.0, 30.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let files = Files::new(POLYGON, [0.0, 0.0, 30.0, 30.0], &[boxed]);
    assert!(files.query([0.0, 0.0, 5.0, 5.0]).is_empty());
    assert_eq!(ids(&files.query([25.0, 25.0, 40.0, 40.0])), [0]);
}

#[test]
fn a_point_file_is_selected_by_the_point_itself() {
    // Verifies: ADR-0024
    let files = three_points();
    assert_eq!(ids(&files.query([4.0, 4.0, 11.0, 11.0])), [1, 2]);
    assert_eq!(ids(&files.query([-1.0, -1.0, 5.0, 5.0])), [0, 1]);
}

#[test]
fn a_point_whose_x_is_not_a_number_is_selected() {
    // Verifies: ADR-0024
    // msSHPReadBounds checks the least x only for files other than points; a NaN point compares
    // as overlapping any extent across the file's bounds.
    let files = Files::new(POINT, BOUNDS, &[point(f64::NAN, 5.0), point(9.0, 9.0)]);
    assert_eq!(ids(&files.query([-1.0, -1.0, 5.0, 5.0])), [0]);
}

#[test]
fn a_nan_in_a_box_beyond_its_least_x_compares_as_overlapping() {
    // Verifies: ADR-0024
    // Only the least x is checked; a NaN least y compares false, so the box reaches below.
    let mut record = lines(POLYGON, &[SQUARE]);
    record[12..20].copy_from_slice(&f64::NAN.to_le_bytes());
    let files = Files::new(POLYGON, [-10.0, -10.0, 10.0, 10.0], &[record]);
    assert_eq!(ids(&files.query([0.0, -5.0, 30.0, -1.0])), [0]);
    let files = Files::new(
        POLYGON,
        [-10.0, -10.0, 10.0, 10.0],
        &[lines(POLYGON, &[SQUARE])],
    );
    assert!(files.query([0.0, -5.0, 30.0, -1.0]).is_empty());
}

// --- Records past MapServer's guards -------------------------------------------------------

/// The file of `bad` and a good record after it, both of `shape_type`, read with the whole plane:
/// the bad one must be no shape, and the good one read.
fn is_no_shape(shape_type: i32, bad: Vec<u8>) {
    let good = match shape_type {
        POINT => point(1.0, 1.0),
        MULTIPOINT => multipoint(LINE),
        _ => lines(shape_type, &[SQUARE]),
    };
    let files = Files::new(shape_type, BOUNDS, &[bad, good]);
    assert_eq!(files.ids(), [1], "type {shape_type}");
}

#[test]
fn a_record_too_short_for_its_type_is_no_shape() {
    // Verifies: ADR-0024
    // Records are a whole number of 16-bit words.
    is_no_shape(POINT, point(1.0, 1.0)[..18].to_vec());
    is_no_shape(MULTIPOINT, multipoint(&[])[..38].to_vec());
    is_no_shape(ARC, lines(ARC, &[])[..42].to_vec());
    is_no_shape(POLYGON, lines(POLYGON, &[])[..42].to_vec());
    // A null shape is content of 4 bytes, whatever the file's type.
    is_no_shape(POLYGON, null());
}

#[test]
fn a_count_below_zero_or_past_the_caps_is_no_shape() {
    // Verifies: ADR-0024
    let at = |mut r: Vec<u8>, offset: usize, value: i32| {
        r[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        r
    };
    let polygon = lines(POLYGON, &[SQUARE]);
    // nParts at byte 36 of the content, nPoints at 40.
    is_no_shape(POLYGON, at(polygon.clone(), 36, -1));
    is_no_shape(POLYGON, at(polygon.clone(), 40, -1));
    is_no_shape(POLYGON, at(polygon.clone(), 36, 10_000_001));
    is_no_shape(POLYGON, at(polygon.clone(), 40, 50_000_001));
    let multi = multipoint(LINE);
    is_no_shape(MULTIPOINT, at(multi.clone(), 36, -1));
    is_no_shape(MULTIPOINT, at(multi, 36, 50_000_001));
}

#[test]
fn counts_that_need_more_bytes_than_the_record_holds_are_no_shape() {
    // Verifies: ADR-0024
    // 44 + 4 × parts + 16 × points against the content: one word short.
    let polygon = lines(POLYGON, &[SQUARE]);
    is_no_shape(POLYGON, polygon[..polygon.len() - 2].to_vec());
    let multi = multipoint(LINE);
    is_no_shape(MULTIPOINT, multi[..multi.len() - 2].to_vec());
    // The caps themselves are under them, and still past the size.
    let mut capped = lines(POLYGON, &[SQUARE]);
    capped[36..40].copy_from_slice(&10_000_000i32.to_le_bytes());
    capped[40..44].copy_from_slice(&50_000_000i32.to_le_bytes());
    is_no_shape(POLYGON, capped);
}

#[test]
fn a_multipoint_with_a_measure_or_height_needs_its_values_too() {
    // Verifies: ADR-0024
    for shape_type in [MULTIPOINT + 10, MULTIPOINT + 20] {
        let short = measured(MULTIPOINT, multipoint(LINE), 2);
        let files = Files::new(
            shape_type,
            BOUNDS,
            &[short[..short.len() - 2].to_vec(), short.clone()],
        );
        assert_eq!(files.ids(), [1], "type {shape_type}");
    }
    // An arc or a polygon of Z or M reads without them.
    let files = Files::new(ARC + 10, BOUNDS, &[lines(ARC, &[LINE])]);
    assert_eq!(files.ids(), [0]);
}

#[test]
fn a_part_that_starts_below_zero_ends_past_the_points_or_holds_none_is_no_shape() {
    // Verifies: ADR-0024
    let points = [(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)];
    for starts in [
        &[-1, 1][..],
        &[0, 4],
        &[0, 1, 1],
        &[0, 2, 1],
        &[3],
        &[0, -1],
    ] {
        is_no_shape(ARC, raw_lines(ARC, BOUNDS, starts, &points));
        is_no_shape(POLYGON, raw_lines(POLYGON, BOUNDS, starts, &points));
    }
    // Parts but no points: the first part holds none.
    is_no_shape(ARC, raw_lines(ARC, BOUNDS, &[0], &[]));
}

/// The file of three records whose middle entry of the `.shx` is `entry`, in raw words.
fn with_middle_entry(entry: (i32, i32)) -> Files {
    let mut files = Files::new(
        POINT,
        BOUNDS,
        &[point(1.0, 1.0), point(2.0, 2.0), point(3.0, 3.0)],
    );
    let mut entries: Vec<(i32, i32)> = (0..3).map(|i| (50 + 14 * i, 10)).collect();
    entries[1] = entry;
    files.set_shx_raw(POINT, BOUNDS, &entries);
    files
}

#[test]
fn an_index_entry_without_a_record_in_the_shp_is_no_shape() {
    // Verifies: ADR-0024
    let past = i32::try_from(three_points().shp.len() / 2).unwrap();
    for entry in [
        // A size under 4 bytes, or of 4: a null shape.
        (64, 0),
        (64, 1),
        (64, 2),
        // A size that is not a positive int when doubled.
        (64, -10),
        (64, i32::MAX / 2),
        // An offset of 0, below 0, or one that does not double.
        (0, 10),
        (-64, 10),
        (i32::MAX / 2, 10),
        // An offset past the .shp, or a record running past its end.
        (past, 10),
        (past - 4, 10),
    ] {
        let files = with_middle_entry(entry);
        assert_eq!(files.ids(), [0, 2], "entry {entry:?}");
    }
}

#[test]
fn a_shx_page_the_file_cuts_short_is_lost_as_a_whole() {
    // Verifies: ADR-0024
    let records: Vec<Vec<u8>> = (0..1500).map(|i| point(f64::from(i), 0.0)).collect();
    let mut files = Files::new(POINT, [0.0, 0.0, 1500.0, 0.0], &records);
    // The header claims 2000 records; the file holds 1500 entries.
    files.shx[24..28].copy_from_slice(&(50i32 + 4 * 2000).to_be_bytes());
    let ids = files.ids();
    assert_eq!(ids.len(), 1024);
    assert_eq!(ids.last(), Some(&1023));
    // Three claimed, two held: the first page is lost, and with it both.
    let mut files = Files::new(POINT, BOUNDS, &[point(1.0, 1.0), point(2.0, 2.0)]);
    files.shx[24..28].copy_from_slice(&(50i32 + 4 * 3).to_be_bytes());
    assert_eq!(files.ids(), [] as [u64; 0]);
}

#[test]
fn records_past_the_byte_total_of_the_shp_are_no_shape() {
    // Verifies: ADR-0024
    // One arc of 160 bytes with its header, named three times: the .shp holds 260 bytes, so the
    // second reading would exceed them.
    let mut files = Files::new(ARC, BOUNDS, &[lines(ARC, &[LINE, OTHER, LINE])]);
    let size = files.shp.len() - 108;
    files.set_shx(ARC, BOUNDS, &[(100, size), (100, size), (100, size)]);
    assert_eq!(files.ids(), [0]);
    // A reordered .shx reads every record.
    let mut files = Files::new(POINT, BOUNDS, &[point(1.0, 1.0), point(2.0, 2.0)]);
    files.set_shx(POINT, BOUNDS, &[(128, 20), (100, 20)]);
    let features = files.query(EVERYWHERE);
    assert_eq!(features[0].geometry, Geometry::Point(c(2.0, 2.0)));
    assert_eq!(features[1].geometry, Geometry::Point(c(1.0, 1.0)));
}

// --- The departure: a box that cannot be read --------------------------------------------

#[test]
fn a_record_whose_box_cannot_be_read_is_no_shape_and_the_layer_reads_on() {
    // Verifies: ADR-0024
    // MapServer fails the layer on each of these when it selects by boxes.
    let cut = [1.5, 1.5, 3.5, 3.5];
    let past = i32::try_from(three_points().shp.len() / 2).unwrap();
    for entry in [(64, 0), (64, 1), (0, 10), (past, 10)] {
        let files = with_middle_entry(entry);
        assert_eq!(ids(&files.query(cut)), [2], "entry {entry:?}");
    }
    // A size of 4 MapServer skips too.
    assert_eq!(ids(&with_middle_entry((64, 2)).query(cut)), [2]);

    // A box cut short by the end of the file, in a file that ends inside the box.
    let mut files = Files::new(
        POLYGON,
        BOUNDS,
        &[lines(POLYGON, &[SQUARE]), lines(POLYGON, &[OTHER])],
    );
    let end = files.shp.len();
    let second = end - lines(POLYGON, &[OTHER]).len() - 8;
    files.shp.truncate(second + 12 + 31);
    files.set_shx(
        POLYGON,
        BOUNDS,
        &[(100, second - 108), (second, end - second - 8)],
    );
    assert_eq!(ids(&files.query([0.0, 0.0, 5.0, 5.0])), [0]);

    // A box whose least x is not a number.
    let mut nan = lines(POLYGON, &[OTHER]);
    nan[4..12].copy_from_slice(&f64::NAN.to_le_bytes());
    let files = Files::new(
        POLYGON,
        BOUNDS,
        &[nan, lines(POLYGON, &[SQUARE]), lines(POLYGON, &[OTHER])],
    );
    assert_eq!(ids(&files.query([0.0, 0.0, 7.0, 7.0])), [1, 2]);
}

// --- Opening -------------------------------------------------------------------------------

fn refused(files: &Files) -> String {
    files.open().expect_err("the shapefile is refused").message
}

#[test]
fn a_shp_or_shx_header_mapserver_refuses_is_an_error() {
    // Verifies: ADR-0024
    let good = three_points();
    let mut f = good.clone();
    f.shp.truncate(99);
    assert!(refused(&f).contains(".shp is shorter"));
    let mut f = good.clone();
    f.shx.truncate(99);
    assert!(refused(&f).contains(".shx is shorter"));
    for (at, byte) in [(0, 1), (1, 1), (2, 0x28), (3, 0x0b), (3, 0x00)] {
        let mut f = good.clone();
        f.shx[at] = byte;
        assert!(refused(&f).contains("file code"), "byte {at}");
    }
    // A file length under 50 words, or one that states over 256,000,000 records.
    for words in [49i32, -1, 50 + 4 * 256_000_001] {
        let mut f = good.clone();
        f.shx[24..28].copy_from_slice(&words.to_be_bytes());
        assert!(refused(&f).contains("file length"), "{words} words");
    }
}

#[test]
fn what_mapserver_does_not_check_in_the_headers_opens() {
    // Verifies: ADR-0024
    let mut f = three_points();
    // The other file code MapServer accepts.
    f.shx[3] = 0x0d;
    // The .shp's file code, length, and type are not read.
    f.shp[..4].fill(0);
    f.shp[24..36].fill(0xff);
    // The largest record count, 256,000,000, in a file that holds three.
    f.shx[24..28].copy_from_slice(&(50i32 + 4 * 256_000_000).to_be_bytes());
    assert!(f.open().is_ok());
    // A file length of 50 words: no records.
    let mut f = three_points();
    f.shx[24..28].copy_from_slice(&50i32.to_be_bytes());
    assert_eq!(f.ids(), [] as [u64; 0]);
}

#[test]
fn a_dbf_header_mapserver_refuses_is_an_error() {
    // Verifies: ADR-0024
    let good = three_points();
    let with = |dbf: Vec<u8>| Files {
        dbf,
        ..good.clone()
    };
    assert!(refused(&with(dbf()[..31].to_vec())).contains("32-byte header"));
    for header_length in [0u16, 1, 32] {
        let mut d = dbf();
        d[8..10].copy_from_slice(&header_length.to_le_bytes());
        assert!(refused(&with(d)).contains("no room"), "{header_length}");
    }
    // A header longer than the file.
    assert!(refused(&with(dbf()[..64].to_vec())).contains("shorter than its header"));
    // Fields past the record length: a C field takes two bytes of size.
    let wide = dbf_with(&[(b'C', 10, 1)], 11);
    assert!(refused(&with(wide)).contains("fields take 267 bytes"));
    let past = dbf_with(&[(b'N', 5, 0), (b'C', 6, 0)], 11);
    assert!(refused(&with(past)).contains("fields take 12 bytes"));
}

#[test]
fn a_dbf_header_mapserver_accepts_opens() {
    // Verifies: ADR-0024
    let good = three_points();
    let opens = |dbf: Vec<u8>| {
        assert!(
            Files {
                dbf,
                ..good.clone()
            }
            .open()
            .is_ok()
        )
    };
    // An update date of zero, and a language driver that names a code page.
    let mut d = dbf();
    d[1..4].fill(0);
    d[29] = 0x57;
    opens(d);
    // An N or F field takes one byte of size and one of decimals.
    opens(dbf_with(&[(b'N', 10, 200), (b'F', 0, 9)], 11));
    // Fields may be zero wide, and the header may hold no field at all.
    opens(dbf_with(&[(b'C', 0, 0), (b'C', 0, 0)], 1));
    let mut d = dbf_with(&[], 0);
    d[8..10].copy_from_slice(&63u16.to_le_bytes());
    d.resize(63, 0);
    opens(d);
    // A record count with the high bit set.
    let mut d = dbf();
    d[4..8].copy_from_slice(&[0xff; 4]);
    opens(d);
}

#[test]
fn a_missing_file_is_an_error_and_upper_case_names_are_tried() {
    // Verifies: ADR-0024
    let files = three_points();
    for missing in ["data/layer.shp", "data/layer.shx", "data/layer.dbf"] {
        let mut loader = files.loader();
        loader.0.remove(missing);
        let error = open(&loader).expect_err(missing).message;
        assert!(error.contains(missing), "{error}");
    }
    let upper = files.loader_named("data/layer", "SHP", "SHX", "DBF");
    assert!(open(&upper).is_ok());
    // Only the extension changes case, as msSHPOpen and msDBFOpen try it, never the name.
    let upper_name = files.loader_named("data/LAYER", "SHP", "SHX", "DBF");
    assert!(open(&upper_name).is_err());
    // Any extension of the name is replaced.
    let renamed = files.loader_named("data/my.layer", "shp", "shx", "dbf");
    let path = forager_ports::ResourcePath::new("data/my.layer.v2").unwrap();
    let source = forager_adapter_shapefile::ShapefileSource::open(&renamed, &path);
    assert!(ready(source).is_ok());
}

#[test]
fn no_mutation_of_a_shapefile_panics() {
    // Verifies: ADR-0024, Q-3
    let base = Files::new(
        POLYGON,
        BOUNDS,
        &[
            lines(POLYGON, &[SQUARE, HOLE]),
            null(),
            lines(POLYGON, &[OTHER]),
        ],
    );
    let extents = [EVERYWHERE, [1.0, 1.0, 3.0, 3.0]];
    let check = |files: &Files| {
        if let Ok(source) = files.open() {
            for extent in extents {
                let _ = query(&source, extent);
            }
        }
    };
    for shape_type in [1u8, 3, 5, 8, 11, 13, 15, 18, 21, 23, 25, 28, 31] {
        let mut f = base.clone();
        f.shx[32] = shape_type;
        for i in 0..f.shp.len() {
            for byte in [0x00, 0x7f, 0x80, 0xff] {
                let mut m = f.clone();
                m.shp[i] = byte;
                check(&m);
            }
            let mut cut = f.clone();
            cut.shp.truncate(i);
            check(&cut);
        }
        for i in 0..f.shx.len() {
            for byte in [0x00, 0x7f, 0x80, 0xff] {
                let mut m = f.clone();
                m.shx[i] = byte;
                check(&m);
            }
            let mut cut = f.clone();
            cut.shx.truncate(i);
            check(&cut);
        }
    }
    for i in 0..base.dbf.len() {
        for byte in [0x00, 0x7f, 0x80, 0xff] {
            let mut m = base.clone();
            m.dbf[i] = byte;
            check(&m);
        }
    }
}

/// Three points, of which the `.dbf` holds records for the first two.
fn points_with(dbf: Vec<u8>) -> Vec<Feature> {
    let mut files = Files::new(
        POINT,
        BOUNDS,
        &[point(1.0, 1.0), point(2.0, 2.0), point(3.0, 3.0)],
    );
    files.dbf = dbf;
    files.query(EVERYWHERE)
}

fn text(value: &str) -> Option<Value<'_>> {
    Some(Value::String(value.into()))
}

#[test]
fn each_field_reads_as_msdbfreadattribute_reads_it() {
    // Verifies: ADR-0026
    let fields: &[(&[u8], u8, u8)] = &[
        (b"NAME", b'C', 8),
        (b"POP", b'N', 8),
        (b"AREA", b'F', 6),
        (b"WHEN", b'D', 8),
        (b"OK", b'L', 1),
        (b"NOTE", b'M', 6),
    ];
    let first: &[&[u8]] = &[
        b"  Wien  ",
        b"   1.50 ",
        b" 41.5 ",
        b"  202610",
        b"T",
        b"ab\0cd ",
    ];
    let null: &[&[u8]] = &[
        b"        ",
        b"********",
        b"******",
        b"00000000",
        b"?",
        b"      ",
    ];
    let features = points_with(table(fields, &[&first.concat(), &null.concat()]));
    let first = &features[0].properties;
    // Trailing blanks are cut from every field, leading ones only from N, F, and D.
    assert_eq!(first.get("NAME"), text("  Wien"));
    assert_eq!(first.get("POP"), text("1.50"));
    assert_eq!(first.get("AREA"), text("41.5"));
    // A D field's leading blanks are cut as well.
    assert_eq!(first.get("WHEN"), text("202610"));
    assert_eq!(first.get("OK"), text("T"));
    // A field ends at its first NUL.
    assert_eq!(first.get("NOTE"), text("ab"));
    // A null N, F, or D field reads as 0; a null L as it stands; a blank C as empty.
    let second = &features[1].properties;
    assert_eq!(second.get("NAME"), text(""));
    assert_eq!(second.get("POP"), text("0"));
    assert_eq!(second.get("AREA"), text("0"));
    assert_eq!(second.get("WHEN"), text("0"));
    assert_eq!(second.get("OK"), text("?"));
    assert_eq!(second.get("NOTE"), text(""));
    // A shape past the .dbf's records carries no properties.
    assert_eq!(features[2].properties.get("NAME"), None);
    let mut names = Vec::new();
    features[2]
        .properties
        .each(|name, _| names.push(name.to_owned()));
    assert!(names.is_empty());
}

#[test]
fn a_field_is_found_by_its_name_as_msdbfgetitemindex_finds_it() {
    // Verifies: ADR-0026
    let fields: &[(&[u8], u8, u8)] = &[
        (b"Name", b'C', 2),
        (b"NAME", b'C', 2),
        (b"WIDE       ", b'C', 2),
        (b"SHORT  ", b'C', 2),
        (b"BAD\xff", b'C', 2),
    ];
    let features = points_with(table(fields, &[b"a1a2a3a4\xffx", b"b1b2b3b4b5"]));
    let p = &features[0].properties;
    // Without regard to ASCII case, the first field of the name winning.
    assert_eq!(p.get("name"), text("a1"));
    assert_eq!(p.get("NAME"), text("a1"));
    // Blanks that fill the name to its 11th byte are cut; blanks before a NUL are kept.
    assert_eq!(p.get("WIDE"), text("a3"));
    assert_eq!(p.get("SHORT"), None);
    assert_eq!(p.get("SHORT  "), text("a4"));
    // A byte that is not UTF-8 is replaced in the value.
    assert_eq!(p.get("NAME "), None);
    let mut seen = Vec::new();
    p.each(|name, value| seen.push((name.to_owned(), Stored::from(value))));
    assert_eq!(seen.len(), 5);
    assert_eq!(
        seen[4],
        ("BAD\u{fffd}".to_owned(), Stored::String("\u{fffd}x".into()))
    );
}

#[test]
fn a_record_the_file_cuts_short_carries_no_properties() {
    // Verifies: ADR-0026
    let mut dbf = table(&[(b"NAME", b'C', 4)], &[b"aaaa", b"bbbb"]);
    dbf.truncate(dbf.len() - 1);
    let features = points_with(dbf);
    assert_eq!(features[0].properties.get("NAME"), text("aaaa"));
    assert_eq!(features[1].properties.get("NAME"), None);
}

#[test]
fn a_record_count_whose_last_byte_is_128_or_more_counts_no_records() {
    // Verifies: ADR-0026
    let mut dbf = table(&[(b"NAME", b'C', 4)], &[b"aaaa", b"bbbb"]);
    dbf[7] = 0x80;
    let features = points_with(dbf);
    assert!(features.iter().all(|f| f.properties.get("NAME").is_none()));
}
