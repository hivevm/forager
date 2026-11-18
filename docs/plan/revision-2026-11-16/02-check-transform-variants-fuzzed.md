# Ticket: A check that the render fuzz target builds every node of the transform tree

## What to build

The fuzz seed convention is prose, and it did not stop three nodes of the transform tree,
`simplify`, `simplifypt`, and `buffer`, from landing unfuzzed (ticket 01 of this revision). For
the transform tree, a script can decide it. A check lists each variant of `TransformShape` in
`crates/style-ir/src/transform.rs`. It fails where `fuzz/fuzz_targets/render.rs` never names
`TransformShape::<Variant>`. `scripts/check-all.sh` runs it, and CI runs it in the `sensors` job,
beside `scripts/check-guards.sh`. A variant left out on purpose is named in an allow-list in the
script with its reason.

If the check also fits other enums the renderer draws, such as `SymbolKind`, it takes them in the
same way. Otherwise the convention stays the rule for them.

## Acceptance criteria

- [ ] Given a fixture where the target names every variant, when the self-test runs the check,
  then it passes.
- [ ] Given a fixture where a variant is missing from the target, when the self-test runs the
  check, then it fails and names the variant.
- [ ] Given the tree after ticket 01, when `scripts/check-all.sh` runs, then the check passes.

## Read first

- Touches: `scripts/check-fuzz-variants.sh` (new), its self-test
  `scripts/test-check-fuzz-variants.sh`, `scripts/check-all.sh`, `.github/workflows/checks.yml`
  (the `sensors` job). `CONTRIBUTING.md` names the scripts by `scripts/check-*.sh` and
  `scripts/test-*.sh`, so a new check needs no line there (checked 2026-11-18).
- ADR-0007 (the checks `check-all.sh` runs); `scripts/lib/check.sh`, `scripts/lib/expect.sh`,
  `scripts/lib/selftest.sh`; `docs/CONVENTIONS.md` (the fuzz seed convention).

## Blocked by

- docs/plan/revision-2026-11-16/01-render-fuzz-fidelity-paths.md (the check fails before it)

## Mode

unattended: the self-test decides it, and a wrong result costs one revert. Stop when the variants
cannot be read without a Rust parser. Whether the check is worth it then is the maintainer's call.
