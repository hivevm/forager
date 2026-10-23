// Token rules follow MapServer's lexer, src/maplexer.l of release 8.6.6.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.

use forager_style_ir::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    /// A bare word: a keyword, or a value such as `ON` or `polygon`.
    Word(String),
    Str {
        value: String,
        ignore_case: bool,
    },
    Number(f64),
    /// `[name]`, an attribute binding.
    Binding(String),
    /// `( … )`, a MapServer expression, kept with its parentheses.
    Expression(String),
    Regex {
        pattern: String,
        ignore_case: bool,
    },
    /// `{ … }`, a list, kept with its braces.
    List(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// The word in upper case, if this token is a bare word.
    pub fn keyword(&self) -> Option<String> {
        match &self.kind {
            TokenKind::Word(w) => Some(w.to_ascii_uppercase()),
            _ => None,
        }
    }

    pub fn is_word(&self, keyword: &str) -> bool {
        matches!(&self.kind, TokenKind::Word(w) if w.eq_ignore_ascii_case(keyword))
    }
}

/// Splits mapfile text into tokens. Errors are reported and the lexer continues after them.
pub fn lex(text: &str, file: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lexer = Lexer {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
        column: 1,
        file,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    lexer.run();
    (lexer.tokens, lexer.diagnostics)
}

struct Lexer<'a> {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    column: u32,
    file: &'a str,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            let span = self.span();
            match c {
                ' ' | '\t' | '\r' | '\n' | '\u{feff}' => self.advance(1),
                '#' => self.skip_line(),
                '/' if self.peek(1) == Some('/') => self.skip_line(),
                '/' if self.peek(1) == Some('*') => self.block_comment(span),
                '[' => self.delimited(span, ']', TokenKind::Binding, false),
                '(' => self.delimited(span, ')', TokenKind::Expression, true),
                '{' => self.delimited(span, '}', TokenKind::List, true),
                '"' | '\'' => self.string(span, c),
                _ => self.atom(span),
            }
        }
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn span(&self) -> Span {
        Span::new(self.file, self.line, self.column)
    }

    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            if let Some(c) = self.peek(0) {
                self.pos += 1;
                if c == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
            }
        }
    }

    fn text(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }

    fn push(&mut self, kind: TokenKind, span: Span) {
        self.tokens.push(Token { kind, span });
    }

    fn error(&mut self, code: &str, message: &str, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, Some(span)));
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                break;
            }
            self.advance(1);
        }
    }

    fn block_comment(&mut self, span: Span) {
        self.advance(2);
        loop {
            match (self.peek(0), self.peek(1)) {
                (Some('*'), Some('/')) => return self.advance(2),
                (Some(_), _) => self.advance(1),
                (None, _) => {
                    return self.error("unterminated-comment", "unterminated comment", span);
                }
            }
        }
    }

    /// A binding runs to the next `]`, across lines. An expression or a list runs to the last
    /// closing character on its own line, as flex's greedy `.*` does.
    fn delimited(&mut self, span: Span, close: char, kind: fn(String) -> TokenKind, greedy: bool) {
        let start = self.pos;
        let mut end = None;
        for (i, &c) in self.chars[start + 1..].iter().enumerate() {
            if greedy && c == '\n' {
                break;
            }
            if c == close {
                end = Some(start + 1 + i);
                if !greedy {
                    break;
                }
            }
        }
        match end {
            Some(end) => {
                let token = if greedy {
                    self.text(start, end + 1)
                } else {
                    self.text(start + 1, end)
                };
                self.advance(end + 1 - start);
                self.push(kind(token), span);
            }
            None => {
                self.error(
                    "unterminated-token",
                    &format!("missing closing `{close}`"),
                    span,
                );
                self.skip_line();
            }
        }
    }

    fn string(&mut self, span: Span, quote: char) {
        self.advance(1);
        let mut value = String::new();
        loop {
            match self.peek(0) {
                None => return self.error("unterminated-string", "unterminated string", span),
                Some('\\') => match self.peek(1) {
                    Some(next @ ('"' | '\'' | '\\')) => {
                        value.push(next);
                        self.advance(2);
                    }
                    _ => {
                        value.push('\\');
                        self.advance(1);
                    }
                },
                Some(c) if c == quote => {
                    self.advance(1);
                    let ignore_case =
                        self.peek(0) == Some('i') && !self.peek(1).is_some_and(is_word_continue);
                    if ignore_case {
                        self.advance(1);
                    }
                    return self.push(TokenKind::Str { value, ignore_case }, span);
                }
                Some(c) => {
                    value.push(c);
                    self.advance(1);
                }
            }
        }
    }

    /// A number, a regular expression, or a bare word: the longest match wins, and on a tie the
    /// rule MapServer's lexer lists first — number, then regular expression, then word.
    fn atom(&mut self, span: Span) {
        let number = self.number_len();
        let regex = self.regex_len();
        let word = self.word_len();
        let longest = number.max(regex).max(word);
        if longest == 0 {
            let c = self.peek(0).unwrap_or_default();
            self.error(
                "unexpected-character",
                &format!("unexpected character `{c}`"),
                span,
            );
            return self.advance(1);
        }
        let text = self.text(self.pos, self.pos + longest);
        self.advance(longest);
        if longest == number {
            match text.parse::<f64>() {
                Ok(n) => self.push(TokenKind::Number(n), span),
                // MapServer reads such a token with atof(), as 0; Forager refuses it instead.
                Err(_) => self.error("invalid-number", &format!("`{text}` is not a number"), span),
            }
        } else if longest == regex {
            let ignore_case = text.ends_with('i');
            let inner = text.trim_end_matches('i');
            let pattern = inner[1..inner.len() - 1].to_owned();
            self.push(
                TokenKind::Regex {
                    pattern,
                    ignore_case,
                },
                span,
            );
        } else {
            self.push(TokenKind::Word(text), span);
        }
    }

    /// `-?[0-9]+`, `-?[0-9]+\.[0-9]*`, `-?\.[0-9]*`, each with an optional exponent.
    fn number_len(&self) -> usize {
        let digits = |from: usize| {
            (from..)
                .take_while(|&i| self.peek(i).is_some_and(|c| c.is_ascii_digit()))
                .count()
        };
        let mut i = usize::from(self.peek(0) == Some('-'));
        let integer = digits(i);
        i += integer;
        if self.peek(i) == Some('.') {
            i += 1 + digits(i + 1);
        } else if integer == 0 {
            return 0;
        }
        if matches!(self.peek(i), Some('e' | 'E')) {
            let sign = usize::from(matches!(self.peek(i + 1), Some('+' | '-')));
            let exponent = digits(i + 1 + sign);
            if exponent > 0 {
                i += 1 + sign + exponent;
            }
        }
        i
    }

    /// `/[^*][^/]*/` with an optional trailing `i`.
    fn regex_len(&self) -> usize {
        if self.peek(0) != Some('/') || matches!(self.peek(1), None | Some('*')) {
            return 0;
        }
        let Some(close) = (2..).find(|&i| self.peek(i).is_none_or(|c| c == '/')) else {
            return 0;
        };
        if self.peek(close).is_none() {
            return 0;
        }
        if self.peek(close + 1) == Some('i') {
            close + 2
        } else {
            close + 1
        }
    }

    /// `[a-z/.][a-z0-9/._-=]*`, case-insensitive.
    fn word_len(&self) -> usize {
        if !self
            .peek(0)
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '.')
        {
            return 0;
        }
        1 + (1..)
            .take_while(|&i| self.peek(i).is_some_and(is_word_continue))
            .count()
    }
}

fn is_word_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '=')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<TokenKind> {
        let (tokens, diagnostics) = lex(text, "t.map");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        tokens.into_iter().map(|t| t.kind).collect()
    }

    fn word(w: &str) -> TokenKind {
        TokenKind::Word(w.into())
    }

    fn string(s: &str) -> TokenKind {
        TokenKind::Str {
            value: s.into(),
            ignore_case: false,
        }
    }

    #[test]
    fn words_numbers_and_strings() {
        assert_eq!(
            kinds("COLOR 255 -1 .5 1e3 \"a \\\"b\\\" c\" 'd' \"x\"i"),
            vec![
                word("COLOR"),
                TokenKind::Number(255.0),
                TokenKind::Number(-1.0),
                TokenKind::Number(0.5),
                TokenKind::Number(1000.0),
                string("a \"b\" c"),
                string("d"),
                TokenKind::Str {
                    value: "x".into(),
                    ignore_case: true
                },
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            kinds("A # one\nB // two\n/* three\nfour */ C"),
            vec![word("A"), word("B"), word("C")]
        );
    }

    #[test]
    fn expressions_bindings_lists_and_regexes() {
        assert_eq!(
            kinds("EXPRESSION ([pop] > (10)) [name] {a,b} /^x.*/i"),
            vec![
                word("EXPRESSION"),
                TokenKind::Expression("([pop] > (10))".into()),
                TokenKind::Binding("name".into()),
                TokenKind::List("{a,b}".into()),
                TokenKind::Regex {
                    pattern: "^x.*".into(),
                    ignore_case: true
                },
            ]
        );
    }

    #[test]
    fn a_path_is_a_word_when_longer_than_a_regex() {
        assert_eq!(kinds("/data/roads.shp"), vec![word("/data/roads.shp")]);
        assert_eq!(
            kinds("/ab/"),
            vec![TokenKind::Regex {
                pattern: "ab".into(),
                ignore_case: false
            }]
        );
    }

    #[test]
    fn spans_count_lines_and_columns_from_one() {
        let (tokens, _) = lex("MAP\n  NAME \"x\"", "t.map");
        assert_eq!(tokens[2].span, Span::new("t.map", 2, 8));
    }

    #[test]
    fn unterminated_tokens_are_errors_not_panics() {
        for text in ["\"abc", "(a", "[a", "/* a", "@", "{a"] {
            let (_, diagnostics) = lex(text, "t.map");
            assert!(diagnostics.iter().any(Diagnostic::is_error), "{text}");
        }
    }
}
