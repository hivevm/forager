# shellcheck shell=bash
# How scripts/package.sh packs an artefact and checks it, sourced by it and by
# scripts/test-package.sh. The sourcing script sets ROOT, the repository root, and sources
# scripts/lib/notice.sh first.

# The files every artefact carries at the top of its directory.
CARRIED=(THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE)

# check_archive <archive> — print each problem of the archive, named; return 1 if it has one.
check_archive() {
  local archive="$1" name top listing problems=0 notices file
  name="$(basename "$archive")"
  top="${name%.tar.gz}"
  if [[ ! -f "$archive" ]] || ! listing="$(tar -tzf "$archive" 2>/dev/null)"; then
    echo "ERROR: $name is no gzipped tar archive" >&2
    return 1
  fi
  for file in "${CARRIED[@]}"; do
    if ! grep -qxF "$top/$file" <<< "$listing"; then
      echo "ERROR: $name does not carry $top/$file" >&2
      problems=1
    fi
  done
  if grep -qxF "$top/THIRD-PARTY-NOTICES.md" <<< "$listing"; then
    notices="$(mktemp)"
    tar -xzOf "$archive" "$top/THIRD-PARTY-NOTICES.md" > "$notices"
    if ! holds_third_party_notices "$notices"; then
      echo "ERROR: $name carries a THIRD-PARTY-NOTICES.md without W. Randolph Franklin's notice" \
        "or MapServer's copyright line" >&2
      problems=1
    fi
    rm -f "$notices"
  fi
  return "$problems"
}

# check_archives <archive>... — check every archive; return 0 when all carry the notices, 1
# otherwise.
check_archives() {
  local archive failures=0
  for archive in "$@"; do
    check_archive "$archive" || failures=$((failures + 1))
  done
  if ((failures)); then
    echo "Package check FAILED ($failures of $# artefact(s) without the notices they carry)." >&2
    return 1
  fi
  echo "Package check passed ($# artefact(s) carry THIRD-PARTY-NOTICES.md, LICENSE, and LICENSE-APACHE)."
}

# pack <dir of the files> <archive> — write the archive of the files and the notices, under one
# directory named as the archive.
pack() {
  local files="$1" archive="$2" top stage status=0
  top="$(basename "$archive" .tar.gz)"
  stage="$(mktemp -d)" || return 1
  (
    mkdir -p "$stage/$top" && cp -R "$files"/. "$stage/$top/" || exit 1
    for file in "${CARRIED[@]}"; do cp "$ROOT/$file" "$stage/$top/" || exit 1; done
    tar -czf "$archive" -C "$stage" "$top"
  ) || status=1
  rm -rf "$stage"
  return "$status"
}
