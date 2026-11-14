//! The modules of `symbolize` import one another in one direction only, as its `lib.rs` states:
//! ADR-0008 decides the direction between crates, and this test holds the one inside the crate,
//! which the pixmap work once broke by an alias `symbol` imported from `draw`, above it.

mod module_direction;

use module_direction::{ROOT_GLOB, TOP_LEVEL_ROOTS, code_outside_tests, imported};

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
    ("label", &["canvas", "extent", "marker", "shade", "symbol"]),
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

#[test]
fn the_modules_of_symbolize_import_one_another_in_one_direction() {
    let problems = module_direction::problems(SOURCES, ALLOWED);
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

#[test]
fn a_glob_of_the_root_outside_tests_is_read_as_one() {
    let source = "\
use crate::*;
use super::{*, style};
#[cfg(test)]
mod tests {
    use super::*;
}
";
    let found = imported(&code_outside_tests(source), TOP_LEVEL_ROOTS);
    assert_eq!(found, [ROOT_GLOB, "style"].map(String::from).into());
    // A nested file's `super::*` is its own module's, not the root's.
    let nested = imported(
        &code_outside_tests("use super::*;\n"),
        &TOP_LEVEL_ROOTS[..1],
    );
    assert!(nested.is_empty());
}
