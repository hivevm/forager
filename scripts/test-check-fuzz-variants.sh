#!/usr/bin/env bash
#
# Self-test of scripts/check-fuzz-variants.sh. Verifies: Q-3
#
# Each case writes a fixture with the three files the check reads, the transform tree's enums in
# crates/style-ir/src/transform.rs, SymbolKind in crates/style-ir/src/model.rs, and what the
# render fuzz target builds, fuzz/fuzz_targets/render_input.rs, laid out as rustfmt lays them out;
# it changes one of them, runs the check against the fixture, and asserts the exit code and the
# line that names the variant or the problem. The fixture's SymbolKind holds the two variants the
# check's allow-list names, Simple and Svg, which its target does not name.
#
# Usage:
#     scripts/test-check-fuzz-variants.sh
# Exit code 0 when every case passes, 1 otherwise.
# Inputs: scripts/check-fuzz-variants.sh scripts/lib/

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"

CHECK="$ROOT/scripts/check-fuzz-variants.sh"

# The transform tree's enums: an enum before TransformShape whose variant is not one of its own,
# doc comments, attributes, a tuple variant broken over lines, as rustfmt breaks a long one, and
# a struct variant with a variant after it.
TRANSFORM='/// The tree of a GEOMTRANSFORM.
pub enum TransformTree {
    Drawn(TransformShape),
}

/// A shape the tree computes.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformShape {
    /// The shape of the feature, [shape] in MapServer.
    Shape,
    /// generalize(shape, tolerance).
    Generalize(
        Box<TransformShape>,
        TransformNumber,
    ),
    /// smoothsia(shape, size).
    SmoothSia {
        shape: Box<TransformShape>,
        preprocessing: Option<Preprocessing>,
    },
    /// buffer(shape, distance).
    Buffer(Box<TransformShape>, TransformNumber),
}

pub enum ShapeOp {
    Densify,
    Simplifypt,
}

pub enum TransformNumber {
    Number(f64),
    MapCellsize,
}

pub enum NumberOp {
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "^")]
    Pow,
}

pub enum Preprocessing {
    All,
    Angle,
}'

MODEL='#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Vector,
    Simple,
    Svg,
}'

# The target: every variant named, but the allow-listed ones.
TARGET='use forager_style_ir::{NumberOp, Preprocessing, ShapeOp, SymbolKind, TransformNumber, TransformShape};

fn build() {
    let mut shape = TransformShape::Shape;
    shape = TransformShape::Generalize(Box::new(shape), number());
    shape = TransformShape::SmoothSia {
        shape: Box::new(shape),
        preprocessing: Some(Preprocessing::All),
    };
    shape = TransformShape::Buffer(Box::new(shape), number());
    let _ = [ShapeOp::Densify, ShapeOp::Simplifypt];
    let _ = Preprocessing::Angle;
    let _ = [NumberOp::Add, NumberOp::Pow];
    let _ = [TransformNumber::Number(1.0), TransformNumber::MapCellsize];
    let _ = SymbolKind::Vector;
}'

# repo <name> [<sed script> <file>]... — a fixture with the three files above, printed as its
# path; each pair edits one of them, named by 'transform', 'model', or 'target', with sed.
repo() {
  local dir="$work/$1" file
  shift
  mkdir -p "$dir/crates/style-ir/src" "$dir/fuzz/fuzz_targets"
  printf '%s\n' "$TRANSFORM" > "$dir/crates/style-ir/src/transform.rs"
  printf '%s\n' "$MODEL" > "$dir/crates/style-ir/src/model.rs"
  printf '%s\n' "$TARGET" > "$dir/fuzz/fuzz_targets/render_input.rs"
  while (($# >= 2)); do
    case "$2" in
      transform) file="$dir/crates/style-ir/src/transform.rs" ;;
      model) file="$dir/crates/style-ir/src/model.rs" ;;
      target) file="$dir/fuzz/fuzz_targets/render_input.rs" ;;
    esac
    sed "$1" "$file" > "$file.tmp" && cat "$file.tmp" > "$file" && rm "$file.tmp"
    shift 2
  done
  echo "$dir"
}

# expect_check <name> <dir> <code> <needle> — run the check; it must exit with the code and its
# output contain the needle (expect, scripts/lib/expect.sh).
expect_check() { expect "$1" "$3" "$4" bash "$CHECK" "$2"; }

dir="$(repo named)"
expect_check "a target that names every variant passes" "$dir" 0 \
  "Fuzz variants check passed (6 enums, 15 variants, 2 allowed)."
expect_check "a passing run lists each enum's variants, and only them, deeper lines not read" \
  "$dir" 0 $'TransformShape: Shape Generalize SmoothSia Buffer\nShapeOp: Densify Simplifypt\nTransformNumber: Number MapCellsize\n'
expect_check "a passing run lists an allowed variant with its reason" "$dir" 0 \
  "SymbolKind::Svg is allowed:"

dir="$(repo missing '/TransformShape::Buffer/d' target)"
expect_check "a variant the target never names fails, naming the variant" "$dir" 1 \
  "fuzz/fuzz_targets/render_input.rs never names TransformShape::Buffer"

dir="$(repo added 's/^    MapCellsize,$/    Op(NumberOp),/' transform)"
expect_check "a variant new to an enum fails until the target names it" "$dir" 1 \
  "fuzz/fuzz_targets/render_input.rs never names TransformNumber::Op"

dir="$(repo symbol '/SymbolKind::Vector/d' target)"
expect_check "a symbol kind the target never names fails" "$dir" 1 \
  "fuzz/fuzz_targets/render_input.rs never names SymbolKind::Vector"

dir="$(repo comment 's|^    shape = TransformShape::Buffer|    // TransformShape::Buffer|' target)"
expect_check "a variant named only in a comment is not named" "$dir" 1 \
  "never names TransformShape::Buffer"

dir="$(repo prefix 's/TransformShape::Shape;/TransformShape::Shapes;/' target)"
expect_check "a name that begins with a variant's is not that variant" "$dir" 1 \
  "never names TransformShape::Shape"

dir="$(repo allowed-named 's/SymbolKind::Vector;/[SymbolKind::Vector, SymbolKind::Svg];/' target)"
expect_check "an allowed variant the target names fails, so the allow-list stays true" "$dir" 1 \
  "fuzz/fuzz_targets/render_input.rs names SymbolKind::Svg, which the allow-list leaves out"

dir="$(repo allowed-gone '/^    Svg,$/d' model)"
expect_check "an allowed variant the enum no longer has fails" "$dir" 1 \
  "the allow-list names SymbolKind::Svg, which crates/style-ir/src/model.rs does not declare"

dir="$(repo unread-allowed 's/^    \([A-Z][a-z]*\),$/  \1,/' model)"
REJECT="does not declare" expect_check \
  "an enum whose variants are not read fails once, not again for its allowed variants" "$dir" 1 \
  "no variant of SymbolKind is read from crates/style-ir/src/model.rs"

# The allow-list is the script's own: this case runs a copy whose ALLOWED names a variant of an
# enum ENUMS does not list.
dir="$(repo unlisted)"
awk '{ print } /^ALLOWED=\($/ { print "  \"Unlisted::Variant a reason\"" }' "$CHECK" \
  > "$dir/check.sh"
mkdir -p "$dir/scripts/lib" && cp "$ROOT/scripts/lib/check.sh" "$dir/scripts/lib/"
mv "$dir/check.sh" "$dir/scripts/check-fuzz-variants.sh"
expect "an allowed variant of an enum the check does not list fails" 1 \
  "the allow-list names Unlisted::Variant, but ENUMS lists no Unlisted" \
  bash "$dir/scripts/check-fuzz-variants.sh" "$dir"

dir="$(repo no-enum 's/^pub enum Preprocessing {$/pub enum Preprocess {/' transform)"
expect_check "an enum the file no longer declares fails" "$dir" 1 \
  "crates/style-ir/src/transform.rs declares no 'pub enum Preprocessing {'"

dir="$(repo unread 's/^    All,$/  All,/;s/^    Angle,$/  Angle,/' transform)"
expect_check "an enum of which no variant is read fails" "$dir" 1 \
  "no variant of Preprocessing is read from crates/style-ir/src/transform.rs"

dir="$(repo no-target)"
rm "$dir/fuzz/fuzz_targets/render_input.rs"
expect_check "a tree without the render target fails" "$dir" 1 \
  "there is no fuzz/fuzz_targets/render_input.rs"

expect_check "a root that is no directory is refused" "$work/absent" 2 "is not a directory"

summary "Fuzz variants check self-test"
