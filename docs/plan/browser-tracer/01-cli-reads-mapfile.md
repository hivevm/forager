# Ticket: The CLI reads a mapfile and prints its style IR

## What to build

`forager read <mapfile> [--format yaml|json]` reads a MapServer 8.x mapfile, resolves its
`INCLUDE`s inside the mapfile's directory, and prints the style IR and every diagnostic with its
source span; it exits non-zero when loading failed. This is the golden path: the workspace, the
toolchain scripts, the CI job, the Dev Container change of ADR-0010, and the dependency test of
ADR-0009 land with it.

## Acceptance criteria

- [ ] Given a mapfile with a map, a polygon layer, a class, and a style with colour, outline
      colour, and width, when the CLI reads it with `--format yaml`, then it prints a style
      document with `version: "1.0"` holding those values, and exits 0.
- [ ] Given the same mapfile, when it is read as YAML and as JSON, then both parse into equal style
      IR.
- [ ] Given a mapfile with a keyword not yet supported, when it is read, then a warning names the
      keyword and its line and column, and the exit code is 0.
- [ ] Given a mapfile with an unterminated block or string, when it is read, then an error
      diagnostic names the line and column, and the exit code is non-zero.
- [ ] Given an `INCLUDE` of a file beside the mapfile, when it is read, then the included content
      is part of the style IR and its spans name the included file.
- [ ] Given an `INCLUDE` with `..` escaping the mapfile's directory, or an absolute path, when it
      is read, then an error diagnostic refuses it and nothing outside is read (Q-8).
- [ ] Given an `INCLUDE` cycle or nesting deeper than the limit, when it is read, then an error
      diagnostic names it (Q-9).
- [ ] Given the workspace, when `scripts/check-rust.sh` runs, then fmt, clippy, tests, the
      `wasm32-unknown-unknown` build of `style-ir` and `mapfile`, and `cargo deny` pass, and the
      dependency test of ADR-0009 passes.

## Read first

- ADR-0008, ADR-0009, ADR-0010, ADR-0012; `docs/GLOSSARY.md` (style IR, style document,
  diagnostic, source span, mapfile frontend, resource loader, base context).
- MapServer 8.x mapfile lexer and parser (`maplexer.l`, `mapfile.c`) for the syntax.

## Blocked by

- None (can start immediately)

## Mode

with the human — it lays the golden path, decides the YAML serializer, and touches the resolution
of resource paths (Q-8), which ADR-0007 keeps with the human.
