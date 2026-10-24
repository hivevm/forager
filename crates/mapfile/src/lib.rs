//! The mapfile frontend: reads a UMN MapServer mapfile, resolves its `INCLUDE`s through a
//! [`ResourceLoader`], and translates it into the style IR. It is the only crate that knows
//! MapServer syntax.

mod lexer;
mod report;
mod syntax;
mod translate;

use std::future::Future;
use std::pin::Pin;

use forager_ports::{ProjectionResolver, ResourceLoader, ResourcePath};
use forager_style_ir::{Diagnostic, Span, StyleDocument};

use crate::lexer::Token;
use crate::report::Report;

/// How deep `INCLUDE`s may nest, as in MapServer (`MAX_INCLUDE_DEPTH` of its lexer).
pub const MAX_INCLUDE_DEPTH: usize = 5;

/// How much text a mapfile with all its `INCLUDE`s may amount to (Q-9). Depth alone does not
/// bound it: five levels of files that each include the next many times multiply.
pub const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;

/// How many diagnostics one load reports (Q-9); the rest are counted in a last one.
pub const MAX_DIAGNOSTICS: usize = 1000;

/// The result of loading a mapfile: the document if no error occurred, and the diagnostics.
#[derive(Debug)]
pub struct Loaded {
    pub document: Option<StyleDocument>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Loads the mapfile at `path` inside the base context of `loader`. As in MapServer, every
/// `INCLUDE` is resolved against the base context, which is the main mapfile's directory.
/// `init=` projections are resolved by `resolver` (ADR-0018); without one they are load errors.
pub async fn load<L: ResourceLoader>(
    path: &ResourcePath,
    loader: &L,
    resolver: Option<&dyn ProjectionResolver>,
) -> Loaded {
    let mut reading = Reading {
        loader,
        stack: Vec::new(),
        report: Report::default(),
        bytes: 0,
    };
    let tokens = reading.read(path, None).await;
    let mut report = reading.report;
    let document = match tokens {
        Some(tokens) if !report.failed() => {
            let (map, parse_diagnostics) = syntax::parse(&tokens);
            report.extend(parse_diagnostics);
            map.map(|block| {
                let mut translate_diagnostics = Vec::new();
                let map = translate::translate(&block, resolver, &mut translate_diagnostics);
                report.extend(translate_diagnostics);
                StyleDocument::new(map)
            })
        }
        _ => None,
    };
    Loaded {
        document: document.filter(|_| !report.failed()),
        diagnostics: report.finish(),
    }
}

/// The state of one load: the files being read, what they reported, and how much was read.
struct Reading<'a, L> {
    loader: &'a L,
    stack: Vec<ResourcePath>,
    report: Report,
    bytes: usize,
}

impl<L: ResourceLoader> Reading<'_, L> {
    /// Reads one file into tokens, splicing in the tokens of its `INCLUDE`s.
    fn read<'s>(
        &'s mut self,
        path: &'s ResourcePath,
        included_at: Option<Span>,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<Token>>> + 's>> {
        Box::pin(async move {
            if self.bytes > MAX_TOTAL_BYTES {
                return None;
            }
            let bytes = match self.loader.load(path).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    let message = format!("cannot read `{path}`: {e}");
                    self.report.push(Diagnostic::error(
                        "resource-unreadable",
                        message,
                        included_at,
                    ));
                    return None;
                }
            };
            self.bytes += bytes.len();
            if self.bytes > MAX_TOTAL_BYTES {
                let message =
                    format!("the mapfile with its INCLUDEs exceeds {MAX_TOTAL_BYTES} bytes");
                self.report
                    .push(Diagnostic::error("mapfile-too-large", message, included_at));
                return None;
            }
            let text = String::from_utf8_lossy(&bytes);
            let (tokens, lex_diagnostics) = lexer::lex(&text, path.as_str());
            self.report.extend(lex_diagnostics);
            self.stack.push(path.clone());
            let mut spliced = Vec::with_capacity(tokens.len());
            let mut iter = tokens.into_iter().peekable();
            while let Some(token) = iter.next() {
                if !token.is_word("INCLUDE") {
                    spliced.push(token);
                    continue;
                }
                // Only a string is taken as the argument, so `INCLUDE END` keeps its END.
                let argument = iter.next_if(|t| matches!(t.kind, lexer::TokenKind::Str { .. }));
                let Some(target) = include_target(&token, argument, &self.stack, &mut self.report)
                else {
                    continue;
                };
                let included = self.read(&target, Some(token.span)).await;
                spliced.extend(included.unwrap_or_default());
            }
            self.stack.pop();
            Some(spliced)
        })
    }
}

/// The file an `INCLUDE` names, if it may be read: inside the base context (Q-8), not already
/// being read, and not nested deeper than [`MAX_INCLUDE_DEPTH`] (Q-9).
fn include_target(
    include: &Token,
    argument: Option<Token>,
    stack: &[ResourcePath],
    report: &mut Report,
) -> Option<ResourcePath> {
    let span = Some(include.span.clone());
    let Some(Token {
        kind: lexer::TokenKind::Str { value, .. },
        ..
    }) = argument
    else {
        report.push(Diagnostic::error(
            "invalid-value",
            "INCLUDE expects a string",
            span,
        ));
        return None;
    };
    let target = match ResourcePath::new(&value) {
        Ok(target) => target,
        Err(e) => {
            let message = format!("INCLUDE `{value}` is refused: {e}");
            report.push(Diagnostic::error("include-refused", message, span));
            return None;
        }
    };
    if stack.contains(&target) {
        let message = format!("INCLUDE `{target}` includes itself");
        report.push(Diagnostic::error("include-cycle", message, span));
        return None;
    }
    // The stack holds the main mapfile and the includes being read.
    if stack.len() > MAX_INCLUDE_DEPTH {
        let message = format!("INCLUDEs nest deeper than {MAX_INCLUDE_DEPTH}");
        report.push(Diagnostic::error("include-too-deep", message, span));
        return None;
    }
    Some(target)
}

#[cfg(test)]
mod tests;
