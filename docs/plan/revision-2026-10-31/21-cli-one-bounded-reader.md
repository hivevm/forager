# Ticket: Read every file the CLI is handed through one bounded reader

## What to build

The CLI has two bounded readers: `read_bounded` in `crates/cli/src/files.rs`, for the base context,
refuses a file that is not regular before it opens it, because opening a pipe or a device may wait
or never end; `read_bounded` in `crates/cli/src/lib.rs`, for `--source` and `--epsg`, opens and
takes, and its comment says that a device or a pipe cannot exhaust memory, while `--source` naming a
FIFO blocks the CLI forever. One reader, the stricter, for both.

## Acceptance criteria

- [ ] Given `--source layer=<fifo>`, when `forager render` runs, then it fails at once with the
  diagnostic of a file that is not regular.
- [ ] Given `--epsg` of a file past its limit, when it runs, then it fails with the limit's
  diagnostic, as today.

## Taken over from `docs/plan/revision-2026-10-24/31-base-context-hardening.md`

Both harden how the CLI reads the files it is handed; the remaining points of 10-24/31 go here
(triage of 2026-11-07, on the maintainer's word).

Of the four points of the port review, two remain (triage of 2026-11-07): the URL rule is already
pinned (`crates/cli/src/files.rs`), and no CI job runs on Windows, so its point waits for one.

- **TOCTOU, low:** the CLI's loader canonicalizes a path, checks it against the base context,
  then opens it by path; one who can write into the base context during a load can swap a
  directory for a symlink, or a file for a FIFO, in between, and the FIFO blocks `open(2)`
  before the check on the opened file. Opening with `O_NOFOLLOW` walking or by `openat`, and
  with `O_NONBLOCK` on unix, closes it; `O_NONBLOCK` needs `libc`, a new dependency
  ([ADR-0012](../../adr/0012-crate-supply-chain.md)).
- **Budget per file, info:** each file may be read up to `MAX_TOTAL_BYTES + 1`, not what remains
  of the bound, so the peak is about twice the bound.

Its criteria, now this ticket's as well:

- [ ] Given a FIFO swapped into the base context, or passed as a file, when the CLI loads it,
  then the load fails with a diagnostic and does not block.
- [ ] Given files whose sizes sum past the total bound, when they are loaded, then no single read
  reserves more than what remains of the bound.

## Read first

- `read_bounded` in `crates/cli/src/files.rs` and `crates/cli/src/lib.rs`; the points taken over
  from revision-2026-10-24/31 above.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is a resource limit and the reading of the host's files, which ADR-0007 keeps
with the maintainer.

## Maintainer's decision (2026-11-07)

The minimal form: the reader checks on the handle it opened that the file is a regular one, and
limits each read to what remains of the total bound; the window between the check and the open
that remains is documented as a residual risk. No dependency is added. The point on Windows waits
until a Windows job runs on CI.
