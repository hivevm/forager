//! A symbol's name is matched in one place, `ms_get_symbol_index` and its `symbol_name_key` in
//! `style-ir`'s module `c`, msGetSymbolIndex's `strcasecmp` (ADR-0017), which the frontend, the
//! engine, and `symbolize` call. `c_numbers.rs` refuses a function of either name outside that
//! module; this check refuses the match written under another name: one of the folds of `FOLDS`
//! outside the module where the identifier `symbol`, `symbols`, or `Symbol` stands on its line or
//! one of the four lines before it. Another form of the comparison passes it.

use forager_arch_test::{manifest_dir, rust_files};

/// The one module that may compare a symbol's name, from the workspace root.
const MODULE: &str = "crates/style-ir/src/c.rs";

/// The calls that compare or fold text without regard to case. `to_lowercase` is not among them:
/// `symbolize` lower-cases a message that names a symbol's kind (`crates/symbolize/src/symbol.rs`,
/// `marker_size_with`), which compares no name.
const FOLDS: [&str; 6] = [
    "eq_ignore_ascii_case",
    "to_ascii_lowercase",
    "to_ascii_uppercase",
    "make_ascii_lowercase",
    "make_ascii_uppercase",
    "to_uppercase",
];

/// The lines before a comparison read with it.
const WINDOW: usize = 4;

/// Whether `code` holds the identifier `symbol`, `symbols`, or `Symbol`.
fn names_a_symbol(code: &str) -> bool {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|w| matches!(w, "symbol" | "symbols" | "Symbol"))
}

/// The lines, from 1, of `source` that compare a symbol's name without regard to case, line
/// comments left out.
fn comparisons(source: &str) -> Vec<usize> {
    let code: Vec<&str> = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect();
    (0..code.len())
        .filter(|&i| {
            FOLDS.iter().any(|fold| code[i].contains(fold))
                && code[i.saturating_sub(WINDOW)..=i]
                    .iter()
                    .any(|line| names_a_symbol(line))
        })
        .map(|i| i + 1)
        .collect()
}

#[test]
fn a_symbols_name_is_compared_only_in_the_module_of_style_ir() {
    let root = manifest_dir().join("../..");
    let mut files = rust_files(&root.join("crates"));
    files.extend(rust_files(&root.join("fuzz")));
    let module = root.join(MODULE);
    let this = root.join("crates/arch-test/tests/symbol_names.rs");
    let mut copies = Vec::new();
    let mut in_module = 0;
    for file in &files {
        let lines = comparisons(&std::fs::read_to_string(file).unwrap());
        if file == &module {
            in_module += lines.len();
        } else if file != &this {
            let shown = file.strip_prefix(&root).unwrap_or(file).display();
            copies.extend(lines.iter().map(|line| format!("{shown}:{line}")));
        }
    }
    assert!(copies.is_empty(), "compared outside {MODULE}: {copies:#?}");
    // The check itself sees the comparison: the module writes it once.
    assert_eq!(
        in_module, 1,
        "{MODULE} compares a symbol's name {in_module} times"
    );
}

#[test]
fn the_check_reads_a_comparison_near_a_symbol_and_not_in_comments() {
    let source = "let found = symbols.iter().position(|s| {\n    s.name\n        .as_deref()\n        \
                  .is_some_and(|n| n.eq_ignore_ascii_case(name))\n});\n\
                  layers.iter().any(|l| l.name.eq_ignore_ascii_case(name));\n\
                  // symbol: n.eq_ignore_ascii_case(name)\n\
                  let symbolic = key.eq_ignore_ascii_case(name);\n\
                  names.entry(symbol.name.to_ascii_lowercase());\n\
                  let key = symbol.name.to_uppercase();\n";
    assert_eq!(comparisons(source), [4, 9, 10]);
}
