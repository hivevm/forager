# Shapefiles

**Where:** a layer whose `DATA` names an ESRI shapefile. **Adapter:** `adapter-shapefile`, a
feature source ([ADR-0024](../adr/0024-shapefile-reader.md)).

**Path.** A layer without `CONNECTIONTYPE` is local, and its `DATA` lies beneath the map's last
`SHAPEPATH`, wherever it stands, against the main mapfile's directory, as `msBuildPath3` in
`msSHPLayerOpen` joins them; an absolute `DATA`, a drive's included, ignores `SHAPEPATH`
(`msBuildPath`). Where that shapefile does not open, MapServer tries `DATA` beside the mapfile,
without `SHAPEPATH`; the frontend keeps one path and the adapter does not try the second, a
departure no case of the suite meets. The frontend stores the joined path in the source's `data`,
normalized and relative to the base context. A path that leaves the base context, by `..` or as an
absolute path, is the warning `data-refused`, and the layer keeps no data, so the engine skips it
(Q-8, G-8; `a_local_layers_data_lies_beneath_shapepath_in_the_base_context`,
`a_data_path_that_leaves_the_base_context_is_refused_and_the_layer_keeps_none`). The CLI binds a
local layer to this adapter where the base context holds its `.shp` or `.SHP`, reading each file
within the data-file limit of Q-9, and only a layer whose features the engine reads, `ON` or
`DEFAULT` and of a `TYPE` it draws: a layer `OFF` or a raster opens no file. Layers whose `DATA`
names one shapefile, cut as `msSHPOpen` cuts it, share the source the CLI opened once. A layer
whose file is missing stays unbound, as the browser bindings leave it, and a `--source` for the
layer wins (`crates/cli/tests/shapefile.rs`). A file the CLI may not read, missing or behind a
link out of the base context, is one and the same error, so that the error tells nothing of the
host.

**Meaning.** The features of a shapefile are the records MapServer reads from its `.shp`, `.shx`,
and `.dbf`, as `src/mapshape.c` and `src/mapxbase.c` read them. Each rule names the function it
follows and the test of `crates/adapter-shapefile/tests/read.rs` that holds it; the memory rules
are held by `memory_follows_the_files_size` in `tests/memory.rs`.

**Files.** `msShapefileOpen` and `msSHPOpen` cut the name at its last `.` after the last
separator, a leading `.` included, and ask the resource loader for `<name>.shp`, `<name>.shx`, and
`<name>.dbf`, each as `<name>.SHP`, `<name>.SHX`, `<name>.DBF` where the lower-case name cannot be
loaded, the `.dbf` in `msDBFOpen`; `data/roads.v2` asks for `data/roads.shp`. Only the extension
changes case, never the name: `DATA "poly"` does not find `POLY.SHP`. A file that cannot be
loaded under either name is an error
(`a_missing_file_is_an_error_and_upper_case_names_are_tried`). Neither `.qix`, `.sbn`,
`.cpg`, nor `.prj` is asked for.

**Headers** (`msSHPOpenVirtualFile`, `msDBFOpenVirtualFile`). An error opens no layer:

- The `.shp` shorter than 100 bytes is an error. Nothing else of it is read: neither its file code,
  its file length, nor its shape type.
- The `.shx` shorter than 100 bytes, or whose first four bytes are neither `0x0000270a` nor
  `0x0000270d`, is an error. Its file length, a big-endian `int` of 16-bit words at byte 24, gives
  the record count, `(words − 50) / 4`; under 50 words, or a count past 256,000,000, is an error.
  The file's shape type is byte 32 of the `.shx`, one byte; the bounds of the file are the four
  little-endian doubles at byte 36 of the `.shx`
  (`a_shp_or_shx_header_mapserver_refuses_is_an_error`,
  `what_mapserver_does_not_check_in_the_headers_opens`,
  `the_shape_type_is_the_one_byte_of_the_shx_header`).
- The `.dbf` shorter than 32 bytes, with a header length (bytes 8–9) of 32 or less, or shorter than
  that header length, is an error. Every whole 32 bytes past the first 32 of the header describe a
  field: an `N` or `F` field takes byte 16 as its size and byte 17 as its decimals, any other type
  bytes 16–17 as its size. Fields may be zero wide. The record starts with a deletion byte, so
  fields that end past the record length (bytes 10–11), counted from 1, are an error. The version,
  the update date, the record count, and the language driver are not checked
  (`a_dbf_header_mapserver_refuses_is_an_error`, `a_dbf_header_mapserver_accepts_opens`). The
  `.dbf` is checked and nothing of it reaches the port yet.

**The index** (`msSHXLoadPage`, `msSHXReadOffset`, `msSHXReadSize`). Entry *i* of the `.shx` is two
big-endian `int`s at byte 100 + 8*i*, the record's offset and content length in 16-bit words; each
is doubled into bytes where it is above 0 and below `INT_MAX / 2`, and is 0 otherwise. The entries
are read in pages of 1024: a page the file cuts short is lost as a whole, its entries all 0, so
entries the file holds are lost with it, and so is every page after it
(`a_shx_page_the_file_cuts_short_is_lost_as_a_whole`).

**Selection** (`msShapefileWhichShapes`, `msRectOverlap`, `msRectContained` in `src/mapsearch.c`).
A query's extent that does not overlap the file's bounds selects no record; one that contains them
selects every record, and the records' own boxes are not read; one across their edge selects each
record whose stored box overlaps it, edges that touch overlapping. A box is the four doubles at
byte 12 of the record (`msSHPReadBounds`); in a file of points (types 1, 11, 21) it is the point
itself, the two doubles at byte 12. Only a comparison that holds separates two boxes, so a NaN
anywhere but the least x of a record's box overlaps; the least x is checked only in files other
than points, so a point whose x is not a number overlaps too. A record whose box cannot be read is
not selected (`an_extent_outside_the_file_bounds_selects_nothing`,
`an_extent_that_contains_the_file_bounds_selects_every_record`,
`an_extent_across_the_file_bounds_selects_by_each_records_box`,
`a_point_file_is_selected_by_the_point_itself`, `a_point_whose_x_is_not_a_number_is_selected`,
`a_nan_in_a_box_beyond_its_least_x_compares_as_overlapping`).

**Records** (`msSHPReadShape`, `msSHPLayerNextShape`). A selected record is read as a shape or as
no shape; no shape is skipped and the next record read. A feature's id is its record's number in
the `.shx`, from 0. The record's 8-byte header and its own shape type are never read: the file's
shape type decides (`the_record_header_and_type_are_not_read`). The size below is the content
length of the `.shx` entry, which does not count the record header; the offsets are into the
content.

- A size under 4 is no shape; a size of exactly 4 is a null shape, no shape too. An offset of 0 is
  no shape, and so is a record whose header and content run past the end of the `.shp`
  (`an_index_entry_without_a_record_in_the_shp_is_no_shape`).
- Content too short for its type is no shape (`a_record_too_short_for_its_type_is_no_shape`).
- **Point** (1, 11, 21): content under 20 bytes is no shape; otherwise the two doubles at content
  byte 4 are a `Point`. A height or measure is not read.
- **Multipoint** (8, 18, 28): content under 40 bytes is no shape; the count at content byte 36
  below 0 or past 50,000,000 is no shape, and so is content under `40 + 16 × points`, and for 18
  and 28 under `56 + 24 × points` — a range and one value per point
  (`a_multipoint_with_a_measure_or_height_needs_its_values_too`). Otherwise the points are a
  `MultiPoint`.
- **Arc** (3, 13, 23) and **polygon** (5, 15, 25): content under 44 bytes is no shape; the parts at
  content byte 36 and the points at byte 40, either below 0, more than 10,000,000 parts, or more
  than 50,000,000 points, are no shape (`a_count_below_zero_or_past_the_caps_is_no_shape`), and so
  is content under `44 + 4 × parts + 16 × points`
  (`counts_that_need_more_bytes_than_the_record_holds_are_no_shape`). A height or measure is not
  required, and not read. A part ends where the next starts, the last at the point count; a part
  that starts below 0, ends past the points, or holds none makes the record no shape
  (`a_part_that_starts_below_zero_ends_past_the_points_or_holds_none_is_no_shape`). Each part
  takes its number of points in sequence from the record's first point, not from the point its
  start names (`points_are_taken_in_sequence_from_the_first_whatever_a_part_starts_at`). An arc of
  one part is a `LineString`, of more a `MultiLineString`; a polygon is one `Polygon` of all its
  parts as rings, in order, which the renderer fills even-odd. A record of no parts is a shape
  without lines, delivered as an empty geometry, as a multipoint of no points is
  (`an_arc_or_polygon_of_no_parts_is_a_shape_without_lines`).
- Every other shape type reads every record as no shape
  (`a_shape_type_mapserver_does_not_read_yields_no_feature`).

The reading of each shape type is held by
`every_shape_type_mapserver_reads_is_read_as_its_geometry`; no truncated or mutated file
panics (`no_mutation_of_a_shapefile_panics`).

**Departures from MapServer** (ADR-0024):

- **A box that cannot be read.** Where the extent crosses the file's bounds and a record's box
  cannot be read — a size of 0 to 3, an offset of 0 or past the `.shp`, a box cut short by the end
  of the file, a least x that is not a number — MapServer fails the whole layer; the adapter reads
  that record as no shape and reads on, so that one broken entry does not hide a layer
  (`a_record_whose_box_cannot_be_read_is_no_shape_and_the_layer_reads_on`). MapServer skips a
  null shape of size 4 there, and so does the adapter.
- **The byte total.** The source returns a layer's features at once, where MapServer draws and
  frees one shape at a time. So the records one query reads from a `.shp`, their headers included,
  add up to no more bytes than the `.shp` holds, and a record past that total is no shape: a
  `.shx` that names one record twice reads it once when the record is larger than the rest of the
  file, where MapServer draws it twice. A reordered `.shx` reads as MapServer reads it
  (`records_past_the_byte_total_of_the_shp_are_no_shape`).
- **Memory.** Nothing is allocated from a count or size the files state before it is checked
  against the bytes present: not the index arrays MapServer sizes by the record count, not a
  record's buffer by its `.shx` size, nor its parts before the size formula. A query's peak memory
  stays under four times the files' size plus one mebibyte (`memory_follows_the_files_size`).
- **No spatial index.** MapServer selects through a `.qix` beside the file where there is one
  (`msSearchDiskTree`), and filters its result by the records' boxes again, keeping a record whose
  box cannot be read (`msFilterTreeSearch` in `src/maptree.c`). For an index that agrees with the
  records, reading every box selects the same records; a stale index selects what it holds in
  MapServer. The index files are out of the ADR's scope.
