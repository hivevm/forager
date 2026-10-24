//! Init files that a mapfile names by a path in an `init=<file>:<code>` projection (ADR-0023):
//! read before translation through the resource loader, as `FONTSET` and `SYMBOLSET` are, and
//! parsed in the format of PROJ's former `epsg` init file, `<code> +proj=… <>`.

use std::collections::BTreeMap;

use forager_ports::ResourcePath;
use forager_style_ir::Span;

use crate::lexer::TokenKind;
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
/// `init=` file name that holds a `/`, in the map's projection and in its layers'.
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
            for token in &b.values {
                let (TokenKind::Str { value, .. } | TokenKind::Word(value)) = &token.kind else {
                    continue;
                };
                for param in value.split_whitespace() {
                    let reference = param.trim_start_matches('+').strip_prefix("init=");
                    let file = reference
                        .and_then(|r| r.split_once(':'))
                        .map(|(file, _)| file);
                    if let Some(file) = file.filter(|f| f.contains('/'))
                        && !named.iter().any(|(n, _)| n == file)
                    {
                        named.push((file.to_owned(), token.span.clone()));
                    }
                }
            }
        }
    }
    named
}

/// Each code's definition, as PROJ's reader takes it: a line is `<code>` and the parameters up
/// to the next `<`, `#` starts a comment. A definition that names a grid file, or refers on with
/// `init=`, is kept as the reason it cannot be used (ADR-0023); a line that is no definition is
/// skipped, as PROJ skips what it does not look for.
fn parse(text: &str) -> BTreeMap<String, Result<String, String>> {
    let mut definitions = BTreeMap::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for raw in text.lines() {
        let content = raw.split('#').next().unwrap_or("").trim();
        let Some((code, rest)) = content.strip_prefix('<').and_then(|r| r.split_once('>')) else {
            continue;
        };
        let code = code.trim();
        let definition = rest.split('<').next().unwrap_or("").trim();
        if code.is_empty() || definition.is_empty() {
            continue;
        }
        definitions.insert(code.to_owned(), usable(code, definition));
    }
    definitions
}

/// The definition, unless the core cannot use it.
fn usable(code: &str, definition: &str) -> Result<String, String> {
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
    Ok(definition.to_owned())
}
