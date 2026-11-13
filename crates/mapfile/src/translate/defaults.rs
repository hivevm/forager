// The defaults follow MapServer's msApplyDefaultSubstitutions, src/mapfile.c, and
// msCaseReplaceSubstring, src/mapstring.cpp, of the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The defaults a `VALIDATION` block gives a `%name%`, as `msApplyDefaultSubstitutions` applies
//! them when a map is loaded, in `FILTER` and a class's `EXPRESSION` alone.

use std::collections::BTreeMap;

use forager_style_ir::Diagnostic;

use crate::budget::{Budget, Charged};
use crate::expression::Problem;
use crate::lexer::{Token, TokenKind};
use crate::report::Report;
use crate::syntax::{Block, Node};

/// The `default_name` values of a block's `VALIDATION`, by the name in lower case: MapServer's hash
/// table takes its keys without case, and a later one replaces an earlier.
#[derive(Default)]
pub(super) struct Defaults(BTreeMap<String, String>);

impl Defaults {
    /// The defaults of the `VALIDATION` blocks in `blocks`, wherever in them they stand. A pair
    /// that is not two strings, or a last key without a value, gives no default and a warning in
    /// `report`, where `loadHashTable` fails the load; the block's other pairs still hold.
    pub(super) fn of<'b>(blocks: impl IntoIterator<Item = &'b Block>, report: &mut Report) -> Self {
        let mut defaults = BTreeMap::new();
        let validation = blocks.into_iter().flat_map(|block| &block.nodes);
        for node in validation {
            let Node::Block(b) = node else { continue };
            if b.keyword != "VALIDATION" {
                continue;
            }
            let (pairs, rest) = b.values.as_chunks::<2>();
            for [key, value] in pairs {
                let (Some(key_text), Some(value)) = (key.string(), value.string()) else {
                    skipped(report, key, "is not two strings");
                    continue;
                };
                if let Some(name) = key_text
                    .get(..8)
                    .filter(|prefix| prefix.eq_ignore_ascii_case("default_"))
                    .and(key_text.get(8..))
                {
                    defaults.insert(name.to_ascii_lowercase(), value.to_owned());
                }
            }
            if let [key] = rest {
                skipped(report, key, "has no value");
            }
        }
        Self(defaults)
    }

    /// `text` with each `%name%` of a default replaced, one pass from the left, the tag found
    /// without case as msCaseReplaceSubstring finds it, and a replaced value not read again.
    /// Scanning for the tags rather than for each default keeps the work to the text's length, and
    /// `added` holds to its bound what the defaults put into all of a load's expressions, as one
    /// default may be named many times, and one expression with them (Q-9).
    fn applied(&self, text: &str, added: &mut Budget) -> Result<String, Problem> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some((before, after)) = rest.split_once('%') {
            let Some((name, tail)) = after.split_once('%') else {
                break;
            };
            match self.0.get(&name.to_ascii_lowercase()) {
                Some(value) => {
                    let whole = out.len() + before.len() + value.len() + tail.len();
                    if added.charge(value.len()) != Charged::Within || !added.admits(whole) {
                        return Err(Problem {
                            error: true,
                            code: "mapfile-too-large",
                            message: format!(
                                "the text the VALIDATION defaults add to the mapfile, or an \
                                 expression with them, exceeds {} bytes",
                                added.bound()
                            ),
                        });
                    }
                    out.push_str(before);
                    out.push_str(value);
                    rest = tail;
                }
                // The closing `%` may open the next tag.
                None => {
                    out.push_str(before);
                    out.push('%');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// The warning of a `VALIDATION` pair that gives no default, named by its key.
fn skipped(report: &mut Report, key: &Token, why: &str) {
    let named = match &key.kind {
        TokenKind::Str { value, .. } | TokenKind::Word(value) => format!("\"{value}\""),
        TokenKind::Number(n) => n.to_string(),
        _ => "a value that is not a string".to_owned(),
    };
    let message = format!(
        "the VALIDATION pair of the key {named} {why} and gives no default; MapServer fails the \
         load on it"
    );
    report.push(Diagnostic::warning(
        "unsupported-value",
        &message,
        Some(key.span.clone()),
    ));
}

/// `token` with the defaults of `tables` applied, each table in turn, a class's before its
/// layer's before the map's, as msApplyDefaultSubstitutions applies them. The token is one of
/// loadExpression's; MapServer substitutes in its text before parsing it, so a default that does
/// not parse is that expression's own warning.
pub(super) fn substituted(
    token: &Token,
    tables: &[&Defaults],
    added: &mut Budget,
) -> Result<Token, Problem> {
    let mut token = token.clone();
    let text = match &mut token.kind {
        TokenKind::Str { value, .. }
        | TokenKind::Word(value)
        | TokenKind::Regex { pattern: value, .. }
        | TokenKind::List(value)
        | TokenKind::Expression(value) => value,
        TokenKind::Number(_) | TokenKind::Binding(_) => return Ok(token),
    };
    for table in tables {
        *text = table.applied(text, added)?;
    }
    Ok(token)
}

#[cfg(test)]
mod tests;
