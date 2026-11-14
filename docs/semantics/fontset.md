# `FONTSET`

**Where:** `MAP`. **IR:** the map's `fonts`, a name for each a resource path.

**Meaning.** The fonts labels name. The frontend reads the file through the resource loader,
resolved against the main mapfile's directory inside the base context (Q-8), within the size limit
of the mapfile (Q-9). As `msLoadFontSet` in `src/maplabel.c` reads it, through GDAL's
`CPLReadLineL`, which ends a line at a line feed, a carriage return, or the two in either order: one
font per line, its name up to the first space or tab, then its file trimmed of C's `isspace`; a line
that starts with `#`, a space, a carriage return, or a line break is skipped, and so is one without
a name (one that starts with a tab), without a file, or whose separator lies past the 63 bytes of
its `alias` buffer; a relative file is beside the font set. MapServer's font set is a hash table
that compares names without case (`msInsertHashTable` and `msLookupHashTable`, `src/maphash.c`): a
later line of a name in any case gives it its file and keeps the name as first written, so the IR's
`fonts` holds no two names that differ only in case, and a name is looked up without case. A style
document's reader leaves out a name that matches one written before it without regard to case, an
exact duplicate among them, with the warning `font-name-twin` naming both, and uses the first
written, as the frontend keeps a name as first written. The lenient read of a newer minor reads the
fonts again from the text for that order. The frontend checks a label's `FONT` against it so: each
entry of the list as `get_face_for_run` in `src/textlayout.c` walks it, which takes no entry after a
trailing comma, and an entry with a script's prefix, `ar:font`, after its first colon and then as
written, as `check_single_font` looks it up. A file outside the base context is a warning, and that
font is left out: a font is needed only when it is drawn. Font files are not read at load.
