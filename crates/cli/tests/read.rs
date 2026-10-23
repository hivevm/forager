//! The CLI end to end: a mapfile in, the style document and its diagnostics out.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use forager_style_ir::StyleDocument;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn forager(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forager"))
        .args(args)
        .output()
        .expect("the forager binary runs")
}

fn read(name: &str, format: &str) -> Output {
    let path = fixture(name);
    forager(&["read", path.to_str().unwrap(), "--format", format])
}

#[test]
fn a_mapfile_is_printed_as_a_style_document_with_its_warnings() {
    // Verifies: G-2, ADR-0008
    let output = read("world.map", "yaml");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("version: \"1.0\"\n"), "{stdout}");
    let document = StyleDocument::from_yaml(&stdout).unwrap();
    let layer = &document.map.layers[0];
    assert_eq!(layer.name.as_deref(), Some("countries"));
    assert_eq!(
        layer.span.as_ref().map(ToString::to_string).as_deref(),
        Some("layers/countries.inc:1:1")
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("world.map:11:3: warning [discarded-keyword]"),
        "{stderr}"
    );
    assert!(
        stderr.contains("layers/countries.inc:6:3: warning [unsupported-keyword]"),
        "{stderr}"
    );
}

#[test]
fn yaml_and_json_carry_the_same_style_ir() {
    // Verifies: G-2
    let yaml = String::from_utf8(read("world.map", "yaml").stdout).unwrap();
    let json = String::from_utf8(read("world.map", "json").stdout).unwrap();
    assert_eq!(
        StyleDocument::from_yaml(&yaml).unwrap(),
        StyleDocument::from_json(&json).unwrap()
    );
}

#[test]
fn a_syntax_error_fails_with_its_location() {
    // Verifies: G-2
    let output = read("broken.map", "yaml");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("broken.map:2:8: error [unterminated-string]"),
        "{stderr}"
    );
}

#[test]
fn an_include_outside_the_mapfile_directory_is_refused() {
    // Verifies: Q-8
    let output = read("nested/escape.map", "yaml");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("error [include-refused]"), "{stderr}");
}

#[test]
fn a_missing_mapfile_and_bad_arguments_are_reported() {
    assert_eq!(read("missing.map", "yaml").status.code(), Some(1));
    assert_eq!(forager(&["read"]).status.code(), Some(2));
    assert_eq!(
        forager(&["read", "x.map", "--format", "xml"]).status.code(),
        Some(2)
    );
    assert_eq!(forager(&["frobnicate"]).status.code(), Some(2));
}
