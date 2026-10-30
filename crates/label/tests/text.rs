//! The text of a label (ADR-0028): its font list, its glyphs read as FreeType reads them
//! unhinted, and its line laid out and centred as MapServer lays it out. The fonts are subsets of
//! DejaVu Sans under `fixtures/`, whose licence and changes `fixtures/LICENSE` records.

use forager_label::{
    Align, Anchor, Attempt, Font, Glyphs, Layout, LineError, TextPath, Unusable, attempts,
    glyph_size, lines, metrics,
};
use forager_render::{PathCommand, Point};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).unwrap()
}

/// `FT_MulFix` of FreeType's src/base/ftcalc.c, with 64-bit integers.
fn mul_fix(a: i64, b: i64) -> i64 {
    let ab = a * b;
    (ab + 0x8000 - i64::from(ab < 0)) >> 16
}

/// `FT_DivFix` of FreeType's src/base/ftcalc.c, with 64-bit integers.
fn div_fix(a: i64, b: i64) -> i64 {
    let q = ((a.unsigned_abs() << 16) + (b.unsigned_abs() >> 1)) / b.unsigned_abs();
    if (a < 0) != (b < 0) {
        -(q as i64)
    } else {
        q as i64
    }
}

/// The points of a path drawn from (0, 0), in 26.6 units, y up as in the font.
fn points(path: &[PathCommand]) -> Vec<(i64, i64)> {
    let unit = |p: &Point| ((p.x * 64.0) as i64, (-p.y * 64.0) as i64);
    path.iter()
        .flat_map(|c| match c {
            PathCommand::MoveTo(p) | PathCommand::LineTo(p) => vec![unit(p)],
            PathCommand::QuadTo(c, p) => vec![unit(c), unit(p)],
            PathCommand::CubicTo(c0, c1, p) => vec![unit(c0), unit(c1), unit(p)],
            PathCommand::Close => vec![],
        })
        .collect()
}

/// A text laid out with its glyphs at `size`, and drawn from (0, 0).
struct Laid {
    path: TextPath,
    glyphs: Glyphs,
}

impl Laid {
    fn draw(&self, origin: Point, visible: [f64; 4]) -> Vec<PathCommand> {
        let drawn = self
            .path
            .draw(std::slice::from_ref(&self.glyphs), origin, 0.0, visible);
        assert_eq!(drawn.out_of_reach, 0);
        drawn.path
    }
}

/// The one line of `text`.
fn line(text: &str) -> Vec<char> {
    let mut lines = lines(text, None).unwrap();
    assert_eq!(lines.len(), 1);
    lines.remove(0)
}

fn laid(font: &Font<'_>, text: &str, size: f64) -> Laid {
    let chars = line(text);
    let ids = font.glyphs(&chars, true).unwrap().unwrap();
    let mut glyphs = Glyphs::new(size);
    let path = TextPath::new(font, &mut glyphs, &ids).unwrap();
    Laid { path, glyphs }
}

fn path_of(font: &Font<'_>, text: &str, size: f64) -> (TextPath, Vec<PathCommand>) {
    let laid = laid(font, text, size);
    let drawn = laid.draw(Point { x: 0.0, y: 0.0 }, EVERYWHERE);
    (laid.path, drawn)
}

const EVERYWHERE: [f64; 4] = [-1e6, -1e6, 1e6, 1e6];

#[test]
fn a_simple_glyph_whose_x_min_is_not_its_side_bearing_is_scaled_as_freetype_scales_it() {
    // Verifies: ADR-0028
    // The fixture's `l` has xMin 193 and, changed for this test, a left side bearing of 93 and an
    // advance of 569 in 2048 units; at a glyph size of 10 points its pixel size is 13.
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let (path, drawn) = path_of(&font, "l", 10.0);
    let scale = div_fix(13 << 6, 2048);
    // The phantom points: xMin - lsb and that plus the advance, each scaled; the outline is
    // shifted by the first, the advance is their difference.
    let pp1 = mul_fix(100, scale);
    let pp2 = mul_fix(100 + 569, scale);
    let expected: Vec<(i64, i64)> = [(193, 1556), (377, 1556), (377, 0), (193, 0)]
        .iter()
        .map(|&(x, y)| (mul_fix(x, scale) - pp1, mul_fix(y, scale)))
        .collect();
    assert_eq!(points(&drawn), expected);
    assert!(matches!(drawn.last(), Some(PathCommand::Close)));
    assert_eq!(path.max_x * 64.0, (pp2 - pp1) as f64, "the advance");
}

#[test]
fn a_composite_glyph_scales_its_component_offsets_apart_from_their_points() {
    // Verifies: ADR-0028
    // `í` is `acute` moved by (-227, 0) and `dotlessi`, whose metrics it uses.
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    for size in [10.0, 13.0, 37.0] {
        let (path, drawn) = path_of(&font, "í", size);
        let ppem = forager_label::pixel_size(size);
        let scale = div_fix(i64::from(ppem) << 6, 2048);
        let s = |v: i64| mul_fix(v, scale);
        let acute = [(651, 1638), (850, 1638), (524, 1262), (371, 1262)];
        let dotlessi = [(193, 1120), (377, 1120), (377, 0), (193, 0), (285, 1147)];
        let expected: Vec<(i64, i64)> = acute
            .iter()
            .map(|&(x, y)| (s(x) + s(-227), s(y)))
            .chain(dotlessi.iter().map(|&(x, y)| (s(x), s(y))))
            .collect();
        assert_eq!(points(&drawn), expected, "at {size} points");
        assert_eq!(
            path.max_x * 64.0,
            s(569) as f64,
            "the advance at {size} points"
        );
    }
}

#[test]
fn a_line_is_laid_out_by_advances_and_centred_on_its_point_by_its_box() {
    // Verifies: ADR-0028
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let fl = laid(&font, "Fl", 10.0);
    let (f, l) = (laid(&font, "F", 10.0), laid(&font, "l", 10.0));
    let (f, l, fl_path) = (&f.path, l, &fl.path);
    // No kerning: the width is the sum of the advances.
    assert_eq!(fl_path.max_x, f.max_x + l.path.max_x);
    assert_eq!(fl_path.min_x, 0.0);
    // The box runs from the highest top to the lowest bottom, y down from the baseline.
    assert_eq!(fl_path.min_y, f.min_y.min(l.path.min_y));
    assert_eq!(
        fl_path.max_y, 0.0,
        "neither glyph reaches below the baseline"
    );
    let text = fl_path.text_box();
    let (at, _) = metrics(Point { x: 50.0, y: 40.0 }, Anchor::Cc, &text, 0, 0, 0.0, 0);
    let (w, h) = (fl_path.max_x - fl_path.min_x, fl_path.max_y - fl_path.min_y);
    assert_eq!(at.x, 50.0 - w / 2.0);
    assert_eq!(at.y, 40.0 + h / 2.0 - fl_path.max_y);
    // A glyph's outline moves to its pen position from the line's point.
    let drawn = fl.draw(at, EVERYWHERE);
    let first_l = drawn
        .iter()
        .filter(|c| matches!(c, PathCommand::MoveTo(_)))
        .nth(1)
        .copied();
    let alone = l.draw(
        Point {
            x: at.x + f.max_x,
            y: at.y,
        },
        EVERYWHERE,
    );
    assert_eq!(first_l, alone.first().copied());
}

#[test]
fn a_glyph_wholly_outside_the_visible_area_is_left_out() {
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let fl = laid(&font, "Fl", 10.0);
    let at = Point { x: 0.0, y: 20.0 };
    let all = fl.draw(at, EVERYWHERE);
    let only_f = fl.draw(at, [0.0, 0.0, 5.0, 40.0]);
    assert_eq!(only_f, laid(&font, "F", 10.0).draw(at, EVERYWHERE));
    assert!(only_f.len() < all.len());
    assert!(fl.draw(at, [100.0, 0.0, 200.0, 40.0]).is_empty());
}

#[test]
fn a_glyph_reaching_too_far_from_the_visible_area_is_left_out_and_counted() {
    // Verifies: ADR-0028
    // A font of 16 units per em whose `1` is a square of 32,000 units around its origin: at a
    // glyph size of 6000 points it is 16,000,000 pixels wide, so that drawn into the image it
    // reaches past the reach.
    let data = fixture("dejavu-sans-huge.ttf");
    let font = Font::new(&data).unwrap();
    let huge = laid(&font, "1", 6000.0);
    let visible = [0.0, 0.0, 100.0, 100.0];
    let into_the_image = Point {
        x: -5_000_000.0,
        y: 100.0,
    };
    let drawn = huge.path.draw(
        std::slice::from_ref(&huge.glyphs),
        into_the_image,
        0.0,
        visible,
    );
    assert_eq!(
        drawn,
        forager_label::Drawn {
            path: vec![],
            out_of_reach: 1
        }
    );
    // A glyph of the same font at a pixel size that keeps within reach is drawn.
    let large = laid(&font, "1", 50.0);
    let into_the_image = Point {
        x: -20_000.0,
        y: 100.0,
    };
    let drawn = large.path.draw(
        std::slice::from_ref(&large.glyphs),
        into_the_image,
        0.0,
        visible,
    );
    assert_eq!(drawn.out_of_reach, 0);
    assert!(!drawn.path.is_empty());
}

#[test]
fn only_a_line_below_u_02b0_is_laid_out() {
    // Verifies: ADR-0028
    assert_eq!(lines("a\u{2AF}", None), Ok(vec![vec!['a', '\u{2AF}']]));
    assert_eq!(lines("a\u{2B0}", None), Err(LineError::NotLatin('\u{2B0}')));
    assert_eq!(
        lines("ok\nit\u{2019}s", None),
        Err(LineError::NotLatin('\u{2019}'))
    );
    // `\r` is skipped, a last `\n` closes the line, any other starts a second one.
    assert_eq!(lines("a\r\nb\r", None), Ok(vec![vec!['a'], vec!['b']]));
    assert_eq!(lines("ab\r\n", None), Ok(vec![vec!['a', 'b']]));
    assert_eq!(
        lines("a\n\nb", None),
        Ok(vec![vec!['a'], vec![], vec!['b']])
    );
    assert_eq!(lines("\n", None), Ok(vec![vec![]]));
    assert_eq!(lines("\r", None), Ok(vec![]));
}

#[test]
fn the_font_list_is_tried_as_get_face_for_run_tries_it_for_a_latin_line() {
    // Verifies: ADR-0028
    let attempt = |names: &[&'static str], last| Attempt {
        names: names.to_vec(),
        last,
    };
    // Entries prefixed `en:` first, then all; a prefix stripped first, the name as written next.
    assert_eq!(
        attempts("a,en:b,de:c"),
        [
            attempt(&["b"], false),
            attempt(&["a"], false),
            attempt(&["b", "en:b"], false),
            attempt(&["c", "de:c"], true),
        ]
    );
    // An empty entry inside the list is tried and finds nothing; a trailing comma ends the list
    // with no last entry, so none takes a `?`.
    assert_eq!(
        attempts("a,,b"),
        [
            attempt(&["a"], false),
            attempt(&[""], false),
            attempt(&["b"], true)
        ]
    );
    assert_eq!(attempts("a,"), [attempt(&["a"], false)]);
    assert_eq!(attempts(""), []);
}

#[test]
fn only_the_last_font_takes_its_question_mark_or_glyph_0_for_a_glyph_it_lacks() {
    // Verifies: ADR-0028
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let chars = line("aé");
    assert_eq!(font.glyphs(&chars, false), Ok(None));
    let question = font.glyph_index('?');
    assert_ne!(question, 0);
    assert_eq!(
        font.glyphs(&chars, true),
        Ok(Some(vec![font.glyph_index('a'), question]))
    );
    // A font without `?` takes glyph 0, which is drawn as any other.
    let digits = fixture("dejavu-sans-digits.ttf");
    let font = Font::new(&digits).unwrap();
    let ids = font.glyphs(&line("1a"), true).unwrap().unwrap();
    assert_eq!(ids, [font.glyph_index('1'), 0]);
    assert!(TextPath::new(&font, &mut Glyphs::new(10.0), &ids).is_ok());
}

#[test]
fn a_font_is_read_through_the_character_map_freetype_and_mapserver_select() {
    // Verifies: ADR-0028
    let digits = fixture("dejavu-sans-digits.ttf");
    let plain = Font::new(&digits).unwrap();
    // Unicode before Symbol, whatever the order of the records; a glyph past the font's glyphs
    // is none, as FT_Get_Char_Index holds it.
    let both = fixture("dejavu-sans-both.ttf");
    let font = Font::new(&both).unwrap();
    assert_eq!(font.glyph_index('1'), plain.glyph_index('1'));
    assert_eq!(font.glyph_index('9'), 0);
    // A font with only a Symbol map is looked up at the code point | 0xF000, as
    // msGetGlyphIndex looks it up, so that lacking a glyph it gives way to the next font; one
    // that would be used cannot be drawn (ADR-0028).
    let symbol = fixture("dejavu-sans-symbol.ttf");
    let font = Font::new(&symbol).unwrap();
    assert_eq!(font.glyph_index('1'), plain.glyph_index('1'));
    assert_eq!(font.glyphs(&['1', 'a'], false), Ok(None));
    assert_eq!(
        font.glyphs(&['1', '2'], false),
        Err(Unusable::SymbolCharacterMap)
    );
}

#[test]
fn a_font_whose_glyphs_cannot_be_found_is_unusable_at_once() {
    // Verifies: ADR-0028
    // Without a Unicode or a Symbol map, FreeType reads a font whose `post` table names its
    // glyphs through a Unicode map it builds from the names.
    let named = fixture("dejavu-sans-mac-names.ttf");
    let font = Font::new(&named).unwrap();
    assert_eq!(
        font.glyphs(&['a'], false),
        Err(Unusable::NoUnicodeCharacterMap)
    );
    // A map of format 2 or 8, which this crate does not read.
    let format2 = fixture("dejavu-sans-format2.ttf");
    let font = Font::new(&format2).unwrap();
    assert_eq!(
        font.glyphs(&['a'], false),
        Err(Unusable::UnreadCharacterMapFormat)
    );
}

#[test]
fn a_font_this_crate_cannot_draw_in_is_unusable_only_once_it_would_be_used() {
    // Verifies: ADR-0028
    // An Apple Roman map, read where the `post` table names no glyph, is looked up as FreeType
    // looks it up, so that a font lacking a glyph is passed over as any other; one that has them
    // all cannot be drawn.
    let mac = fixture("dejavu-sans-mac.ttf");
    let font = Font::new(&mac).unwrap();
    assert_eq!(font.glyphs(&['1', 'a'], false), Ok(None));
    assert_eq!(
        font.glyphs(&['1'], false),
        Err(Unusable::AppleRomanCharacterMap)
    );
    let cff = fixture("boxes-cff.otf");
    let font = Font::new(&cff).unwrap();
    assert_eq!(font.glyphs(&['1', 'a'], false), Ok(None));
    assert_eq!(font.glyphs(&['1'], false), Err(Unusable::NoGlyfOutlines));
    assert!(Font::new(b"font").is_err());
}

#[test]
fn glyph_sizes_follow_ms_compute_text_path() {
    // Verifies: ADR-0028
    assert_eq!(glyph_size(8.5, 1.0, 4.0, 256.0, 1.0), 8.0);
    assert_eq!(glyph_size(9.5, 1.0, 4.0, 256.0, 1.0), 10.0);
}

#[test]
fn a_glyph_that_cannot_be_drawn_takes_the_fonts_question_mark_or_fails() {
    // `1` and glyph 0 of this font cannot be read, and it has no `?`.
    let data = fixture("dejavu-sans-broken.ttf");
    let font = Font::new(&data).unwrap();
    let ids = font.glyphs(&['2', '1'], true).unwrap().unwrap();
    let one = font.glyph_index('1');
    assert_eq!(
        TextPath::new(&font, &mut Glyphs::new(10.0), &ids).err(),
        Some(one)
    );
    let two = font.glyphs(&['2'], true).unwrap().unwrap();
    assert!(TextPath::new(&font, &mut Glyphs::new(10.0), &two).is_ok());
}

/// The text of `lines` in `font` at a glyph size of 10, aligned by `align`, its glyphs in one
/// cache.
fn laid_lines(font: &Font<'_>, text: &[&str], align: Align) -> Laid {
    let mut glyphs = Glyphs::new(10.0);
    let mut layout = Layout::new(10.0, text.len());
    for line in text {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            layout.empty_line();
        } else {
            let ids = font.glyphs(&chars, true).unwrap().unwrap();
            layout.line(font, &mut glyphs, 0, &ids).unwrap();
        }
    }
    Laid {
        path: layout.finish(align),
        glyphs,
    }
}

/// The first point of a path.
fn first(path: &[PathCommand]) -> Point {
    match path.first() {
        Some(PathCommand::MoveTo(p)) => *p,
        other => panic!("no first point: {other:?}"),
    }
}

#[test]
fn lines_stand_a_line_height_apart_and_are_aligned_by_their_lengths_cut_to_whole_pixels() {
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let (fl, f) = (laid(&font, "Fl", 10.0), laid(&font, "F", 10.0));
    let origin = Point { x: 20.0, y: 40.0 };
    for (align, shift) in [
        (Align::Left, 0.0),
        // (int lengths) / 2.0 and their difference, as line_desc holds a length in an int.
        (
            Align::Center,
            f64::from(fl.path.max_x as i32 - f.path.max_x as i32) / 2.0,
        ),
        (
            Align::Right,
            f64::from(fl.path.max_x as i32 - f.path.max_x as i32),
        ),
    ] {
        let two = laid_lines(&font, &["Fl", "F"], align);
        assert_eq!(two.path.num_lines, 2);
        assert_eq!(two.path.max_x, fl.path.max_x, "the longest line's length");
        // The first line's top, ceil(10 × 1.33) = 14 above the last line's baseline, and the
        // last line's bottom.
        assert_eq!(two.path.min_y, fl.path.min_y - 14.0);
        assert_eq!(two.path.max_y, f.path.max_y);
        let drawn = two.draw(origin, EVERYWHERE);
        // The first line stands a line height above the origin, the second on it.
        assert_eq!(
            first(&drawn),
            first(&fl.draw(Point { x: 20.0, y: 26.0 }, EVERYWHERE))
        );
        let second = drawn.len() - f.draw(origin, EVERYWHERE).len();
        assert_eq!(
            drawn[second..],
            f.draw(
                Point {
                    x: 20.0 + shift,
                    y: 40.0
                },
                EVERYWHERE
            )[..],
            "{align:?}"
        );
    }
}

#[test]
fn an_empty_line_takes_its_height_and_shifts_the_alignment_of_the_lines_after_it() {
    // msLayoutTextSymbol counts the lines as the glyphs' height changes: the empty second line
    // is skipped, so that the third takes the second's length, 0, and stands right of all.
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let f = laid(&font, "F", 10.0);
    let three = laid_lines(&font, &["Fl", "", "F"], Align::Right);
    assert_eq!(three.path.num_lines, 3);
    let longest = laid(&font, "Fl", 10.0).path.max_x as i32;
    let drawn = three.draw(Point { x: 0.0, y: 50.0 }, EVERYWHERE);
    let last = drawn.len() - f.draw(Point { x: 0.0, y: 0.0 }, EVERYWHERE).len();
    assert_eq!(
        drawn[last..],
        f.draw(
            Point {
                x: f64::from(longest),
                y: 50.0
            },
            EVERYWHERE
        )[..]
    );
    // Its first line stands two line heights above.
    assert_eq!(three.path.min_y, laid(&font, "Fl", 10.0).path.min_y - 28.0);
}

#[test]
fn a_wrap_character_breaks_the_text_into_lines_only_where_it_is_ascii() {
    assert_eq!(
        lines("a b\nc", Some(' ')),
        Ok(vec![vec!['a'], vec!['b'], vec!['c']])
    );
    // A last wrap closes the line as a last line break does.
    assert_eq!(lines("ab ", Some(' ')), Ok(vec![vec!['a', 'b']]));
    // MapServer holds WRAP in a C char: a character outside ASCII never matches, NUL wraps
    // nothing.
    assert_eq!(
        lines("a\u{e9}b", Some('\u{e9}')),
        Ok(vec![vec!['a', '\u{e9}', 'b']])
    );
    assert_eq!(lines("a\0b", Some('\0')), Ok(vec![vec!['a', '\0', 'b']]));
}

#[test]
fn a_turned_text_turns_its_glyphs_points_and_outlines_about_the_label_point() {
    let data = fixture("dejavu-sans-subset.ttf");
    let font = Font::new(&data).unwrap();
    let fl = laid(&font, "Fl", 10.0);
    let origin = Point { x: 50.0, y: 50.0 };
    let flat = fl.draw(origin, EVERYWHERE);
    let quarter = std::f64::consts::FRAC_PI_2;
    let turned = fl.path.draw(
        std::slice::from_ref(&fl.glyphs),
        origin,
        quarter,
        EVERYWHERE,
    );
    assert_eq!(turned.out_of_reach, 0);
    assert_eq!(turned.path.len(), flat.len());
    // A quarter turn counter-clockwise, y down: (x, y) from the point goes to (y, -x).
    let near = |a: Point, b: Point| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9;
    for (t, f) in turned.path.iter().zip(&flat) {
        let (
            PathCommand::MoveTo(t) | PathCommand::LineTo(t),
            PathCommand::MoveTo(f) | PathCommand::LineTo(f),
        ) = (t, f)
        else {
            continue;
        };
        let expected = Point {
            x: origin.x + (f.y - origin.y),
            y: origin.y - (f.x - origin.x),
        };
        assert!(near(*t, expected), "{t:?} for {expected:?}");
    }
    // Turned out of the visible area, nothing is drawn: the turn, not the flat text, decides.
    let above = [0.0, 0.0, 100.0, 49.0];
    assert!(!fl.draw(origin, above).is_empty());
    let left = [0.0, 0.0, 49.0, 100.0];
    assert!(
        fl.path
            .draw(std::slice::from_ref(&fl.glyphs), origin, -quarter, left)
            .path
            .is_empty()
    );
}
