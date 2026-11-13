#!/usr/bin/env bash
#
# Self-test of scripts/mutants.sh, without cargo-mutants.
#
# Each case copies the script into a throwaway git repository with a Rust file and a README, and
# runs it with a stand-in 'cargo' first on PATH that records its arguments and answers with the
# pinned version, the exit code, and the missed mutants a case asks for. The case asserts the
# exit code, a line of the output, and the arguments cargo-mutants would have been given. Without
# a base the arguments are those of the weekly run; with one, the diff of the Rust code from the
# merge base is handed over, or nothing runs when there is none.
#
# Usage:
#     scripts/test-mutants.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn the fixtures' git on its
# repository; they run on their own (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/scripts/mutants.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

# The stand-in cargo: 'mutants --version' prints the pinned version; any other call appends its
# arguments as one line to $FAKE_LOG, writes $FAKE_MISSED as the report's missed.txt when set,
# and exits with $FAKE_STATUS.
mkdir -p "$work/bin"
cat > "$work/bin/cargo" << 'EOF'
#!/usr/bin/env bash
if [[ "$*" == "mutants --version" ]]; then echo "cargo-mutants 27.1.0"; exit 0; fi
echo "$*" >> "$FAKE_LOG"
if [[ -n "${FAKE_MISSED:-}" ]]; then
  mkdir -p target/mutants/mutants.out
  printf '%s\n' "$FAKE_MISSED" > target/mutants/mutants.out/missed.txt
fi
exit "${FAKE_STATUS:-0}"
EOF
chmod +x "$work/bin/cargo"

WEEKLY="mutants --workspace --test-workspace=false --no-shuffle --output target/mutants"

# fixture <dir> — refuse an empty fixture path: 'git -C ""' would act on this repository.
fixture() { [[ -n "${1:-}" && -d "$1/.git" ]] || { echo "fixture: '$1' is no fixture repository" >&2; exit 1; }; }

# commit <dir> <message> — stage everything and commit with a fixed identity.
commit() {
  fixture "$1"
  git -C "${1:?}" add -A
  git -C "${1:?}" -c user.name=fixture -c user.email=fixture@example.invalid commit -q -m "$2"
}

# repo <name> — a repository with the script, a Rust file, and a README on 'main', checked out on
# the branch 'work', printed as its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/scripts" "$dir/src"
  git -C "${dir:?}" init -q -b main
  cp "$SCRIPT" "$dir/scripts/mutants.sh"
  printf 'target/\n' > "$dir/.gitignore"
  printf 'pub fn one() -> u8 {\n    1\n}\n' > "$dir/src/lib.rs"
  printf '# fixture\n' > "$dir/README.md"
  commit "$dir" "base"
  git -C "${dir:?}" checkout -q -b work
  echo "$dir"
}

# expect <exit code> <name> <dir> <needle> <cargo calls> [args...] — run the script in the
# repository; the output must contain the needle, and not $REJECT when that is set, and the
# recorded cargo calls, one per line, must equal the given ones exactly.
expect() {
  local want="$1" name="$2" dir="$3" needle="$4" calls="$5"
  shift 5
  local out rc log="$work/$name.log"
  fixture "$dir"
  : > "$log"
  out="$(cd "$dir" && PATH="$work/bin:$PATH" FAKE_LOG="$log" bash scripts/mutants.sh "$@" 2>&1)"; rc=$?
  if [[ $rc -eq "$want" && "$out" == *"$needle"* && "$(cat "$log")" == "$calls" ]] \
    && [[ -z "${REJECT:-}" || "$out" != *"$REJECT"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $want with \"$needle\"${REJECT:+ and without \"$REJECT\"}, got exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
  echo "    cargo calls expected: $calls" >&2
  while IFS= read -r l; do printf '    cargo calls got:      %s\n' "$l"; done < "$log" >&2
}

# --- without a base: the weekly run --------------------------------------------------------

d="$(repo weekly)"
expect 0 "no argument mutates the whole workspace" "$d" "" "$WEEKLY"
expect 0 "a shard is handed over" "$d" "" "$WEEKLY --shard 1/4" 1/4
FAKE_STATUS=2 FAKE_MISSED="src/lib.rs:2:5: replace one -> u8 with 0" \
  expect 0 "missed mutants are listed and the run passes" "$d" \
  "Mutants no test caught (1):"$'\n'"src/lib.rs:2:5: replace one -> u8 with 0" "$WEEKLY"
FAKE_STATUS=3 expect 0 "timed-out mutants are a finding, not a broken run" "$d" "" "$WEEKLY"
FAKE_STATUS=1 expect 1 "a broken run fails" "$d" "the run failed (exit 1)" "$WEEKLY"

# --- with a base: the review's run ---------------------------------------------------------

d="$(repo rust-changed)"
printf 'pub fn two(x: bool) -> u8 {\n    if x { 2 } else { 0 }\n}\n' >> "$d/src/lib.rs"
commit "$d" "a Rust change"
expect 0 "a Rust change is handed over as the diff" "$d" "" \
  "$WEEKLY --in-diff target/mutants.diff" --base main
if grep -qF '+pub fn two(x: bool) -> u8 {' "$d/target/mutants.diff" \
  && ! grep -qF 'README' "$d/target/mutants.diff"; then
  passed=$((passed + 1))
else
  failed=$((failed + 1))
  echo "FAIL: the diff holds the Rust change and nothing else" >&2
fi
expect 0 "--base= and a shard together" "$d" "" \
  "$WEEKLY --in-diff target/mutants.diff --shard 2/3" 2/3 --base=main
FAKE_STATUS=2 FAKE_MISSED="src/lib.rs:5:12: replace == with != in two" \
  expect 0 "a surviving mutant in the diff is listed and the run passes" "$d" \
  "src/lib.rs:5:12: replace == with != in two" "$WEEKLY --in-diff target/mutants.diff" --base main

# A user's git config must not change the diff cargo-mutants reads: it strips only the b/
# prefix and reads no colour.
mkdir -p "$work/hostile"
printf '[diff]\n\tmnemonicPrefix = true\n[color]\n\tui = always\n' > "$work/hostile/.gitconfig"
HOME="$work/hostile" expect 0 "a hostile git config still gives cargo-mutants its diff" "$d" "" \
  "$WEEKLY --in-diff target/mutants.diff" --base main
if grep -qxF '+++ b/src/lib.rs' "$d/target/mutants.diff" && ! grep -q $'\e' "$d/target/mutants.diff"; then
  passed=$((passed + 1))
else
  failed=$((failed + 1))
  echo "FAIL: under a hostile git config the diff keeps the b/ prefix and no colour" >&2
fi

# A run that writes no report, as cargo-mutants does when the diff holds no mutant, must not
# print an earlier run's.
mkdir -p "$d/target/mutants/mutants.out"
printf 'src/lib.rs:2:5: a stale mutant\n' > "$d/target/mutants/mutants.out/missed.txt"
REJECT="a stale mutant" expect 0 "an earlier run's report is not this run's" "$d" "" \
  "$WEEKLY --in-diff target/mutants.diff" --base main

d="$(repo rust-uncommitted)"
printf '// more\n' >> "$d/src/lib.rs"
expect 0 "a change not yet committed counts, as cargo-mutants mutates the working tree" "$d" "" \
  "$WEEKLY --in-diff target/mutants.diff" --base main

d="$(repo no-rust)"
printf 'more\n' >> "$d/README.md"
commit "$d" "a README change"
expect 0 "a change without Rust code has nothing to mutate" "$d" "nothing to mutate" "" --base main

d="$(repo base-moved-on)"
printf 'more\n' >> "$d/README.md"
commit "$d" "a README change"
git -C "${d:?}" checkout -q main
printf '// on main\n' >> "$d/src/lib.rs"
commit "$d" "a Rust change on main"
git -C "${d:?}" checkout -q work
expect 0 "a Rust change on the base after the fork is not the branch's" "$d" "nothing to mutate" "" \
  --base main

d="$(repo bad-base)"
expect 1 "a base that does not resolve" "$d" "does not resolve to a commit" "" --base nosuchref
expect 1 "--base without a revision" "$d" "--base needs a revision" "" --base
expect 1 "--base= without a revision" "$d" "--base needs a revision" "" --base=
expect 1 "an unknown option" "$d" "unknown option '--all'" "" --all
expect 1 "two shards" "$d" "more than one shard" "" 1/4 2/4
expect 1 "two bases" "$d" "more than one base" "" --base main --base=HEAD

# --- summary ---------------------------------------------------------------------------------

if ((failed)); then
  echo "Mutants script self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "Mutants script self-test passed ($passed cases)."
exit 0
