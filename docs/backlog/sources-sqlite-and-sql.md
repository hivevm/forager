# A source for SQLite and GeoPackage, and for an SQL database with a spatial extension

**Observed:** the maintainer's function matrix against MapServer 8.6.6 (2026-11-14) excludes OGR,
PostGIS, Oracle Spatial, and plugins as sources, but keeps two open: a source for SQLite and
GeoPackage, and one for a generic SQL database with a spatial extension.

**Where:** the Non-Goals of `docs/SPECIFICATION.md` name the excluded sources; these two are not
named, and not decided.

**What a decision weighs:** a C SQLite links C code, which Q-1 forbids in the browser, so a
GeoPackage source is pure Rust or lives on the server path only. An SQL database is reached from the
server path only, and its credentials stay with the host (Q-7). How a mapfile names either source
is open: MapServer reaches GeoPackage through `OGR` and PostgreSQL through `POSTGIS`, both excluded.
