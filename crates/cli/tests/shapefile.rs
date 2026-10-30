//! `forager render` of a mapfile whose local layers read shapefiles by their `DATA`, beneath the
//! map's `SHAPEPATH`, inside the base context.

use std::path::{Path, PathBuf};
use std::process::Command;

use forager_style_ir::Rgba;

const GREEN: Rgba = Rgba::opaque(200, 220, 180);
const WHITE: Rgba = Rgba::opaque(255, 255, 255);

/// A fresh directory for one test.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("forager-shp-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The `.shp`, `.shx`, and `.dbf` of one polygon, the square 0..50 in both axes.
fn square() -> [Vec<u8>; 3] {
    let ring = [
        (0.0, 0.0),
        (0.0, 50.0),
        (50.0, 50.0),
        (50.0, 0.0),
        (0.0, 0.0),
    ];
    let mut content = Vec::new();
    content.extend(5i32.to_le_bytes());
    for v in [0.0f64, 0.0, 50.0, 50.0] {
        content.extend(v.to_le_bytes());
    }
    content.extend(1i32.to_le_bytes());
    content.extend((ring.len() as i32).to_le_bytes());
    content.extend(0i32.to_le_bytes());
    for (x, y) in ring {
        content.extend(f64::to_le_bytes(x));
        content.extend(f64::to_le_bytes(y));
    }
    let header = |words: usize| {
        let mut h = vec![0u8; 100];
        h[0..4].copy_from_slice(&9994i32.to_be_bytes());
        h[24..28].copy_from_slice(&(words as i32).to_be_bytes());
        h[28..32].copy_from_slice(&1000i32.to_le_bytes());
        h[32..36].copy_from_slice(&5i32.to_le_bytes());
        for (i, v) in [0.0f64, 0.0, 50.0, 50.0].iter().enumerate() {
            h[36 + 8 * i..44 + 8 * i].copy_from_slice(&v.to_le_bytes());
        }
        h
    };
    let words = content.len() / 2;
    let mut shp = header(50 + 4 + words);
    shp.extend(1i32.to_be_bytes());
    shp.extend((words as i32).to_be_bytes());
    shp.extend(&content);
    let mut shx = header(50 + 4);
    shx.extend(50i32.to_be_bytes());
    shx.extend((words as i32).to_be_bytes());
    // One numeric field `ID` of width 4, and one record.
    let mut dbf = vec![3u8, 126, 1, 1];
    dbf.extend(1u32.to_le_bytes());
    dbf.extend(65u16.to_le_bytes());
    dbf.extend(5u16.to_le_bytes());
    dbf.extend([0u8; 20]);
    let mut field = [0u8; 32];
    field[..2].copy_from_slice(b"ID");
    field[11] = b'N';
    field[16] = 4;
    dbf.extend(field);
    dbf.push(0x0d);
    dbf.extend(b"    1");
    dbf.push(0x1a);
    [shp, shx, dbf]
}

/// Writes the square's three files as `<base>.<ext>` in `dir`, the extensions as given.
fn write_square(dir: &Path, base: &str, extensions: [&str; 3]) {
    std::fs::create_dir_all(dir.join(base).parent().unwrap()).unwrap();
    for (bytes, extension) in square().iter().zip(extensions) {
        std::fs::write(dir.join(format!("{base}.{extension}")), bytes).unwrap();
    }
}

/// A map of 101 × 101 pixels over 0..100 with one polygon layer named `poly`.
fn mapfile(shape_path: &str, data: &str) -> String {
    format!(
        "MAP\n  EXTENT 0 0 100 100\n  SIZE 101 101\n  IMAGECOLOR 255 255 255\n  {shape_path}\n  \
         LAYER\n    NAME 'poly'\n    TYPE POLYGON\n    STATUS ON\n    DATA '{data}'\n    \
         CLASS STYLE COLOR 200 220 180 END END\n  END\nEND\n"
    )
}

/// Renders `map` with `args`, and returns the exit code, standard error, and the image.
fn render(map: &Path, args: &[String]) -> (Option<i32>, String, Option<forager_render::RgbaImage>) {
    let output = map.with_extension("png");
    let result = Command::new(env!("CARGO_BIN_EXE_forager"))
        .arg("render")
        .arg(map)
        .args(args)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    let image = std::fs::read(&output)
        .ok()
        .map(|png| forager_encode::decode_png(&png).unwrap());
    (
        result.status.code(),
        String::from_utf8(result.stderr).unwrap(),
        image,
    )
}

#[test]
fn a_layer_draws_its_shapefile_beneath_shapepath() {
    let dir = scratch("shapepath");
    write_square(&dir, "data/poly", ["shp", "shx", "dbf"]);
    std::fs::write(dir.join("m.map"), mapfile("SHAPEPATH 'data'", "poly")).unwrap();
    let (code, stderr, image) = render(&dir.join("m.map"), &[]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(stderr, "");
    let image = image.unwrap();
    assert_eq!(image.pixel(25, 75), Some(GREEN));
    assert_eq!(image.pixel(75, 25), Some(WHITE));
}

#[test]
fn a_shapepath_that_climbs_stays_inside_the_base_context() {
    // As the suite's renderers read ../misc/data, with the suite as the base context.
    let dir = scratch("climbs");
    write_square(&dir, "misc/data/poly", ["shp", "shx", "dbf"]);
    std::fs::create_dir_all(dir.join("renderers")).unwrap();
    let map = dir.join("renderers/m.map");
    std::fs::write(&map, mapfile("SHAPEPATH '../misc/data'", "poly.shp")).unwrap();
    let (code, stderr, image) = render(&map, &["--base".into(), dir.display().to_string()]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(image.unwrap().pixel(25, 75), Some(GREEN));
}

#[test]
fn a_data_path_that_leaves_the_base_context_is_refused_and_its_layer_skipped() {
    // Verifies: Q-8, G-8
    // The square lies beside the base context; nothing outside it is read.
    let dir = scratch("leaves");
    write_square(&dir, "poly", ["shp", "shx", "dbf"]);
    std::fs::create_dir_all(dir.join("base")).unwrap();
    let map = dir.join("base/m.map");
    for (shape_path, data) in [("", "../poly"), ("SHAPEPATH '..'", "poly")] {
        std::fs::write(&map, mapfile(shape_path, data)).unwrap();
        let (code, stderr, image) = render(&map, &[]);
        assert_eq!(code, Some(0), "{stderr}");
        assert!(stderr.contains("warning [data-refused]"), "{stderr}");
        assert!(
            stderr.contains("warning [layer-skipped]: layer `poly`"),
            "{stderr}"
        );
        assert_eq!(image.unwrap().pixel(25, 75), Some(WHITE));
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn upper_case_files_are_read_where_lower_case_ones_are_missing() {
    let dir = scratch("upper");
    write_square(&dir, "POLY", ["SHP", "SHX", "DBF"]);
    std::fs::write(dir.join("m.map"), mapfile("", "POLY")).unwrap();
    let (code, stderr, image) = render(&dir.join("m.map"), &[]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(image.unwrap().pixel(25, 75), Some(GREEN));
}

#[test]
fn a_broken_shapefile_skips_its_layer_naming_layer_kind_and_span() {
    // Verifies: G-8
    let dir = scratch("broken");
    write_square(&dir, "poly", ["shp", "shx", "dbf"]);
    // A .shx shorter than its 100-byte header is an error of the source.
    std::fs::write(dir.join("poly.shx"), [0u8; 10]).unwrap();
    std::fs::write(dir.join("m.map"), mapfile("", "poly")).unwrap();
    let (code, stderr, image) = render(&dir.join("m.map"), &[]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stderr.contains("warning [source-unreadable]"), "{stderr}");
    assert!(
        stderr.contains("m.map:10:5: warning [layer-skipped]: layer `poly` (source local)"),
        "{stderr}"
    );
    assert!(stderr.contains("local"), "{stderr}");
    assert_eq!(image.unwrap().pixel(25, 75), Some(WHITE));
}

#[test]
fn layers_that_name_one_shapefile_both_draw() {
    // The two layers share the shapefile the CLI opened once; each draws it in its own style.
    let dir = scratch("shared");
    write_square(&dir, "poly", ["shp", "shx", "dbf"]);
    let layer = |name: &str, data: &str, color: &str| {
        format!(
            "  LAYER\n    NAME '{name}'\n    TYPE POLYGON\n    STATUS ON\n    DATA '{data}'\n    \
             CLASS STYLE {color} END END\n  END\n"
        )
    };
    let text = format!(
        "MAP\n  EXTENT 0 0 100 100\n  SIZE 101 101\n  IMAGECOLOR 255 255 255\n{}{}END\n",
        layer("fill", "poly", "COLOR 200 220 180"),
        layer("outline", "poly.shp", "OUTLINECOLOR 0 0 0 WIDTH 3"),
    );
    std::fs::write(dir.join("m.map"), text).unwrap();
    let (code, stderr, image) = render(&dir.join("m.map"), &[]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(stderr, "");
    let image = image.unwrap();
    assert_eq!(image.pixel(25, 75), Some(GREEN));
    assert_eq!(image.pixel(50, 75), Some(Rgba::opaque(0, 0, 0)));
}

#[test]
fn a_layer_the_engine_does_not_draw_opens_no_shapefile() {
    // Verifies: Q-9
    // The .shp is broken, so a layer bound to it would report source-unreadable: an OFF layer,
    // one without STATUS, and a raster beside a .shp of its DATA's name open nothing.
    let dir = scratch("undrawn");
    write_square(&dir, "img", ["shp", "shx", "dbf"]);
    std::fs::write(dir.join("img.shp"), [0u8; 10]).unwrap();
    for (kind, status) in [
        ("POLYGON", "STATUS OFF"),
        ("POLYGON", ""),
        ("RASTER", "STATUS ON"),
    ] {
        let text = format!(
            "MAP\n  EXTENT 0 0 100 100\n  SIZE 101 101\n  LAYER\n    NAME 'l'\n    TYPE {kind}\n    \
             {status}\n    DATA 'img.tif'\n    CLASS STYLE COLOR 200 220 180 END END\n  END\nEND\n"
        );
        std::fs::write(dir.join("m.map"), text).unwrap();
        let (code, stderr, _) = render(&dir.join("m.map"), &[]);
        assert_eq!(code, Some(0), "{kind} {status}: {stderr}");
        assert!(
            !stderr.contains("source-unreadable"),
            "{kind} {status}: {stderr}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The CLI's data-file limit, `MAX_SOURCE_BYTES` (Q-9).
const DATA_FILE_LIMIT: u64 = 256 * 1024 * 1024;

#[test]
fn each_file_of_a_shapefile_past_the_data_file_limit_skips_its_layer() {
    // Verifies: Q-9
    // Each of the three files in turn is one byte past the limit, sparse so that the fixture
    // takes no disk space; the files before it in the adapter's order are whole.
    let dir = scratch("limit");
    std::fs::write(dir.join("m.map"), mapfile("", "poly")).unwrap();
    for extension in ["shp", "shx", "dbf"] {
        write_square(&dir, "poly", ["shp", "shx", "dbf"]);
        std::fs::File::options()
            .write(true)
            .open(dir.join(format!("poly.{extension}")))
            .unwrap()
            .set_len(DATA_FILE_LIMIT + 1)
            .unwrap();
        let (code, stderr, image) = render(&dir.join("m.map"), &[]);
        assert_eq!(code, Some(0), "{extension}: {stderr}");
        assert!(
            stderr.contains(&format!(
                "warning [source-unreadable]: source of layer `poly`: poly: the shapefile file \
                 poly.{extension} cannot be read: the file exceeds the limit of {DATA_FILE_LIMIT} \
                 bytes"
            )),
            "{extension}: {stderr}"
        );
        assert_eq!(image.unwrap().pixel(25, 75), Some(WHITE), "{extension}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_layer_without_its_shapefile_is_skipped_without_a_source_error() {
    // As the browser bindings report a layer they bind nothing to.
    let dir = scratch("missing");
    std::fs::write(dir.join("m.map"), mapfile("", "poly")).unwrap();
    let (code, stderr, _) = render(&dir.join("m.map"), &[]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert!(!stderr.contains("source-unreadable"), "{stderr}");
    assert!(
        stderr.contains("warning [layer-skipped]: layer `poly`"),
        "{stderr}"
    );
}

#[test]
fn a_geojson_source_wins_over_the_layers_shapefile() {
    let dir = scratch("wins");
    write_square(&dir, "poly", ["shp", "shx", "dbf"]);
    std::fs::write(dir.join("m.map"), mapfile("", "poly")).unwrap();
    // A GeoJSON square in the other corner, 50..100.
    let geojson = dir.join("other.geojson");
    std::fs::write(
        &geojson,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},
        "geometry":{"type":"Polygon","coordinates":[[[50,50],[50,100],[100,100],[100,50],[50,50]]]}}]}"#,
    )
    .unwrap();
    let source = format!("poly={}", geojson.display());
    let (code, stderr, image) = render(&dir.join("m.map"), &["--source".into(), source]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    let image = image.unwrap();
    assert_eq!(image.pixel(75, 25), Some(GREEN));
    assert_eq!(image.pixel(25, 75), Some(WHITE));
}

/// Standard error with the scratch directory's name taken out, to compare two runs.
#[cfg(unix)]
fn quiet(stderr: &str, dir: &Path) -> String {
    stderr.replace(&dir.display().to_string(), "<dir>")
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_base_context_tells_nothing_of_what_it_points_at() {
    // Verifies: Q-8
    // A link to a file outside that exists and one to a file outside that does not report alike,
    // for the .shp, which is then not bound, and for a sibling the adapter loads.
    let dir = scratch("links");
    write_square(&dir, "outside/poly", ["shp", "shx", "dbf"]);
    let base = dir.join("base");
    let mut runs = Vec::new();
    for target in ["poly", "missing"] {
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("m.map"), mapfile("", "a")).unwrap();
        std::os::unix::fs::symlink(
            dir.join(format!("outside/{target}.shp")),
            base.join("a.shp"),
        )
        .unwrap();
        write_square(&base, "b", ["shp", "shx", "dbf"]);
        std::fs::remove_file(base.join("b.shx")).unwrap();
        std::os::unix::fs::symlink(
            dir.join(format!("outside/{target}.shx")),
            base.join("b.shx"),
        )
        .unwrap();
        let (_, linked_shp, _) = render(&base.join("m.map"), &[]);
        std::fs::write(base.join("m.map"), mapfile("", "b")).unwrap();
        let (_, linked_shx, _) = render(&base.join("m.map"), &[]);
        runs.push((quiet(&linked_shp, &dir), quiet(&linked_shx, &dir)));
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(!runs[0].0.contains("source-unreadable"), "{}", runs[0].0);
    assert!(
        runs[0]
            .1
            .contains("no such file, or the path leaves the base context"),
        "{}",
        runs[0].1
    );
    assert_eq!(runs[0], runs[1]);
}

#[test]
fn a_layer_with_inline_features_draws_them_and_leaves_its_data_unread() {
    // ADR-0033: an inline layer's DATA is not bound, so nothing is read and nothing reported,
    // as MapServer ignores it (msInitializeVirtualTable).
    let dir = scratch("inline");
    write_square(&dir, "poly", ["shp", "shx", "dbf"]);
    let map = mapfile("", "poly").replace(
        "    CLASS",
        "    FEATURE POINTS 60 60 90 60 90 90 60 90 60 60 END END\n    CLASS",
    );
    std::fs::write(dir.join("m.map"), map).unwrap();
    let (code, stderr, image) = render(&dir.join("m.map"), &[]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(stderr, "");
    let image = image.unwrap();
    assert_eq!(image.pixel(75, 25), Some(GREEN), "the inline feature");
    assert_eq!(
        image.pixel(25, 75),
        Some(WHITE),
        "the shapefile is not drawn"
    );
}
