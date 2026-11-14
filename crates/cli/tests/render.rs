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

/// The CLI's data-file limit, `MAX_SOURCE_BYTES` (Q-9).
const DATA_FILE_LIMIT: u64 = 256 * 1024 * 1024;

#[test]
fn a_geojson_source_past_the_data_file_limit_skips_its_layer() {
    // Verifies: Q-9
    // One byte past the limit, sparse on disk.
    let big = std::env::temp_dir().join(format!("forager-big-{}.geojson", std::process::id()));
    std::fs::File::create(&big)
        .and_then(|f| f.set_len(DATA_FILE_LIMIT + 1))
        .unwrap();
    let (code, stderr) = render(&["--source".into(), format!("areas={}", big.display())]);
    std::fs::remove_file(&big).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "warning [source-unreadable]: source of layer `areas`: {}: the file exceeds the \
             limit of {DATA_FILE_LIMIT} bytes",
            big.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains("square.map:10:5: warning [layer-skipped]: layer `areas`"),
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
    // Verifies: ADR-0031
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

#[test]
fn a_numeric_in_list_reads_its_items_as_atof_does() {
    // Verifies: ADR-0017
    // The grammar's `math_exp IN string_exp` compares the number with `atof` of
    // each item, so `0x10` reads 16, as the frontend folds it at load and `expr` reads it.
    let dir = std::env::temp_dir().join(format!("forager-in-hex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("in.map"),
        r#"MAP
  EXTENT 0 0 100 100
  SIZE 101 101
  IMAGECOLOR 255 255 255
  LAYER
    NAME "areas"
    TYPE POLYGON
    STATUS ON
    CLASS
      EXPRESSION ([n] IN "0x10,1")
      STYLE
        COLOR 255 0 0
      END
    END
  END
END
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("areas.geojson"),
        r#"{"type": "Feature", "properties": {"n": 16}, "geometry": {"type": "Polygon",
            "coordinates": [[[10, 10], [90, 10], [90, 90], [10, 90], [10, 10]]]}}"#,
    )
    .unwrap();
    let output = dir.join("in.png");
    let areas = format!("areas={}", dir.join("areas.geojson").display());
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(dir.join("in.map"))
        .args(["--source", &areas, "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let image = forager_encode::decode_png(&std::fs::read(&output).unwrap()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(image.pixel(50, 50), Some(Rgba::opaque(255, 0, 0)));
}

#[test]
fn a_pixmap_symbol_is_read_from_the_base_context_and_drawn_at_its_point() {
    // pixmap.map draws pin.png, beside it, at the pixel (50.5, 41.5): the engine's reference
    // image of the same map, which the browser bindings' test reads too, so that both hosts draw
    // it byte for byte alike.
    let output = std::env::temp_dir().join(format!("forager-pixmap-{}.png", std::process::id()));
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(fixture("pixmap.map"))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(result.status.success(), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    let image = forager_encode::decode_png(&std::fs::read(&output).unwrap()).unwrap();
    std::fs::remove_file(&output).unwrap();
    let reference =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/tests/fixtures/pixmap_point.png");
    let reference = forager_encode::decode_png(&std::fs::read(reference).unwrap()).unwrap();
    assert!(
        image == reference,
        "pixmap.map differs from the engine's reference"
    );
}

#[test]
fn a_pixmap_file_past_32_mib_is_not_drawn_and_the_map_is() {
    // Verifies: Q-9
    // The CLI's loader reads an image no further than the engine's bound on a file, 32 MiB, and
    // refuses one byte more: the style is not drawn, with the bound's warning, and the square
    // of the next style is.
    let dir = std::env::temp_dir().join(format!("forager-pixmap-big-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // One byte past the limit, sparse on disk.
    std::fs::File::create(dir.join("big.png"))
        .and_then(|f| f.set_len(32 * 1024 * 1024 + 1))
        .unwrap();
    std::fs::write(
        dir.join("m.map"),
        "MAP SIZE 21 21 EXTENT 0 0 20 20 IMAGECOLOR 255 255 255\n\
         SYMBOL NAME 'big' TYPE PIXMAP IMAGE 'big.png' END\n\
         LAYER NAME 'p' TYPE POINT STATUS ON FEATURE POINTS 10 10 END END\n\
         CLASS STYLE SYMBOL 'big' END STYLE COLOR 0 0 255 SIZE 5 END END END END\n",
    )
    .unwrap();
    let output = dir.join("m.png");
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(dir.join("m.map"))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    let image = std::fs::read(&output).map(|png| forager_encode::decode_png(&png).unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(result.status.success(), "{stderr}");
    assert!(stderr.contains("warning [pixmap-not-drawn]"), "{stderr}");
    assert!(
        stderr.contains("`big`") && stderr.contains("big.png"),
        "{stderr}"
    );
    assert!(
        stderr.contains("the file exceeds the limit of 33554432 bytes"),
        "{stderr}"
    );
    let image = image.unwrap();
    assert_eq!(image.pixel(10, 10), Some(Rgba::opaque(0, 0, 255)));
    assert_eq!(image.pixel(1, 1), Some(Rgba::opaque(255, 255, 255)));
}

#[test]
fn a_style_naming_an_image_file_draws_it_and_one_missing_or_by_url_draws_nothing() {
    // MapServer's msAddImageSymbol reads a name no symbol carries as an image file beside the
    // mapfile: `pin.png` is drawn at (10, 10); `nowhere` and the quoted `'1'`, files that do not
    // exist, warn as they are drawn and are not, and the square of the layer after them is; a
    // name by URL warns at load, is never fetched, and takes its index as a symbol without an
    // image, which warns as it is drawn and draws nothing.
    let dir = std::env::temp_dir().join(format!("forager-by-file-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("maps")).unwrap();
    std::fs::copy(fixture("pin.png"), dir.join("maps/pin.png")).unwrap();
    let point = |at: &str, styles: &str| {
        format!("LAYER TYPE POINT STATUS ON FEATURE POINTS {at} END END CLASS {styles} END END\n")
    };
    let map = [
        "MAP SIZE 41 41 EXTENT 0 0 40 40 IMAGECOLOR 255 255 255\n".to_owned(),
        point("10 10", "STYLE SYMBOL 'pin.png' END"),
        point(
            "30 10",
            "STYLE SYMBOL 'http://localhost:8000/x.png' COLOR 255 0 0 SIZE 5 END",
        ),
        point(
            "10 30",
            "STYLE SYMBOL 'nowhere' SIZE 5 END STYLE SYMBOL '1' SIZE 5 END",
        ),
        point("30 30", "STYLE COLOR 0 0 255 SIZE 5 END"),
        "END\n".to_owned(),
    ];
    std::fs::write(dir.join("maps/m.map"), map.concat()).unwrap();
    let output = dir.join("m.png");
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(dir.join("maps/m.map"))
        .arg("-o")
        .arg(&output)
        .current_dir(&dir)
        .output()
        .unwrap();
    let image = std::fs::read(&output).map(|png| forager_encode::decode_png(&png).unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(result.status.success(), "{stderr}");
    assert_eq!(
        stderr.matches("warning [symbol-unknown]").count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains("http://localhost:8000/x.png"), "{stderr}");
    for name in ["`nowhere`", "`1`"] {
        assert!(stderr.contains(name), "{name}: {stderr}");
    }
    assert_eq!(
        stderr.matches("warning [pixmap-not-drawn]").count(),
        3,
        "{stderr}"
    );
    let image = image.unwrap();
    let white = Some(Rgba::opaque(255, 255, 255));
    let pin = (0..20).any(|x| (21..41).any(|y| image.pixel(x, y) != white));
    assert!(pin, "the pin");
    assert_eq!(image.pixel(30, 30), white, "the name by URL");
    assert_eq!(image.pixel(10, 10), white, "the files that do not exist");
    assert_eq!(image.pixel(30, 10), Some(Rgba::opaque(0, 0, 255)));
}

#[test]
fn a_class_style_of_labelcenter_is_warned_of_once_as_the_mapfile_loads_and_renders() {
    // The engine warns of a line layer's class style of labelcenter, which draws nothing, for a
    // mapfile as for a style document; the frontend does not warn of it again.
    let dir = std::env::temp_dir().join(format!("forager-labelcenter-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("centre.map"),
        r#"MAP
  EXTENT 0 0 100 100
  SIZE 101 101
  LAYER
    NAME "lines"
    TYPE LINE
    STATUS ON
    CLASS
      STYLE
        COLOR 255 0 0
        GEOMTRANSFORM labelcenter
      END
    END
  END
END
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("lines.geojson"),
        r#"{"type": "LineString", "coordinates": [[0, 50], [100, 50]]}"#,
    )
    .unwrap();
    let lines = format!("lines={}", dir.join("lines.geojson").display());
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(dir.join("centre.map"))
        .args(["--source", &lines, "-o", "/dev/null"])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(result.status.success(), "{stderr}");
    let warnings: Vec<_> = stderr.lines().filter(|l| l.contains("warning [")).collect();
    assert_eq!(warnings.len(), 1, "{stderr}");
    assert!(
        warnings[0].contains("centre.map:9:7: warning [geomtransform-not-drawn]"),
        "{stderr}"
    );
}

#[test]
fn a_label_style_of_a_name_that_names_no_transform_is_drawn_at_the_label_point() {
    // MapServer gives a label's style of a name that names no transform its default, labelpnt
    // (docs/semantics/label.md, and the frontend's test of the same name, which cites the C): the
    // map renders as it does without the GEOMTRANSFORM, the marker drawn at the label's point, and
    // the load warns of the name once, at its directive.
    let dir = std::env::temp_dir().join(format!("forager-unknown-name-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    let sans = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../label/tests/fixtures/dejavu-sans-subset.ttf");
    std::fs::copy(sans, dir.join("fonts/sans.ttf")).unwrap();
    std::fs::write(dir.join("fonts.lst"), "sans fonts/sans.ttf\n").unwrap();
    let map = |transform: &str| {
        format!(
            "MAP SIZE 41 41 EXTENT 0 0 40 40 IMAGECOLOR 255 255 255 FONTSET 'fonts.lst'\n\
             SYMBOL NAME 'circle' TYPE ELLIPSE POINTS 1 1 END FILLED TRUE END\n\
             LAYER NAME 'p' TYPE POINT STATUS ON FEATURE POINTS 20 20 END END\n\
             CLASS LABEL FONT 'sans' TEXT 'x' POSITION UR\n\
             STYLE {transform} SYMBOL 'circle' SIZE 9 COLOR 255 0 0 END\n\
             END END END END\n"
        )
    };
    let draw = |name: &str, transform: &str| {
        std::fs::write(dir.join(name), map(transform)).unwrap();
        let output = dir.join(format!("{name}.png"));
        let result = Command::new(env!("CARGO_BIN_EXE_forager"))
            .arg("render")
            .arg(dir.join(name))
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(result.status.success(), "{name}: {stderr}");
        let image = forager_encode::decode_png(&std::fs::read(&output).unwrap()).unwrap();
        (image, stderr)
    };
    let (unknown, stderr) = draw("foo.map", "GEOMTRANSFORM \"foo\"");
    let (plain, plain_stderr) = draw("plain.map", "");
    std::fs::remove_dir_all(&dir).unwrap();
    let warnings: Vec<_> = stderr.lines().filter(|l| l.contains("warning [")).collect();
    assert_eq!(warnings.len(), 1, "{stderr}");
    assert!(
        warnings[0].contains("foo.map:5:7: warning [unsupported-value]"),
        "{stderr}"
    );
    assert!(!plain_stderr.contains("warning ["), "{plain_stderr}");
    assert_eq!(
        unknown.pixel(20, 20),
        Some(Rgba::opaque(255, 0, 0)),
        "the marker at the label point"
    );
    assert!(unknown == plain, "drawn as without the GEOMTRANSFORM");
}
