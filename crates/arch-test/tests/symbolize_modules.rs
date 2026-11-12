//! The modules of `symbolize` import one another in one direction only, as its `lib.rs` states:
//! ADR-0008 decides the direction between crates, and this test holds the one inside the crate,
//! which the pixmap work once broke by an alias `symbol` imported from `draw`, above it.

#![expect(
    clippy::string_slice,
    reason = "test code may slice a str: a slice off a boundary panics, and the test fails"
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Each module of the crate and the modules it may import, in the order of `lib.rs`'s sentence.
const ALLOWED: &[(&str, &[&str])] = &[
    (
        "draw",
        &[
            "canvas",
            "extent",
            "geom_transform",
            "hatch",
            "line",
            "marker",
            "shade",
            "symbol",
        ],
    ),
    (
        "shade",
        &[
            "bounds", "canvas", "extent", "hatch", "line", "style", "symbol", "tile",
        ],
    ),
    (
        "marker",
        &["bounds", "canvas", "line", "line_marker", "style", "symbol"],
    ),
    (
        "line",
        &[
            "bounds",
            "canvas",
            "line_marker",
            "offset",
            "style",
            "symbol",
        ],
    ),
    ("canvas", &["hatch", "line_marker", "symbol", "tile"]),
    ("hatch", &["bounds", "extent", "style", "symbol"]),
    ("line_marker", &["bounds", "symbol"]),
    ("tile", &["bounds", "symbol"]),
    ("symbol", &["bounds", "style"]),
    ("geom_transform", &["extent"]),
    ("bounds", &[]),
    ("extent", &[]),
    ("layer_transform", &[]),
    ("offset", &[]),
    ("style", &[]),
];

/// The sources of the crate, from the workspace root.
const SOURCES: &str = "crates/symbolize/src";

/// The code of `source` outside a test, its lines joined so that a path wrapped across lines
/// reads as one: comments, doc comments included, cut off, and the item after each
/// `#[cfg(test)]`, an inline `mod tests` or the declaration of a file of its own, left out.
fn code_outside_tests(source: &str) -> String {
    let mut lines = Vec::new();
    let (mut skipping, mut depth) = (false, 0_i32);
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default();
        if code.trim() == "#[cfg(test)]" {
            skipping = true;
            continue;
        }
        if skipping {
            depth += i32::try_from(code.matches('{').count()).unwrap();
            depth -= i32::try_from(code.matches('}').count()).unwrap();
            if depth <= 0 && (code.contains('}') || code.trim_end().ends_with(';')) {
                skipping = false;
                depth = 0;
            }
            continue;
        }
        lines.push(code);
    }
    lines.join("\n")
}

/// The roots a path in a top-level module names the crate's modules from: `super::` is the crate
/// root there as `crate::` is.
const TOP_LEVEL_ROOTS: &[&str] = &["crate::", "super::"];

/// The modules of the crate `code` names by a path from one of `roots`, `crate::<module>` say,
/// a group `crate::{a, b::{c, d}}` included, whitespace and line breaks in it allowed.
fn imported(code: &str, roots: &[&str]) -> BTreeSet<String> {
    let ident = |s: &str| -> String {
        s.trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    };
    let mut modules = BTreeSet::new();
    for root in roots {
        for (at, _) in code.match_indices(root) {
            let rest = code[at + root.len()..].trim_start();
            let Some(group) = rest.strip_prefix('{') else {
                modules.insert(ident(rest));
                continue;
            };
            // The first name of each member at the group's own depth, up to its closing brace.
            let (mut depth, mut member_start) = (0_usize, true);
            for (i, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' if depth == 0 => break,
                    '}' => depth -= 1,
                    ',' if depth == 0 => member_start = true,
                    c if member_start && !c.is_whitespace() => {
                        modules.insert(ident(&group[i..]));
                        member_start = false;
                    }
                    _ => {}
                }
            }
        }
    }
    modules.remove("");
    modules.remove("self");
    modules.remove("super");
    modules
}

/// The module each source file belongs to, by its path below the sources: `a.rs` and `a/b.rs`
/// belong to `a`. `lib.rs`, the root, and the files `tests.rs`, which are test code, belong to
/// none.
fn module_of(file: &Path) -> Option<String> {
    if file.file_name()? == "tests.rs" || file == Path::new("lib.rs") {
        return None;
    }
    let first = file.components().next()?.as_os_str().to_str()?;
    Some(first.trim_end_matches(".rs").to_owned())
}

/// Every `.rs` file under `dir`, relative to `root`.
fn rust_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().map(Result::unwrap) {
        let path = entry.path();
        if path.is_dir() {
            rust_files(root, &path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

#[test]
fn the_modules_of_symbolize_import_one_another_in_one_direction() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SOURCES);
    let mut files = Vec::new();
    rust_files(&root, &root, &mut files);
    files.sort();
    let mut problems = Vec::new();
    for file in &files {
        let Some(module) = module_of(file) else {
            continue;
        };
        let Some((_, allowed)) = ALLOWED.iter().find(|(m, _)| *m == module) else {
            problems.push(format!(
                "{}: the module {module} has no row in the table",
                file.display()
            ));
            continue;
        };
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        // A file below a module's directory is nested: its `super::` is that module, not the root.
        let roots = if file.components().count() == 1 {
            TOP_LEVEL_ROOTS
        } else {
            &TOP_LEVEL_ROOTS[..1]
        };
        for import in imported(&code_outside_tests(&source), roots) {
            if import != module && !allowed.contains(&import.as_str()) {
                problems.push(format!(
                    "{}: the module {module} imports {import}",
                    file.display()
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "against the direction of crates/symbolize/src/lib.rs: {problems:#?}"
    );
}

#[test]
fn an_import_is_read_outside_tests_and_comments() {
    let source = "\
//! Above `crate::draw` in a doc comment.
use crate::bounds::{self, ms_max};
use crate::{extent::View, style};
/// [`crate::tile`]
fn f() { crate::symbol::mark(); }
#[cfg(test)]
mod tests;
#[cfg(test)]
mod inline {
    use crate::draw::Canvas;
    fn g() { if true { } }
}
fn h() {}
";
    let found = imported(&code_outside_tests(source), TOP_LEVEL_ROOTS);
    assert_eq!(
        found,
        ["bounds", "extent", "style", "symbol"]
            .map(String::from)
            .into()
    );
    assert!(
        code_outside_tests(source)
            .trim_end()
            .ends_with("\nfn h() {}")
    );
}

#[test]
fn an_import_is_read_across_lines_and_through_super() {
    let source = "\
use crate::{
    bounds::{self, ms_max},
    extent::View, // a comment between members
    style,
};
use super::draw::Canvas;
fn f() { super::tile::fill(); }
";
    let found = imported(&code_outside_tests(source), TOP_LEVEL_ROOTS);
    assert_eq!(
        found,
        ["bounds", "draw", "extent", "style", "tile"]
            .map(String::from)
            .into()
    );
}
