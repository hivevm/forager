// The token values these tests state are those of src/maplexer.l, src/mapfile.h,
// src/mapserver.h, and src/mapsymbol.h, of the release pinned in scripts/fetch-msautotest.sh.
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

use super::*;

#[test]
fn the_table_is_sorted_once_by_word_in_lower_case() {
    for pair in KEYWORDS.windows(2) {
        assert!(pair[0].0 < pair[1].0, "{pair:?}");
    }
    for (word, _) in KEYWORDS {
        assert_eq!(word, word.to_ascii_lowercase());
    }
}

#[test]
fn a_keyword_has_its_token_value_without_regard_to_case() {
    // src/mapserver.h: MS_OFF and MS_FALSE are 0, MS_ON and MS_TRUE 1, MS_TINY 5, MS_UL 101;
    // src/mapfile.h: CLASS is 1004; src/mapsymbol.h: MS_SYMBOL_SIMPLE is 1000.
    for (word, value) in [
        ("off", 0),
        ("FALSE", 0),
        ("On", 1),
        ("true", 1),
        ("tiny", 5),
        ("ul", 101),
        ("CLASS", 1004),
        ("simple", 1000),
    ] {
        assert_eq!(token_value(word), Some(value), "{word}");
    }
    for word in ["roads", "name1", "/data", "includes"] {
        assert_eq!(token_value(word), None, "{word}");
    }
}

#[test]
fn words_of_one_token_value_stand_for_each_other() {
    // LINEJOIN: none is 0, as are OFF, FALSE, INCHES, POINT; bevel is 1, as are TRUE, ON, FEET,
    // LINE. LINECAP: butt is 2, as are POLYGON, DEFAULT, MILES, CENTER. A label's ALIGN on is
    // left, 1; its SIZE dd is tiny, 5. SYMBOL TYPE: truetype is 0, as are FALSE and OFF.
    for (word, name) in [
        ("off", "none"),
        ("false", "none"),
        ("inches", "none"),
        ("point", "none"),
        ("true", "bevel"),
        ("on", "bevel"),
        ("feet", "bevel"),
        ("line", "bevel"),
        ("polygon", "butt"),
        ("default", "butt"),
        ("miles", "butt"),
        ("center", "butt"),
        ("on", "left"),
        ("dd", "tiny"),
        ("false", "truetype"),
        ("OFF", "TRUETYPE"),
        ("Round", "ROUND"),
    ] {
        assert!(stands_for(word, name), "{word} for {name}");
    }
    for (word, name) in [("round", "butt"), ("roads", "butt"), ("on", "off")] {
        assert!(!stands_for(word, name), "{word} for {name}");
    }
}
