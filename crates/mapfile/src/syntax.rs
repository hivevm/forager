// The statement keywords of each block follow MapServer's mapfile parser, src/mapfile.c and
// src/mapsymbol.c of release 8.6.6.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.

//! The generic syntax of a mapfile: blocks closed by `END`, and directives with their values.
//!
//! What opens a block depends on the enclosing block — `SYMBOL` opens one in `MAP` and names a
//! symbol in `STYLE` — so each block kind lists its statements and its child blocks. A directive
//! takes every following token up to the next statement keyword of its block, so one-line
//! blocks such as `STYLE COLOR 0 0 0 END` read as MapServer reads them.

use forager_style_ir::{Diagnostic, Span};

use crate::lexer::Token;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Block(Block),
    Directive(Directive),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The keyword that opened it, in upper case.
    pub keyword: String,
    pub span: Span,
    /// The statements of a structured block.
    pub nodes: Vec<Node>,
    /// The raw tokens of a value block such as `METADATA` or `PROJECTION`.
    pub values: Vec<Token>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Directive {
    pub keyword: String,
    pub span: Span,
    pub values: Vec<Token>,
}

struct Schema {
    statements: &'static [&'static str],
    blocks: &'static [&'static str],
}

/// Blocks whose content is a flat list of values up to `END`.
const VALUE_BLOCKS: &[&str] = &[
    "BINDVALS",
    "CONNECTIONOPTIONS",
    "METADATA",
    "PATTERN",
    "POINTS",
    "PROJECTION",
    "VALIDATION",
    "VALUES",
];

/// Each block kind with its statement keywords and the keywords that open a child block.
#[rustfmt::skip]
const SCHEMAS: &[(&str, &[&str], &[&str])] = &[
    ("MAP",
        &[
            "ANGLE", "CONFIG", "DEBUG", "DEFRESOLUTION", "EXTENT", "FONTSET", "IMAGECOLOR",
            "IMAGETYPE", "LATLON", "MAXSIZE", "NAME", "RESOLUTION", "SCALE", "SCALEDENOM",
            "SHAPEPATH", "SIZE", "STATUS", "SYMBOLSET", "UNITS",
        ],
        &[
            "LAYER", "LEGEND", "OUTPUTFORMAT", "PROJECTION", "QUERYMAP", "REFERENCE",
            "SCALEBAR", "SYMBOL", "WEB",
        ],
    ),
    ("LAYER",
        &[
            "CLASSGROUP", "CLASSITEM", "CONNECTION", "CONNECTIONTYPE", "DATA", "DEBUG",
            "ENCODING", "EXTENT", "FILTER", "FILTERITEM", "FOOTER", "GEOMTRANSFORM", "GROUP",
            "HEADER", "LABELCACHE", "LABELITEM", "LABELMAXSCALE", "LABELMAXSCALEDENOM",
            "LABELMINSCALE", "LABELMINSCALEDENOM", "LABELREQUIRES", "MASK", "MAXFEATURES",
            "MAXGEOWIDTH", "MAXSCALE", "MAXSCALEDENOM", "MINFEATURESIZE", "MINGEOWIDTH",
            "MINSCALE", "MINSCALEDENOM", "NAME", "OFFSITE", "PLUGIN", "POSTLABELCACHE",
            "PROCESSING", "REQUIRES", "SIZEUNITS", "STATUS", "STYLEITEM", "SYMBOLSCALE",
            "SYMBOLSCALEDENOM", "TEMPLATE", "TILEINDEX", "TILEITEM", "TILESRS", "TOLERANCE",
            "TOLERANCEUNITS", "TRANSFORM", "TYPE", "UNITS", "UTFDATA", "UTFITEM",
        ],
        &[
            "BINDVALS", "CLASS", "CLUSTER", "COMPOSITE", "CONNECTIONOPTIONS", "FEATURE",
            "GRID", "IDENTIFY", "JOIN", "METADATA", "PROJECTION", "SCALETOKEN", "VALIDATION",
        ],
    ),
    ("CLASS",
        &[
            "DEBUG", "EXPRESSION", "FALLBACK", "GROUP", "KEYIMAGE", "MAXSCALE",
            "MAXSCALEDENOM", "MINFEATURESIZE", "MINSCALE", "MINSCALEDENOM", "NAME", "STATUS",
            "TEMPLATE", "TEXT", "TITLE",
        ],
        &["LABEL", "LEADER", "METADATA", "STYLE", "VALIDATION"],
    ),
    ("STYLE",
        &[
            "ANGLE", "ANTIALIAS", "COLOR", "COLORRANGE", "DATARANGE", "GAP", "GEOMTRANSFORM",
            "INITIALGAP", "LINECAP", "LINEJOIN", "LINEJOINMAXSIZE", "MAXSCALEDENOM", "MAXSIZE",
            "MAXWIDTH", "MINSCALEDENOM", "MINSIZE", "MINWIDTH", "OFFSET", "OPACITY",
            "OUTLINECOLOR", "OUTLINEWIDTH", "POLAROFFSET", "RANGEITEM", "SIZE", "SYMBOL",
            "WIDTH",
        ],
        &["PATTERN"],
    ),
    ("LABEL",
        &[
            "ALIGN", "ANGLE", "ANTIALIAS", "BUFFER", "COLOR", "ENCODING", "EXPRESSION", "FONT",
            "FORCE", "MAXLENGTH", "MAXOVERLAPANGLE", "MAXSCALEDENOM", "MAXSIZE", "MINDISTANCE",
            "MINFEATURESIZE", "MINSCALEDENOM", "MINSIZE", "OFFSET", "OUTLINECOLOR",
            "OUTLINEWIDTH", "PARTIALS", "POSITION", "PRIORITY", "REPEATDISTANCE",
            "SHADOWCOLOR", "SHADOWSIZE", "SIZE", "TEXT", "TYPE", "WRAP",
        ],
        &["STYLE"],
    ),
    ("LEADER", &["GRIDSTEP", "MAXDISTANCE"], &["STYLE"]),
    ("CLUSTER", &["BUFFER", "FILTER", "GROUP", "MAXDISTANCE", "REGION"], &[]),
    ("COMPOSITE", &["COMPFILTER", "COMPOP", "OPACITY"], &[]),
    ("FEATURE", &["ITEMS", "TEXT", "WKT"], &["POINTS"]),
    ("GRID",
        &[
            "LABELFORMAT", "MAXARCS", "MAXINTERVAL", "MAXSUBDIVIDE", "MINARCS", "MININTERVAL",
            "MINSUBDIVIDE",
        ],
        &[],
    ),
    ("IDENTIFY",
        &["CLASSAUTO", "CLASSGROUP", "TOLERANCE", "TOLERANCEUNITS"],
        &[],
    ),
    ("JOIN",
        &[
            "CONNECTION", "CONNECTIONTYPE", "FOOTER", "FROM", "HEADER", "NAME", "TABLE",
            "TEMPLATE", "TO", "TYPE",
        ],
        &[],
    ),
    ("LEGEND",
        &[
            "IMAGECOLOR", "KEYSIZE", "KEYSPACING", "OUTLINECOLOR", "POSITION",
            "POSTLABELCACHE", "STATUS", "TEMPLATE", "TRANSPARENT",
        ],
        &["LABEL"],
    ),
    ("OUTPUTFORMAT",
        &[
            "DRIVER", "EXTENSION", "FORMATOPTION", "IMAGEMODE", "MIMETYPE", "NAME",
            "TRANSPARENT",
        ],
        &[],
    ),
    ("QUERYMAP", &["COLOR", "SIZE", "STATUS", "STYLE", "TYPE"], &[]),
    ("REFERENCE",
        &[
            "COLOR", "EXTENT", "IMAGE", "MARKER", "MARKERSIZE", "MAXBOXSIZE", "MINBOXSIZE",
            "OUTLINECOLOR", "SIZE", "STATUS",
        ],
        &[],
    ),
    ("SCALEBAR",
        &[
            "ALIGN", "BACKGROUNDCOLOR", "COLOR", "IMAGECOLOR", "INTERVALS", "OFFSET",
            "OUTLINECOLOR", "POSITION", "POSTLABELCACHE", "SIZE", "STATUS", "STYLE",
            "TRANSPARENT", "UNITS",
        ],
        &["LABEL"],
    ),
    ("SCALETOKEN", &["NAME"], &["VALUES"]),
    ("SYMBOL",
        &[
            "ANCHORPOINT", "ANTIALIAS", "CHARACTER", "FILLED", "FONT", "IMAGE", "NAME",
            "TRANSPARENT", "TYPE",
        ],
        &["POINTS"],
    ),
    ("WEB",
        &[
            "BROWSEFORMAT", "EMPTY", "ERROR", "FOOTER", "HEADER", "IMAGEPATH", "IMAGEURL",
            "LEGENDFORMAT", "MAXSCALEDENOM", "MAXTEMPLATE", "MINSCALEDENOM", "MINTEMPLATE",
            "QUERYFORMAT", "TEMPLATE", "TEMPPATH",
        ],
        &["METADATA", "VALIDATION"],
    ),
];

fn schema(block: &str) -> Option<Schema> {
    SCHEMAS
        .iter()
        .find(|(name, _, _)| *name == block)
        .map(|&(_, statements, blocks)| Schema { statements, blocks })
}

/// Parses the tokens of a mapfile, its `INCLUDE`s already spliced in, into the `MAP` block.
pub fn parse(tokens: &[Token]) -> (Option<Block>, Vec<Diagnostic>) {
    let mut parser = Parser {
        tokens,
        pos: 0,
        diagnostics: Vec::new(),
    };
    let map = match tokens.first() {
        Some(first) if first.is_word("MAP") => {
            parser.pos = 1;
            parser.block("MAP".to_owned(), first.span.clone())
        }
        Some(first) => {
            parser.error(
                "expected-map",
                "a mapfile starts with MAP",
                first.span.clone(),
            );
            None
        }
        None => {
            parser.diagnostics.push(Diagnostic::error(
                "expected-map",
                "the mapfile is empty",
                None,
            ));
            None
        }
    };
    if map.is_some()
        && let Some(rest) = tokens.get(parser.pos)
    {
        parser.diagnostics.push(Diagnostic::warning(
            "content-after-map",
            "content after the END of MAP is ignored",
            Some(rest.span.clone()),
        ));
    }
    (map, parser.diagnostics)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn error(&mut self, code: &str, message: &str, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, Some(span)));
    }

    /// Parses the content of a block whose keyword was just consumed, through its `END`.
    fn block(&mut self, keyword: String, span: Span) -> Option<Block> {
        let mut block = Block {
            keyword,
            span,
            nodes: Vec::new(),
            values: Vec::new(),
        };
        if VALUE_BLOCKS.contains(&block.keyword.as_str()) {
            return self.value_block(block);
        }
        let schema = schema(&block.keyword)?;
        loop {
            let Some(token) = self.tokens.get(self.pos) else {
                let message = format!("{} is not closed by END", block.keyword);
                self.error("unterminated-block", &message, block.span.clone());
                return None;
            };
            self.pos += 1;
            let Some(keyword) = token.keyword() else {
                self.error("expected-keyword", "expected a keyword", token.span.clone());
                continue;
            };
            if keyword == "END" {
                return Some(block);
            }
            if schema.blocks.contains(&keyword.as_str()) {
                block
                    .nodes
                    .push(Node::Block(self.block(keyword, token.span.clone())?));
                continue;
            }
            if !schema.statements.contains(&keyword.as_str()) {
                self.diagnostics.push(Diagnostic::warning(
                    "unknown-keyword",
                    format!("{keyword} is not a keyword of {}", block.keyword),
                    Some(token.span.clone()),
                ));
            }
            let values = self.values(&schema);
            block.nodes.push(Node::Directive(Directive {
                keyword,
                span: token.span.clone(),
                values,
            }));
        }
    }

    fn value_block(&mut self, mut block: Block) -> Option<Block> {
        while let Some(token) = self.tokens.get(self.pos) {
            self.pos += 1;
            if token.is_word("END") {
                return Some(block);
            }
            block.values.push(token.clone());
        }
        let message = format!("{} is not closed by END", block.keyword);
        self.error("unterminated-block", &message, block.span.clone());
        None
    }

    fn values(&mut self, schema: &Schema) -> Vec<Token> {
        let start = self.pos;
        while let Some(token) = self.tokens.get(self.pos) {
            if let Some(keyword) = token.keyword()
                && (keyword == "END"
                    || schema.statements.contains(&keyword.as_str())
                    || schema.blocks.contains(&keyword.as_str()))
            {
                break;
            }
            self.pos += 1;
        }
        self.tokens[start..self.pos].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{TokenKind, lex};

    fn parse_text(text: &str) -> (Option<Block>, Vec<Diagnostic>) {
        let (tokens, lex_diagnostics) = lex(text, "t.map");
        assert!(lex_diagnostics.is_empty(), "{lex_diagnostics:?}");
        parse(&tokens)
    }

    fn keywords(nodes: &[Node]) -> Vec<&str> {
        nodes
            .iter()
            .map(|n| match n {
                Node::Block(b) => b.keyword.as_str(),
                Node::Directive(d) => d.keyword.as_str(),
            })
            .collect()
    }

    #[test]
    fn one_line_blocks_read_as_mapserver_reads_them() {
        let (map, diagnostics) =
            parse_text("MAP LAYER TYPE POLYGON CLASS STYLE COLOR 1 2 3 SYMBOL 'x' END END END END");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let map = map.unwrap();
        let Node::Block(layer) = &map.nodes[0] else {
            panic!()
        };
        assert_eq!(keywords(&layer.nodes), ["TYPE", "CLASS"]);
        let Node::Block(class) = &layer.nodes[1] else {
            panic!()
        };
        let Node::Block(style) = &class.nodes[0] else {
            panic!()
        };
        assert_eq!(keywords(&style.nodes), ["COLOR", "SYMBOL"]);
        let Node::Directive(color) = &style.nodes[0] else {
            panic!()
        };
        assert_eq!(color.values.len(), 3);
    }

    #[test]
    fn symbol_opens_a_block_in_map_but_not_in_style() {
        let (map, diagnostics) = parse_text(
            "MAP SYMBOL NAME 'c' TYPE ELLIPSE POINTS 1 1 END END \
             LAYER CLASS STYLE SYMBOL 'c' END END END END",
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let map = map.unwrap();
        let Node::Block(symbol) = &map.nodes[0] else {
            panic!()
        };
        assert_eq!(symbol.keyword, "SYMBOL");
        assert_eq!(keywords(&symbol.nodes), ["NAME", "TYPE", "POINTS"]);
    }

    #[test]
    fn value_blocks_keep_their_tokens() {
        let (map, _) = parse_text("MAP PROJECTION 'proj=utm' 'zone=32' END END");
        let Node::Block(projection) = &map.unwrap().nodes[0] else {
            panic!()
        };
        assert_eq!(projection.values.len(), 2);
        assert!(
            matches!(&projection.values[0].kind, TokenKind::Str { value, .. } if value == "proj=utm")
        );
    }

    #[test]
    fn an_unknown_keyword_is_a_warning() {
        let (map, diagnostics) = parse_text("MAP FROBNICATE 1 2 NAME 'x' END");
        assert!(map.is_some());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "unknown-keyword");
        assert_eq!(diagnostics[0].span, Some(Span::new("t.map", 1, 5)));
    }

    #[test]
    fn an_unclosed_block_is_an_error() {
        let (map, diagnostics) = parse_text("MAP\n  LAYER NAME 'x'\n");
        assert!(map.is_none());
        assert_eq!(diagnostics[0].code, "unterminated-block");
        assert_eq!(diagnostics[0].span, Some(Span::new("t.map", 2, 3)));
    }

    #[test]
    fn a_file_without_map_is_an_error() {
        let (map, diagnostics) = parse_text("LAYER END");
        assert!(map.is_none());
        assert_eq!(diagnostics[0].code, "expected-map");
    }
}
