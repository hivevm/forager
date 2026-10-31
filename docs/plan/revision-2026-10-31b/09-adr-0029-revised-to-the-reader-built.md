# Ticket: Revise ADR-0029 to the reader and the records as built, before it is accepted

## What to build

[ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md) is proposed, implemented
by #137, and waits for the maintainer's acceptance. The implementation and its text have moved
apart, and the revision belongs in an ADR-only pull request
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 4):

- **The cases.** *The same render* says the noise measurement runs the reader "for every case in
  scope", and the Enforcement that it fails when the reader's image "of a case in scope" differs;
  the code runs it for the text cases only (`crates/cli/tests/msautotest.rs`).
- **The records.** The Decision says nothing of the records the comparison reads,
  `crates/cli/tests/conformance/labels/**/*.tsv`, of how one is taken (from the noise run's
  artifact `noise/labels/`), of the list's mark `labels-hold` in `cases.txt`, or of the noise run
  failing on a record that differs. Those are the decision's lasting parts.
- **Byte-exact against unpinned packages.** The reader writes boxes with `%.17g` and the noise run
  holds them byte for byte, while the runner's freetype and harfbuzz come unpinned from the Ubuntu
  archive (`scripts/build-mapserver.sh`); a package update that moves a box by 1e-12 fails the run.
  The push harness pairs within 1 px. Options for the maintainer: a tolerance in the noise run, a
  line in each record naming the package versions it was taken under, or taking the records again
  on drift as a rule. Unverified: whether the reader, compiled fresh against `gdal-config`, can
  drift in ABI from the cached `libmapserver`, which is keyed on the two scripts alone.
- **The patch rule.** The Enforcement's refusal of a patched checkout is a text match in
  `scripts/test-build-mapserver.sh`: a comment that says "patch" trips it, `perl -pi` or a
  redirect into the checkout pass it. It belongs under *Not mechanically decidable* or in wider
  patterns.
- **The priority.** The reader writes the priority and the layer's name; the comparison reads
  neither, so a label in another slot drawn at the same box matches (#137's doubt). Whether G-10's
  "the same labels" asks for the slot is the maintainer's; else the fields leave the record.
- **Stale.** The Follow-ups name "ticket 09", which has landed; the Enforcement's "short of the
  comparison ticket 09 builds" too; `scripts/check-notice.sh` reaches the reader only through
  `src/mapdraw.c` (ticket 08 of this revision); `scripts/check-all.sh` and `checks.yml` say of
  `test-build-mapserver.sh` only `ADR-0027`, where the script also cites ADR-0029.

Draft the revised Context, Decision, and Enforcement, each question above in the Context for the
maintainer, not answered.

## Acceptance criteria

- [ ] Given the revised ADR, when it is read beside `msautotest.rs`, `build-mapserver.sh`, and the
  records, then each clause says what the code does or names the question it leaves open.
- [ ] Given `scripts/check-docs.sh`, when it runs, then the ADR and its index row pass.

## Read first

- ADR-0029, [ADR-0027](../../adr/0027-mapserver-built-from-the-pin.md); #137's description and
  doubts (`gh pr view 137`); G-10 in `docs/SPECIFICATION.md`; the skill `propose-adr`.

## Blocked by

- None (can start immediately)

## Mode

with the human: ADRs are the maintainer's ([ADR-0007](../../adr/0007-harness.md)), and the
questions it raises are his to answer.
