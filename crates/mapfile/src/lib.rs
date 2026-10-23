//! The mapfile frontend: reads a UMN MapServer mapfile, resolves its `INCLUDE`s through a
//! [`ResourceLoader`], and translates it into the style IR. It is the only crate that knows
//! MapServer syntax.

mod lexer;
mod syntax;
mod translate;

use std::future::Future;
use std::pin::Pin;

use forager_ports::{ResourceLoader, ResourcePath};
use forager_style_ir::{Diagnostic, Span, StyleDocument};

use crate::lexer::Token;

/// How deep `INCLUDE`s may nest, as in MapServer (`MAX_INCLUDE_DEPTH` of its lexer).
pub const MAX_INCLUDE_DEPTH: usize = 5;

/// How much text a mapfile with all its `INCLUDE`s may amount to (Q-9). Depth alone does not
/// bound it: five levels of files that each include the next many times multiply.
pub const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

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
pub async fn load<L: ResourceLoader>(path: &ResourcePath, loader: &L) -> Loaded {
    let mut reading = Reading {
        loader,
        stack: Vec::new(),
        diagnostics: Vec::new(),
        bytes: 0,
    };
    let tokens = reading.read(path, None).await;
    let mut diagnostics = reading.diagnostics;
    let document = match tokens {
        Some(tokens) if !diagnostics.iter().any(Diagnostic::is_error) => {
            let (map, mut parse_diagnostics) = syntax::parse(&tokens);
            diagnostics.append(&mut parse_diagnostics);
            map.map(|block| StyleDocument::new(translate::translate(&block, &mut diagnostics)))
        }
        _ => None,
    };
    let document = document.filter(|_| !diagnostics.iter().any(Diagnostic::is_error));
    if diagnostics.len() > MAX_DIAGNOSTICS {
        let dropped = diagnostics.len() - MAX_DIAGNOSTICS;
        diagnostics.truncate(MAX_DIAGNOSTICS);
        diagnostics.push(Diagnostic::warning(
            "diagnostics-truncated",
            format!("{dropped} more diagnostics are not shown"),
            None,
        ));
    }
    Loaded {
        document,
        diagnostics,
    }
}

/// The state of one load: the files being read, what they reported, and how much was read.
struct Reading<'a, L> {
    loader: &'a L,
    stack: Vec<ResourcePath>,
    diagnostics: Vec<Diagnostic>,
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
                    self.diagnostics.push(Diagnostic::error(
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
                self.diagnostics
                    .push(Diagnostic::error("mapfile-too-large", message, included_at));
                return None;
            }
            let text = String::from_utf8_lossy(&bytes);
            let (tokens, mut lex_diagnostics) = lexer::lex(&text, path.as_str());
            self.diagnostics.append(&mut lex_diagnostics);
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
                let Some(target) =
                    include_target(&token, argument, &self.stack, &mut self.diagnostics)
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
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResourcePath> {
    let span = Some(include.span.clone());
    let Some(Token {
        kind: lexer::TokenKind::Str { value, .. },
        ..
    }) = argument
    else {
        diagnostics.push(Diagnostic::error(
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
            diagnostics.push(Diagnostic::error("include-refused", message, span));
            return None;
        }
    };
    if stack.contains(&target) {
        let message = format!("INCLUDE `{target}` includes itself");
        diagnostics.push(Diagnostic::error("include-cycle", message, span));
        return None;
    }
    // The stack holds the main mapfile and the includes being read.
    if stack.len() > MAX_INCLUDE_DEPTH {
        let message = format!("INCLUDEs nest deeper than {MAX_INCLUDE_DEPTH}");
        diagnostics.push(Diagnostic::error("include-too-deep", message, span));
        return None;
    }
    Some(target)
}

#[cfg(test)]
mod tests;
