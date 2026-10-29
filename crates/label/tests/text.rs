//! The text of a label (ADR-0028): its font list, its glyphs read as FreeType reads them
//! unhinted, and its line laid out and centred as MapServer lays it out. The fonts are subsets of
//! DejaVu Sans under `fixtures/`, whose licence and changes `fixtures/LICENSE` records.

use forager_label::{Attempt, Font, LineError, TextPath, Unusable, attempts, glyph_size, line};
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

fn path_of(font: &Font<'_>, text: &str, size: f64) -> (TextPath, Vec<PathCommand>) {
    let chars = line(text).unwrap();
    let glyphs = font.glyphs(&chars, true).unwrap().unwrap();
    let path = TextPath::new(font, &glyphs, size).unwrap();
    let drawn = path.draw(Point { x: 0.0, y: 0.0 }, [-1e6, -1e6, 1e6, 1e6]);
    (path, drawn)
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
    let (fl, _) = path_of(&font, "Fl", 10.0);
    let (f, _) = path_of(&font, "F", 10.0);
    let (l, _) = path_of(&font, "l", 10.0);
    // No kerning: the width is the sum of the advances.
    assert_eq!(fl.max_x, f.max_x + l.max_x);
    assert_eq!(fl.min_x, 0.0);
    // The box runs from the highest top to the lowest bottom, y down from the baseline.
    assert_eq!(fl.min_y, f.min_y.min(l.min_y));
    assert_eq!(fl.max_y, 0.0, "neither glyph reaches below the baseline");
    let at = fl.centred_on(Point { x: 50.0, y: 40.0 });
    let (w, h) = (fl.max_x - fl.min_x, fl.max_y - fl.min_y);
    assert_eq!(at.x, 50.0 - w / 2.0);
    assert_eq!(at.y, 40.0 + h / 2.0 - fl.max_y);
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
    let (fl, _) = path_of(&font, "Fl", 10.0);
    let all = fl.draw(Point { x: 0.0, y: 20.0 }, EVERYWHERE);
    let only_f = fl.draw(Point { x: 0.0, y: 20.0 }, [0.0, 0.0, 5.0, 40.0]);
    let (f, _) = path_of(&font, "F", 10.0);
    assert_eq!(only_f, f.draw(Point { x: 0.0, y: 20.0 }, EVERYWHERE));
    assert!(only_f.len() < all.len());
    assert!(
        fl.draw(Point { x: 0.0, y: 20.0 }, [100.0, 0.0, 200.0, 40.0])
            .is_empty()
    );
}

#[test]
fn only_a_line_below_u_02b0_is_laid_out() {
    // Verifies: ADR-0028
    assert_eq!(line("a\u{2AF}"), Ok(vec!['a', '\u{2AF}']));
    assert_eq!(line("a\u{2B0}"), Err(LineError::NotLatin('\u{2B0}')));
    assert_eq!(line("it\u{2019}s"), Err(LineError::NotLatin('\u{2019}')));
    // `\r` is skipped, a last `\n` closes the line, any other starts a second one.
    assert_eq!(line("a\r\nb\r"), Err(LineError::SeveralLines));
    assert_eq!(line("ab\r\n"), Ok(vec!['a', 'b']));
    assert_eq!(line("\r"), Ok(vec![]));
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
    let chars = line("aé").unwrap();
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
    let glyphs = font.glyphs(&line("1a").unwrap(), true).unwrap().unwrap();
    assert_eq!(glyphs, [font.glyph_index('1'), 0]);
    assert!(TextPath::new(&font, &glyphs, 10.0).is_ok());
}

#[test]
fn a_font_without_a_unicode_map_or_glyf_outlines_cannot_draw_a_line() {
    // Verifies: ADR-0028
    let symbol = fixture("dejavu-sans-symbol.ttf");
    let font = Font::new(&symbol).unwrap();
    assert_eq!(
        font.glyphs(&['1'], true),
        Err(Unusable::NoUnicodeCharacterMap)
    );
    // A font of CFF outlines that lacks a glyph is passed over as any other; one that has them
    // all cannot be drawn.
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
