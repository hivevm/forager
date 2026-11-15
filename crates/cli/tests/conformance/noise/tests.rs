use std::ffi::OsString;
use std::path::PathBuf;

use super::{MapServer, NOISE_THRESHOLDS, mapserver, noise_report, record_differences, reproduced};
use crate::conformance::Category;
use crate::metric::Metric;
use crate::{GEOMETRY, TEXT};

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

/// The push checks run the ignored tests without MapServer, and the noise workflow with it: the
/// measurement skips itself where neither variable is set, runs where both name a file, and fails
/// where one is missing or empty.
#[test]
fn the_noise_measurement_runs_only_where_mapserver_is_named() {
    let named = |s: &str| Some(OsString::from(s));
    assert_eq!(mapserver(None, None, None), Ok(None));
    assert_eq!(
        mapserver(
            named("bin/map2img"),
            named("bin/mapserver_labels"),
            named("bin/mapserver_proj")
        ),
        Ok(Some(MapServer {
            map2img: PathBuf::from("bin/map2img"),
            labels: PathBuf::from("bin/mapserver_labels"),
            proj: PathBuf::from("bin/mapserver_proj"),
        }))
    );
    for (map2img, labels, proj) in [
        (named("bin/map2img"), named("bin/mapserver_labels"), None),
        (named("bin/map2img"), None, named("bin/mapserver_proj")),
        (
            None,
            named("bin/mapserver_labels"),
            named("bin/mapserver_proj"),
        ),
        (
            named(""),
            named("bin/mapserver_labels"),
            named("bin/mapserver_proj"),
        ),
        (named("bin/map2img"), named(""), named("bin/mapserver_proj")),
        (
            named("bin/map2img"),
            named("bin/mapserver_labels"),
            named(""),
        ),
    ] {
        assert!(mapserver(map2img, labels, proj).is_err());
    }
}

#[test]
fn the_reprojection_s_output_of_points_and_rectangles_holds_its_record_within_a_millimetre() {
    // Verifies: ADR-0038
    let header = "# PROJ 9.4.0\n# MapServer abc\n";
    let ll = "+proj=longlat +datum=WGS84";
    let wm = "+proj=merc +a=6378137 +b=6378137";
    let record = format!(
        "{header}{ll}\t{wm}\t10\t50\t1113194.9079327357\t6446275.8410171578\n\
         {wm}\t{ll}\t0\t0\t0\t0\n{ll}\t{wm}\t0\t90\tfail\n\
         {ll}\t{wm}\t5\t45\t15\t55\t556597.45\t5621521.48\t1669792.36\t7361866.11\n\
         {ll}\t{wm}\t0\t100\t1\t120\tfail\n"
    );
    assert!(record_differences(&record, &record).is_empty());
    // Within a millimetre in metres, and in degrees for a geographic target.
    let close = record
        .replace("1113194.9079327357", "1113194.9085")
        .replace("\t0\t0\t0\t0", "\t0\t0\t0.000000005\t0");
    assert!(record_differences(&record, &close).is_empty());
    for (what, written) in [
        (
            "a metre off",
            record.replace("1113194.9079327357", "1113195.9"),
        ),
        (
            "a degree off",
            record.replace("\t0\t0\t0\t0", "\t0\t0\t0.0001\t0"),
        ),
        (
            "a failure on one side",
            record.replace("\tfail", "\t0\t1e300"),
        ),
        ("another PROJ", record.replace("9.4.0", "9.5.0")),
        (
            "a point not recorded",
            format!("{record}{ll}\t{wm}\t1\t1\t1\t1\n"),
        ),
        ("another point", record.replace("\t10\t50\t", "\t11\t50\t")),
        (
            "a rectangle's corner a metre off",
            record.replace("1669792.36", "1669793.36"),
        ),
        (
            "a rectangle that fails on one side",
            record.replace("\t120\tfail", "\t120\t0\t0\t1\t1"),
        ),
        (
            "a rectangle with a corner missing",
            record.replace("\t7361866.11", ""),
        ),
    ] {
        assert!(!record_differences(&record, &written).is_empty(), "{what}");
    }
}
