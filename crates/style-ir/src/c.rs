// C's and MapServer's small functions, ported once for the frontend, the core, and the adapters
// (ADR-0036): glibc's strtod in the C locale and the atof and atoi it backs, its lrint on x86-64,
// and the (int) cast of a double as x86-64 computes it; MapServer's MS_NINT, MS_MIN, MS_MAX, and
// MS_DEG_TO_RAD of src/mapserver.h, msRectOverlap of src/mapsearch.c, msStringSplit of
// src/mapstring.cpp, msComputeBounds of src/mapprimitive.cpp, msGetSymbolIndex's match of a name
// of src/mapfile.c, and the alpha of an opacity of loadStyle, src/mapfile.c, bindStyle,
// src/maputil.c, and aggCompositeRasterBufferNoPixman, src/mapagg.cpp, of the release pinned in
// scripts/fetch-msautotest.sh.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Copyright (c) 1996-2007 Regents of the University of Minnesota (src/mapagg.cpp).
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

//! C's and MapServer's small functions as MapServer's build at the pin runs them: `strtod`,
//! `atof`, and `atoi` of glibc in the C locale, `MS_NINT`, which is `lrint` on x86-64, the `(int)`
//! cast of a `double`, a C string's end, `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`,
//! `msStringSplit`, `msComputeBounds`, `msGetSymbolIndex`'s match of a symbol's name, and the
//! alpha `MS_NINT(opacity * 2.55)` of an opacity (ADR-0036).

use crate::Symbol;

/// C's `isspace` in the C locale.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// C's `strtod`, glibc's in the C locale: after leading white space and a sign, the longest
/// prefix that reads as a decimal number, a hexadecimal one (`0x1.8p3`), `inf`, `infinity`, or
/// `nan` with an optional `(tag)`, each without regard to case; with the count of bytes it reads,
/// white space and sign included, which is 0 where no number is read and the value is 0.
/// Correctly rounded, as glibc rounds, a number past the range of a double reading as an infinity.
pub fn strtod(text: &[u8]) -> (f64, usize) {
    let mut i = text.iter().take_while(|&&b| is_space(b)).count();
    let negative = text.get(i) == Some(&b'-');
    if matches!(text.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let rest = &text[i..];
    let (magnitude, len) = if starts_with_ignore_case(rest, b"inf") {
        let len = if starts_with_ignore_case(rest, b"infinity") {
            8
        } else {
            3
        };
        (f64::INFINITY, len)
    } else if starts_with_ignore_case(rest, b"nan") {
        (f64::NAN, 3 + nan_tag(&rest[3..]))
    } else if starts_with_ignore_case(rest, b"0x")
        && let Some((value, len)) = hex_number(&rest[2..])
    {
        (value, 2 + len)
    } else {
        decimal(rest)
    };
    if len == 0 {
        return (0.0, 0);
    }
    (if negative { -magnitude } else { magnitude }, i + len)
}

/// C's `atof`: [`strtod`]'s value, 0 where it reads no number.
pub fn atof(text: &[u8]) -> f64 {
    strtod(text).0
}

fn starts_with_ignore_case(text: &[u8], prefix: &[u8]) -> bool {
    text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// The length of the `(n-char-sequence)` glibc's strtod reads after `nan`: letters, digits, and
/// `_` in parentheses; 0 where it does not close.
fn nan_tag(text: &[u8]) -> usize {
    if text.first() != Some(&b'(') {
        return 0;
    }
    let inner = text[1..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .count();
    if text.get(1 + inner) == Some(&b')') {
        inner + 2
    } else {
        0
    }
}

/// The longest decimal number at the start of `text`, and its length; 0 and 0 where none.
fn decimal(text: &[u8]) -> (f64, usize) {
    let digits = |from: usize| {
        from + text[from.min(text.len())..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let int_end = digits(0);
    let mut end = int_end;
    let mut has_digits = int_end > 0;
    if text.get(end) == Some(&b'.') {
        let frac_end = digits(end + 1);
        has_digits |= frac_end > end + 1;
        end = frac_end;
    }
    if !has_digits {
        return (0.0, 0);
    }
    if matches!(text.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(text.get(end + 1), Some(b'+' | b'-')));
        let exp_end = digits(end + 1 + sign);
        if exp_end > end + 1 + sign {
            end = exp_end;
        }
    }
    // ASCII digits, a point, and an exponent: Rust reads them as strtod does, correctly rounded.
    let value = std::str::from_utf8(&text[..end])
        .ok()
        .and_then(|number| number.parse().ok())
        .unwrap_or(0.0);
    (value, end)
}

/// A hexadecimal number after its `0x`: hex digits with an optional point, and an optional binary
/// exponent `p±n`; with the count of bytes read, `None` where there is no digit, as strtod then
/// reads the `0` alone.
fn hex_number(text: &[u8]) -> Option<(f64, usize)> {
    let mut mantissa: u64 = 0;
    let mut sticky = false;
    // The power of two the mantissa is scaled by.
    let mut exponent: i64 = 0;
    let mut count = 0;
    let mut i = 0;
    let mut point = false;
    while let Some(&b) = text.get(i) {
        if b == b'.' && !point {
            point = true;
            i += 1;
            continue;
        }
        let Some(digit) = (b as char).to_digit(16) else {
            break;
        };
        count += 1;
        if mantissa >> 60 == 0 {
            mantissa = mantissa << 4 | u64::from(digit);
            if point {
                exponent -= 4;
            }
        } else {
            sticky |= digit != 0;
            if !point {
                exponent += 4;
            }
        }
        i += 1;
    }
    if count == 0 {
        return None;
    }
    if matches!(text.get(i), Some(b'p' | b'P')) {
        let mut j = i + 1;
        let negative = text.get(j) == Some(&b'-');
        if matches!(text.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let start = j;
        let mut value: i64 = 0;
        while let Some(d) = text.get(j).filter(|b| b.is_ascii_digit()) {
            value = (value * 10 + i64::from(d - b'0')).min(1 << 20);
            j += 1;
        }
        if j > start {
            exponent += if negative { -value } else { value };
            i = j;
        }
    }
    Some((scaled(mantissa, sticky, exponent), i))
}

/// `mantissa · 2^exponent`, `sticky` telling that bits below the mantissa were not zero, rounded
/// to a double half to even, as strtod rounds.
fn scaled(mantissa: u64, sticky: bool, exponent: i64) -> f64 {
    if mantissa == 0 {
        return 0.0;
    }
    let zeros = mantissa.leading_zeros();
    let m = mantissa << zeros;
    // The value is m · 2^(e - 63) with m's top bit set: its binary exponent is e.
    let e = exponent - i64::from(zeros) + 63;
    if e > 1023 {
        return f64::INFINITY;
    }
    // The bits a double keeps of it: 53, fewer below the normal range.
    let keep = if e >= -1022 { 53 } else { 53 + (e + 1022) };
    if keep < 0 {
        return 0.0;
    }
    let shift = 64 - keep as u32;
    let (kept, rest) = if shift == 64 {
        (0, m)
    } else {
        (m >> shift, m & ((1u64 << shift) - 1))
    };
    let half = 1u64 << (shift - 1);
    let up = rest > half || (rest == half && (sticky || kept & 1 == 1));
    let kept = kept + u64::from(up);
    // kept · 2^(e - keep + 1), exact: kept has at most 54 bits and the result is representable.
    let mut value = kept as f64;
    let mut power = e - keep + 1;
    while power != 0 {
        let step = power.clamp(-1000, 1000);
        value *= f64::from_bits(((step + 1023) as u64) << 52);
        power -= step;
    }
    value
}

/// C's `atoi`: `strtol` in base 10 cut to an `int`, as glibc returns it: white space, a sign,
/// digits; 0 where none; past the range of a `long`, its bound, whose low 32 bits are kept.
pub fn atoi(text: &[u8]) -> i32 {
    let mut i = text.iter().take_while(|&&b| is_space(b)).count();
    let negative = text.get(i) == Some(&b'-');
    if matches!(text.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let mut value: i64 = 0;
    let mut overflow = false;
    for &b in text[i..].iter().take_while(|b| b.is_ascii_digit()) {
        match value
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(b - b'0')))
        {
            Some(v) => value = v,
            None => overflow = true,
        }
    }
    let long = match (overflow, negative) {
        (true, false) => i64::MAX,
        (true, true) => i64::MIN,
        (false, false) => value,
        (false, true) => -value,
    };
    long as i32
}

/// C's `lrint` as glibc computes it on x86-64, with `cvtsd2si`: the nearest `long`, half to even,
/// and `LONG_MIN` for a value past a `long` and for no number.
pub fn lrint(x: f64) -> i64 {
    const LONG: f64 = 9_223_372_036_854_775_808.0;
    let rounded = x.round_ties_even();
    if (-LONG..LONG).contains(&rounded) {
        rounded as i64
    } else {
        i64::MIN
    }
}

/// `MS_NINT` into an `int`, as MapServer stores it: `lrint`, which src/mapserver.h makes it where
/// the build finds one, as the Linux build at the pin does, narrowed as GCC narrows a `long`, to
/// its low 32 bits. So 3e9 gives -1294967296, and a value past a `long` 0.
pub fn ms_nint(x: f64) -> i32 {
    lrint(x) as i32
}

/// The alpha of an opacity, `MS_NINT(opacity * 2.55)` of an `int` opacity, kept in a byte: the
/// alpha loadStyle and bindStyle give a style's colours below 100 (`src/mapfile.c:2799`,
/// `src/maputil.c:270`), an `int` AGG's `rgba8` keeps in an `int8u`, and the cover
/// aggCompositeRasterBufferNoPixman lays a layer over at (`src/mapagg.cpp:1389-1395`), an
/// `unsigned` AGG's `blend_from` takes as a `cover_type`, an `unsigned char`. Each keeps the low 8
/// bits, so -1 gives 253 and an opacity of -1294967296 179.
pub fn opacity_alpha(opacity: i32) -> u8 {
    ms_nint(f64::from(opacity) * 2.55).to_le_bytes()[0]
}

/// C's `(int)` cast of a `double`, and its assignment to an `int`, as the x86-64 build at the pin
/// computes it with `cvttsd2si`: toward zero, and `INT_MIN` for a value past an `int` and for no
/// number, where Rust's `as` saturates and gives 0 for NaN.
pub fn int_cast(x: f64) -> i32 {
    if x > -2_147_483_649.0 && x < 2_147_483_648.0 {
        // Within an `int` after truncation, so `as` truncates exactly as the C does.
        x as i32
    } else {
        i32::MIN
    }
}

/// `MS_MIN(a, b)`, `src/mapserver.h:78`: `a` where it compares below `b`, otherwise `b`, so `b`
/// where either is no number.
pub fn ms_min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// `MS_MAX(a, b)`, `src/mapserver.h:81`: `a` where it compares above `b`, otherwise `b`, so `b`
/// where either is no number.
pub fn ms_max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// `MS_DEG_TO_RAD`, `src/mapserver.h:251`, `.0174532925199432958`, read to the nearest double.
pub const DEG_TO_RAD: f64 = 0.017_453_292_519_943_295;

/// `msRectOverlap`, `src/mapsearch.c:44-54`, of two rectangles each given as
/// `[min x, min y, max x, max y]`: whether they share a point, their edges included. A bound that
/// is no number fails no comparison, so it overlaps.
pub fn rect_overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    let [a_min_x, a_min_y, a_max_x, a_max_y] = a;
    let [b_min_x, b_min_y, b_max_x, b_max_y] = b;
    !(a_min_x > b_max_x || a_max_x < b_min_x || a_min_y > b_max_y || a_max_y < b_min_y)
}

/// `msComputeBounds`, `src/mapprimitive.cpp:260-286`, of a shape's points, its parts run
/// together, as `[min x, min y, max x, max y]`: from its first point, each bound folded over every
/// point by [`ms_min`] or [`ms_max`]. So a point that is no number makes a bound no number, and
/// the next point takes its place, what came before forgotten. `None` for a shape without points,
/// whose bounds the C leaves as they were.
pub fn ms_compute_bounds(points: impl IntoIterator<Item = [f64; 2]>) -> Option<[f64; 4]> {
    let mut points = points.into_iter().peekable();
    let &[x, y] = points.peek()?;
    let mut bounds = [x, y, x, y];
    for [x, y] in points {
        let [min_x, min_y, max_x, max_y] = bounds;
        bounds = [
            ms_min(min_x, x),
            ms_min(min_y, y),
            ms_max(max_x, x),
            ms_max(max_y, y),
        ];
    }
    Some(bounds)
}

/// `msStringSplit`, `src/mapstring.cpp:863-908`, of `text` at `separator`: one token more than
/// the runs of separators, a run parting once; a leading or a trailing separator leaves an empty
/// token, and an empty text is one empty token. `text` is read as C reads it, up to its first NUL.
pub fn ms_string_split(text: &[u8], separator: u8) -> Vec<&[u8]> {
    let text = c_bytes(text);
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut last = 0u8;
    for (i, &b) in text.iter().enumerate() {
        if b == separator {
            if last != separator {
                tokens.push(&text[start..i]);
            }
            start = i + 1;
        }
        last = b;
    }
    tokens.push(&text[start..]);
    tokens
}

/// A C string's bytes: up to the first NUL, where `strlen` stops.
pub fn c_bytes(bytes: &[u8]) -> &[u8] {
    bytes.split(|&b| b == 0).next().unwrap_or_default()
}

/// `text` up to its first NUL, where a C string of it ends.
pub fn c_str(text: &str) -> &str {
    text.split('\0').next().unwrap_or_default()
}

/// `msGetSymbolIndex`, `src/mapfile.c:408-427`, without the `msAddImageSymbol` it may call where
/// no symbol matches: the index, from 1, of the first of `symbols` whose name matches `name`, the
/// IR's symbols being MapServer's from index 1, since its symbol 0 has no name. A name matches as
/// `strcasecmp`, glibc's in the C locale, compares: byte by byte, ASCII letters folded, so where
/// their [`symbol_name_key`]s are equal. `None` for a name no symbol carries, where the C gives -1.
pub fn ms_get_symbol_index(name: &str, symbols: &[Symbol]) -> Option<usize> {
    symbols
        .iter()
        .position(|symbol| {
            symbol
                .name
                .as_ref()
                .is_some_and(|known| known.eq_ignore_ascii_case(name))
        })
        .map(|position| position + 1)
}

/// The key by which two names match in [`ms_get_symbol_index`]: equal for exactly the names
/// `strcasecmp` in the C locale finds equal, for a map from a name to its symbol.
pub fn symbol_name_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atof_reads_the_longest_number_strtod_reads() {
        // Verifies: ADR-0036
        let cases: &[(&str, f64)] = &[
            ("12abc", 12.0),
            ("abc", 0.0),
            ("", 0.0),
            ("  \t-3.5e2x", -350.0),
            ("+.5", 0.5),
            ("1.", 1.0),
            (".", 0.0),
            ("-", 0.0),
            ("1e", 1.0),
            ("1e+", 1.0),
            ("1e-2", 0.01),
            ("0x10", 16.0),
            ("0x1.8p1", 3.0),
            ("-0X.8", -0.5),
            ("0x", 0.0),
            ("0xg", 0.0),
            ("1e400", f64::INFINITY),
            ("-Infinity", f64::NEG_INFINITY),
            ("INFx", f64::INFINITY),
            ("1e-400", 0.0),
            ("0x1p-1074", f64::from_bits(1)),
            ("0x1p-1075", 0.0),
            ("0x1.8p-1074", f64::from_bits(2)),
            ("0x1p1024", f64::INFINITY),
            ("0x1.fffffffffffff8p0", 2.0),
            ("0x1.fffffffffffff7p0", 2.0 - f64::EPSILON),
        ];
        for &(text, expected) in cases {
            assert_eq!(atof(text.as_bytes()), expected, "atof({text:?})");
        }
        assert!(atof(b"nan").is_nan());
        assert!(atof(b" -NaN(1)").is_nan());
        assert_eq!(atof(b"0.1"), 0.1);
    }

    #[test]
    fn strtod_reads_up_to_where_glibc_stops() {
        // Verifies: ADR-0036
        let cases: &[(&str, usize)] = &[
            ("", 0),
            ("abc", 0),
            ("-", 0),
            (" .", 0),
            ("  +12abc", 5),
            ("1e", 1),
            ("1e+", 1),
            ("1.5e-3x", 6),
            ("0x", 1),
            ("0xg", 1),
            ("-0x10,", 5),
            ("0x1p", 3),
            ("0x1.8p-2)", 8),
            ("inf", 3),
            ("-Infinity", 9),
            ("infinit", 3),
            ("nan", 3),
            ("NaN(abc_1)", 10),
            ("nan(", 3),
            ("nan(a-b)", 3),
        ];
        for &(text, len) in cases {
            assert_eq!(strtod(text.as_bytes()).1, len, "strtod({text:?})");
        }
    }

    #[test]
    fn atoi_reads_an_int_as_glibc_does() {
        // Verifies: ADR-0036
        assert_eq!(atoi(b" 42x"), 42);
        assert_eq!(atoi(b"-7"), -7);
        assert_eq!(atoi(b"\t255"), 255);
        assert_eq!(atoi(b"x"), 0);
        assert_eq!(atoi(b"4294967297"), 1);
        assert_eq!(atoi(b"99999999999999999999"), -1);
    }

    #[test]
    fn lrint_rounds_half_to_even_and_gives_long_min_past_a_long() {
        // Verifies: ADR-0036
        assert_eq!(lrint(2.5), 2);
        assert_eq!(lrint(3.5), 4);
        assert_eq!(lrint(-2.5), -2);
        assert_eq!(lrint(-0.4), 0);
        assert_eq!(lrint(3e9), 3_000_000_000);
        assert_eq!(lrint(-9_223_372_036_854_775_808.0), i64::MIN);
        assert_eq!(
            lrint(9_223_372_036_854_774_784.0),
            9_223_372_036_854_774_784
        );
        for past in [
            9_223_372_036_854_775_808.0,
            1e19,
            -1e19,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert_eq!(lrint(past), i64::MIN, "lrint({past})");
        }
    }

    #[test]
    fn ms_nint_keeps_the_low_32_bits_of_lrint() {
        // Verifies: ADR-0036
        assert_eq!(ms_nint(2.5), 2);
        assert_eq!(ms_nint(-2_147_483_648.5), i32::MIN);
        assert_eq!(ms_nint(2_147_483_648.0), i32::MIN);
        assert_eq!(ms_nint(3e9), -1_294_967_296);
        assert_eq!(ms_nint(-3_302_166_604.8), 992_800_691);
        assert_eq!(ms_nint(4_294_967_297.0), 1);
        assert_eq!(ms_nint(1e19), 0);
        assert_eq!(ms_nint(f64::NAN), 0);
    }

    #[test]
    fn opacity_alpha_is_ms_nint_of_the_opacity_times_2_55_kept_in_a_byte() {
        // Verifies: ADR-0036
        // 50 × 2.55 is 127.4999…, 99 × 2.55 252.45, -1 × 2.55 -2.55, whose MS_NINT -3 is 253 in a
        // byte; MS_NINT(-1294967296 × 2.55) is 992800691, whose low byte is 179.
        assert_eq!(opacity_alpha(0), 0);
        assert_eq!(opacity_alpha(50), 127);
        assert_eq!(opacity_alpha(99), 252);
        assert_eq!(opacity_alpha(100), 255);
        assert_eq!(opacity_alpha(101), 2);
        assert_eq!(opacity_alpha(-1), 253);
        assert_eq!(opacity_alpha(-1_294_967_296), 179);
    }

    #[test]
    fn int_cast_truncates_toward_zero_and_gives_int_min_past_an_int() {
        // Verifies: ADR-0036
        assert_eq!(int_cast(2.9), 2);
        assert_eq!(int_cast(-2.9), -2);
        assert_eq!(int_cast(-0.5), 0);
        assert_eq!(int_cast(2_147_483_647.9), i32::MAX);
        assert_eq!(int_cast(-2_147_483_648.9), i32::MIN);
        for past in [
            2_147_483_648.0,
            -2_147_483_649.0,
            3e9,
            -3e9,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert_eq!(int_cast(past), i32::MIN, "int_cast({past})");
        }
    }

    #[test]
    fn ms_min_and_ms_max_keep_their_second_operand_where_they_cannot_compare() {
        // Verifies: ADR-0036
        // src/mapserver.h:78 and 81: `((a) < (b)) ? (a) : (b)` and `((a) > (b)) ? (a) : (b)`.
        assert_eq!(ms_min(1.0, 2.0), 1.0);
        assert_eq!(ms_max(1.0, 2.0), 2.0);
        assert_eq!(ms_min(f64::NAN, 2.0), 2.0);
        assert_eq!(ms_max(f64::NAN, 2.0), 2.0);
        assert!(ms_min(2.0, f64::NAN).is_nan());
        assert!(ms_max(2.0, f64::NAN).is_nan());
        // -0.0 does not compare below 0.0: the second is kept.
        assert!(ms_min(-0.0, 0.0).is_sign_positive());
    }

    #[test]
    fn deg_to_rad_is_mapservers_constant() {
        // Verifies: ADR-0036
        assert_eq!(DEG_TO_RAD, ".0174532925199432958".parse::<f64>().unwrap());
        assert_eq!(DEG_TO_RAD, std::f64::consts::PI / 180.0);
    }

    #[test]
    fn rect_overlap_holds_edges_and_lets_no_number_through() {
        // Verifies: ADR-0036
        let a = [0.0, 0.0, 10.0, 10.0];
        assert!(rect_overlap(a, [10.0, 10.0, 20.0, 20.0]));
        assert!(rect_overlap(a, [2.0, 2.0, 3.0, 3.0]));
        assert!(!rect_overlap(a, [10.5, 0.0, 20.0, 10.0]));
        assert!(!rect_overlap(a, [0.0, -5.0, 10.0, -0.5]));
        // No number across, but apart along y.
        assert!(!rect_overlap(a, [f64::NAN, 50.0, f64::NAN, 60.0]));
        assert!(rect_overlap(a, [f64::NAN; 4]));
    }

    #[test]
    fn ms_compute_bounds_folds_by_ms_min_and_ms_max_from_the_first_point() {
        // Verifies: ADR-0036
        // src/mapprimitive.cpp:260-286: no line with a point leaves the bounds as they were.
        assert_eq!(ms_compute_bounds([]), None);
        assert_eq!(ms_compute_bounds([[1.0, 2.0]]), Some([1.0, 2.0, 1.0, 2.0]));
        // The first line with a point gives the first bounds, one without any passed over.
        let parts: [&[[f64; 2]]; 2] = [&[], &[[1.0, 2.0], [3.0, 0.0]]];
        assert_eq!(
            ms_compute_bounds(parts.into_iter().flatten().copied()),
            Some([1.0, 0.0, 3.0, 2.0])
        );
        assert_eq!(
            ms_compute_bounds([[3.0, 0.0], [-1.0, 5.0], [2.0, -4.0]]),
            Some([-1.0, -4.0, 3.0, 5.0])
        );
        // A vertex of no number: `MS_MIN(0, NaN)` is NaN and `MS_MIN(NaN, 5)` is 5, so the 0
        // before it is forgotten, where `f64::min` would keep it.
        assert_eq!(
            ms_compute_bounds([[0.0, 0.0], [f64::NAN, 1.0], [5.0, 2.0], [6.0, 3.0]]),
            Some([5.0, 0.0, 6.0, 3.0])
        );
        // The last one leaves its bounds no number.
        let [min_x, min_y, max_x, max_y] =
            ms_compute_bounds([[0.0, 0.0], [3.0, f64::NAN]]).unwrap();
        assert_eq!([min_x, max_x], [0.0, 3.0]);
        assert!(min_y.is_nan() && max_y.is_nan());
    }

    #[test]
    fn ms_string_split_parts_a_run_once_and_keeps_empty_ends() {
        // Verifies: ADR-0036
        fn split(text: &str) -> Vec<&[u8]> {
            ms_string_split(text.as_bytes(), b',')
        }
        assert_eq!(split("a,b"), [b"a".as_slice(), b"b"]);
        assert_eq!(split("a,,b"), [b"a".as_slice(), b"b"]);
        assert_eq!(split(",a,"), [b"".as_slice(), b"a", b""]);
        assert_eq!(split(""), [b"".as_slice()]);
        assert_eq!(split(",,"), [b"".as_slice(), b""]);
        assert_eq!(split("a\0,b"), [b"a".as_slice()]);
    }

    #[test]
    fn a_c_string_ends_at_its_first_nul() {
        // Verifies: ADR-0036
        assert_eq!(c_str("ab\0cd"), "ab");
        assert_eq!(c_str("ab"), "ab");
        assert_eq!(c_str("\0"), "");
        assert_eq!(c_bytes(b"ab\0cd"), b"ab");
        assert_eq!(c_bytes(b""), b"");
    }

    #[test]
    fn ms_get_symbol_index_finds_the_first_name_that_matches_from_index_1() {
        // Verifies: ADR-0036
        // src/mapfile.c:416-420: from symbol 1, the first whose name strcasecmp finds equal.
        let symbols = [
            Symbol {
                name: None,
                ..symbol_named("")
            },
            symbol_named("Pin"),
            symbol_named("pin"),
            symbol_named("Straße"),
        ];
        assert_eq!(ms_get_symbol_index("pin", &symbols), Some(2));
        assert_eq!(ms_get_symbol_index("PIN", &symbols), Some(2));
        assert_eq!(ms_get_symbol_index("STRAßE", &symbols), Some(4));
        // Only ASCII letters fold in the C locale; a symbol without a name matches nothing.
        assert_eq!(ms_get_symbol_index("STRASSE", &symbols), None);
        assert_eq!(ms_get_symbol_index("", &symbols), None);
        assert_eq!(ms_get_symbol_index("pin", &[]), None);
    }

    #[test]
    fn two_names_have_one_key_exactly_where_they_match() {
        // Verifies: ADR-0036
        let names = [
            "Pin", "pin", "PIN", "pín", "PÍN", "p_n", "P_N", "", "[", "{",
        ];
        for a in names {
            for b in names {
                let matched = ms_get_symbol_index(a, &[symbol_named(b)]).is_some();
                assert_eq!(
                    symbol_name_key(a) == symbol_name_key(b),
                    matched,
                    "{a:?} {b:?}"
                );
            }
        }
    }

    fn symbol_named(name: &str) -> Symbol {
        Symbol {
            name: Some(name.to_owned()),
            kind: crate::SymbolKind::Ellipse,
            paths: Vec::new(),
            extent: None,
            filled: false,
            anchor_point: None,
            character: None,
            font: None,
            image: None,
            transparent: None,
            span: None,
        }
    }
}
