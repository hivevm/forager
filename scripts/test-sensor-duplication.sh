#!/usr/bin/env bash
#
# Self-test of scripts/sensor-duplication.sh. Verifies: ADR-0038
#
# Each case builds a throwaway git repository under a temporary directory with the repository's
# own .jscpd.json and tracked Rust or bash files, runs the sensor against it with the jscpd on
# PATH, and asserts the exit code and the lines a reader would meet. The cases cover a clone in
# Rust and one in bash, each named with both its places and exiting 0, a hook without an
# extension read by its shebang, a tree without clones, a clone in an untracked file or in
# another format left out, and the five ways the sensor measures nothing: no .jscpd.json, no pin
# beside it, no jscpd, a jscpd of another version, and a jscpd that fails.
#
# Needs jscpd at the version scripts/install-jscpd.sh pins, which that script installs.
#
# Inputs: scripts/sensor-duplication.sh scripts/install-jscpd.sh .jscpd.json scripts/lib/
#
# Usage:
#     scripts/test-sensor-duplication.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn the fixtures' git on its
# repository; they run on their own (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
SENSOR="$ROOT/scripts/sensor-duplication.sh"
pinned="$(sed -n 's/^JSCPD_VERSION="\(.*\)"$/\1/p' "$ROOT/scripts/install-jscpd.sh")"

if [[ "$(jscpd --version 2>/dev/null)" != "jscpd $pinned" ]]; then
  echo "Duplication sensor self-test: needs jscpd $pinned on PATH — scripts/install-jscpd.sh installs it." >&2
  exit 1
fi

# repo <name> — an empty git repository with this repository's .jscpd.json, printed as its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir"
  git -C "${dir:?}" init -q -b main
  cp "$ROOT/.jscpd.json" "$dir/.jscpd.json"
  echo "$dir"
}

# rust_fn <name> — a Rust function of about 90 tokens and 12 lines, the same body under any name.
rust_fn() {
  cat <<EOF
pub fn $1(values: &[i64], limit: i64) -> Vec<i64> {
    let mut kept = Vec::new();
    for value in values {
        if *value > limit {
            kept.push(value * 2 + 1);
        } else if *value < -limit {
            kept.push(value * 3 - 1);
        } else {
            kept.push(*value);
        }
    }
    kept.sort_unstable();
    kept
}
EOF
}

# bash_fn <name> — a bash function of about 70 tokens and 12 lines, the same body under any name.
bash_fn() {
  cat <<EOF
$1() {
  local dir="\$1" name count=0
  for name in "\$dir"/*.txt; do
    if [[ -f "\$name" && -s "\$name" ]]; then
      count=\$((count + 1))
      printf '%s has %s lines\n' "\$name" "\$(wc -l < "\$name")"
    elif [[ -d "\$name" ]]; then
      echo "skipping directory \$name" >&2
    fi
  done
  echo "\$count files read in \$dir"
}
EOF
}

# draft <dir> <path> <content> — write a file in the fixture, untracked.
draft() {
  fixture "$1"
  mkdir -p "$(dirname "$1/$2")"
  printf '%s\n' "$3" > "$1/$2"
}

# put <dir> <path> <content> — write a file in the fixture and track it.
put() {
  draft "$@"
  git -C "${1:?}" add -- "$2"
}

# expect <exit code> <name> <dir> <needle...> — run the sensor; its output must contain every needle.
expect() {
  local want="$1" name="$2" dir="$3" out rc needle
  shift 3
  fixture "$dir"
  out="$(bash "$SENSOR" "$dir" 2>&1)"; rc=$?
  local ok=1
  [[ $rc -eq "$want" ]] || ok=0
  for needle in "$@"; do [[ "$out" == *"$needle"* ]] || ok=0; done
  if ((ok)); then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $want with $*, got exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

# --- cases -----------------------------------------------------------------------------------

d="$(repo rust)"
put "$d" "crates/a/src/lib.rs" "$(rust_fn widen)"
put "$d" "crates/b/src/lib.rs" "$(rust_fn stretch)"
expect 0 "a Rust clone names both places and exits 0" "$d" \
  "Duplication sensor: 1 clone in the Rust and bash files" "Clone found (rust)" \
  "crates/a/src/lib.rs [" "crates/b/src/lib.rs ["

d="$(repo bash)"
put "$d" "scripts/a.sh" "$(printf '#!/usr/bin/env bash\n%s' "$(bash_fn count_a)")"
put "$d" "scripts/b.sh" "$(printf '#!/usr/bin/env bash\n%s' "$(bash_fn count_b)")"
expect 0 "a bash clone names both places and exits 0" "$d" \
  "Duplication sensor: 1 clone in" "Clone found (bash)" "scripts/a.sh [" "scripts/b.sh ["

d="$(repo hook)"
put "$d" "scripts/a.sh" "$(printf '#!/usr/bin/env bash\n%s' "$(bash_fn count_a)")"
put "$d" ".githooks/pre-push" "$(printf '#!/usr/bin/env bash\n%s' "$(bash_fn count_b)")"
expect 0 "a hook without an extension is read as bash by its shebang" "$d" \
  "Duplication sensor: 1 clone in" "scripts/a.sh [" ".githooks/pre-push ["

d="$(repo none)"
put "$d" "crates/a/src/lib.rs" "$(rust_fn widen)"
put "$d" "scripts/a.sh" "$(printf '#!/usr/bin/env bash\n%s' "$(bash_fn count_a)")"
expect 0 "a tree without clones reports none and exits 0" "$d" "Duplication sensor: 0 clones in"

d="$(repo untracked)"
put "$d" "crates/a/src/lib.rs" "$(rust_fn widen)"
draft "$d" "crates/b/src/lib.rs" "$(rust_fn stretch)"
expect 0 "a clone in an untracked file that .gitignore does not ignore is left out" "$d" \
  "Duplication sensor: 0 clones in"

d="$(repo other-format)"
put "$d" "a.c" "$(rust_fn widen)"
put "$d" "b.c" "$(rust_fn stretch)"
expect 0 "a format .jscpd.json does not name is left out" "$d" "Duplication sensor: 0 clones in"

d="$(repo no-config)"
rm "$d/.jscpd.json"
expect 1 "no .jscpd.json measures nothing" "$d" ".jscpd.json not found"

d="$(repo tools)"
put "$d" "crates/a/src/lib.rs" "$(rust_fn widen)"
mkdir -p "$work/no-pin"
cp "$SENSOR" "$work/no-pin/sensor-duplication.sh"
out="$(bash "$work/no-pin/sensor-duplication.sh" "$d" 2>&1)"; rc=$?
check "no pinned version beside the sensor measures nothing and says so" \
  test "$rc" -eq 1 -a -n "$(grep -F 'no pinned version found' <<< "$out")"

out="$(JSCPD="$work/no-such-jscpd" bash "$SENSOR" "$d" 2>&1)"; rc=$?
check "no jscpd measures nothing and names the installer" \
  test "$rc" -eq 1 -a -n "$(grep -F 'scripts/install-jscpd.sh installs it' <<< "$out")"

mkdir -p "$work/bin"
printf '#!/usr/bin/env bash\necho "jscpd 4.0.0"\n' > "$work/bin/jscpd-old"
chmod +x "$work/bin/jscpd-old"
out="$(JSCPD="$work/bin/jscpd-old" bash "$SENSOR" "$d" 2>&1)"; rc=$?
check "a jscpd of another version names both versions and fails" \
  test "$rc" -eq 1 -a -n "$(grep -F "jscpd 4.0.0 runs, where jscpd $pinned is pinned" <<< "$out")"

cat > "$work/bin/jscpd-failing" <<EOF
#!/usr/bin/env bash
[[ "\$1" == --version ]] && { echo "jscpd $pinned"; exit 0; }
echo "boom" >&2
exit 3
EOF
chmod +x "$work/bin/jscpd-failing"
out="$(JSCPD="$work/bin/jscpd-failing" bash "$SENSOR" "$d" 2>&1)"; rc=$?
check "a jscpd that fails fails the sensor with its output" \
  test "$rc" -eq 1 -a -n "$(grep -F 'jscpd exited 3 without a count of clones' <<< "$out")" \
  -a -n "$(grep -Fx 'boom' <<< "$out")"

# --- summary ---------------------------------------------------------------------------------

summary "Duplication sensor self-test"
