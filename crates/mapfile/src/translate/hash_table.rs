// The reading of a block of string pairs follows MapServer's loadHashTable, src/mapfile.c, and
// msInsertHashTable, src/maphash.c, of the release pinned in scripts/fetch-msautotest.sh.
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

//! The blocks of string pairs a map, a layer, or a class carries, `METADATA` and `VALIDATION`, as
//! `loadHashTable` reads them into a hash table.

use std::collections::BTreeMap;

use forager_style_ir::Diagnostic;

use crate::lexer::{Token, TokenKind};
use crate::report::Report;
use crate::syntax::{Block, Node};

/// The pairs of the `keyword` blocks of some blocks, by the key in lower case: msInsertHashTable
/// finds a key without case (`strcasecmp`), and a later pair of a key replaces the earlier's value.
pub(super) struct HashTable<'b>(BTreeMap<String, &'b str>);

impl<'b> HashTable<'b> {
    /// The pairs of the `keyword` blocks in `blocks`, wherever in them they stand. A pair that is
    /// not two strings, or a last key without a value, is left out with a warning in `report`,
    /// where `loadHashTable` fails the load (src/mapfile.c:2473-2505); the block's other pairs
    /// still hold.
    pub(super) fn of(
        blocks: impl IntoIterator<Item = &'b Block>,
        keyword: &str,
        report: &mut Report,
    ) -> Self {
        let mut table = BTreeMap::new();
        let tables = blocks.into_iter().flat_map(|block| &block.nodes);
        for node in tables {
            let Node::Block(b) = node else { continue };
            if b.keyword != keyword {
                continue;
            }
            let (pairs, rest) = b.values.as_chunks::<2>();
            for [key, value] in pairs {
                let (Some(key_text), Some(value)) = (key.string(), value.string()) else {
                    skipped(report, keyword, key, "is not two strings");
                    continue;
                };
                table.insert(key_text.to_ascii_lowercase(), value);
            }
            if let [key] = rest {
                skipped(report, keyword, key, "has no value");
            }
        }
        Self(table)
    }

    /// The value of `key`, found without case, as msLookupHashTable finds it.
    pub(super) fn get(&self, key: &str) -> Option<&'b str> {
        self.0.get(&key.to_ascii_lowercase()).copied()
    }

    /// The pairs, each key in lower case.
    pub(super) fn iter(&self) -> impl Iterator<Item = (&str, &'b str)> {
        self.0.iter().map(|(key, value)| (key.as_str(), *value))
    }
}

/// The warning of a pair of a `keyword` block that is left out, named by its key.
fn skipped(report: &mut Report, keyword: &str, key: &Token, why: &str) {
    let named = match &key.kind {
        TokenKind::Str { value, .. } | TokenKind::Word(value) => format!("\"{value}\""),
        TokenKind::Number(n) => n.to_string(),
        _ => "a value that is not a string".to_owned(),
    };
    let message = format!(
        "the {keyword} pair of the key {named} {why} and is left out; MapServer fails the load on \
         it"
    );
    report.push(Diagnostic::warning(
        "unsupported-value",
        &message,
        Some(key.span.clone()),
    ));
}
