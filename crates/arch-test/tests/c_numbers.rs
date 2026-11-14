//! C's and MapServer's small ports live once (ADR-0017): `atof`,
//! `atoi`, `strtod`, `lrint`, C's `(int)` cast of a `double` and its string end, and MapServer's
//! `MS_NINT`, `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, `msStringSplit`,
//! `msComputeBounds`, `msGetSymbolIndex` with the key of its match of a name, and the alpha of an
//! opacity are defined in the module of `style-ir` and nowhere else in the workspace.

use std::path::{Path, PathBuf};

/// The one module that may define them, from the workspace root.
const MODULE: &str = "crates/style-ir/src/c.rs";

/// The names of the module's ports.
const PORTS: [&str; 16] = [
    "atof",
    "atoi",
    "strtod",
    "lrint",
    "ms_nint",
    "int_cast",
    "ms_min",
    "ms_max",
    "rect_overlap",
    "ms_string_split",
    "ms_compute_bounds",
    "opacity_alpha",
    "ms_get_symbol_index",
    "symbol_name_key",
    "c_bytes",
    "c_str",
];

/// Whether a function of this name ports one of them, a variant such as `lrint_int` included.
fn ports_a_c_number_reading(name: &str) -> bool {
    PORTS.contains(&name) || name.starts_with("lrint_") || name.starts_with("ms_nint_")
}

/// The names of the functions `source` defines: each identifier after the keyword `fn`, outside
/// line comments.
fn functions(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default();
        let mut words = code
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| !w.is_empty());
        while let Some(word) = words.next() {
            if word == "fn"
                && let Some(name) = words.next()
            {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// The lines, from 1, whose code outside a line comment turns degrees to radians by `PI / 180`
/// rather than by the module's `DEG_TO_RAD`, its spaces aside.
fn degrees_to_radians(source: &str) -> impl Iterator<Item = usize> + '_ {
    source.lines().enumerate().filter_map(|(i, line)| {
        let code: String = line
            .split("//")
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        code.contains(concat!("PI/", "180")).then_some(i + 1)
    })
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
fn c_number_reading_is_defined_only_in_the_module_of_style_ir() {
    // Verifies: ADR-0017
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("fuzz"), &mut files);
    let module = root.join(MODULE);
    let mut copies = Vec::new();
    let mut in_module = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).unwrap();
        let names = functions(&source)
            .into_iter()
            .filter(|n| ports_a_c_number_reading(n));
        if file == &module {
            in_module.extend(names);
        } else {
            let shown = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            copies.extend(names.map(|n| format!("{shown}: fn {n}")));
            copies.extend(
                degrees_to_radians(&source)
                    .map(|l| format!("{shown}:{l}: degrees to radians outside DEG_TO_RAD")),
            );
        }
    }
    assert!(copies.is_empty(), "ported outside {MODULE}: {copies:#?}");
    // The check itself sees a definition: the module defines each of them.
    for name in PORTS {
        assert!(
            in_module.iter().any(|n| n == name),
            "{MODULE} defines no `{name}`"
        );
    }
}

#[test]
fn the_check_sees_pi_over_180_in_code_and_not_in_comments() {
    let source = concat!(
        "let r = a * (std::f64::consts::PI / 1",
        "80.0);\n",
        "// PI / 1",
        "80\n",
        "let r = a * DEG_TO_RAD; // not PI/1",
        "80\n",
        "let r = PI/1",
        "80.0 * a;\n"
    );
    assert_eq!(degrees_to_radians(source).collect::<Vec<_>>(), [1, 4]);
}

#[test]
fn the_check_reads_definitions_and_not_calls_or_comments() {
    // `\x66n` is `fn`, which this file would otherwise define in the scan above.
    let source = "pub(crate) \x66n lrint_int(x: f64) -> i32 {\n    atof(b\"1\") // \x66n atoi\n}\n\
                  \x66n atof_reads_hex() {}\nconst \x66n ms_nint(x: f64) {}";
    let names: Vec<_> = functions(source)
        .into_iter()
        .filter(|n| ports_a_c_number_reading(n))
        .collect();
    assert_eq!(names, ["lrint_int", "ms_nint"]);
}
