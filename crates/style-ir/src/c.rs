// C's reading of numbers, ported once for the frontend and the core (ADR-0017): glibc's strtod
// in the C locale and the atof and atoi it backs, its lrint on x86-64, and MapServer's MS_NINT of
// src/mapserver.h, of the release pinned in scripts/fetch-msautotest.sh.
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

//! C's reading of numbers as MapServer's build at the pin runs it: `strtod`, `atof`, and `atoi` of
//! glibc in the C locale, and `MS_NINT`, which is `lrint` on x86-64 (ADR-0017).

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atof_reads_the_longest_number_strtod_reads() {
        // Verifies: ADR-0017
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
        // Verifies: ADR-0017
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
        // Verifies: ADR-0017
        assert_eq!(atoi(b" 42x"), 42);
        assert_eq!(atoi(b"-7"), -7);
        assert_eq!(atoi(b"\t255"), 255);
        assert_eq!(atoi(b"x"), 0);
        assert_eq!(atoi(b"4294967297"), 1);
        assert_eq!(atoi(b"99999999999999999999"), -1);
    }

    #[test]
    fn lrint_rounds_half_to_even_and_gives_long_min_past_a_long() {
        // Verifies: ADR-0017
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
        // Verifies: ADR-0017
        assert_eq!(ms_nint(2.5), 2);
        assert_eq!(ms_nint(-2_147_483_648.5), i32::MIN);
        assert_eq!(ms_nint(2_147_483_648.0), i32::MIN);
        assert_eq!(ms_nint(3e9), -1_294_967_296);
        assert_eq!(ms_nint(-3_302_166_604.8), 992_800_691);
        assert_eq!(ms_nint(4_294_967_297.0), 1);
        assert_eq!(ms_nint(1e19), 0);
        assert_eq!(ms_nint(f64::NAN), 0);
    }
}
