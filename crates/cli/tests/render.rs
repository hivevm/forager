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

#[test]
fn invalid_geojson_fails_without_a_panic() {
    // Verifies: Q-3
    let broken =
        std::env::temp_dir().join(format!("forager-broken-{}.geojson", std::process::id()));
    std::fs::write(&broken, "{\"type\": \"Point\"").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(fixture("square.map"))
        .arg("--source")
        .arg(format!("areas={}", broken.display()))
        .args(["-o", "/dev/null"])
        .output()
        .unwrap();
    std::fs::remove_file(&broken).unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("invalid GeoJSON")
    );
}
