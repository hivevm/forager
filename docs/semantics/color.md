# `COLOR` and `OUTLINECOLOR`

**Where:** `STYLE`; `COLOR` also as the map's `IMAGECOLOR`. **IR:** the style's `color` and
`outline_color`, a value `#rrggbb` or `#rrggbbaa`, or a [binding](binding.md); the map's
`image_color`, a value only.

**Meaning.** The fill or stroke colour, and the outline colour of a polygon. As `loadColor` in
`src/mapfile.c` reads them:

- A string: `#rrggbb` or `#rrggbbaa`, case-insensitive; the alpha is 255 where it is not given.
  Any other string, a bare word among them, is a load error. MapServer takes a `#` and six or
  eight characters of any kind, each pair read with `msHexToInt` (`src/mapstring.cpp`), which
  does not check its digits; the frontend refuses a character that is no hex digit.
- Three numbers, each from −255 to 255 — checked before it is truncated to an integer, so 255.9
  is an error and 254.9 is 254. In a style or a label, a channel of −1 means no colour:
  MapServer's `MS_VALID_COLOR` (`src/mapserver.h`) refuses a colour with any channel −1.
  MapServer leaves another negative channel as it is, and both of its renderers store a channel
  in a byte — `rb_cmap` in `src/mapdrawgdal.c` is `unsigned char`, AGG's `rgba8` takes
  `unsigned` and keeps an `int8u` — so it draws as the channel modulo 256: −255 is 1, −2 is 254.
  The frontend stores that byte.
- `IMAGECOLOR` is drawn as the background without `MS_VALID_COLOR`: `msPrepareImage`
  (`src/mapdraw.c`) hands it to the renderer unchecked, so −1 is a channel there like any other,
  the byte 255. MapServer also sets its alpha to 255. The frontend deliberately keeps the alpha of
  `#rrggbbaa`, so that a map can ask for a translucent background; three numbers and `#rrggbb`
  stay opaque, as in MapServer.
- `[name]` binds the colour in a style; `IMAGECOLOR` takes no binding, and one is a load error.
