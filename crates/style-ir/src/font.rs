// A label's font list follows get_face_for_run and check_single_font, src/textlayout.c, and a
// map's fonts msLoadFontSet, src/maplabel.c, and its hash table, src/maphash.c, of MapServer at
// the release pinned in scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2013 Regents of the University of Minnesota.
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

//! A map's fonts and a label's font list, each read once for the frontend and the core.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A map's fonts: each name with the resource path of its font file. A name is found without
/// regard to case, as MapServer's font set, a hash table that compares its keys with
/// `strcasecmp` (msLookupHashTable, src/maphash.c), finds it; so no two names differ only in
/// case.
#[derive(Clone, Debug, Default)]
pub struct Fonts {
    /// Each font's path, by its name as first written.
    paths: BTreeMap<String, String>,
    /// Each name as first written, by its lower case, so that a name is found at once whatever
    /// the number of fonts (Q-9).
    names: BTreeMap<String, String>,
    /// Each name a style document gives after one it holds in another case, left out, with the
    /// name kept, in the order read; the document's reader warns of each.
    twins: Vec<(String, String)>,
}

impl Fonts {
    /// Puts the font at `path` under `name` as msInsertHashTable, src/maphash.c, does: a name
    /// held in another case keeps the name as first written and takes `path`.
    pub fn insert(&mut self, name: &str, path: String) {
        let key = self
            .names
            .entry(name.to_ascii_lowercase())
            .or_insert_with(|| name.to_owned());
        self.paths.insert(key.clone(), path);
    }

    /// The path of the font `name`, matched without regard to case.
    pub fn get(&self, name: &str) -> Option<&str> {
        let key = self.names.get(&name.to_ascii_lowercase())?;
        self.paths.get(key).map(String::as_str)
    }

    /// The names as first written, in byte order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.paths.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Each name a style document gave after one in another case, which its reader left out,
    /// with the name it kept.
    pub(crate) fn take_twins(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.twins)
    }
}

impl PartialEq for Fonts {
    fn eq(&self, other: &Self) -> bool {
        self.paths == other.paths
    }
}

impl FromIterator<(String, String)> for Fonts {
    /// The fonts of `iter` in turn, each put as [`Fonts::insert`] puts it.
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let mut fonts = Self::default();
        for (name, path) in iter {
            fonts.insert(&name, path);
        }
        fonts
    }
}

impl Serialize for Fonts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.paths.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Fonts {
    /// The fonts of a style document in the order read: a name that matches one before it
    /// without regard to case is left out, the first used, and kept among the twins its reader
    /// warns of. A typed read takes them in the order written; a read through a
    /// `serde_json::Value` in byte order, so the lenient read reads them again from the text.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FontsVisitor;

        impl<'de> Visitor<'de> for FontsVisitor {
            type Value = Fonts;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of font names to paths")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Fonts, A::Error> {
                let mut fonts = Fonts::default();
                while let Some((name, path)) = map.next_entry::<String, String>()? {
                    if let Some(kept) = fonts.names.get(&name.to_ascii_lowercase()) {
                        let kept = kept.clone();
                        fonts.twins.push((name, kept));
                    } else {
                        fonts.insert(&name, path);
                    }
                }
                Ok(fonts)
            }
        }

        deserializer.deserialize_map(FontsVisitor)
    }
}

/// An entry of a label's font list `a,b`, and whether it is the last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontListEntry<'a> {
    pub text: &'a str,
    pub last: bool,
}

/// The names `check_single_font` looks the font list entry `entry` up by in the font set: the
/// name after its first colon, which skips a script's prefix such as `ar:`, then the entry as
/// written.
pub fn font_names(entry: &str) -> Vec<&str> {
    match entry.split_once(':') {
        Some((_, name)) => vec![name, entry],
        None => vec![entry],
    }
}

/// The entries of the font list `list`, as `get_face_for_run` walks it: each up to the next
/// comma. The walk stops at the end of the list, so an empty entry after a trailing comma is no
/// entry and leaves the one before it not the last; an empty entry inside the list is one.
pub fn font_list(list: &str) -> Vec<FontListEntry<'_>> {
    let mut entries = Vec::new();
    let mut rest = list;
    while !rest.is_empty() {
        match rest.split_once(',') {
            Some((text, after)) => {
                entries.push(FontListEntry { text, last: false });
                rest = after;
            }
            None => {
                entries.push(FontListEntry {
                    text: rest,
                    last: true,
                });
                rest = "";
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(list: &str) -> Vec<(&str, bool)> {
        font_list(list).iter().map(|e| (e.text, e.last)).collect()
    }

    fn fonts(pairs: &[(&str, &str)]) -> Fonts {
        pairs
            .iter()
            .map(|(name, path)| ((*name).to_owned(), (*path).to_owned()))
            .collect()
    }

    #[test]
    fn a_font_is_found_without_case_and_a_later_path_keeps_the_first_name() {
        let fonts = fonts(&[("Sans", "a.ttf"), ("mono", "m.ttf"), ("SANS", "b.ttf")]);
        assert_eq!(fonts.get("sANs"), Some("b.ttf"));
        assert_eq!(fonts.get("serif"), None);
        let names: Vec<&str> = fonts.keys().collect();
        assert_eq!(names, ["Sans", "mono"]);
    }

    #[test]
    fn a_font_list_is_split_at_its_commas_up_to_its_end() {
        assert_eq!(texts("a,b"), [("a", false), ("b", true)]);
        assert_eq!(
            texts("a,"),
            [("a", false)],
            "no entry after a trailing comma"
        );
        assert_eq!(texts("a,,b"), [("a", false), ("", false), ("b", true)]);
        assert_eq!(texts(""), []);
        assert_eq!(texts(","), [("", false)]);
    }

    #[test]
    fn an_entry_is_looked_up_after_its_first_colon_then_as_written() {
        assert_eq!(font_names("ar:a:b"), ["a:b", "ar:a:b"]);
        assert_eq!(font_names("sans"), ["sans"]);
    }
}
