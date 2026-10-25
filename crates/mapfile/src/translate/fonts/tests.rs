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
    assert_eq!(fonts["sans"], "f/DejaVuSans.ttf", "beside the font set");
    assert_eq!(fonts["mono"], "f/ttf/mono.ttf");
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
    let names: Vec<&str> = fonts.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        ["a", "f", "g", long.as_str()],
        "a line led by a tab has no name; one without a file is skipped; the separator must \
         fall in the alias buffer's first 63 bytes"
    );
    assert_eq!(fonts["a"], "other.ttf", "a later line replaces a name");
    assert_eq!(fonts["f"], "f.ttf", "trimmed by isspace");
    assert_eq!(fonts["g"], "g\u{a0}", "which takes no Unicode space");
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
