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
    let document = StyleDocument::from_yaml(&stdout).unwrap().document;
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
        StyleDocument::from_yaml(&yaml).unwrap().document,
        StyleDocument::from_json(&json).unwrap().document
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
fn a_base_context_above_the_mapfile_lets_an_include_climb_inside_it() {
    // Verifies: Q-8
    let mapfile = fixture("nested/escape.map");
    let base = fixture("");
    let output = forager(&[
        "read",
        mapfile.to_str().unwrap(),
        "--base",
        base.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    let document = StyleDocument::from_yaml(&String::from_utf8(output.stdout).unwrap())
        .unwrap()
        .document;
    assert_eq!(
        document
            .map
            .span
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("nested/escape.map:1:1")
    );
    assert_eq!(document.map.layers[0].name.as_deref(), Some("countries"));
}

#[test]
fn an_include_that_climbs_out_of_the_base_context_is_refused() {
    // Verifies: Q-8
    let mapfile = fixture("nested/escape.map");
    let base = fixture("nested");
    let output = forager(&[
        "read",
        mapfile.to_str().unwrap(),
        "--base",
        base.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("error [include-refused]"), "{stderr}");
}

#[test]
fn a_mapfile_outside_the_base_context_is_an_error_naming_both() {
    let mapfile = fixture("world.map");
    let base = fixture("nested");
    let output = forager(&[
        "read",
        mapfile.to_str().unwrap(),
        "--base",
        base.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("world.map"), "{stderr}");
    assert!(
        stderr.contains("outside the base context") && stderr.contains("nested"),
        "{stderr}"
    );
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

#[cfg(unix)]
#[test]
fn a_path_that_is_not_utf8_is_an_error_not_a_panic() {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::OsStr::from_bytes(b"/nonexistent/\xff.map");
    let output = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("read")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_out_of_the_mapfile_directory_is_refused() {
    // Verifies: Q-8
    let root = std::env::temp_dir().join(format!("forager-symlink-{}", std::process::id()));
    let base = root.join("base");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(root.join("secret.inc"), "NAME 'secret'\n").unwrap();
    std::os::unix::fs::symlink(root.join("secret.inc"), base.join("link.inc")).unwrap();
    std::fs::write(base.join("m.map"), "MAP INCLUDE 'link.inc' END\n").unwrap();
    let output = forager(&["read", base.join("m.map").to_str().unwrap()]);
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("the path leaves the base context"),
        "{stderr}"
    );
}

#[test]
fn a_built_in_epsg_code_resolves_without_a_table() {
    // Verifies: ADR-0018
    let output = read("wgs84.map", "json");
    assert!(output.status.success(), "{output:?}");
    let document = StyleDocument::from_json(&String::from_utf8(output.stdout).unwrap())
        .unwrap()
        .document;
    assert_eq!(
        document.map.projection.as_deref(),
        Some("+proj=longlat +datum=WGS84 +no_defs")
    );
}

#[test]
fn other_epsg_codes_need_the_hosts_table() {
    // Verifies: ADR-0018
    let without = read("national-grid.map", "json");
    assert_eq!(without.status.code(), Some(1));
    assert!(
        String::from_utf8(without.stderr)
            .unwrap()
            .contains("[projection-unresolved]")
    );

    let mapfile = fixture("national-grid.map");
    let table = fixture("national-grid.epsg");
    let with = forager(&[
        "read",
        mapfile.to_str().unwrap(),
        "--format",
        "json",
        "--epsg",
        table.to_str().unwrap(),
    ]);
    assert!(with.status.success(), "{with:?}");
    let document = StyleDocument::from_json(&String::from_utf8(with.stdout).unwrap())
        .unwrap()
        .document;
    assert!(
        document
            .map
            .projection
            .unwrap()
            .starts_with("+proj=tmerc +lat_0=49")
    );
}

#[test]
fn an_unusable_epsg_table_is_reported() {
    let mapfile = fixture("wgs84.map");
    let broken = fixture("broken.epsg");
    let output = forager(&[
        "read",
        mapfile.to_str().unwrap(),
        "--epsg",
        broken.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("line 2:")
    );
}

#[cfg(unix)]
#[test]
fn an_endless_epsg_file_is_cut_at_the_limit() {
    // Verifies: Q-9
    let mapfile = fixture("wgs84.map");
    let output = forager(&["read", mapfile.to_str().unwrap(), "--epsg", "/dev/zero"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("exceeds the limit")
    );
}
