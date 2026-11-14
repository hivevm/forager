//! The reading of the imports between the modules of one crate, which the tests of the direction
//! of `symbolize`'s modules and of the engine's share: a path from the crate's root names a module
//! of it, and an item of the root itself is no module, so a module that takes one is refused.

#![expect(
    clippy::string_slice,
    reason = "test code may slice a str: a slice off a boundary panics, and the test fails"
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The problems of the crate whose sources lie at `sources`, from the workspace root, against
/// `allowed`, each module of the crate and the modules it may import: an import outside a test
/// that its row does not allow, naming the module and the one it imports, and a module the table
/// has no row for.
pub fn problems(sources: &str, allowed: &[(&str, &[&str])]) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(sources);
    let mut files = Vec::new();
    rust_files(&root, &root, &mut files);
    files.sort();
    let mut problems = Vec::new();
    for file in &files {
        let Some(module) = module_of(file) else {
            continue;
        };
        let Some((_, allowed)) = allowed.iter().find(|(m, _)| *m == module) else {
            problems.push(format!(
                "{}: the module {module} has no row in the table",
                file.display()
            ));
            continue;
        };
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        problems.extend(imports_against(file, &module, allowed, &source));
    }
    problems
}

/// The imports of `source`, the file `file` of `module`, that `allowed`, the modules its row lets
/// it import, does not hold, each naming the module and the one it imports.
pub fn imports_against(file: &Path, module: &str, allowed: &[&str], source: &str) -> Vec<String> {
    // A file below a module's directory is nested: its `super::` is that module, not the root.
    let roots = if file.components().count() == 1 {
        TOP_LEVEL_ROOTS
    } else {
        &TOP_LEVEL_ROOTS[..1]
    };
    imported(&code_outside_tests(source), roots)
        .into_iter()
        .filter(|import| import != module && !allowed.contains(&import.as_str()))
        .map(|import| format!("{}: the module {module} imports {import}", file.display()))
        .collect()
}

/// The code of `source` outside a test, its lines joined so that a path wrapped across lines
/// reads as one: comments, doc comments included, cut off, and the item after each
/// `#[cfg(test)]`, an inline `mod tests` or the declaration of a file of its own, left out.
pub fn code_outside_tests(source: &str) -> String {
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
pub const TOP_LEVEL_ROOTS: &[&str] = &["crate::", "super::"];

/// The name [`imported`] gives a glob of the crate's root, which no module's row allows.
pub const ROOT_GLOB: &str = "* (every item of the root)";

/// The modules of the crate `code` names by a path from one of `roots`, `crate::<module>` say,
/// a group `crate::{a, b::{c, d}}` included, whitespace and line breaks in it allowed. A glob of
/// the root, `crate::*` or a member `*` of its group, is named [`ROOT_GLOB`], since it takes
/// every item of the root.
pub fn imported(code: &str, roots: &[&str]) -> BTreeSet<String> {
    let ident = |s: &str| -> String {
        let s = s.trim_start();
        if s.starts_with('*') {
            return ROOT_GLOB.to_owned();
        }
        s.chars()
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
