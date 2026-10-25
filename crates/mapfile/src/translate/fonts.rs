// The font set follows MapServer's msLoadFontSet, src/maplabel.c, of the release pinned in
// scripts/fetch-msautotest.sh.
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

//! The font set a map names, and the check of a label's fonts against it.

use std::collections::BTreeMap;

use forager_ports::ResourcePath;
use forager_style_ir::Span;

use super::Translator;

impl Translator<'_> {
    /// A font set as msLoadFontSet reads it: a line `name path` per font, lines that start with
    /// `#`, a space, or nothing skipped; a path resolved beside the font set, and one outside the
    /// base context a warning (Q-8).
    pub(super) fn fonts(&mut self, path: &ResourcePath, text: &str) -> BTreeMap<String, String> {
        let span = Span::new(path.as_str(), 1, 1);
        let mut fonts = BTreeMap::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with(['#', ' ', '\r']) {
                continue;
            }
            let Some((name, file)) = line.split_once([' ', '\t']) else {
                continue;
            };
            let file = file.trim();
            if file.is_empty() {
                continue;
            }
            match path.resolve(file) {
                Ok(font) => {
                    fonts.insert(name.to_owned(), font.as_str().to_owned());
                }
                // One font is needed only when it is drawn, so its refusal leaves it out of
                // the map rather than failing the load.
                Err(e) => {
                    let message = format!("font `{name}` at `{file}` is refused and left out: {e}");
                    self.warn("font-refused", &message, &span);
                }
            }
        }
        fonts
    }

    /// Warns of each font of a `FONT` list, `a,b` naming fallbacks, that the font set lacks.
    pub(super) fn check_fonts(&mut self, names: &str, span: &Span) {
        for name in names.split(',') {
            if !self.fonts.contains_key(name) {
                let message = format!("the font set has no font `{name}`");
                self.warn("font-unknown", &message, span);
            }
        }
    }
}

#[cfg(test)]
mod tests;
