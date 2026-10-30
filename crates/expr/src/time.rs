// Times as MapServer's expressions read and compare them: msParseTime and msTimeCompare of
// src/maptime.c, with glibc's strptime for the formats msParseTime hands it, of the release
// pinned in scripts/fetch-msautotest.sh.
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

/// The fields of a `struct tm` msTimeCompare compares, in its order and as `struct tm` holds them:
/// the year less 1900, the month less 1, the day, hour, minute, and second. A field strptime did
/// not reach stays 0, as msStrptime clears the struct first: a month not read is January.
pub type Time = [i32; 6];

/// The formats of msParseTime, in its order: a pattern its regular expression matches at the
/// start of the text, `9` standing for a digit, and the strptime format it reads it with.
const FORMATS: &[(&str, &str)] = &[
    ("99999999", "%Y%m%d"),
    ("9999-99-99T99:99:99Z", "%Y-%m-%dT%H:%M:%SZ"),
    ("9999-99-99T99:99:99", "%Y-%m-%dT%H:%M:%S"),
    ("9999-99-99 99:99:99", "%Y-%m-%d %H:%M:%S"),
    ("9999-99-99T99:99", "%Y-%m-%dT%H:%M"),
    ("9999-99-99 99:99", "%Y-%m-%d %H:%M"),
    ("9999-99-99T99", "%Y-%m-%dT%H"),
    ("9999-99-99 99", "%Y-%m-%d %H"),
    ("9999-99-99", "%Y-%m-%d"),
    ("9999-99", "%Y-%m"),
    ("9999", "%Y"),
    ("T99:99:99Z", "T%H:%M:%SZ"),
    ("T99:99:99", "T%H:%M:%S"),
    ("99:99:99Z", "%H:%M:%SZ"),
    ("99:99:99", "%H:%M:%S"),
];

/// msParseTime: the time of the first format whose pattern matches the start of `text`, as
/// strptime reads it, or `None` where none matches or the text is empty. strptime stops at the
/// first field it cannot read, and msParseTime takes what it read so far.
pub fn parse(text: &[u8]) -> Option<Time> {
    let (_, format) = FORMATS.iter().find(|(pattern, _)| {
        text.len() >= pattern.len()
            && pattern.bytes().zip(text).all(|(p, &t)| match p {
                b'9' => t.is_ascii_digit(),
                p => p == t,
            })
    })?;
    let mut time = [0; 6];
    strptime(text, format.as_bytes(), &mut time);
    Some(time)
}

/// glibc's strptime for the conversions of [`FORMATS`]: each field written as it is read, and
/// the reading stopped at the first that fails.
fn strptime(text: &[u8], format: &[u8], time: &mut Time) {
    let mut at = 0;
    let mut f = 0;
    while f < format.len() {
        if format[f] != b'%' {
            if text.get(at) != Some(&format[f]) {
                return;
            }
            at += 1;
            f += 1;
            continue;
        }
        // The field, its range, its digits, and what struct tm holds less.
        let (field, from, to, digits, base) = match format[f + 1] {
            b'Y' => (0, 0, 9999, 4, 1900),
            b'm' => (1, 1, 12, 2, 1),
            b'd' => (2, 1, 31, 2, 0),
            b'H' => (3, 0, 23, 2, 0),
            b'M' => (4, 0, 59, 2, 0),
            _ => (5, 0, 61, 2, 0),
        };
        let Some(value) = number(text, &mut at, from, to, digits) else {
            return;
        };
        time[field] = value - base;
        f += 2;
    }
}

/// glibc's `get_number`: white space, then digits while fewer than `digits` are read and one
/// more could stay within `to`; `None` past the range or without a digit.
fn number(text: &[u8], at: &mut usize, from: i32, to: i32, digits: u32) -> Option<i32> {
    while text
        .get(*at)
        .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
    {
        *at += 1;
    }
    if !text.get(*at).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let mut value = 0;
    let mut left = digits;
    loop {
        value = value * 10 + i32::from(text[*at] - b'0');
        *at += 1;
        left -= 1;
        if !(left > 0 && value * 10 <= to && text.get(*at).is_some_and(u8::is_ascii_digit)) {
            break;
        }
    }
    (from..=to).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_ms_parse_time_reads_them() {
        // Verifies: ADR-0031
        assert_eq!(parse(b"2010-12-01"), Some([110, 11, 1, 0, 0, 0]));
        assert_eq!(
            parse(b"2010-12-01T10:20:30Z"),
            Some([110, 11, 1, 10, 20, 30])
        );
        assert_eq!(parse(b"20101201"), Some([110, 11, 1, 0, 0, 0]));
        assert_eq!(parse(b"2010-12"), Some([110, 11, 0, 0, 0, 0]));
        assert_eq!(parse(b"10:20:30"), Some([0, 0, 0, 10, 20, 30]));
        assert_eq!(parse(b"1999 and more"), Some([99, 0, 0, 0, 0, 0]));
        // strptime stops at a month past 12 and keeps the year; the month left is January.
        assert_eq!(parse(b"2010-13-01"), Some([110, 0, 0, 0, 0, 0]));
        assert_eq!(parse(b"2010-13"), parse(b"2010-01"));
        assert_eq!(parse(b"2010"), parse(b"2010-01"));
        // A day of 45 reads its first digit, as a second would pass 31.
        assert_eq!(parse(b"2010-12-45"), Some([110, 11, 4, 0, 0, 0]));
        assert_eq!(parse(b""), None);
        assert_eq!(parse(b"yesterday"), None);
        assert_eq!(parse(b"201"), None);
    }
}
