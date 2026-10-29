//! The patterns of `regexp_like`, read in PCRE2's syntax in byte mode, as MapServer's ms_regcomp
//! reads them through PCRE2's POSIX interface at the pin (ADR-0030), and matched by the `regex`
//! crate's byte matcher with its Unicode features off (ADR-0031).
//!
//! A pattern is read once into the `regex` crate's syntax, every literal byte written as `\xHH`
//! so that the crate cannot read it otherwise. What PCRE2 reads and the crate cannot match as
//! PCRE2 does is *unsupported*: the expression that holds it does not match, and the engine
//! reports it. What PCRE2 refuses is *refused*: the term is false, as `mapparser.y` sets it.

use regex::bytes::{Regex, RegexBuilder};

/// The largest compiled pattern, in bytes (Q-9).
pub const SIZE_LIMIT: usize = 1 << 20;

/// PCRE2's largest number in a `{n,m}` quantifier.
const MAX_REPEAT: u32 = 65_535;

/// PCRE2's longest group name.
const MAX_NAME: usize = 32;

/// A pattern compiled for matching.
#[derive(Clone, Debug)]
pub enum Compiled {
    Matcher(Regex),
    /// PCRE2 refuses it: the term is false.
    Refused,
    /// PCRE2 reads it, and the crate cannot match it as PCRE2 does: the construct.
    Unsupported(&'static str),
}

/// Compiles `pattern`, ASCII case-insensitive where `caseless`.
pub fn compile(pattern: &[u8], caseless: bool) -> Compiled {
    let translated = match translate(pattern) {
        Ok(translated) => translated,
        Err(read) => return read,
    };
    match RegexBuilder::new(&translated)
        .unicode(false)
        .case_insensitive(caseless)
        .size_limit(SIZE_LIMIT)
        .build()
    {
        Ok(regex) => Compiled::Matcher(regex),
        Err(regex::Error::CompiledTooBig(_)) => {
            Compiled::Unsupported("a pattern past the size limit of 1 MiB")
        }
        Err(_) => Compiled::Refused,
    }
}

fn unsupported<T>(what: &'static str) -> Result<T, Compiled> {
    Err(Compiled::Unsupported(what))
}

fn refused<T>() -> Result<T, Compiled> {
    Err(Compiled::Refused)
}

fn hex(out: &mut String, byte: u8) {
    out.push_str(&format!("\\x{byte:02X}"));
}

/// An escape `\c` PCRE2 reads as one byte, outside a class or in one, where `in_class` gives
/// `\b` its meaning there, the backspace.
fn char_escape(c: u8, in_class: bool) -> Option<u8> {
    match c {
        b'a' => Some(0x07),
        b'f' => Some(0x0c),
        b'n' => Some(b'\n'),
        b'r' => Some(b'\r'),
        b't' => Some(b'\t'),
        b'b' if in_class => Some(0x08),
        c if !c.is_ascii_alphanumeric() => Some(c),
        _ => None,
    }
}

/// What an escape of PCRE2 that is no single byte is, where the crate reads it otherwise or not
/// at all; `None` for a letter PCRE2 refuses.
fn unsupported_escape(c: u8) -> Option<&'static str> {
    Some(match c {
        b'1'..=b'9' => "a back-reference",
        b'0' | b'o' => "an octal escape",
        b'g' | b'k' => "a back-reference or a subroutine call",
        b'e' => "the escape \\e",
        b'c' => "a control escape \\cX",
        b'h' | b'H' => "the class \\h",
        b'v' | b'V' => "the class \\v",
        b'R' => "the escape \\R",
        b'N' => "the escape \\N",
        b'X' => "the escape \\X",
        b'C' => "the escape \\C",
        b'p' | b'P' => "a Unicode property \\p",
        b'Z' => "the assertion \\Z",
        b'G' => "the assertion \\G",
        b'K' => "the escape \\K",
        b'Q' | b'E' => "a quotation \\Q…\\E",
        _ => return None,
    })
}

/// `\x` after its letter at `at`: `\x{h…}` or up to two hex digits, the byte and the index past
/// it.
fn hex_escape(p: &[u8], at: usize) -> Result<(u8, usize), Compiled> {
    if p.get(at) == Some(&b'{') {
        let close = p[at..]
            .iter()
            .position(|&b| b == b'}')
            .map(|i| at + i)
            .ok_or(Compiled::Refused)?;
        let digits = &p[at + 1..close];
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_hexdigit) {
            return refused();
        }
        let digits = std::str::from_utf8(digits).expect("hex digits");
        // Past 0xFF in byte mode, PCRE2 refuses the code point.
        let value = u32::from_str_radix(digits.trim_start_matches('0'), 16).unwrap_or(256);
        let value = if digits.trim_start_matches('0').is_empty() {
            0
        } else {
            value
        };
        return u8::try_from(value)
            .map(|b| (b, close + 1))
            .map_err(|_| Compiled::Refused);
    }
    let mut value = 0u8;
    let mut end = at;
    while end < at + 2 && p.get(end).is_some_and(u8::is_ascii_hexdigit) {
        value = value * 16 + (p[end] as char).to_digit(16).expect("hex") as u8;
        end += 1;
    }
    Ok((value, end))
}

/// The pattern in the `regex` crate's syntax, or why it is not read.
pub fn translate(p: &[u8]) -> Result<String, Compiled> {
    let mut out = String::with_capacity(p.len() * 2);
    let mut i = 0;
    // Whether what precedes is an item a quantifier repeats: PCRE2 refuses one at the start, after
    // `(` or `|`, after an assertion, and after another quantifier.
    let mut repeatable = false;
    while i < p.len() {
        let b = p[i];
        let quantifier = matches!(b, b'*' | b'+' | b'?') || (b == b'{' && counted(p, i)?.is_some());
        if quantifier && !repeatable {
            return refused();
        }
        repeatable = !quantifier
            && !matches!(b, b'(' | b'|' | b'^' | b'$')
            && !(b == b'\\' && matches!(p.get(i + 1), Some(b'b' | b'B' | b'A' | b'z')));
        match b {
            b'\\' => {
                let Some(&c) = p.get(i + 1) else {
                    return refused();
                };
                i += 2;
                match c {
                    b'd' | b'D' | b'w' | b'W' | b's' | b'S' | b'b' | b'B' | b'A' | b'z' => {
                        out.push('\\');
                        out.push(c as char);
                    }
                    b'x' => {
                        let (byte, next) = hex_escape(p, i)?;
                        hex(&mut out, byte);
                        i = next;
                    }
                    c => match char_escape(c, false) {
                        Some(byte) => hex(&mut out, byte),
                        None => match unsupported_escape(c) {
                            Some(what) => return unsupported(what),
                            None => return refused(),
                        },
                    },
                }
            }
            b'[' => i = class(p, i + 1, &mut out)?,
            b'(' => i = group(p, i, &mut out)?,
            b'*' | b'+' | b'?' => {
                out.push(b as char);
                i = quantifier_suffix(p, i + 1, &mut out)?;
            }
            b'{' => match counted(p, i)? {
                Some((text, next)) => {
                    out.push_str(&text);
                    i = quantifier_suffix(p, next, &mut out)?;
                }
                None => {
                    hex(&mut out, b);
                    i += 1;
                }
            },
            b')' | b'|' | b'.' | b'^' | b'$' => {
                out.push(b as char);
                i += 1;
            }
            b if b.is_ascii_alphanumeric() => {
                out.push(b as char);
                i += 1;
            }
            _ => {
                hex(&mut out, b);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// After a quantifier: `?` makes it lazy in both; `+` makes it possessive in PCRE2 only.
fn quantifier_suffix(p: &[u8], i: usize, out: &mut String) -> Result<usize, Compiled> {
    match p.get(i) {
        Some(b'+') => unsupported("a possessive quantifier"),
        Some(b'?') => {
            out.push('?');
            Ok(i + 1)
        }
        _ => Ok(i),
    }
}

/// A counted quantifier at `i`, as PCRE2 10.42 reads one: `{n}`, `{n,}`, or `{n,m}`; `None`
/// where the brace is a literal one, as PCRE2 takes any other.
fn counted(p: &[u8], i: usize) -> Result<Option<(String, usize)>, Compiled> {
    let digits = |from: usize| {
        from + p[from.min(p.len())..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let n_end = digits(i + 1);
    if n_end == i + 1 {
        return Ok(None);
    }
    let number = |from: usize, to: usize| {
        std::str::from_utf8(&p[from..to])
            .expect("digits")
            .parse::<u32>()
            .unwrap_or(u32::MAX)
    };
    let n = number(i + 1, n_end);
    let (m, end) = match p.get(n_end) {
        Some(b'}') => (Some(n), n_end + 1),
        Some(b',') => {
            let m_end = digits(n_end + 1);
            if p.get(m_end) != Some(&b'}') {
                return Ok(None);
            }
            let m = (m_end > n_end + 1).then(|| number(n_end + 1, m_end));
            (m, m_end + 1)
        }
        _ => return Ok(None),
    };
    if n > MAX_REPEAT || m.is_some_and(|m| m > MAX_REPEAT || m < n) {
        return refused();
    }
    let text = match (m, p[n_end]) {
        (_, b'}') => format!("{{{n}}}"),
        (Some(m), _) => format!("{{{n},{m}}}"),
        (None, _) => format!("{{{n},}}"),
    };
    Ok(Some((text, end)))
}

/// A group opened at `i`, its opening written; the index past the opening.
fn group(p: &[u8], i: usize, out: &mut String) -> Result<usize, Compiled> {
    match p.get(i + 1) {
        Some(b'*') => return unsupported("a verb (*…)"),
        Some(b'?') => {}
        _ => {
            out.push('(');
            return Ok(i + 1);
        }
    }
    let at = i + 2;
    match p.get(at) {
        Some(b':') => {
            out.push_str("(?:");
            Ok(at + 1)
        }
        Some(b'=' | b'!') => unsupported("a look-ahead assertion"),
        Some(b'<') if matches!(p.get(at + 1), Some(b'=' | b'!')) => {
            unsupported("a look-behind assertion")
        }
        Some(b'<') => named(p, at + 1, b'>', out),
        Some(b'P') => match p.get(at + 1) {
            Some(b'<') => named(p, at + 2, b'>', out),
            Some(b'=') => unsupported("a back-reference"),
            Some(b'>') => unsupported("a subroutine call"),
            _ => refused(),
        },
        Some(b'\'') => unsupported("a group named with quotes"),
        Some(b'>') => unsupported("an atomic group"),
        Some(b'|') => unsupported("a branch reset group"),
        Some(b'#') => unsupported("a comment (?#…)"),
        Some(b'(') => unsupported("a conditional group"),
        Some(b'C') => unsupported("a callout"),
        Some(b'R' | b'&' | b'+' | b'0'..=b'9') => unsupported("recursion or a subroutine call"),
        Some(b'-') if p.get(at + 1).is_some_and(u8::is_ascii_digit) => {
            unsupported("recursion or a subroutine call")
        }
        _ => flags(p, at, out),
    }
}

/// A group's name up to `close`, written as a named group.
fn named(p: &[u8], from: usize, close: u8, out: &mut String) -> Result<usize, Compiled> {
    let end = p[from..]
        .iter()
        .position(|&b| b == close)
        .map(|n| from + n)
        .ok_or(Compiled::Refused)?;
    let name = &p[from..end];
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME
        && !name[0].is_ascii_digit()
        && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_');
    if !valid {
        return refused();
    }
    out.push_str("(?P<");
    out.push_str(std::str::from_utf8(name).expect("ASCII"));
    out.push('>');
    Ok(end + 1)
}

/// An option setting `(?imsU-imsU)` or `(?imsU-imsU:`, at the letters after `(?`.
fn flags(p: &[u8], from: usize, out: &mut String) -> Result<usize, Compiled> {
    let end = p[from..]
        .iter()
        .position(|&b| b == b')' || b == b':')
        .map(|n| from + n)
        .ok_or(Compiled::Refused)?;
    let letters = &p[from..end];
    if letters.is_empty() {
        return unsupported("an empty option setting");
    }
    for &letter in letters {
        match letter {
            b'i' | b'm' | b's' | b'U' | b'-' => {}
            b'x' => return unsupported("the option x"),
            b'n' | b'J' | b'^' => return unsupported("an option the crate does not have"),
            _ => return refused(),
        }
    }
    if letters.iter().filter(|&&b| b == b'-').count() > 1 {
        return refused();
    }
    out.push_str("(?");
    out.push_str(std::str::from_utf8(letters).expect("ASCII"));
    out.push(p[end] as char);
    Ok(end + 1)
}

/// One item of a class.
enum Item {
    Byte(u8),
    /// A class escape such as `\d`, or a POSIX class, as written in the crate's syntax.
    Set(String),
}

/// The POSIX classes PCRE2 and the crate both read.
const POSIX: &[&str] = &[
    "alnum", "alpha", "ascii", "blank", "cntrl", "digit", "graph", "lower", "print", "punct",
    "space", "upper", "word", "xdigit",
];

/// A class item at `i`, and the index past it.
fn class_item(p: &[u8], i: usize) -> Result<(Item, usize), Compiled> {
    match p[i] {
        b'[' if matches!(p.get(i + 1), Some(b':' | b'.' | b'=')) => {
            let kind = p[i + 1];
            let close = p[i + 2..]
                .windows(2)
                .position(|w| w[0] == kind && w[1] == b']')
                .map(|n| i + 2 + n);
            let Some(close) = close else {
                return Ok((Item::Byte(b'['), i + 1));
            };
            if kind != b':' {
                return refused();
            }
            let inner = &p[i + 2..close];
            let name = inner.strip_prefix(b"^").unwrap_or(inner);
            if name == b"<" || name == b">" {
                return unsupported("a word boundary [[:<:]]");
            }
            if !POSIX.iter().any(|n| n.as_bytes() == name) {
                return refused();
            }
            let text = format!("[:{}:]", std::str::from_utf8(inner).expect("ASCII"));
            Ok((Item::Set(text), close + 2))
        }
        b'\\' => {
            let Some(&c) = p.get(i + 1) else {
                return refused();
            };
            match c {
                b'd' | b'D' | b'w' | b'W' | b's' | b'S' => {
                    Ok((Item::Set(format!("\\{}", c as char)), i + 2))
                }
                b'x' => hex_escape(p, i + 2).map(|(b, next)| (Item::Byte(b), next)),
                c => match char_escape(c, true) {
                    Some(byte) => Ok((Item::Byte(byte), i + 2)),
                    None => match unsupported_escape(c) {
                        Some(what) => unsupported(what),
                        None => refused(),
                    },
                },
            }
        }
        b => Ok((Item::Byte(b), i + 1)),
    }
}

/// A class whose `[` lies before `i`, written; the index past its `]`.
fn class(p: &[u8], mut i: usize, out: &mut String) -> Result<usize, Compiled> {
    out.push('[');
    if p.get(i) == Some(&b'^') {
        out.push('^');
        i += 1;
    }
    let mut first = true;
    loop {
        let Some(&b) = p.get(i) else {
            return refused();
        };
        if b == b']' && !first {
            out.push(']');
            return Ok(i + 1);
        }
        first = false;
        let (item, next) = class_item(p, i)?;
        i = next;
        let ranged = p.get(i) == Some(&b'-') && p.get(i + 1).is_some_and(|&b| b != b']');
        match item {
            Item::Byte(low) if ranged => {
                let (high, next) = class_item(p, i + 1)?;
                let Item::Byte(high) = high else {
                    return unsupported("a range that ends in a class");
                };
                if high < low {
                    return refused();
                }
                hex(out, low);
                out.push('-');
                hex(out, high);
                i = next;
            }
            Item::Set(_) if ranged => return unsupported("a range that starts with a class"),
            Item::Byte(byte) => hex(out, byte),
            Item::Set(text) => out.push_str(&text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(pattern: &str) -> String {
        translate(pattern.as_bytes()).unwrap_or_else(|e| panic!("{pattern}: {e:?}"))
    }

    fn unsupported_in(pattern: &str) -> bool {
        matches!(translate(pattern.as_bytes()), Err(Compiled::Unsupported(_)))
    }

    fn refused_in(pattern: &str) -> bool {
        matches!(compile(pattern.as_bytes(), false), Compiled::Refused)
    }

    #[test]
    fn literals_are_written_as_bytes() {
        assert_eq!(reads("a.b"), "a.b");
        assert_eq!(reads("a-b"), r"a\x2Db");
        assert_eq!(reads("é"), r"\xC3\xA9");
        assert_eq!(reads(r"\é"), r"\xC3\xA9");
        assert_eq!(reads(r"\x7\x{41}\t"), r"\x07\x41\x09");
        assert_eq!(reads("[]a-c]"), r"[\x5D\x61-\x63]");
        assert_eq!(reads("[^[:alpha:]\\d-]"), r"[^[:alpha:]\d\x2D]");
        assert_eq!(reads("[a&&b]"), r"[\x61\x26\x26\x62]");
        assert_eq!(reads(r"[\b]"), r"[\x08]");
    }

    #[test]
    fn braces_are_quantifiers_only_where_pcre2_10_42_reads_them() {
        assert_eq!(reads("a{2}"), "a{2}");
        assert_eq!(reads("a{2,}?"), "a{2,}?");
        assert_eq!(reads("a{2,3}"), "a{2,3}");
        assert_eq!(reads("a{,3}"), r"a\x7B\x2C3\x7D");
        assert_eq!(reads("a{x}"), r"a\x7Bx\x7D");
        assert!(refused_in("a{3,2}"));
        assert!(refused_in("a{70000}"));
    }

    #[test]
    fn constructs_pcre2_reads_and_the_crate_does_not_are_unsupported() {
        // Verifies: ADR-0031
        for pattern in [
            r"(a)\1", "(?=a)", "(?!a)", "(?<=a)b", "(?<!a)b", "(?>a)", "(?R)", "(?1)", "(?&n)",
            "(?|a)", "(?#x)", r"\Qa\E", r"a\Kb", "a*+", "a++", "a{2}+", r"\v", r"\h", r"\R", r"\e",
            r"\cA", r"\N", r"\012", r"\o{12}", r"\pL", r"\Z", r"\G", r"\k<n>", r"\g1", "(*UTF)a",
            "(?x)a b", "(?'n'a)", "[[:<:]]a", r"[\d-z]", r"[a-\d]", r"[\v]",
        ] {
            assert!(unsupported_in(pattern), "{pattern}");
        }
    }

    #[test]
    fn what_pcre2_refuses_is_refused() {
        for pattern in [
            "(",
            "a)",
            "[a",
            "*a",
            r"\",
            r"\y",
            concat!("\\", "u0041"),
            "(?u)a",
            "[z-a]",
            "[[:foo:]]",
            r"\x{100}",
        ] {
            assert!(refused_in(pattern), "{pattern}");
        }
    }

    #[test]
    fn matching_is_byte_wise_and_ascii() {
        // Verifies: ADR-0031
        let matcher = |pattern: &str, caseless| match compile(pattern.as_bytes(), caseless) {
            Compiled::Matcher(regex) => regex,
            other => panic!("{pattern}: {other:?}"),
        };
        assert!(!matcher(r"^\d$", false).is_match("٣".as_bytes()));
        assert!(matcher("^..$", false).is_match("é".as_bytes()));
        assert!(matcher("^É$", true).is_match("É".as_bytes()));
        assert!(!matcher("^É$", true).is_match("é".as_bytes()));
        assert!(matcher("^ROAD$", true).is_match(b"road"));
        assert!(matcher(r"^\w+$", false).is_match(b"a_1"));
        assert!(!matcher(r"^\w$", false).is_match("é".as_bytes()));
        assert!(matcher("(?i)x(?-i)Y", false).is_match(b"XY"));
        assert!(matcher("(?<n>a)|(?P<m>b)", false).is_match(b"b"));
    }

    #[test]
    fn a_pattern_past_the_size_limit_is_unsupported() {
        // Verifies: ADR-0031, Q-9
        assert!(matches!(
            compile(br"\w{1000}[a-z]{1000}x{60000}", false),
            Compiled::Unsupported(_)
        ));
        assert!(matches!(compile(br"\w{100}", false), Compiled::Matcher(_)));
    }
}
