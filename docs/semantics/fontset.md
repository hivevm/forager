# `FONTSET`

**Where:** `MAP`. **IR:** the map's `fonts`, a name for each a resource path.

**Meaning.** The fonts labels name. The frontend reads the file through the resource loader,
resolved against the main mapfile's directory inside the base context (Q-8), within the size
limit of the mapfile (Q-9). As `msLoadFontSet` in `src/maplabel.c` reads it: one font per line,
its name up to the first space or tab, then its file trimmed of C's `isspace`; a line that starts
with `#`, a space, a carriage return, or a line break is skipped, and so is one without a name (one
that starts with a tab), without a file, or whose separator lies past the 63 bytes of its `alias`
buffer; a later line of a name replaces an earlier one; a relative file is beside the font set.
MapServer's font set is a hash table that compares names without case (`msLookupHashTable`,
`src/maphash.c`), and the frontend checks a label's `FONT` against it so; the IR's `fonts` keeps
each name as written. A file outside the base context is
a warning, and that font is left out: a font is needed only when it is drawn. Font files are not
read at load.
