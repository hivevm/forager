//! Init files that a mapfile names by a path in an `init=<file>:<code>` projection (ADR-0019):
//! read before translation through the resource loader, as `FONTSET` and `SYMBOLSET` are, and
//! parsed in the format of PROJ's former `epsg` init file, `<code> +proj=… <>`.

use std::collections::BTreeMap;

use forager_ports::ResourcePath;
use forager_style_ir::Span;

use crate::projection::{self, Projection};
use crate::syntax::{Block, Node};

/// An init file as read: where it lies, and each code's definition, or why it cannot be used.
pub struct InitFile {
    pub path: ResourcePath,
    definitions: BTreeMap<String, Result<String, String>>,
}

impl InitFile {
    pub fn new(path: ResourcePath, text: &str) -> Self {
        Self {
            path,
            definitions: parse(text),
        }
    }

    /// The definition of `code`, or why there is none.
    pub fn definition(&self, code: &str) -> Result<&str, String> {
        match self.definitions.get(code) {
            Some(Ok(definition)) => Ok(definition),
            Some(Err(why)) => Err(why.clone()),
            None => Err(format!("the init file `{}` holds no `{code}`", self.path)),
        }
    }
}

/// The init files named by a path, each with the span of a projection that names it: every
/// `init=` file name that holds a `/`, in the map's projection and in its layers', except in a
/// projection that is `AUTO`, which resolves none.
pub fn named(map: &Block) -> Vec<(String, Span)> {
    let mut named: Vec<(String, Span)> = Vec::new();
    let mut pending = vec![map];
    while let Some(block) = pending.pop() {
        for node in &block.nodes {
            let Node::Block(b) = node else { continue };
            if b.keyword != "PROJECTION" {
                pending.push(b);
                continue;
            }
            // A projection that does not read is reported by the translation.
            let Ok(Projection::Parameters {
                init: Some(init), ..
            }) = projection::read(b)
            else {
                continue;
            };
            if let Some(file) = init.file()
                && !named.iter().any(|(n, _)| n == file)
            {
                named.push((file.to_owned(), init.span.clone()));
            }
        }
    }
    named
}

/// Each code's definition, as PROJ 9.5.0's reader takes it (`get_init_string` in its
/// `src/init.cpp`): `<code>` at the start of a line, the code compared as written, and the
/// parameters up to the next `<`, across lines, `#` starting a comment to the end of its line;
/// the first definition of a code holds, an empty one too. A `<` whose `>` is not on its line, or
/// a `<code>` inside a line, is no code. A definition that cannot be used is kept as the reason
/// (ADR-0019). The host table of `adapter-epsg` is read by the same rules.
fn parse(text: &str) -> BTreeMap<String, Result<String, String>> {
    let mut definitions = BTreeMap::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect();
    let joined = lines.join("\n");
    let mut current: Option<&str> = None;
    // The text not read yet: from the start, a code's `>`, or the break of a line skipped.
    let mut rest = joined.as_str();
    loop {
        let (before, after) = rest
            .split_once('<')
            .map_or((rest, None), |(before, after)| (before, Some(after)));
        if let Some(code) = current.take() {
            definitions
                .entry(code.to_owned())
                .or_insert_with(|| usable(code, before.trim()));
        }
        let Some(after) = after else { break };
        let starts_its_line = match before.rsplit_once('\n') {
            Some((_, line)) => line.trim().is_empty(),
            // On the line of the code before it, unless at the start.
            None => rest.len() == joined.len() && before.trim().is_empty(),
        };
        // The code's `>` is the first after its `<`, on its line.
        match after.split_once('>') {
            Some((code, tail)) if !code.contains('\n') => {
                if !code.is_empty() && starts_its_line {
                    current = Some(code);
                }
                rest = tail;
            }
            _ => rest = after.trim_start_matches(|c| c != '\n'),
        }
    }
    definitions
}

/// The definition, unless the core cannot use it.
fn usable(code: &str, definition: &str) -> Result<String, String> {
    if definition.is_empty() {
        return Err(format!("`{code}` has no definition"));
    }
    let params = || {
        definition
            .split_whitespace()
            .map(|p| p.trim_start_matches('+'))
    };
    if params().any(|p| p.starts_with("init=")) {
        return Err(format!(
            "`{code}` refers to another definition with init=, which the core cannot read"
        ));
    }
    let grids = [
        "nadgrids=",
        "geoidgrids=",
        "grids=",
        "xy_grids=",
        "z_grids=",
    ];
    let names_a_grid = params().any(|param| {
        let value = grids.iter().find_map(|key| param.strip_prefix(key));
        value.is_some_and(|v| v.split(',').any(|g| g.trim_start_matches('@') != "null"))
    });
    if names_a_grid {
        return Err(format!(
            "`{code}` names a grid file, which the core cannot read"
        ));
    }
    Ok(definition.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn codes_start_their_lines_close_on_them_and_the_first_definition_holds() {
        let text = "<1> +proj=a # a comment\n +x=1\n<1> +proj=b\n<2 +proj=c\n<3> +proj=d <4> +e\n\
                    x <5> +proj=f\n<6>\n<é> +proj=g <> <7> +proj=h\n  <8> +proj=i";
        let read = parse(text);
        let ok = |code: &str| read.get(code).and_then(|d| d.as_ref().ok()).cloned();
        assert_eq!(
            ok("1").as_deref(),
            Some("+proj=a +x=1"),
            "across lines, first holds"
        );
        assert!(!read.contains_key("2"), "a `<` without its `>` on its line");
        assert_eq!(
            ok("3").as_deref(),
            Some("+proj=d"),
            "a definition ends at the next `<`"
        );
        assert!(!read.contains_key("4"), "a code inside a line");
        assert!(!read.contains_key("5"), "a code after text on its line");
        assert!(
            read.get("6").is_some_and(Result::is_err),
            "an empty definition"
        );
        assert_eq!(ok("é").as_deref(), Some("+proj=g"));
        assert!(!read.contains_key("7"), "a code after a `<>` on its line");
        assert_eq!(ok("8").as_deref(), Some("+proj=i"), "spaces before a code");
        assert_eq!(read.len(), 5);
    }
}
