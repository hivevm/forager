# ADR-0018: `init=` projections are resolved through a resolver port the host passes in, with a minimal built-in table and a host-supplied file for the rest

- **Status:** 🟡 proposed
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/ports`, `crates/mapfile`, `crates/adapter-epsg`, and the hosts `crates/cli` and `crates/bindings-wasm`

## Context

The core accepts projections only as proj4 strings, and an EPSG database is not part of the core
(*Constraints*, *Non-Goals*). Mapfiles often name their projection as `init=epsg:4326`: in the
msautotest suite of MapServer 8.6.6, 180 of 503 mapfiles fail to load for this reason alone, across
12 EPSG codes, 151 of them for 4326. The brief asks for an optional resolver the frontend is given;
without one, `init=` stays a load error.

The definitions behind EPSG codes are the EPSG Geodetic Parameter Dataset of IOGP. Its terms of
use permit extracting subsets and including them in commercial packages, but require IOGP's
ownership to be acknowledged and forbid distributing the dataset for profit — terms that are not
`MIT OR Apache-2.0`, under which Forager is released (Q-6). Some public proj4 renderings of EPSG
codes also name grid files (`+nadgrids=…tif`), which a core without I/O cannot read.

## Decision

We will resolve `init=<authority>:<code>` in the mapfile frontend through a `ProjectionResolver`
port — a synchronous lookup from authority and code to a proj4 string — that the host passes in,
and provide `adapter-epsg`, which builds in only EPSG:4326, EPSG:3857, and the WGS84 UTM zones
(EPSG:32601–32660 and 32701–32760, derived from the zone number) and takes every other code from a
table the host supplies in the format of PROJ's former `epsg` init file (`<code> +proj=… <>`).

- The CLI accepts the table with `--epsg <file>`; the browser bindings accept its text as an
  optional argument of `render`.
- Without a resolver, or for a code the resolver does not know, `init=` remains a load error that
  names the code.
- A definition that names a grid file is refused by the adapter with a diagnostic, since the core
  cannot read one.

**Out of scope:** `PROJECTION AUTO`; layer projections and reprojection, which come with the
projection work; other authorities than EPSG, which the port allows and no adapter provides yet.

## Alternatives considered

- **A curated subset of the EPSG dataset in the repository** — the simplest to use, as PROJ and
  proj4js do, but a file under IOGP's terms in a repository released as `MIT OR Apache-2.0`.
- **No built-in definitions at all** — the cleanest licence, but the commonest codes would need a
  file for every use, the browser example included.
- **An asynchronous resolver** — would let a host fetch definitions on demand; the frontend's
  translation is synchronous, and a host can load its table before it loads the mapfile.

## Sources / Prior art

- EPSG Dataset terms of use, <https://epsg.org/terms-of-use.html>, read 2026-10-24.
- The msautotest survey of 2026-10-24 (MapServer `rel-8-6-6`): the counts above.
- proj4js and PROJ, which ship EPSG definitions with an acknowledgment of IOGP.

## Consequences

- Positive: 4326, 3857, and WGS84 UTM — enough for 160 of the 180 msautotest mapfiles that fail
  on `init=` — load without any file; the repository holds no EPSG data and stays under its licence.
- Negative / trade-offs: other codes need a table from the host; whoever supplies it is bound by
  the EPSG terms. Definitions without their datum grids are less accurate than PROJ's (G-11).
- Follow-ups: the resolver's use for layer projections and reprojection; a test of G-11 that
  compares the built-in definitions with PROJ.

## Enforcement

Tests of `adapter-epsg` resolve 4326, 3857, and UTM zones and read a table file; tests of the
mapfile frontend load `init=epsg:` with and without a resolver; all cite this ADR with `Verifies:`.
