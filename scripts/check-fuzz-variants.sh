#!/usr/bin/env bash
#
# Check: the render fuzz target builds every variant of the enums of the style IR it fuzzes the
# renderer through, each node of the transform tree and each kind of symbol (Q-3).
#
# The fuzz seed convention of docs/CONVENTIONS.md did not stop three nodes of the transform tree
# from landing unfuzzed (ticket 01 of revision-2026-11-16). For the enums of ENUMS below, this
# check decides it. It reads each enum's variants from the file that declares it, and fails where:
#
#   1. fuzz/fuzz_targets/render.rs names no '<Enum>::<Variant>' outside a '//' comment, and
#      ALLOWED does not name the variant;
#   2. ALLOWED names a variant the target names, one the enum does not declare, or one of an enum
#      ENUMS does not list;
#   3. the file declares no 'pub enum <Enum> {', or no variant of it is read;
#   4. the tree has no fuzz/fuzz_targets/render.rs.
#
# The variants are read as rustfmt lays an enum out, and check-rust.sh holds the crates to cargo
# fmt: a line of four spaces and a capitalised name, at the enum's own depth of braces, so that a
# struct variant's fields are not read; a brace or a name in a comment is not counted. That the
# target draws what it builds, and that its seeds reach it, no check decides; the convention does.
#
# Pure bash + grep, sed, and awk, and _contains of scripts/lib/check.sh. Usage:
#     scripts/check-fuzz-variants.sh [repository root]
# The root defaults to this repository; scripts/test-check-fuzz-variants.sh passes fixtures.
# Exit code 0 when the target names every variant the allow-list leaves out, 1 otherwise, 2 on a
# root that is no directory.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/check.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/check.sh"

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Fuzz variants check: '${1:-}' is not a directory." >&2; exit 2; }
TARGET=fuzz/fuzz_targets/render.rs

# '<file> <enum>': each enum whose variants the target must name, and the file that declares it.
ENUMS=(
  "crates/style-ir/src/transform.rs TransformShape"
  "crates/style-ir/src/transform.rs TransformNumber"
  "crates/style-ir/src/transform.rs NumberOp"
  "crates/style-ir/src/transform.rs Preprocessing"
  "crates/style-ir/src/model.rs SymbolKind"
)

# '<Enum>::<Variant> <reason>': each variant the target leaves out on purpose.
ALLOWED=(
  "SymbolKind::Simple a symbol of TYPE simple is a non-goal of docs/SPECIFICATION.md"
  "SymbolKind::Svg an SVG symbol is a non-goal of docs/SPECIFICATION.md"
)

failures=0
fail() { echo "ERROR: $*" >&2; failures=$((failures + 1)); }

if [[ ! -f "$ROOT/$TARGET" ]]; then
  echo "ERROR: there is no $TARGET under $ROOT to hold the variants to" >&2
  echo "Fuzz variants check FAILED." >&2
  exit 1
fi
# The target without its comments, so that a name in one is not taken for the target's.
target="$(sed 's://.*$::' "$ROOT/$TARGET")"

# allowed <Enum>::<Variant> — the allow-list's reason for the variant, or nothing.
allowed() {
  local entry
  for entry in "${ALLOWED[@]}"; do
    [[ "${entry%% *}" == "$1" ]] && { echo "${entry#* }"; return; }
  done
}

# variants <file> <enum> — the enum's variants, one a line.
variants() {
  awk -v header="pub enum $2 {" '
    !inside && $0 == header { inside = 1; depth = 1; next }
    inside {
      line = $0
      sub(/\/\/.*/, "", line)
      if (depth == 1 && match(line, /^    [A-Z][A-Za-z0-9_]*/)) print substr(line, 5, RLENGTH - 4)
      depth += gsub(/\{/, "{", line) - gsub(/\}/, "}", line)
      if (depth <= 0) exit
    }' "$1"
}

enums=0 count=0 allowed_count=0
# The enums ENUMS lists, those whose variants were read, and the variants read, as
# '<Enum>::<Variant>'.
listed=() parsed=() declared=()
declare -A files=()
for pair in "${ENUMS[@]}"; do
  file="${pair% *}" enum="${pair#* }"
  listed+=("$enum")
  files[$enum]="$file"
  if ! grep -qxF "pub enum $enum {" "$ROOT/$file" 2>/dev/null; then
    fail "$file declares no 'pub enum $enum {'"
    continue
  fi
  mapfile -t names < <(variants "$ROOT/$file" "$enum")
  if ((${#names[@]} == 0)); then
    fail "no variant of $enum is read from $file; is it laid out as cargo fmt lays it out?"
    continue
  fi
  parsed+=("$enum")
  enums=$((enums + 1)) count=$((count + ${#names[@]}))
  echo "$enum: ${names[*]}"
  for name in "${names[@]}"; do
    declared+=("$enum::$name")
    reason="$(allowed "$enum::$name")"
    named=0
    grep -qE "(^|[^A-Za-z0-9_])$enum::$name([^A-Za-z0-9_]|$)" <<< "$target" && named=1
    if [[ -n "$reason" ]] && ((named)); then
      fail "$TARGET names $enum::$name, which the allow-list leaves out; drop it from ALLOWED"
    elif [[ -n "$reason" ]]; then
      allowed_count=$((allowed_count + 1))
      echo "  $enum::$name is allowed: $reason"
    elif ((!named)); then
      fail "$TARGET never names $enum::$name; build it in the target with a seed that reaches" \
        "it, or name it in ALLOWED with its reason"
    fi
  done
done

# An enum whose variants were not read has failed above already.
for entry in "${ALLOWED[@]}"; do
  variant="${entry%% *}"
  enum="${variant%%::*}"
  if ! _contains "$enum" "${listed[@]}"; then
    fail "the allow-list names $variant, but ENUMS lists no $enum"
  elif _contains "$enum" "${parsed[@]}" && ! _contains "$variant" "${declared[@]}"; then
    fail "the allow-list names $variant, which ${files[$enum]} does not declare"
  fi
done

if ((failures)); then
  echo "Fuzz variants check FAILED ($failures problem(s) among $count variant(s))." >&2
  exit 1
fi
echo "Fuzz variants check passed ($enums enums, $count variants, $allowed_count allowed)."
