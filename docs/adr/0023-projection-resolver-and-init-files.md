# ADR-0023: `init=` projections are resolved through a resolver port the host passes in, and from an init file the mapfile names by a path inside the base context

- **Status:** 🟡 proposed
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/ports`, `crates/mapfile`, `crates/adapter-epsg`, and the hosts `crates/cli` and `crates/bindings-wasm`
- **Supersedes:** [ADR-0018](0018-projection-resolver.md)

## Context

The core accepts projections only as proj4 strings, and an EPSG database is not part of the core
(*Constraints*, *Non-Goals*). Mapfiles often name their projection as `init=epsg:4326`: in the
msautotest suite of MapServer 8.6.6, 180 of 503 mapfiles fail to load for this reason alone, across
12 EPSG codes, 151 of them for 4326. [ADR-0018](0018-projection-resolver.md) decided an optional
resolver the frontend is given; without one, `init=` stays a load error.

The definitions behind EPSG codes are the EPSG Geodetic Parameter Dataset of IOGP. Its terms of
use permit extracting subsets and including them in commercial packages, but require IOGP's
ownership to be acknowledged and forbid distributing the dataset for profit — terms that are not
`MIT OR Apache-2.0`, under which Forager is released (Q-6). Some public proj4 renderings of EPSG
codes also name grid files (`+nadgrids=…tif`), which a core without I/O cannot read.

Mapfiles also name init files of their own. MapServer hands `init=<file>:<code>` to PROJ unchanged
(`msProcessProjection` in `src/mapproject.c`), and PROJ opens a name that begins with `/`, `./`,
or `../` as a path from the working directory, and searches any other name in its search paths,
`PROJ_DATA` first, before it tries it from the working directory (`pj_open_lib_internal` in
`src/filemanager.cpp`). msautotest runs each mapfile from its own directory: 75 of its `init=`
parameters name the suite's file `wxs/data/epsg2` by a path (`./data/epsg2:42304`,
`../wxs/data/epsg2:42304`, `data/epsg2:42304`), one names it bare (`epsg2:42304`) and finds it
through `PROJ_DATA`. Under ADR-0018 the path became the resolver's authority, which no resolver
knows, so 39 mapfiles failed to load once layer projections were carried. The file is in the
format of PROJ's former `epsg` init file, which the host table already uses. The mapfile frontend
reads files the mapfile names, `INCLUDE`, `FONTSET`, and `SYMBOLSET`, through the resource loader
against the main mapfile's directory inside the base context (Q-8), and may depend only on the IR
and the ports ([ADR-0009](0009-ports-and-adapters-crates.md)).

## Decision

We will resolve `init=<authority>:<code>` in the mapfile frontend through a `ProjectionResolver`
port — a synchronous lookup from authority and code to a proj4 string — that the host passes in,
with `adapter-epsg` building in only EPSG:4326, EPSG:3857, and the WGS84 UTM zones (EPSG:32601–32660
and 32701–32760, derived from the zone number) and taking every other code from a table the host
supplies in the format of PROJ's former `epsg` init file (`<code> +proj=… <>`), and an `init=` whose
file name holds a `/` from that file, which the frontend reads through the resource loader against
the main mapfile's directory inside the base context and parses itself.

- The CLI accepts the table with `--epsg <file>`; the browser bindings accept its text as an
  optional argument of `render`.
- Without a resolver, or for a code the resolver does not know, `init=` remains a load error that
  names the code.
- A definition that names a grid file is refused with a diagnostic, by the adapter and by the
  frontend alike, since the core cannot read one.
- An init file is found as `INCLUDE` and `FONTSET` are: a relative name resolves against the main
  mapfile's directory, `..` included while the path stays inside the base context; an absolute
  name, or one outside the base context, is a load error naming it (Q-8). Its text counts toward
  the size limit of the mapfile (Q-9), and each file is read once per load. A code the file does
  not hold is the load error `projection-unresolved`, naming the file and the code.
- A file name without a `/`, such as `epsg2`, is an authority like `epsg` and goes to the resolver.

**Out of scope:** reprojection; a search path for init files named without a `/`, as PROJ's
`PROJ_DATA` or a mapfile's `CONFIG "PROJ_DATA"` gives one; other authorities than EPSG in an
adapter, which the port allows and none provides yet.

## Alternatives considered

- **A curated subset of the EPSG dataset in the repository** — the simplest to use, as PROJ and
  proj4js do, but a file under IOGP's terms in a repository released as `MIT OR Apache-2.0`.
- **No built-in definitions at all** — the cleanest licence, but the commonest codes would need a
  file for every use, the browser example included.
- **An asynchronous resolver** — would let a host fetch definitions on demand; the frontend's
  translation is synchronous, and a host can load its table before it loads the mapfile.
- **Only `/`, `./`, and `../` as paths, as PROJ takes them** — PROJ's own rule for fixed paths,
  but `data/epsg2`, which PROJ finds from the working directory after its search paths, would go
  to a resolver that does not know it, and two msautotest mapfiles would stay load errors.
- **Init file names against the root of the base context** — independent of where the mapfile
  lies, but unlike MapServer, which runs from the mapfile's directory in msautotest, and the
  suite's `./data/epsg2` would not be found.
- **The port resolves from the file's text** (`resolve_in(text, code)`, parsed by `adapter-epsg`)
  — one parser for the format, but an init file would need a resolver to load at all, and the
  port's public interface would grow.
- **The table parser in a crate of its own**, used by the frontend and the adapter — one parser,
  but a new edge in the dependency rule of ADR-0009 for some thirty lines.
- **A search path for bare names now** (`--proj-data <dir>` in the CLI) — would load the last
  msautotest case without a stand-in, at the cost of a second base for resource paths; left for
  when a host needs it.

## Sources / Prior art

- EPSG Dataset terms of use, <https://epsg.org/terms-of-use.html>, read 2026-10-24.
- The msautotest survey of 2026-10-24 (MapServer `rel-8-6-6`): the counts above, and the `init=`
  parameters of the suite, counted 2026-10-24.
- MapServer `rel-8-6-6`, `msProcessProjection` in `src/mapproject.c`.
- PROJ 9.5.0, `pj_open_lib_internal` and `is_rel_or_absolute_filename` in `src/filemanager.cpp`,
  <https://github.com/OSGeo/PROJ/blob/9.5.0/src/filemanager.cpp>, read 2026-10-24.
- proj4js and PROJ, which ship EPSG definitions with an acknowledgment of IOGP.

## Consequences

- Positive: 4326, 3857, and WGS84 UTM — enough for 160 of the 180 msautotest mapfiles that fail
  on `init=` — load without any file; the repository holds no EPSG data and stays under its
  licence. A mapfile that brings its own init file loads without a resolver, as it does in
  MapServer, and the msautotest suite needs a stand-in only for the one bare name.
- Negative / trade-offs: other codes need a table from the host; whoever supplies it is bound by
  the EPSG terms. Definitions without their datum grids are less accurate than PROJ's (G-11). The
  frontend and the adapter each parse the init file format.
- Follow-ups: a search path for init files named without a `/`; a test of G-11 that compares the
  built-in definitions with PROJ.

## Enforcement

Tests of `adapter-epsg` resolve 4326, 3857, and UTM zones and read a table file; tests of the
mapfile frontend load `init=epsg:` with and without a resolver, an init file named by a path
inside the base context, one outside it, and a code it does not hold; the msautotest test of G-1
loads every mapfile whose init file is named by a path through the CLI without a stand-in. All
cite this ADR with `Verifies:` once it is accepted.
