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
    // src/mapserver.h:277-281.
    assert_eq!((MIN_SIZE, MAX_SIZE), (0.0, 500.0));
    assert_eq!((MIN_WIDTH, MAX_WIDTH), (0.0, 32.0));
}

#[test]
fn marker_pixels_are_bounded_unscaled_and_cut_to_whole() {
    // msGetMarkerSize, src/mapsymbol.c:754-755: MS_MAX with MINSIZE, then MS_MIN with MAXSIZE,
    // into an int each time.
    let style: Style = Style {
        min_size: Some(4.5),
        max_size: Some(9.5),
        ..Style::default()
    };
    assert_eq!(marker_pixels(3.0, &style), 4.0);
    assert_eq!(marker_pixels(12.0, &style), 9.0);
    assert_eq!(marker_pixels(800.0, &<Style>::default()), 500.0);
}

#[test]
fn the_least_clip_buffer_is_the_larger_of_minsize_and_minwidth_scaled() {
    // msDrawShape, src/mapdraw.c:2491-2492.
    let style = Style {
        min_size: Some(3.0),
        min_width: Some(10.0),
        ..Style::default()
    };
    assert_eq!(clip_buffer_minimum(&style, 2.0), 20.0);
    assert_eq!(clip_buffer_minimum(&Style::default(), 2.0), 0.0);
}
