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

/// The result of loading a mapfile: the document if no error occurred, and every diagnostic.
#[derive(Debug)]
pub struct Loaded {
    pub document: Option<StyleDocument>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Loads the mapfile at `path` inside the base context of `loader`. As in MapServer, every
/// `INCLUDE` is resolved against the base context, which is the main mapfile's directory.
pub async fn load<L: ResourceLoader>(path: &ResourcePath, loader: &L) -> Loaded {
    let mut diagnostics = Vec::new();
    let mut stack = Vec::new();
    let tokens = read(path, None, loader, &mut stack, &mut diagnostics).await;
    let document = match tokens {
        Some(tokens) if !diagnostics.iter().any(Diagnostic::is_error) => {
            let (map, mut parse_diagnostics) = syntax::parse(&tokens);
            diagnostics.append(&mut parse_diagnostics);
            map.map(|block| StyleDocument::new(translate::translate(&block, &mut diagnostics)))
        }
        _ => None,
    };
    let document = document.filter(|_| !diagnostics.iter().any(Diagnostic::is_error));
    Loaded {
        document,
        diagnostics,
    }
}

/// Reads one file into tokens, splicing in the tokens of its `INCLUDE`s.
fn read<'a, L: ResourceLoader>(
    path: &'a ResourcePath,
    included_at: Option<Span>,
    loader: &'a L,
    stack: &'a mut Vec<ResourcePath>,
    diagnostics: &'a mut Vec<Diagnostic>,
) -> Pin<Box<dyn Future<Output = Option<Vec<Token>>> + 'a>> {
    Box::pin(async move {
        let bytes = match loader.load(path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                let message = format!("cannot read `{path}`: {e}");
                diagnostics.push(Diagnostic::error(
                    "resource-unreadable",
                    message,
                    included_at,
                ));
                return None;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let (tokens, mut lex_diagnostics) = lexer::lex(&text, path.as_str());
        diagnostics.append(&mut lex_diagnostics);
        stack.push(path.clone());
        let mut spliced = Vec::with_capacity(tokens.len());
        let mut iter = tokens.into_iter();
        while let Some(token) = iter.next() {
            if !token.is_word("INCLUDE") {
                spliced.push(token);
                continue;
            }
            let Some(target) = include_target(&token, iter.next(), stack, diagnostics) else {
                continue;
            };
            let included = read(&target, Some(token.span), loader, stack, diagnostics).await;
            spliced.extend(included.unwrap_or_default());
        }
        stack.pop();
        Some(spliced)
    })
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
