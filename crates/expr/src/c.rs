// The C functions MapServer's expressions call, as glibc runs them in the C locale: strtod behind
// atof, the printf conversions msToString admits, and toupper and tolower; and msToString,
// msCommifyString, msStringInitCap, and msStringFirstCap of src/mapstring.cpp, which src/mapparser.y
// calls, of the release pinned in scripts/fetch-msautotest.sh.
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

/// C's `isspace` in the C locale.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// C's `atof`, glibc's `strtod` in the C locale: after leading white space and a sign, the
/// longest prefix that reads as a decimal number, a hexadecimal one (`0x1.8p3`), `inf`,
/// `infinity`, or `nan`, each without regard to case; 0 where none does. Correctly rounded, as
/// glibc rounds, a number past the range of a double reading as an infinity.
pub fn atof(text: &[u8]) -> f64 {
    let mut i = text.iter().take_while(|&&b| is_space(b)).count();
    let negative = text.get(i) == Some(&b'-');
    if matches!(text.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let rest = &text[i..];
    let magnitude = if starts_with_ignore_case(rest, b"inf") {
        f64::INFINITY
    } else if starts_with_ignore_case(rest, b"nan") {
        f64::NAN
    } else if (starts_with_ignore_case(rest, b"0x"))
        && hex_number(&rest[2..]).is_some_and(|(_, len)| len > 0)
    {
        hex_number(&rest[2..]).map_or(0.0, |(value, _)| value)
    } else {
        decimal(rest)
    };
    if negative { -magnitude } else { magnitude }
}

fn starts_with_ignore_case(text: &[u8], prefix: &[u8]) -> bool {
    text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// The longest decimal number at the start of `text`, or 0.
fn decimal(text: &[u8]) -> f64 {
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
        return 0.0;
    }
    if matches!(text.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(text.get(end + 1), Some(b'+' | b'-')));
        let exp_end = digits(end + 1 + sign);
        if exp_end > end + 1 + sign {
            end = exp_end;
        }
    }
    // ASCII digits, a point, and an exponent: Rust reads them as strtod does, correctly rounded.
    std::str::from_utf8(&text[..end])
        .ok()
        .and_then(|number| number.parse().ok())
        .unwrap_or(0.0)
}

/// A hexadecimal number after its `0x`: hex digits with an optional point, and an optional binary
/// exponent `p±n`; with the count of digits read, 0 where there are none.
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
        return Some((0.0, 0));
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
        }
    }
    Some((scaled(mantissa, sticky, exponent), count))
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

/// msToString: `value` formatted by the C `printf` format `format`, which may hold one conversion
/// `%e`, `%E`, `%f`, `%F`, `%g`, or `%G` with the flags `+ - space ' 0`, a width up to 1024,
/// and a precision, and `%%` for a percent sign; `None` for any other, as msToString refuses it.
/// The result is cut to the buffer msToString sizes for it, as `snprintf` cuts it.
pub fn to_string(format: &[u8], value: f64) -> Option<Vec<u8>> {
    let mut conversion = None;
    let mut extra_size: usize = 0;
    let mut i = 0;
    while i < format.len() {
        if format[i] == b'%' && format.get(i + 1) == Some(&b'%') {
            i += 2;
            continue;
        }
        if format[i] != b'%' {
            i += 1;
            continue;
        }
        if conversion.is_some() {
            return None;
        }
        let start = i;
        i += 1;
        while matches!(format.get(i), Some(b'+' | b'-' | b' ' | b'\'' | b'0')) {
            i += 1;
        }
        if matches!(format.get(i), Some(b'1'..=b'9')) {
            let width_start = i;
            while format.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            let width = atoi(&format[width_start..i]);
            if width > 1024 {
                return None;
            }
            extra_size = usize::try_from(width).unwrap_or(0);
        }
        // DBL_MAX_10_EXP, and a margin.
        extra_size = extra_size.max(308) + 32;
        if format.get(i) == Some(&b'.') {
            i += 1;
            while format.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        if !matches!(format.get(i), Some(b'e' | b'E' | b'f' | b'F' | b'g' | b'G')) {
            return None;
        }
        conversion = Some((start, i));
        i += 1;
    }
    let size = format.len() + extra_size + extra_size / 3 + 1;
    let mut out = Vec::new();
    let mut i = 0;
    while i < format.len() && out.len() < size {
        match conversion {
            Some((start, end)) if i == start => {
                out.extend(convert(&format[start + 1..=end], value, size)?);
                i = end + 1;
            }
            _ if format[i] == b'%' => {
                out.push(b'%');
                i += 2;
            }
            _ => {
                out.push(format[i]);
                i += 1;
            }
        }
    }
    out.truncate(size - 1);
    Some(out)
}

/// The digits of a double past which a decimal expansion is exact: no conversion rounds there.
const EXACT_DIGITS: usize = 1100;

/// One conversion, `spec` after its `%`: flags, width, precision, and the letter msToString
/// checked. `limit` bounds the length worth producing.
fn convert(spec: &[u8], value: f64, limit: usize) -> Option<Vec<u8>> {
    let mut i = 0;
    let (mut left, mut plus, mut space, mut zero) = (false, false, false, false);
    while let Some(&flag) = spec.get(i) {
        match flag {
            b'-' => left = true,
            b'+' => plus = true,
            b' ' => space = true,
            b'0' => zero = true,
            b'\'' => {}
            _ => break,
        }
        i += 1;
    }
    let mut width = 0usize;
    while let Some(d) = spec.get(i).filter(|b| b.is_ascii_digit()) {
        width = width * 10 + usize::from(d - b'0');
        i += 1;
    }
    let mut precision = None;
    if spec.get(i) == Some(&b'.') {
        i += 1;
        let mut p: u64 = 0;
        while let Some(d) = spec.get(i).filter(|b| b.is_ascii_digit()) {
            p = p * 10 + u64::from(d - b'0');
            // printf refuses a precision past an int.
            if p > i32::MAX as u64 {
                return None;
            }
            i += 1;
        }
        precision = Some(p as usize);
    }
    let letter = spec[i];
    let upper = letter.is_ascii_uppercase();
    let sign: &[u8] = if value.is_sign_negative() {
        b"-"
    } else if plus {
        b"+"
    } else if space {
        b" "
    } else {
        b""
    };
    let magnitude = value.abs();
    let body = if !value.is_finite() {
        // printf pads an infinity or a NaN with spaces, whatever the flag 0 asks.
        zero = false;
        let text: &[u8] = if value.is_nan() { b"nan" } else { b"inf" };
        if upper {
            text.to_ascii_uppercase()
        } else {
            text.to_vec()
        }
    } else {
        let precision = precision.unwrap_or(6);
        let digits = match letter.to_ascii_lowercase() {
            b'e' => exponential(magnitude, precision, limit),
            b'f' => fixed(magnitude, precision, limit),
            _ => general(magnitude, precision),
        };
        if upper {
            digits.to_ascii_uppercase()
        } else {
            digits
        }
    };
    let length = sign.len() + body.len();
    let pad = width.saturating_sub(length);
    let mut out = Vec::with_capacity(length + pad);
    if left {
        out.extend_from_slice(sign);
        out.extend(body);
        out.resize(out.len() + pad, b' ');
    } else if zero {
        out.extend_from_slice(sign);
        out.resize(out.len() + pad, b'0');
        out.extend(body);
    } else {
        out.resize(pad, b' ');
        out.extend_from_slice(sign);
        out.extend(body);
    }
    Some(out)
}

/// `%f` of a finite, non-negative value. Past the digits that can round, the expansion is exact
/// and continues in zeros, up to `limit`.
fn fixed(value: f64, precision: usize, limit: usize) -> Vec<u8> {
    let exact = precision.min(EXACT_DIGITS);
    let mut out = format!("{value:.exact$}").into_bytes();
    if precision > exact {
        out.resize(out.len() + (precision - exact).min(limit), b'0');
    }
    out
}

/// `%e` of a finite, non-negative value, its exponent signed and of at least two digits.
fn exponential(value: f64, precision: usize, limit: usize) -> Vec<u8> {
    let exact = precision.min(EXACT_DIGITS);
    let (mantissa, exponent) = split_exponential(value, exact);
    let mut out = mantissa.into_bytes();
    if precision > exact {
        out.resize(out.len() + (precision - exact).min(limit), b'0');
    }
    out.extend(exponent_suffix(exponent));
    out
}

/// Rust's `{:.p$e}` of `value`, split into its mantissa and its exponent.
fn split_exponential(value: f64, precision: usize) -> (String, i32) {
    let text = format!("{value:.precision$e}");
    let (mantissa, exponent) = text.split_once('e').expect("an exponent");
    (mantissa.to_owned(), exponent.parse().expect("an integer"))
}

fn exponent_suffix(exponent: i32) -> Vec<u8> {
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("e{sign}{:02}", exponent.unsigned_abs()).into_bytes()
}

/// `%g` of a finite, non-negative value: `%e` or `%f` by the exponent at `precision` significant
/// digits, without trailing zeros, as C defines it without the flag `#`.
fn general(value: f64, precision: usize) -> Vec<u8> {
    let significant = precision.clamp(1, EXACT_DIGITS);
    let (_, exponent) = split_exponential(value, significant - 1);
    let exponent = i64::from(exponent);
    if (significant as i64) > exponent && exponent >= -4 {
        let decimals = (significant as i64 - 1 - exponent) as usize;
        let mut out = format!("{value:.decimals$}").into_bytes();
        trim_fraction(&mut out);
        out
    } else {
        let (mantissa, exponent) = split_exponential(value, significant - 1);
        let mut out = mantissa.into_bytes();
        trim_fraction(&mut out);
        out.extend(exponent_suffix(exponent));
        out
    }
}

/// Drops the trailing zeros of a fraction, and its point where nothing is left after it.
fn trim_fraction(text: &mut Vec<u8>) {
    if text.contains(&b'.') {
        while text.last() == Some(&b'0') {
            text.pop();
        }
        if text.last() == Some(&b'.') {
            text.pop();
        }
    }
}

/// msCommifyString: a comma before every third digit of the part before the decimal point,
/// counted from the point, as the C loop places them, a sign counted as a digit; unchanged with
/// more than one point, or with nothing before it.
pub fn commify(text: &[u8]) -> Vec<u8> {
    let points = text.iter().filter(|&&b| b == b'.').count();
    if points > 1 {
        return text.to_vec();
    }
    let int_len = text.iter().position(|&b| b == b'.').unwrap_or(text.len());
    // (int)strlen - 1 in C; with nothing before the point, the unsigned subtraction wraps and
    // the count is no positive int.
    if int_len == 0 {
        return text.to_vec();
    }
    let mut commas = (int_len - 1) / 3;
    if commas < 1 {
        return text.to_vec();
    }
    let old: Vec<u8> = text.to_vec();
    let new_length = old.len() + commas;
    let mut out = old.clone();
    out.resize(new_length, 0);
    let mut add_commas = points == 0;
    let mut j = 0;
    let mut i = new_length;
    while i > 0 {
        i -= 1;
        if points == 1 && !add_commas {
            out[i] = old[i - commas];
            if out[i] == b'.' {
                add_commas = true;
            }
        } else if add_commas && j > 2 {
            out[i] = b',';
            commas -= 1;
            j = 0;
        } else {
            out[i] = old[i - commas];
            j += 1;
        }
        if commas == 0 {
            break;
        }
    }
    out
}

/// msStringInitCap: the first character of each word upper case, the rest lower, words parted
/// by spaces, in ASCII.
pub fn initcap(text: &mut [u8]) {
    let mut start = true;
    for b in text {
        if *b == b' ' {
            start = true;
        } else if start {
            b.make_ascii_uppercase();
            start = false;
        } else {
            b.make_ascii_lowercase();
        }
    }
}

/// msStringFirstCap: the first character that is no space upper case, every other lower, in
/// ASCII.
pub fn firstcap(text: &mut [u8]) {
    let mut start = true;
    for b in text.iter_mut().filter(|b| **b != b' ') {
        if start {
            b.make_ascii_uppercase();
            start = false;
        } else {
            b.make_ascii_lowercase();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(format: &str, value: f64) -> String {
        String::from_utf8(to_string(format.as_bytes(), value).unwrap()).unwrap()
    }

    #[test]
    fn atof_reads_the_longest_number_strtod_reads() {
        // Verifies: ADR-0031
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
    fn atoi_reads_an_int_as_glibc_does() {
        assert_eq!(atoi(b" 42x"), 42);
        assert_eq!(atoi(b"-7"), -7);
        assert_eq!(atoi(b"\t255"), 255);
        assert_eq!(atoi(b"x"), 0);
        assert_eq!(atoi(b"4294967297"), 1);
        assert_eq!(atoi(b"99999999999999999999"), -1);
    }

    #[test]
    fn to_string_formats_as_printf() {
        // Verifies: ADR-0031
        assert_eq!(s("%g", 0.0001), "0.0001");
        assert_eq!(s("%g", 0.00001), "1e-05");
        assert_eq!(s("%g", 123456.0), "123456");
        assert_eq!(s("%g", 1234567.0), "1.23457e+06");
        assert_eq!(s("%g", 100.0), "100");
        assert_eq!(s("%g", 2.5), "2.5");
        assert_eq!(s("%g", -0.0), "-0");
        assert_eq!(s("%.0f", 2.5), "2");
        assert_eq!(s("%.0f", 3.5), "4");
        assert_eq!(s("%.2f", 1.005), "1.00");
        assert_eq!(s("%.1f", 0.25), "0.2");
    }

    #[test]
    fn to_string_applies_flags_width_and_exponents_as_printf() {
        // Verifies: ADR-0031
        assert_eq!(s("%e", 1234.5), "1.234500e+03");
        assert_eq!(s("%E", 1e-300), "1.000000E-300");
        assert_eq!(s("%+08.2f", 1.23456), "+0001.23");
        assert_eq!(s("%-8.1f|", 2.0), "2.0     |");
        assert_eq!(s("% g", 5.0), " 5");
        assert_eq!(s("%'.1f%%", 1.0), "1.0%");
        assert_eq!(s("%05f", f64::INFINITY), "  inf");
        assert_eq!(s("%G", f64::NEG_INFINITY), "-INF");
        assert_eq!(s("%f", f64::NAN), "nan");
        assert_eq!(s("%.3g", 0.0), "0");
        assert_eq!(s("%g", 1e100), "1e+100");
        assert_eq!(s("value: %.1f km", 12.34), "value: 12.3 km");
        assert_eq!(s("100%%", 1.0), "100%");
        assert_eq!(s("no conversion", 1.0), "no conversion");
    }

    #[test]
    fn to_string_refuses_what_ms_to_string_refuses() {
        // Verifies: ADR-0031
        for format in [
            "%d",
            "%s",
            "%f %f",
            "%",
            "%lf",
            "%*f",
            "%1025f",
            "%.99999999999f",
        ] {
            assert_eq!(to_string(format.as_bytes(), 1.0), None, "{format}");
        }
        assert!(to_string(b"%1024f", 1.0).is_some());
    }

    #[test]
    fn to_string_is_cut_to_its_buffer_and_bounded() {
        // A precision far past the buffer: the exact digits, then zeros, cut to the buffer.
        let out = to_string(b"%.100000f", 0.5).unwrap();
        assert_eq!(out.len(), 9 + 340 + 113);
        assert!(out.starts_with(b"0.5000"));
        let out = to_string(b"%.5000e", 1.0).unwrap();
        assert_eq!(out.len(), 7 + 340 + 113);
        assert!(!out.ends_with(b"e+00"), "the exponent lies past the buffer");
    }

    #[test]
    fn commify_places_commas_as_ms_commify_string() {
        // Verifies: ADR-0031
        let c = |t: &str| String::from_utf8(commify(t.as_bytes())).unwrap();
        assert_eq!(c("1234567"), "1,234,567");
        assert_eq!(c("123"), "123");
        assert_eq!(c("1234.5678"), "1,234.5678");
        assert_eq!(c("-123456"), "-,123,456");
        assert_eq!(c(".12345"), ".12345");
        assert_eq!(c("1.2.3"), "1.2.3");
        assert_eq!(c(""), "");
        assert_eq!(c("abcd"), "a,bcd");
    }

    #[test]
    fn the_case_functions_change_ascii_byte_by_byte() {
        // Verifies: ADR-0031
        let mut text = b"hELLO wORLD  \xc3\xa9t\xc3\xa9".to_vec();
        initcap(&mut text);
        assert_eq!(text, b"Hello World  \xc3\xa9t\xc3\xa9");
        let mut text = b"  hELLO wORLD".to_vec();
        firstcap(&mut text);
        assert_eq!(text, b"  Hello world");
    }
}
