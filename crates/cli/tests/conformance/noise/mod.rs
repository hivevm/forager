//! The noise measurement of G-6 and G-13 (ADR-0033): MapServer at the pinned release renders the
//! cases in scope, and the harness compares its renderings with the references and the labels of
//! its label cache with the records beside the list, and its reprojection of G-11's reference
//! points with the record beside the test of `proj` (ADR-0038).
//! Only `.github/workflows/noise.yml` builds MapServer; the measurement runs where
//! `FORAGER_MAP2IMG`, `FORAGER_MAPSERVER_LABELS`, and `FORAGER_MAPSERVER_PROJ` name it and skips
//! itself where none does, so the push checks never run it, whatever it is named.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Case, Category, Entry, labels};
use crate::metric::{self, Metric};
use crate::{CASES, metric_of, report, suite};

#[cfg(test)]
mod tests;

/// The thresholds the noise measurement reports the share of differing pixels at.
const NOISE_THRESHOLDS: &[u8] = &[0, 1, 2, 4, 8, 16, 32, 64, 128];

/// The noise of G-6 and G-13: MapServer at the pinned release, which `.github/workflows/noise.yml`
/// builds (ADR-0033), renders every case the list puts in scope, of every category, as
/// msautotest's test library runs it, and each rendering is compared with its reference at each of
/// [`NOISE_THRESHOLDS`]. The report, a table of the share per case and threshold and its maximum
/// over each category, goes to standard output and to the job summary; it names apart each case
/// MapServer does not reproduce: `map2img` fails, writes no PNG, writes one of another size, or
/// differs beyond its category's tolerance at every threshold of the set. The list moves such a
/// case out of scope, so the test fails when one is in scope. For each text case the reader of
/// MapServer's label cache (ADR-0033) renders it beside `map2img`, its image byte for byte the
/// same, and its labels must hold the record beside the list, each box within [`labels::NOISE`] and
/// every other field exactly; the test fails on a record that differs or is missing, and the
/// reader's labels stay in `noise/labels/` to take the record from. The renderings stay in
/// `noise/` under the test's target directory. Last, the reader of MapServer's reprojection
/// reprojects G-11's reference points and rectangles ([`reference_points`]); the test fails where
/// its output and a record beside the test of `proj` disagree, and the output stays in
/// `noise/proj/`.
#[test]
#[ignore = "needs MapServer; .github/workflows/noise.yml builds it and runs this test, and \
            without it the test skips itself"]
fn mapserver_renders_the_cases_in_scope_within_its_noise() {
    // Verifies: ADR-0033
    // Verifies: ADR-0038
    let mapserver = mapserver(
        std::env::var_os("FORAGER_MAP2IMG"),
        std::env::var_os("FORAGER_MAPSERVER_LABELS"),
        std::env::var_os("FORAGER_MAPSERVER_PROJ"),
    )
    .unwrap();
    let Some(MapServer {
        map2img,
        labels: reader,
        proj,
    }) = mapserver
    else {
        skipped();
        return;
    };
    let suite = suite();
    let cases = super::cases(&suite, super::DIRECTORIES);
    let entries = super::list(CASES).unwrap();
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("noise");
    let _ = std::fs::remove_dir_all(&out);
    let mut measured = Vec::new();
    let mut unreproduced = Vec::new();
    let mut records = Vec::new();
    for case in &cases {
        let Some(Entry::InScope { category, .. }) = entries.get(&case.key()).map(|l| &l.entry)
        else {
            continue;
        };
        let name = format!("{} {}", case.mapfile, case.result);
        let rendering = out
            .join(case.mapfile.trim_end_matches(".map"))
            .join(&case.result);
        match measure(&suite, case, metric_of(*category), &map2img, &rendering) {
            Ok(shares) => {
                if *category == Category::Text
                    && let Err(why) = read_labels(&suite, case, &reader, &rendering, &out)
                {
                    records.push(format!("{name}: {why}"));
                }
                measured.push((name, *category, shares));
            }
            Err(reason) => unreproduced.push(format!("{name}: {reason}")),
        }
    }
    let report = noise_report(&measured, &unreproduced);
    self::report(&report);
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary)
            .and_then(|mut file| file.write_all(report.as_bytes()))
            .unwrap();
    }
    // Run before anything is judged, so that the artifact holds its output whatever the rest shows.
    let differences = reference_points(&proj, &out.join("proj")).unwrap();
    assert!(
        unreproduced.is_empty(),
        "{} cases in scope are not reproduced by MapServer itself; the list moves each out of \
         scope with its reason:\n{}",
        unreproduced.len(),
        unreproduced.join("\n")
    );
    assert!(
        records.is_empty(),
        "{} text cases do not hold the record of MapServer's labels beside the list; the reader's \
         labels are in {}:\n{}",
        records.len(),
        out.join("labels").display(),
        records.join("\n")
    );
    assert!(
        differences.is_empty(),
        "MapServer's reprojection of G-11's reference points and rectangles does not hold its \
         record; the reader's output is in {}:\n{}",
        out.join("proj").display(),
        differences.join("\n")
    );
}

/// The lists of G-11's reference points, rectangles, and shapes, each with the record of
/// MapServer's reprojection of it and the name of the reader's output, from the workspace root
/// (ADR-0038).
const REFERENCES: [(&str, &str, &str); 3] = [
    (
        "crates/proj/tests/reference/points.tsv",
        "crates/proj/tests/reference/mapserver.tsv",
        "mapserver.tsv",
    ),
    (
        "crates/proj/tests/reference/rects.tsv",
        "crates/proj/tests/reference/mapserver-rects.tsv",
        "mapserver-rects.tsv",
    ),
    (
        "crates/proj/tests/reference/shapes.tsv",
        "crates/proj/tests/reference/mapserver-shapes.tsv",
        "mapserver-shapes.tsv",
    ),
];

/// The largest difference of a coordinate between the reader's output and the record: 1 mm, in
/// metres for a projected target and in degrees, about 1.0 mm, for a geographic one.
const PROJECTED_SLACK: f64 = 1e-3;
const GEOGRAPHIC_SLACK: f64 = 9e-9;

/// Runs the reader of MapServer's reprojection, `reader`, over each list of [`REFERENCES`], keeps
/// its output in `out` under the list's name, and returns how each differs from its record; an
/// error is a reader that does not run or fails.
fn reference_points(reader: &Path, out: &Path) -> Result<Vec<String>, String> {
    let mut differences = Vec::new();
    for (list, record, name) in REFERENCES {
        differences.extend(
            reference(reader, list, record, &out.join(name))?
                .into_iter()
                .map(|d| format!("{record}: {d}")),
        );
    }
    Ok(differences)
}

/// Runs `reader` over `list`, keeps its output at `out`, and returns how it differs from `record`.
fn reference(reader: &Path, list: &str, record: &str, out: &Path) -> Result<Vec<String>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(reader)
        .arg(root.join(list))
        .output()
        .map_err(|e| format!("{}: {e}", reader.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} fails: {}",
            reader.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let output = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(out, &output).map_err(|e| e.to_string())?;
    let Ok(record) = std::fs::read_to_string(root.join(record)) else {
        return Ok(vec![format!("{record} is missing")]);
    };
    Ok(record_differences(&record, &output))
}

/// How the reader's `output` differs from the `record`: a line one has and the other lacks, a
/// header or a point or rectangle that differs, one that fails in one only, or a coordinate past
/// the slack of its target ([`PROJECTED_SLACK`], [`GEOGRAPHIC_SLACK`]). A line of the record is
/// a point's, its four fields followed by two coordinates or `fail`, a rectangle's, its six
/// followed by four or `fail`, or a shape's, its four, the third its type, followed by its lines,
/// each `x,y x,y ...` or `-`, or by `fail`.
fn record_differences(record: &str, output: &str) -> Vec<String> {
    let mut differences = Vec::new();
    let (record, output): (Vec<&str>, Vec<&str>) =
        (record.lines().collect(), output.lines().collect());
    if record.len() != output.len() {
        differences.push(format!(
            "the record holds {} lines, the reader wrote {}",
            record.len(),
            output.len()
        ));
    }
    for (number, (recorded, written)) in record.iter().zip(&output).enumerate() {
        let line = number + 1;
        if recorded.starts_with('#') || written.starts_with('#') {
            if recorded != written {
                differences.push(format!("line {line}: {written:?}, recorded {recorded:?}"));
            }
            continue;
        }
        let r: Vec<&str> = recorded.split('\t').collect();
        let w: Vec<&str> = written.split('\t').collect();
        let shape = matches!(r.get(2), Some(&("point" | "line" | "polygon")));
        let fields = match r.len() {
            _ if shape => 4,
            5 | 6 => 4,
            7 | 10 => 6,
            _ => 0,
        };
        if fields == 0 || w.len() <= fields || r[..fields] != w[..fields] {
            differences.push(format!("line {line}: {written:?}, recorded {recorded:?}"));
            continue;
        }
        let geographic = r[1].split_whitespace().any(|p| {
            matches!(
                p,
                "+proj=longlat" | "+proj=latlong" | "+proj=lonlat" | "+proj=latlon"
            )
        });
        let slack = if geographic {
            GEOGRAPHIC_SLACK
        } else {
            PROJECTED_SLACK
        };
        let (r, w) = (&r[fields..], &w[fields..]);
        let close = |a: &str, b: &str| match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(a), Ok(b)) => (a - b).abs() <= slack,
            _ => false,
        };
        let agree = match (r, w) {
            _ if shape => {
                r.len() == w.len()
                    && r.iter().zip(w).all(|(a, b)| {
                        let (a, b): (Vec<&str>, Vec<&str>) =
                            (a.split(' ').collect(), b.split(' ').collect());
                        a.len() == b.len()
                            && a.iter().zip(&b).all(|(a, b)| {
                                match (a.split_once(','), b.split_once(',')) {
                                    (Some((ax, ay)), Some((bx, by))) => {
                                        close(ax, bx) && close(ay, by)
                                    }
                                    (None, None) => a == b,
                                    _ => false,
                                }
                            })
                    })
            }
            (["fail"], ["fail"]) => true,
            _ if r.len() == fields - 2 && w.len() == r.len() && !r.contains(&"fail") => r
                .iter()
                .zip(w)
                .all(|(a, b)| match (a.parse::<f64>(), b.parse::<f64>()) {
                    (Ok(a), Ok(b)) => (a - b).abs() <= slack,
                    _ => false,
                }),
            _ => false,
        };
        if !agree {
            differences.push(format!("line {line}: {written:?}, recorded {recorded:?}"));
        }
    }
    differences
}

/// The programs of the MapServer build the noise measurement runs.
#[derive(Debug, PartialEq)]
struct MapServer {
    map2img: PathBuf,
    /// The reader of the label cache (ADR-0033).
    labels: PathBuf,
    /// The reader of the reprojection (ADR-0038).
    proj: PathBuf,
}

/// `map2img` and the two readers, as `FORAGER_MAP2IMG`, `FORAGER_MAPSERVER_LABELS`, and
/// `FORAGER_MAPSERVER_PROJ` name them, or none where none is set: the noise measurement runs
/// where all three name a file and skips itself where none does. Some without the others, or one
/// empty, is an error, so that a noise run whose build named nothing does not pass unmeasured.
fn mapserver(
    map2img: Option<OsString>,
    labels: Option<OsString>,
    proj: Option<OsString>,
) -> Result<Option<MapServer>, String> {
    match (map2img, labels, proj) {
        (None, None, None) => Ok(None),
        (Some(map2img), Some(labels), Some(proj))
            if !map2img.is_empty() && !labels.is_empty() && !proj.is_empty() =>
        {
            Ok(Some(MapServer {
                map2img: PathBuf::from(map2img),
                labels: PathBuf::from(labels),
                proj: PathBuf::from(proj),
            }))
        }
        _ => Err(
            "FORAGER_MAP2IMG, FORAGER_MAPSERVER_LABELS, and FORAGER_MAPSERVER_PROJ must all name a \
             file, as .github/workflows/noise.yml sets them, or none be set"
                .to_owned(),
        ),
    }
}

/// Says that the noise measurement skipped itself, without MapServer.
#[expect(
    clippy::print_stderr,
    reason = "the skip of the noise measurement, on stderr, shown with --nocapture"
)]
fn skipped() {
    eprintln!(
        "the noise measurement needs MapServer, which only .github/workflows/noise.yml builds; \
         skipped without FORAGER_MAP2IMG, FORAGER_MAPSERVER_LABELS, and FORAGER_MAPSERVER_PROJ"
    );
}

/// Renders `case` with the reader of MapServer's label cache beside `map2img`'s `rendering`,
/// holds the two images byte for byte equal, keeps the reader's labels in `out/labels/`, and
/// holds them to the record beside the list as [`labels::beyond_noise`] does; the error says what
/// does not hold.
fn read_labels(
    suite: &Path,
    case: &Case,
    reader: &Path,
    rendering: &Path,
    out: &Path,
) -> Result<(), String> {
    let image = rendering.with_extension("reader.png");
    let mapfile = suite.join(&case.mapfile);
    let output = Command::new(reader)
        .args(case.map2img_arguments(&image)?)
        .current_dir(mapfile.parent().unwrap())
        .output()
        .map_err(|e| format!("the reader does not start: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "the reader fails, {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let ours = std::fs::read(&image).map_err(|e| format!("the reader writes no image: {e}"))?;
    let theirs = std::fs::read(rendering).map_err(|e| e.to_string())?;
    if ours != theirs {
        return Err("the reader's image differs from map2img's".to_owned());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "the reader's labels are not UTF-8".to_owned())?;
    let read = labels::read(&text).map_err(|e| format!("the reader's labels: {e}"))?;
    let record = labels::record_path(case);
    let relative = record.strip_prefix(labels::records_dir()).unwrap();
    let kept = out.join("labels").join(relative);
    std::fs::create_dir_all(kept.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&kept, &text).map_err(|e| e.to_string())?;
    let recorded =
        std::fs::read_to_string(&record).map_err(|e| format!("{}: {e}", record.display()))?;
    let recorded = labels::read(&recorded).map_err(|e| format!("{}: {e}", record.display()))?;
    let differences = labels::beyond_noise(&recorded, &read);
    if differences.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "MapServer's labels differ from {}:\n  {}",
            record.display(),
            differences.join("\n  ")
        ))
    }
}

/// Runs `map2img` for `case` in its mapfile's directory, writing `rendering`, and compares the
/// rendering with the reference at each of [`NOISE_THRESHOLDS`]. The shares are in their order;
/// the error is why MapServer does not reproduce the case at all, by `metric`.
fn measure(
    suite: &Path,
    case: &Case,
    metric: &Metric,
    map2img: &Path,
    rendering: &Path,
) -> Result<Vec<f64>, String> {
    std::fs::create_dir_all(rendering.parent().unwrap()).map_err(|e| e.to_string())?;
    let mapfile = suite.join(&case.mapfile);
    let output = Command::new(map2img)
        .args(case.map2img_arguments(rendering)?)
        .current_dir(mapfile.parent().unwrap())
        .output()
        .map_err(|e| format!("map2img does not start: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "map2img fails, {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let rendering = std::fs::read(rendering)
        .map_err(|e| format!("map2img writes no image: {e}"))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("map2img's image: {e}")))?;
    let directory = &case.mapfile[..case.mapfile.find('/').unwrap()];
    let reference = suite.join(directory).join("expected").join(&case.result);
    let reference = std::fs::read(&reference)
        .map_err(|e| format!("its reference {}: {e}", reference.display()))
        .and_then(|bytes| metric::decode(&bytes).map_err(|e| format!("its reference: {e}")))?;
    let shares = NOISE_THRESHOLDS
        .iter()
        .map(|&threshold| {
            let metric = Metric {
                threshold,
                tolerance: 0.0,
            };
            let comparison = metric::compare(&rendering, &reference, &metric);
            comparison.share().ok_or_else(|| comparison.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    reproduced(shares, metric)
}

/// The shares of a case MapServer reproduces: one at some threshold of the set within the
/// tolerance of `metric`. The shares fall as the threshold rises, so the last decides; the error
/// names a case that differs beyond the tolerance at every threshold.
fn reproduced(shares: Vec<f64>, metric: &Metric) -> Result<Vec<f64>, String> {
    match shares.last() {
        Some(&share) if share > metric.tolerance => Err(format!(
            "{share:.6} of its pixels differ by more than {}, above the tolerance {}",
            NOISE_THRESHOLDS[NOISE_THRESHOLDS.len() - 1],
            metric.tolerance
        )),
        _ => Ok(shares),
    }
}

/// The report of the noise measurement, as Markdown: a row per measured case with its category and
/// its share of differing pixels at each of [`NOISE_THRESHOLDS`], the maximum of each column over
/// each category that has a case measured, and the cases MapServer does not reproduce, each with
/// its reason.
fn noise_report(measured: &[(String, Category, Vec<f64>)], unreproduced: &[String]) -> String {
    let mut report =
        String::from("## MapServer's noise over the cases in scope\n\n| Case | Category |");
    for threshold in NOISE_THRESHOLDS {
        report.push_str(&format!(" > {threshold} |"));
    }
    report.push_str("\n| --- | --- |");
    report.push_str(&" ---: |".repeat(NOISE_THRESHOLDS.len()));
    report.push('\n');
    let mut maxima = BTreeMap::new();
    for (name, category, shares) in measured {
        report.push_str(&format!("| {name} | {} |", category.name()));
        let maximum = maxima
            .entry(*category)
            .or_insert_with(|| vec![0.0_f64; NOISE_THRESHOLDS.len()]);
        for (max, share) in maximum.iter_mut().zip(shares) {
            *max = max.max(*share);
            report.push_str(&format!(" {share:.6} |"));
        }
        report.push('\n');
    }
    for (category, maximum) in Category::ALL
        .iter()
        .filter_map(|c| maxima.get(c).map(|m| (c, m)))
    {
        report.push_str(&format!("| **maximum, {}** | |", category.name()));
        for max in maximum {
            report.push_str(&format!(" **{max:.6}** |"));
        }
        report.push('\n');
    }
    report.push_str(&format!(
        "\n{} of {} cases measured.\n",
        measured.len(),
        measured.len() + unreproduced.len()
    ));
    if !unreproduced.is_empty() {
        report.push_str("\n### Not reproduced by MapServer\n\n");
        for line in unreproduced {
            report.push_str(&format!("- {line}\n"));
        }
    }
    report
}
