// The keywords and their token values follow the rules of MapServer's lexer, src/maplexer.l,
// and the values src/mapfile.h, src/mapserver.h, and src/mapsymbol.h give them, of the release
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

//! The bare words MapServer's lexer reads as keywords, with the token value each returns: the
//! rules of `src/maplexer.l` that return a token, `MS_LEXER_RETURN_TOKEN`, in the `INITIAL` state
//! a mapfile is read in, without regard to case. A word that is one is no string to `getString`,
//! and `getSymbol` compares a keyword choice by its value, so words of one value stand for each
//! other: `OFF`, `FALSE`, `INCHES`, and `POINT` are all 0. The byte order mark the lexer also
//! returns a token for is skipped before a word is read, and `include` begins a state of its own.

/// Each keyword in lower case, sorted, with its token value: a block or directive's keyword from
/// 1000 up (`src/mapfile.h`), a value of one of MapServer's enums below that, or the symbol types
/// from 1000 up, `MS_SYMBOL_SIMPLE` first (`src/mapsymbol.h`).
pub(crate) const KEYWORDS: [(&str, i32); 262] = [
    ("align", 1203),              // ALIGN
    ("anchorpoint", 1230),        // ANCHORPOINT
    ("angle", 1000),              // ANGLE
    ("annotation", 4),            // MS_LAYER_ANNOTATION
    ("antialias", 1001),          // ANTIALIAS
    ("auto", 110),                // MS_AUTO
    ("auto2", 113),               // MS_AUTO2
    ("backgroundcolor", 1002),    // BACKGROUNDCOLOR
    ("bandsitem", 1159),          // BANDSITEM
    ("bevel", 1),                 // MS_CJC_BEVEL
    ("bindvals", 2000),           // BINDVALS
    ("bitmap", 1),                // MS_BITMAP
    ("browseformat", 1191),       // BROWSEFORMAT
    ("buffer", 1003),             // BUFFER
    ("butt", 2),                  // MS_CJC_BUTT
    ("cc", 109),                  // MS_CC
    ("center", 2),                // MS_ALIGN_CENTER
    ("character", 1100),          // CHARACTER
    ("chart", 8),                 // MS_LAYER_CHART
    ("circle", 6),                // MS_LAYER_CIRCLE
    ("cl", 106),                  // MS_CL
    ("class", 1004),              // CLASS
    ("classauto", 1104),          // CLASSAUTO
    ("classgroup", 1202),         // CLASSGROUP
    ("classitem", 1005),          // CLASSITEM
    ("cluster", 1226),            // CLUSTER
    ("color", 1006),              // COLOR
    ("colorrange", 1170),         // COLORRANGE
    ("compfilter", 1292),         // COMPFILTER
    ("compop", 1291),             // COMPOP
    ("composite", 1290),          // COMPOSITE
    ("config", 1158),             // CONFIG
    ("connection", 1101),         // CONNECTION
    ("connectionoptions", 2001),  // CONNECTIONOPTIONS
    ("connectiontype", 1102),     // CONNECTIONTYPE
    ("contour", 16),              // MS_CONTOUR
    ("cr", 105),                  // MS_CR
    ("csv", 1),                   // MS_DB_CSV
    ("data", 1007),               // DATA
    ("datarange", 1172),          // DATARANGE
    ("dd", 5),                    // MS_DD
    ("debug", 1154),              // DEBUG
    ("default", 2),               // MS_DEFAULT
    ("defresolution", 1221),      // DEFRESOLUTION
    ("driver", 1137),             // DRIVER
    ("ellipse", 1002),            // MS_SYMBOL_ELLIPSE
    ("embed", 3),                 // MS_EMBED
    ("empty", 1010),              // EMPTY
    ("encoding", 1162),           // ENCODING
    ("error", 1012),              // ERROR
    ("expression", 1013),         // EXPRESSION
    ("extension", 1155),          // EXTENSION
    ("extent", 1014),             // EXTENT
    ("fallback", 1125),           // FALLBACK
    ("false", 0),                 // MS_FALSE
    ("feature", 1015),            // FEATURE
    ("feet", 1),                  // MS_FEET
    ("filled", 1016),             // FILLED
    ("filter", 1116),             // FILTER
    ("filteritem", 1117),         // FILTERITEM
    ("flatgeobuf", 19),           // MS_FLATGEOBUF
    ("follow", 114),              // MS_FOLLOW
    ("font", 1018),               // FONT
    ("fontset", 1019),            // FONTSET
    ("footer", 1017),             // FOOTER
    ("force", 1098),              // FORCE
    ("formatoption", 1139),       // FORMATOPTION
    ("from", 1020),               // FROM
    ("gap", 1115),                // GAP
    ("geomtransform", 1220),      // GEOMTRANSFORM
    ("giant", 16),                // MS_GIANT
    ("graticule", 1140),          // GRATICULE
    ("grid", 1141),               // GRID
    ("gridstep", 1261),           // GRIDSTEP
    ("group", 1021),              // GROUP
    ("hatch", 1005),              // MS_SYMBOL_HATCH
    ("header", 1022),             // HEADER
    ("hilite", 1),                // MS_HILITE
    ("identify", 1103),           // IDENTIFY
    ("idw", 18),                  // MS_IDW
    ("image", 1023),              // IMAGE
    ("imagecolor", 1024),         // IMAGECOLOR
    ("imagemode", 1138),          // IMAGEMODE
    ("imagepath", 1025),          // IMAGEPATH
    ("imagetype", 1113),          // IMAGETYPE
    ("imageurl", 1026),           // IMAGEURL
    ("inches", 0),                // MS_INCHES
    ("index", 1027),              // INDEX
    ("initialgap", 1229),         // INITIALGAP
    ("intervals", 1029),          // INTERVALS
    ("items", 1206),              // ITEMS
    ("join", 1030),               // JOIN
    ("kerneldensity", 17),        // MS_KERNELDENSITY
    ("keyimage", 1156),           // KEYIMAGE
    ("keysize", 1031),            // KEYSIZE
    ("keyspacing", 1032),         // KEYSPACING
    ("kilometers", 4),            // MS_KILOMETERS
    ("label", 1033),              // LABEL
    ("labelcache", 1035),         // LABELCACHE
    ("labelformat", 1148),        // LABELFORMAT
    ("labelitem", 1036),          // LABELITEM
    ("labelmaxscale", 1037),      // LABELMAXSCALE
    ("labelmaxscaledenom", 1198), // LABELMAXSCALEDENOM
    ("labelminscale", 1038),      // LABELMINSCALE
    ("labelminscaledenom", 1199), // LABELMINSCALEDENOM
    ("labelrequires", 1119),      // LABELREQUIRES
    ("large", 13),                // MS_LARGE
    ("latlon", 1121),             // LATLON
    ("layer", 1040),              // LAYER
    ("lc", 108),                  // MS_LC
    ("leader", 1260),             // LEADER
    ("left", 1),                  // MS_ALIGN_LEFT
    ("legend", 1041),             // LEGEND
    ("legendformat", 1190),       // LEGENDFORMAT
    ("line", 1),                  // MS_LAYER_LINE
    ("linecap", 1127),            // LINECAP
    ("linejoin", 1128),           // LINEJOIN
    ("linejoinmaxsize", 1129),    // LINEJOINMAXSIZE
    ("ll", 104),                  // MS_LL
    ("lr", 102),                  // MS_LR
    ("map", 1044),                // MAP
    ("marker", 1131),             // MARKER
    ("markersize", 1132),         // MARKERSIZE
    ("mask", 1250),               // MASK
    ("maxarcs", 1142),            // MAXARCS
    ("maxboxsize", 1134),         // MAXBOXSIZE
    ("maxdistance", 1227),        // MAXDISTANCE
    ("maxfeatures", 1047),        // MAXFEATURES
    ("maxgeowidth", 1204),        // MAXGEOWIDTH
    ("maxinterval", 1144),        // MAXINTERVAL
    ("maxlength", 1210),          // MAXLENGTH
    ("maxoverlapangle", 1223),    // MAXOVERLAPANGLE
    ("maxscale", 1048),           // MAXSCALE
    ("maxscaledenom", 1196),      // MAXSCALEDENOM
    ("maxsize", 1049),            // MAXSIZE
    ("maxsubdivide", 1146),       // MAXSUBDIVIDE
    ("maxtemplate", 1050),        // MAXTEMPLATE
    ("maxwidth", 1165),           // MAXWIDTH
    ("medium", 10),               // MS_MEDIUM
    ("metadata", 1120),           // METADATA
    ("meters", 3),                // MS_METERS
    ("miles", 2),                 // MS_MILES
    ("mimetype", 1136),           // MIMETYPE
    ("minarcs", 1143),            // MINARCS
    ("minboxsize", 1133),         // MINBOXSIZE
    ("mindistance", 1051),        // MINDISTANCE
    ("minfeaturesize", 1053),     // MINFEATURESIZE
    ("mingeowidth", 1205),        // MINGEOWIDTH
    ("mininterval", 1145),        // MININTERVAL
    ("minscale", 1054),           // MINSCALE
    ("minscaledenom", 1197),      // MINSCALEDENOM
    ("minsize", 1055),            // MINSIZE
    ("minsubdivide", 1147),       // MINSUBDIVIDE
    ("mintemplate", 1056),        // MINTEMPLATE
    ("minwidth", 1164),           // MINWIDTH
    ("miter", 3),                 // MS_CJC_MITER
    ("multiple", 1),              // MS_MULTIPLE
    ("mysql", 2),                 // MS_DB_MYSQL
    ("name", 1057),               // NAME
    ("nauticalmiles", 8),         // MS_NAUTICALMILES
    ("none", 0),                  // MS_CJC_NONE
    ("normal", 0),                // MS_NORMAL
    ("off", 0),                   // MS_OFF
    ("offset", 1060),             // OFFSET
    ("offsite", 1061),            // OFFSITE
    ("ogr", 4),                   // MS_OGR
    ("on", 1),                    // MS_ON
    ("one-to-many", 1),           // MS_JOIN_ONE_TO_MANY
    ("one-to-one", 0),            // MS_JOIN_ONE_TO_ONE
    ("opacity", 1193),            // OPACITY
    ("oraclespatial", 8),         // MS_ORACLESPATIAL
    ("outlinecolor", 1062),       // OUTLINECOLOR
    ("outlinewidth", 1166),       // OUTLINEWIDTH
    ("outputformat", 1135),       // OUTPUTFORMAT
    ("partials", 1063),           // PARTIALS
    ("pattern", 1195),            // PATTERN
    ("percentages", 7),           // MS_PERCENTAGES
    ("pixels", 6),                // MS_PIXELS
    ("pixmap", 1003),             // MS_SYMBOL_PIXMAP
    ("plugin", 13),               // MS_PLUGIN
    ("point", 0),                 // MS_LAYER_POINT
    ("points", 1064),             // POINTS
    ("polaroffset", 1251),        // POLAROFFSET
    ("polygon", 2),               // MS_LAYER_POLYGON
    ("position", 1065),           // POSITION
    ("postgis", 6),               // MS_POSTGIS
    ("postgresql", 4),            // MS_DB_POSTGRES
    ("postlabelcache", 1098),     // POSTLABELCACHE
    ("priority", 1194),           // PRIORITY
    ("processing", 1153),         // PROCESSING
    ("projection", 1066),         // PROJECTION
    ("query", 5),                 // MS_LAYER_QUERY
    ("queryformat", 1157),        // QUERYFORMAT
    ("querymap", 1069),           // QUERYMAP
    ("rangeitem", 1173),          // RANGEITEM
    ("raster", 3),                // MS_LAYER_RASTER
    ("rasterlabel", 20),          // MS_RASTER_LABEL
    ("reference", 1070),          // REFERENCE
    ("region", 1228),             // REGION
    ("relativeto", 1192),         // RELATIVETO
    ("repeatdistance", 1222),     // REPEATDISTANCE
    ("requires", 1118),           // REQUIRES
    ("resolution", 1122),         // RESOLUTION
    ("right", 3),                 // MS_ALIGN_RIGHT
    ("round", 4),                 // MS_CJC_ROUND
    ("scale", 1071),              // SCALE
    ("scalebar", 1072),           // SCALEBAR
    ("scaledenom", 1201),         // SCALEDENOM
    ("scaletoken", 1270),         // SCALETOKEN
    ("selected", 2),              // MS_SELECTED
    ("shadowcolor", 1074),        // SHADOWCOLOR
    ("shadowsize", 1075),         // SHADOWSIZE
    ("shapepath", 1076),          // SHAPEPATH
    ("simple", 1000),             // MS_SYMBOL_SIMPLE
    ("single", 0),                // MS_SINGLE
    ("size", 1077),               // SIZE
    ("sizeunits", 1123),          // SIZEUNITS
    ("small", 7),                 // MS_SMALL
    ("square", 5),                // MS_CJC_SQUARE
    ("status", 1078),             // STATUS
    ("style", 1080),              // STYLE
    ("styleitem", 1124),          // STYLEITEM
    ("svg", 1006),                // MS_SYMBOL_SVG
    ("symbol", 1081),             // SYMBOL
    ("symbolscale", 1082),        // SYMBOLSCALE
    ("symbolscaledenom", 1200),   // SYMBOLSCALEDENOM
    ("symbolset", 1105),          // SYMBOLSET
    ("table", 1083),              // TABLE
    ("template", 1084),           // TEMPLATE
    ("temppath", 1224),           // TEMPPATH
    ("text", 1085),               // TEXT
    ("tileindex", 1088),          // TILEINDEX
    ("tileitem", 1089),           // TILEITEM
    ("tilesrs", 1272),            // TILESRS
    ("tiny", 5),                  // MS_TINY
    ("title", 1126),              // TITLE
    ("to", 1091),                 // TO
    ("tolerance", 1090),          // TOLERANCE
    ("toleranceunits", 1099),     // TOLERANCEUNITS
    ("transform", 1093),          // TRANSFORM
    ("transparent", 1092),        // TRANSPARENT
    ("triangle", 6),              // MS_CJC_TRIANGLE
    ("true", 1),                  // MS_TRUE
    ("truetype", 0),              // MS_TRUETYPE
    ("type", 1094),               // TYPE
    ("uc", 107),                  // MS_UC
    ("ul", 101),                  // MS_UL
    ("union", 14),                // MS_UNION
    ("units", 1095),              // UNITS
    ("ur", 103),                  // MS_UR
    ("utfdata", 1280),            // UTFDATA
    ("utfitem", 1281),            // UTFITEM
    ("uvraster", 15),             // MS_UVRASTER
    ("validation", 1212),         // VALIDATION
    ("values", 1271),             // VALUES
    ("vector", 1001),             // MS_SYMBOL_VECTOR
    ("web", 1096),                // WEB
    ("wfs", 9),                   // MS_WFS
    ("width", 1163),              // WIDTH
    ("wkt", 1180),                // WKT
    ("wms", 7),                   // MS_WMS
    ("wrap", 1097),               // WRAP
];

/// The token value MapServer's lexer gives `word` where it is a keyword, without regard to case;
/// `None` for a word it reads as a string.
pub(crate) fn token_value(word: &str) -> Option<i32> {
    let lower = word.to_ascii_lowercase();
    KEYWORDS
        .binary_search_by(|(keyword, _)| (*keyword).cmp(lower.as_str()))
        .ok()
        .map(|i| KEYWORDS[i].1)
}

/// Whether `word` stands for the keyword `name` where `getSymbol` reads it: the same word, or one
/// the lexer gives the same token value.
pub(crate) fn stands_for(word: &str, name: &str) -> bool {
    word.eq_ignore_ascii_case(name)
        || matches!((token_value(word), token_value(name)), (Some(a), Some(b)) if a == b)
}

/// The first of `names` that `word` stands for (see [`stands_for`]), as `getSymbol` returns the
/// value of the first of its choices the token equals.
pub(crate) fn chosen<'n>(word: &str, names: &[&'n str]) -> Option<&'n str> {
    names.iter().copied().find(|name| stands_for(word, name))
}

#[cfg(test)]
mod tests;
