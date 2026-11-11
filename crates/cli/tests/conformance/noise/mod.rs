//! The noise measurement of G-6 and G-13 (ADR-0033): MapServer at the pinned release renders the
//! cases in scope, and the harness compares its renderings with the references and the labels of
//! its label cache with the records beside the list. Only `.github/workflows/noise.yml` builds
//! MapServer; the measurement runs where `FORAGER_MAP2IMG` and `FORAGER_MAPSERVER_LABELS` name it
//! and skips itself where neither does, so the push checks never run it, whatever it is named.

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
/// `noise/` under the test's target directory.
#[test]
#[ignore = "needs MapServer; .github/workflows/noise.yml builds it and runs this test, and \
            without it the test skips itself"]
fn mapserver_renders_the_cases_in_scope_within_its_noise() {
    // Verifies: ADR-0033
    let mapserver = mapserver(
        std::env::var_os("FORAGER_MAP2IMG"),
        std::env::var_os("FORAGER_MAPSERVER_LABELS"),
    )
    .unwrap();
    let Some((map2img, reader)) = mapserver else {
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
}

/// `map2img` and the reader of MapServer's label cache, as `FORAGER_MAP2IMG` and
/// `FORAGER_MAPSERVER_LABELS` name them, or none where neither is set: the noise measurement runs
/// where both name a file and skips itself where neither does. One without the other, or one
/// empty, is an error, so that a noise run whose build named nothing does not pass unmeasured.
fn mapserver(
    map2img: Option<OsString>,
    reader: Option<OsString>,
) -> Result<Option<(PathBuf, PathBuf)>, String> {
    match (map2img, reader) {
        (None, None) => Ok(None),
        (Some(map2img), Some(reader)) if !map2img.is_empty() && !reader.is_empty() => {
            Ok(Some((PathBuf::from(map2img), PathBuf::from(reader))))
        }
        _ => Err(
            "FORAGER_MAP2IMG and FORAGER_MAPSERVER_LABELS must both name a file, as \
                  .github/workflows/noise.yml sets them, or neither be set"
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
         skipped without FORAGER_MAP2IMG and FORAGER_MAPSERVER_LABELS"
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
