//! `forager render` end to end: a mapfile and GeoJSON in, a PNG out.

use std::path::{Path, PathBuf};
use std::process::Command;

use forager_style_ir::Rgba;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn a_mapfile_layer_is_rendered_from_geojson_to_png() {
    // Verifies: G-8
    let output = std::env::temp_dir().join(format!("forager-render-{}.png", std::process::id()));
    let areas = format!("areas={}", fixture("areas.geojson").display());
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(fixture("square.map"))
        .args(["--source", &areas, "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(
        stderr.contains("square.map:23:5: warning [layer-skipped]: layer `unbound`"),
        "{stderr}"
    );
    let image = forager_encode::decode_png(&std::fs::read(&output).unwrap()).unwrap();
    std::fs::remove_file(&output).unwrap();
    assert_eq!((image.width, image.height), (101, 101));
    assert_eq!(image.pixel(25, 75), Some(Rgba::opaque(200, 220, 180)));
    assert_eq!(image.pixel(50, 50), Some(Rgba::opaque(255, 255, 255)));
}

fn render(args: &[String]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(fixture("square.map"))
        .args(args)
        .args(["-o", "/dev/null"])
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn invalid_geojson_skips_its_layer_with_diagnostics() {
    // Verifies: G-8, Q-3
    let broken =
        std::env::temp_dir().join(format!("forager-broken-{}.geojson", std::process::id()));
    std::fs::write(&broken, "{\"type\": \"Point\"").unwrap();
    let (code, stderr) = render(&["--source".into(), format!("areas={}", broken.display())]);
    std::fs::remove_file(&broken).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stderr.contains("warning [source-unreadable]"), "{stderr}");
    assert!(
        stderr.contains("square.map:10:5: warning [layer-skipped]"),
        "{stderr}"
    );
}

#[test]
fn a_source_for_no_layer_is_reported_and_a_doubled_one_refused() {
    let areas = format!("areas={}", fixture("areas.geojson").display());
    let (code, stderr) = render(&["--source".into(), "rivers=x.geojson".into()]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stderr.contains("warning [source-unbound]"), "{stderr}");
    let (code, _) = render(&["--source".into(), areas.clone(), "--source".into(), areas]);
    assert_eq!(code, Some(2));
}

#[test]
fn the_cli_and_the_bindings_report_the_same_codes() {
    // The bindings' test of the same inputs reads the same expectation, in
    // `crates/bindings-wasm/src/lib.rs`. The sources are given out of the order of their names.
    // `unbound` names a layer of square.map whose source does not read (`source-unreadable`);
    // `area` names no layer (`source-unbound`).
    let broken =
        std::env::temp_dir().join(format!("forager-misbound-{}.geojson", std::process::id()));
    std::fs::write(&broken, "{").unwrap();
    let areas = fixture("areas.geojson");
    let (code, stderr) = render(&[
        "--source".into(),
        format!("unbound={}", broken.display()),
        "--source".into(),
        format!("areas={}", areas.display()),
        "--source".into(),
        format!("area={}", areas.display()),
    ]);
    std::fs::remove_file(&broken).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    let codes: Vec<_> = stderr
        .lines()
        .filter_map(|line| line.split_once(" [")?.1.split_once(']'))
        .map(|(code, _)| code)
        .collect();
    let expected = std::fs::read_to_string(fixture("misbound.codes")).unwrap();
    assert_eq!(codes, expected.lines().collect::<Vec<_>>(), "{stderr}");
}

/// A mapfile in a directory of its own whose point layer is labelled in `font`, with a font set
/// naming `sans`, which lies beside it, and `gone`, which does not.
fn labelled(font: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("forager-fonts-{}-{font}", std::process::id()));
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    // A test font of the label crate, a subset of DejaVu Sans whose licence lies beside it.
    let sans = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
    std::fs::copy(sans, dir.join("fonts/sans.ttf")).unwrap();
    std::fs::write(
        dir.join("fonts.lst"),
        "sans fonts/sans.ttf\ngone fonts/gone.ttf\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("points.geojson"),
        r#"{"type": "Point", "coordinates": [5, 5]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("m.map"),
        format!(
            "MAP SIZE 10 10 EXTENT 0 0 10 10 FONTSET 'fonts.lst'\n\
             LAYER NAME 'p' TYPE POINT STATUS ON\n\
             CLASS LABEL FONT '{font}' TEXT 'x' END END END END\n"
        ),
    )
    .unwrap();
    dir
}

#[test]
fn the_fonts_a_label_names_are_read_from_the_base_context() {
    // Verifies: ADR-0028
    for (font, unusable) in [("sans", false), ("gone", true)] {
        let dir = labelled(font);
        let output = Command::new(env!("CARGO_BIN_EXE_forager"))
            .arg("render")
            .arg(dir.join("m.map"))
            .arg("--source")
            .arg(format!("p={}", dir.join("points.geojson").display()))
            .args(["-o", "/dev/null"])
            .output()
            .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{stderr}");
        assert_eq!(
            stderr.contains("label-font-unusable") && stderr.contains("fonts/gone.ttf"),
            unusable,
            "{font}: {stderr}"
        );
    }
}
