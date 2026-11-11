use std::ffi::OsString;
use std::path::PathBuf;

use super::{NOISE_THRESHOLDS, mapserver, noise_report, reproduced};
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
    assert_eq!(mapserver(None, None), Ok(None));
    assert_eq!(
        mapserver(named("bin/map2img"), named("bin/mapserver_labels")),
        Ok(Some((
            PathBuf::from("bin/map2img"),
            PathBuf::from("bin/mapserver_labels")
        )))
    );
    for (map2img, reader) in [
        (named("bin/map2img"), None),
        (None, named("bin/mapserver_labels")),
        (named(""), named("bin/mapserver_labels")),
        (named("bin/map2img"), named("")),
    ] {
        assert!(mapserver(map2img, reader).is_err());
    }
}
