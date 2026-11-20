//! C's and MapServer's small ports live once (IR-c-ports-in-style-ir): `atof`,
//! `atoi`, `strtod`, `lrint`, C's `(int)` cast of a `double` and its string end, and MapServer's
//! `MS_NINT`, `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, its test of a point in a
//! polygon, `msStringSplit`, `msComputeBounds`, `msGetSymbolIndex` with the key of its match of a
//! name, and the alpha of an opacity are defined in the module of `style-ir` and nowhere else in
//! the workspace.

use forager_arch_test::{manifest_dir, rust_files};

/// The one module that may define them, from the workspace root.
const MODULE: &str = "crates/style-ir/src/c.rs";

/// The names of the module's ports.
const PORTS: [&str; 17] = [
    "atof",
    "atoi",
    "strtod",
    "lrint",
    "ms_nint",
    "int_cast",
    "ms_min",
    "ms_max",
    "rect_overlap",
    "point_in_polygon",
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

#[test]
fn c_number_reading_is_defined_only_in_the_module_of_style_ir() {
    // Verifies: IR-c-ports-in-style-ir
    let root = manifest_dir().join("../..");
    let mut files = rust_files(&root.join("crates"));
    files.extend(rust_files(&root.join("fuzz")));
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

/// This binary's run of the scan above over the tree of a second checkout, whose module of
/// `style-ir` holds `module`: whether it passed, and what it wrote.
fn scan_in_second_checkout(name: &str, module: &str) -> (bool, String) {
    let checkout =
        std::env::temp_dir().join(format!("forager-arch-test-{name}-{}", std::process::id()));
    let crate_dir = checkout.join("crates/arch-test");
    std::fs::create_dir_all(&crate_dir).unwrap();
    let path = checkout.join(MODULE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, module).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "c_number_reading_is_defined_only_in_the_module_of_style_ir",
        ])
        .env("CARGO_MANIFEST_DIR", &crate_dir)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&checkout).unwrap();
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        text.contains("1 passed") || text.contains("1 failed"),
        "{text}"
    );
    (output.status.success(), text)
}

#[test]
fn the_scan_reads_the_tree_of_the_checkout_it_runs_in() {
    // A binary built in one checkout reads the files of the checkout `cargo test` runs it for,
    // not those of the path it was compiled at: the real module passes there, and an empty one
    // fails where the compiled-in tree would have passed.
    let real = std::fs::read_to_string(manifest_dir().join("../..").join(MODULE)).unwrap();
    let (passed, text) = scan_in_second_checkout("real", &real);
    assert!(passed, "{text}");
    let (passed, text) = scan_in_second_checkout("empty", "");
    assert!(!passed, "{text}");
    assert!(text.contains("defines no `atof`"), "{text}");
}
