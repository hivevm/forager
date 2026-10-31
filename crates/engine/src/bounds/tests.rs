// The tests of the bounds of a style quote MapServer's MS_MIN and MS_MAX and its default bounds of a
// style, src/mapserver.h, of the release pinned in scripts/fetch-msautotest.sh.
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
fn the_constants_are_mapservers() {
    // src/mapserver.h:251 and 277-281.
    assert_eq!(DEG_TO_RAD, "0.0174532925199432958".parse::<f64>().unwrap());
    assert_eq!((MIN_SIZE, MAX_SIZE), (0.0, 500.0));
    assert_eq!((MIN_WIDTH, MAX_WIDTH), (0.0, 32.0));
}

#[test]
fn ms_min_and_ms_max_keep_their_second_operand_where_they_cannot_compare() {
    // src/mapserver.h:78 and 81: `((a) < (b)) ? (a) : (b)` and `((a) > (b)) ? (a) : (b)`.
    assert_eq!(ms_min(1.0, 2.0), 1.0);
    assert_eq!(ms_max(1.0, 2.0), 2.0);
    assert_eq!(ms_min(f64::NAN, 2.0), 2.0);
    assert_eq!(ms_max(f64::NAN, 2.0), 2.0);
    assert!(ms_min(2.0, f64::NAN).is_nan());
    assert!(ms_max(2.0, f64::NAN).is_nan());
}
