// The tests pin MapServer's msLoadFontSet, src/maplabel.c, and its hash table, src/maphash.c,
// of the release pinned in scripts/fetch-msautotest.sh.
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

use forager_style_ir::Bound;

use crate::test_support::{codes, first_label, load_files};

#[test]
fn fonts_come_from_the_font_set_and_a_label_names_one() {
    let files = [
        (
            "m.map",
            "MAP FONTSET 'f/fonts.txt' LAYER CLASS LABEL FONT 'sans,serif' END END END END",
        ),
        (
            "f/fonts.txt",
            "# fonts\nsans  DejaVuSans.ttf\n indented skipped.ttf\nmono\tttf/mono.ttf  \n",
        ),
    ];
    let (loaded, label) = first_label(&files);
    assert_eq!(codes(&loaded), ["font-unknown"], "serif is not in the set");
    assert_eq!(
        label.unwrap().font,
        Some(Bound::Value("sans,serif".to_owned()))
    );
    let fonts = loaded.document.unwrap().map.fonts;
    assert_eq!(
        fonts.get("sans"),
        Some("f/DejaVuSans.ttf"),
        "beside the font set"
    );
    assert_eq!(fonts.get("mono"), Some("f/ttf/mono.ttf"));
    assert_eq!(fonts.len(), 2);
}

#[test]
fn a_font_set_or_font_outside_the_base_context_is_refused() {
    // Verifies: Q-8
    let loaded = load_files(&[("m.map", "MAP FONTSET '../fonts.txt' END")]);
    assert_eq!(codes(&loaded), ["fontset-refused"]);
    let loaded = load_files(&[
        ("m.map", "MAP FONTSET 'f.txt' END"),
        ("f.txt", "sans /usr/x.ttf\n"),
    ]);
    assert_eq!(codes(&loaded), ["font-refused"]);
    assert!(loaded.document.unwrap().map.fonts.is_empty());
}

#[test]
fn a_font_set_line_is_read_as_ms_load_font_set_reads_it() {
    let long = "n".repeat(62);
    let too_long = "l".repeat(63);
    let set: &'static str = format!(
        "a a.ttf\r\n\tb b.ttf\n\rc c.ttf\nd\ne   \nf  \x0bf.ttf \x0c\ng g\u{a0}\n\
         {long} n.ttf\n{too_long} l.ttf\na other.ttf\n"
    )
    .leak();
    let loaded = load_files(&[("m.map", "MAP FONTSET 'fonts.txt' END"), ("fonts.txt", set)]);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
    let fonts = loaded.document.unwrap().map.fonts;
    let names: Vec<&str> = fonts.keys().collect();
    assert_eq!(
        names,
        ["a", "c", "f", "g", long.as_str()],
        "a line led by a tab has no name; a line feed and a carriage return end one line; one \
         without a file is skipped; the separator must fall in the alias buffer's first 63 bytes"
    );
    assert_eq!(
        fonts.get("a"),
        Some("other.ttf"),
        "a later line replaces a name"
    );
    assert_eq!(fonts.get("f"), Some("f.ttf"), "trimmed by isspace");
    assert_eq!(
        fonts.get("g"),
        Some("g\u{a0}"),
        "which takes no Unicode space"
    );
}

#[test]
fn a_label_names_a_font_without_case() {
    // msLookupHashTable compares the keys with strcasecmp.
    let files = [
        (
            "m.map",
            "MAP FONTSET 'f.txt' LAYER CLASS LABEL FONT 'SANS,Mono' END END END END",
        ),
        ("f.txt", "Sans s.ttf\nmono m.ttf\n"),
    ];
    let (loaded, _) = first_label(&files);
    assert_eq!(codes(&loaded), Vec::<&str>::new());
}

#[test]
fn font_names_match_without_case_and_the_last_path_wins() {
    let files = [
        (
            "m.map",
            "MAP FONTSET 'fonts.txt' LAYER CLASS LABEL FONT 'SANS' END END END END",
        ),
        ("fonts.txt", "Sans a.ttf\nmono m.ttf\nSANS b.ttf\n"),
    ];
    let (loaded, _) = first_label(&files);
    assert!(codes(&loaded).is_empty(), "{:?}", codes(&loaded));
    let fonts = loaded.document.unwrap().map.fonts;
    assert_eq!(
        fonts.keys().collect::<Vec<_>>(),
        ["Sans", "mono"],
        "the name as first written"
    );
    assert_eq!(fonts.get("Sans"), Some("b.ttf"), "the last path");
}

#[test]
fn a_font_list_entry_with_a_script_prefix_is_looked_up_after_its_colon() {
    let files = [
        (
            "m.map",
            "MAP FONTSET 'fonts.txt' LAYER CLASS LABEL FONT 'sans,ar:arabic,cn:none' END END END \
             END",
        ),
        ("fonts.txt", "sans s.ttf\narabic a.ttf\n"),
    ];
    let (loaded, _) = first_label(&files);
    let unknown: Vec<_> = loaded
        .diagnostics
        .iter()
        .filter(|d| d.code == "font-unknown")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(unknown, ["the font set has no font `cn:none`"]);
}

#[test]
fn a_font_set_s_lines_end_as_gdal_ends_them() {
    // CPLReadLineL ends a line at a line feed, a carriage return, or a pair of the two in either
    // order, which ends one line: a lone carriage return separates two fonts.
    let files = [
        ("m.map", "MAP FONTSET 'fonts.txt' END"),
        (
            "fonts.txt",
            "a a.ttf\rb b.ttf\r\nc c.ttf\n\rd d.ttf\n\ne e.ttf",
        ),
    ];
    let loaded = load_files(&files);
    let fonts = loaded.document.unwrap().map.fonts;
    let names: Vec<_> = fonts.keys().collect();
    assert_eq!(names, ["a", "b", "c", "d", "e"]);
    assert_eq!(fonts.get("a"), Some("a.ttf"));
    assert_eq!(fonts.get("e"), Some("e.ttf"));
}

#[test]
fn a_font_list_entry_is_looked_up_as_written_where_its_name_after_the_colon_is_unknown() {
    // check_single_font tries the name after the first colon, then the entry as written.
    let files = [
        (
            "m.map",
            "MAP FONTSET 'fonts.txt' LAYER CLASS LABEL FONT 'a:b' END END END END",
        ),
        ("fonts.txt", "a:b ab.ttf\n"),
    ];
    let (loaded, _) = first_label(&files);
    assert!(codes(&loaded).is_empty(), "{:?}", codes(&loaded));
}

#[test]
fn a_large_font_set_is_read_in_time_that_grows_with_its_lines() {
    // Verifies: Q-9
    // Each line finds its name's key at once: 200,000 distinct names, which a scan of the names
    // held for each line would take minutes over, and a label that names the last of them.
    let count = 200_000;
    let set: &'static str = (0..count)
        .map(|i| format!("font{i} f{i}.ttf\n"))
        .collect::<String>()
        .leak();
    let map: &'static str = format!(
        "MAP FONTSET 'fonts.txt' LAYER CLASS LABEL FONT 'FONT{}' END END END END",
        count - 1
    )
    .leak();
    let (loaded, _) = first_label(&[("m.map", map), ("fonts.txt", set)]);
    assert!(codes(&loaded).is_empty(), "{:?}", codes(&loaded));
    assert_eq!(loaded.document.unwrap().map.fonts.len(), count);
}

#[test]
fn a_font_list_s_trailing_comma_names_no_font() {
    // get_face_for_run stops at the list's end, so the empty entry after the comma is none.
    let files = [
        (
            "m.map",
            "MAP FONTSET 'fonts.txt' LAYER CLASS LABEL FONT 'sans,' END END END END",
        ),
        ("fonts.txt", "sans s.ttf\n"),
    ];
    let (loaded, _) = first_label(&files);
    assert!(codes(&loaded).is_empty(), "{:?}", codes(&loaded));
}

#[test]
fn an_empty_font_list_names_no_font_to_warn_of_at_load() {
    // get_face_for_run takes no entry from an empty list; the engine reports the label at render.
    let files = [
        (
            "m.map",
            "MAP FONTSET 'fonts.txt' LAYER CLASS LABEL FONT '' END END END END",
        ),
        ("fonts.txt", "sans s.ttf\n"),
    ];
    let (loaded, label) = first_label(&files);
    assert!(codes(&loaded).is_empty(), "{:?}", codes(&loaded));
    assert_eq!(label.unwrap().font, Some(Bound::Value(String::new())));
}
