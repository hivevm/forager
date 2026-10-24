# `FONTSET`

**Where:** `MAP`. **IR:** the map's `fonts`, a name for each a resource path.

**Meaning.** The fonts labels name. The frontend reads the file through the resource loader,
resolved against the main mapfile's directory inside the base context (Q-8), within the size
limit of the mapfile (Q-9). As `msLoadFontSet` in `src/maplabel.c` reads it: one font per line,
its name up to the first space or tab, then its file; a line that starts with `#`, a space, or a
line break is skipped; a relative file is beside the font set. A file outside the base context is
a warning, and that font is left out: a font is needed only when it is drawn. Font files are not
read at load.
