//! The mapfile frontend: reads a UMN MapServer mapfile, resolves its `INCLUDE`s through a
//! [`ResourceLoader`], and translates it into the style IR. It is the only crate that knows
//! MapServer syntax.

mod expression;
mod init_file;
mod lexer;
mod report;
mod syntax;
mod translate;

use std::collections::BTreeMap;
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
/// `INCLUDE` is resolved against the main mapfile's directory, nested ones too; a `..` may leave
/// that directory but not the base context (Q-8).
/// `init=` projections are resolved by `resolver` (ADR-0023); without one they are load errors,
/// unless they name an init file by a path, which is read against the main mapfile's directory
/// as an `INCLUDE` is.
pub async fn load<L: ResourceLoader>(
    path: &ResourcePath,
    loader: &L,
    resolver: Option<&dyn ProjectionResolver>,
) -> Loaded {
    let mut reading = Reading {
        loader,
        main: path.clone(),
        stack: Vec::new(),
        report: Report::default(),
        bytes: 0,
    };
    let tokens = reading.read(path, None).await;
    let map = match tokens {
        Some(tokens) if !reading.report.failed() => {
            syntax::parse_into(&tokens, &mut reading.report)
        }
        _ => None,
    };
    let (symbol_set, font_set, init_files) = match &map {
        Some(map) => (
            reading.symbol_set(path, map).await,
            reading.font_set(path, map).await,
            reading.init_files(path, map).await,
        ),
        None => (None, None, BTreeMap::new()),
    };
    let mut report = reading.report;
    let document = map.filter(|_| !report.failed()).map(|block| {
        let files = translate::Files {
            main: path,
            symbol_set: symbol_set.as_ref(),
            font_set: font_set.as_ref(),
            init_files: &init_files,
        };
        let map = translate::translate(&block, &files, resolver, &mut report);
        StyleDocument::new(map)
    });
    Loaded {
        document: document.filter(|_| !report.failed()),
        diagnostics: report.finish(),
    }
}

/// The state of one load: the files being read, what they reported, and how much was read.
struct Reading<'a, L> {
    loader: &'a L,
    /// The main mapfile, whose directory every `INCLUDE` resolves against, also one in the
    /// symbol file.
    main: ResourcePath,
    stack: Vec<ResourcePath>,
    report: Report,
    bytes: usize,
}

impl<L: ResourceLoader> Reading<'_, L> {
    /// The text of the font set the map's last `FONTSET` names, with the path it was read from,
    /// resolved and bounded as the symbol file is.
    async fn font_set(
        &mut self,
        main: &ResourcePath,
        map: &syntax::Block,
    ) -> Option<(ResourcePath, String)> {
        let (directive, target) = self.named_file(main, map, "FONTSET")?;
        let span = Some(directive.span.clone());
        let bytes = match self.loader.load(&target).await {
            Ok(bytes) => bytes,
            Err(e) => {
                let message = format!("cannot read `{target}`: {e}");
                self.report
                    .push(Diagnostic::error("resource-unreadable", message, span));
                return None;
            }
        };
        self.bytes += bytes.len();
        if self.bytes > MAX_TOTAL_BYTES {
            let message = format!("the mapfile with its files exceeds {MAX_TOTAL_BYTES} bytes");
            self.report
                .push(Diagnostic::error("mapfile-too-large", message, span));
            return None;
        }
        Some((target, String::from_utf8_lossy(&bytes).into_owned()))
    }

    /// The init files the map's projections name by a path, each read once and bounded as an
    /// `INCLUDE` is (ADR-0023), keyed by the name as written. One that is refused or cannot be
    /// read is the reason, which the projection that names it reports.
    async fn init_files(
        &mut self,
        main: &ResourcePath,
        map: &syntax::Block,
    ) -> BTreeMap<String, Result<init_file::InitFile, String>> {
        let mut files: BTreeMap<String, Result<init_file::InitFile, String>> = BTreeMap::new();
        let mut read: BTreeMap<ResourcePath, String> = BTreeMap::new();
        for (name, span) in init_file::named(map) {
            let target = match main.resolve(&name) {
                Ok(target) => target,
                Err(e) => {
                    files.insert(
                        name.clone(),
                        Err(format!("the init file `{name}` is refused: {e}")),
                    );
                    continue;
                }
            };
            let text = match read.get(&target) {
                Some(text) => text.clone(),
                None => match self.loader.load(&target).await {
                    Ok(bytes) => {
                        self.bytes += bytes.len();
                        if self.bytes > MAX_TOTAL_BYTES {
                            let message = format!(
                                "the mapfile with its files exceeds {MAX_TOTAL_BYTES} bytes"
                            );
                            self.report.push(Diagnostic::error(
                                "mapfile-too-large",
                                message,
                                Some(span),
                            ));
                            return files;
                        }
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        read.insert(target.clone(), text.clone());
                        text
                    }
                    Err(e) => {
                        let why = format!("cannot read the init file `{target}`: {e}");
                        files.insert(name, Err(why));
                        continue;
                    }
                },
            };
            files.insert(name, Ok(init_file::InitFile::new(target, &text)));
        }
        files
    }

    /// The last `keyword` directive of the map and the file it names, resolved against the main
    /// mapfile's directory inside the base context (Q-8).
    fn named_file<'b>(
        &mut self,
        main: &ResourcePath,
        map: &'b syntax::Block,
        keyword: &str,
    ) -> Option<(&'b syntax::Directive, ResourcePath)> {
        let directive = map.nodes.iter().rev().find_map(|node| match node {
            syntax::Node::Directive(d) if d.keyword == keyword => Some(d),
            _ => None,
        })?;
        let span = Some(directive.span.clone());
        let [
            Token {
                kind: lexer::TokenKind::Str { value, .. } | lexer::TokenKind::Word(value),
                ..
            },
        ] = directive.values.as_slice()
        else {
            let message = format!("{keyword} expects a file");
            self.report
                .push(Diagnostic::error("invalid-value", message, span));
            return None;
        };
        match main.resolve(value) {
            Ok(target) => Some((directive, target)),
            Err(e) => {
                let message = format!("{keyword} `{value}` is refused: {e}");
                let code = format!("{}-refused", keyword.to_ascii_lowercase());
                self.report.push(Diagnostic::error(&code, message, span));
                None
            }
        }
    }

    /// The symbol file the map's last `SYMBOLSET` names, parsed, with the path it was read from:
    /// resolved against the main mapfile's directory inside the base context, as MapServer
    /// resolves it against the map's path (Q-8), and read within the limits of `INCLUDE` (Q-9).
    async fn symbol_set(
        &mut self,
        main: &ResourcePath,
        map: &syntax::Block,
    ) -> Option<(ResourcePath, syntax::Block)> {
        let (directive, target) = self.named_file(main, map, "SYMBOLSET")?;
        let tokens = self.read(&target, Some(directive.span.clone())).await?;
        let block = syntax::parse_symbol_set(&tokens, &mut self.report);
        block.map(|block| (target, block))
    }

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
            let tokens = lexer::lex_into(&text, path.as_str(), &mut self.report);
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
                let target =
                    include_target(&token, argument, &self.main, &self.stack, &mut self.report);
                let Some(target) = target else {
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

/// The file an `INCLUDE` names, resolved against the main mapfile `main`, if it may be read:
/// inside the base context (Q-8), not already
/// being read, and not nested deeper than [`MAX_INCLUDE_DEPTH`] (Q-9).
fn include_target(
    include: &Token,
    argument: Option<Token>,
    main: &ResourcePath,
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
    let target = match main.resolve(&value) {
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
