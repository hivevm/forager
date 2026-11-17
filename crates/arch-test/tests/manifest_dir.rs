//! A test that reads the tree, under `tests/` or in a crate's `src/`, finds it at run time: `cargo test` sets `CARGO_MANIFEST_DIR` for
//! the package it runs, and the path compiled in is only the fallback where it is unset. A binary
//! built in another checkout over the same `target/` holds that checkout's path, which may be gone,
//! and Cargo may judge the binary fresh; a test that walks the tree there passes or fails on files
//! it did not read.

use std::path::{Path, PathBuf};

/// This crate's directory: the run time's `CARGO_MANIFEST_DIR`, else the compiled-in path.
fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

/// The compiled-in path, written so that this file holds no use of it.
const COMPILED: &str = concat!("env!(\"CARGO_", "MANIFEST_DIR\")");

/// The run time's variable, read on the line before the fallback.
const RUN_TIME: &str = concat!("var_os(\"CARGO_", "MANIFEST_DIR\")");

/// The one line that may use the compiled-in path, its indentation and a closing `;` aside.
fn fallback() -> String {
    format!(".map_or_else(|| PathBuf::from({COMPILED}), PathBuf::from)")
}

/// The lines, from 1, of `source` that use the compiled-in path other than as the fallback behind
/// the run time's variable, and the number of fallbacks.
fn uses(source: &str) -> (Vec<usize>, usize) {
    let lines: Vec<&str> = source.lines().collect();
    let mut wrong = Vec::new();
    let mut fallbacks = 0;
    for (i, line) in lines.iter().enumerate() {
        if !line.contains(COMPILED) {
            continue;
        }
        let behind = i > 0 && lines[i - 1].trim_end().ends_with(RUN_TIME);
        if behind && line.trim().trim_end_matches(';') == fallback() {
            fallbacks += 1;
        } else {
            wrong.push(i + 1);
        }
    }
    (wrong, fallbacks)
}

/// Every `.rs` file under `dir`, `target` directories left out.
fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.map(Result::unwrap) {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() != "target" {
                rust_files(&path, files);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn the_crates_take_the_manifest_dir_from_the_run_time_first() {
    let crates = manifest_dir().join("..");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    files.sort();
    let mut wrong = Vec::new();
    let mut fallbacks = 0;
    for file in &files {
        let (lines, found) = uses(&std::fs::read_to_string(file).unwrap());
        fallbacks += found;
        let shown = file.strip_prefix(&crates).unwrap_or(file).display();
        wrong.extend(lines.into_iter().map(|l| format!("{shown}:{l}")));
    }
    assert!(
        wrong.is_empty(),
        "the compiled-in path read first: {wrong:#?}"
    );
    // The check itself sees the fallbacks: this file holds one.
    assert!(
        fallbacks > 0,
        "no fallback found under {}",
        crates.display()
    );
}

#[test]
fn the_check_accepts_the_fallback_alone() {
    // The fallback behind the variable; the compiled-in path alone; the fallback behind no
    // variable; another use behind the variable.
    let source = format!(
        "    std::env::{RUN_TIME}\n        {}\nlet root = Path::new({COMPILED});\n\
         let a = 1;\n        {}\n    std::env::{RUN_TIME}\n        .unwrap_or(PathBuf::from({COMPILED}).into())\n",
        fallback(),
        fallback()
    );
    assert_eq!(uses(&source), (vec![3, 5, 7], 1));
}
