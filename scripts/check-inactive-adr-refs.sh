#!/usr/bin/env bash
#
# Check: no inactive ADR cited outside the record. Forager's own check: the template's
# scripts/check-docs.sh stays as the release ships it.
#
#   docs/inactive-adr-refs
#      A reference to a ⚪ superseded or 🔴 rejected ADR in a file outside docs/adr/ is an error
#      naming the file, the line, the ADR, and the ADR that supersedes it. Such an ADR explains
#      why and is not a rule (AGENTS.md, section 2), so a reader sent to it as the decision in
#      force is sent to one the project has left; the rule docs/adr-refs of check-docs.sh passes
#      it, as the file exists, and the README went on citing an ADR after its successor landed
#      until a review found it. Inside docs/adr/ it is cited by right, as the history a successor
#      answers. A line outside that names a superseded ADR as history names its successor too,
#      and one that names a rejected ADR says 'rejected'. A superseded ADR whose status line names
#      no successor is held as a rejected one.
#
# It reads every text file of the repository, as check-docs.sh does: the files git tracks and the
# new ones it does not ignore, or every file outside .git/ where git cannot answer. An ADR is a
# file docs/adr/<CAT>-<slug>.md, its status the first '- **Status:**' line, and a citation an
# identifier of a category the ADR index defines, as a word of its own (scripts/lib/adr.sh).
#
# Pure bash + git, grep, sed, and find. Usage:
#     scripts/check-inactive-adr-refs.sh [--root <dir>]
# The root defaults to this repository; scripts/test-check-inactive-adr-refs.sh passes fixtures.
# Exit code 0 when no inactive ADR is cited outside the record, 1 otherwise, and 2 on an argument
# it does not read or a root that is no directory.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it names (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"
# shellcheck source-path=SCRIPTDIR source=lib/adr.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/adr.sh"
# shellcheck source-path=SCRIPTDIR source=lib/check.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/check.sh"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/check-inactive-adr-refs.sh [--root <dir>]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
rest_args "${BASH_SOURCE[0]}" "$usage" 0 "${ARGS[@]}"
ADR_DIR="$ROOT/docs/adr"

adr_categories "$ADR_DIR/README.md"

# The inactive ADRs, each identifier mapped to its successor, empty for none.
declare -A inactive=()
if [[ -d "$ADR_DIR" ]]; then
  while IFS= read -r adr; do
    status="$(adr_field_line "$adr" Status)"
    [[ "$status" == *⚪* || "$status" == *🔴* ]] || continue
    adr_key_of "$adr"
    re="superseded by \\[?($ADR_REF_RE)([^$ADR_WORD_CHARS]|\$)"
    successor=""
    [[ "$status" =~ $re ]] && successor="${BASH_REMATCH[1]}"
    inactive["$ADR_ID"]="$successor"
  done < <(adr_files "$ADR_DIR")
fi

# The text files outside docs/adr/.
files=()
while IFS= read -r f; do
  [[ -f "$f" && "$f" != "$ADR_DIR"/* ]] && files+=("$f")
done < <(
  if git -C "$ROOT" rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    git -C "$ROOT" ls-files --cached --others --exclude-standard | sed "s|^|$ROOT/|"
  else
    find "$ROOT" -type f -not -path '*/.git/*'
  fi | LC_ALL=C sort
)

if ((${#inactive[@]} && ${#files[@]})); then
  # One grep over every file; --null ends each file name with a NUL, which no path carries, and -I
  # passes a binary file over.
  while IFS= read -r -d '' f && IFS=: read -r lineno text; do
    refs=() rest="$text"
    while [[ "$rest" =~ $ADR_REF_WORD_RE ]]; do
      refs+=("${BASH_REMATCH[2]}")
      rest="${rest#*"${BASH_REMATCH[0]}"}"
    done
    declare -A seen=() named=()
    for id in "${refs[@]}"; do named["$id"]=1; done
    for id in "${refs[@]}"; do
      [[ -v inactive["$id"] && ! -v seen["$id"] ]] || continue
      seen["$id"]=1
      successor="${inactive["$id"]}"
      if [[ -n "$successor" ]]; then
        [[ -v named["$successor"] ]] && continue
        add_error "${f#"$ROOT"/}:$lineno: cites $id, which is superseded by $successor — cite $successor, or name it on the same line to cite $id as history [docs/inactive-adr-refs:superseded]"
      else
        [[ "$text" == *rejected* ]] && continue
        add_error "${f#"$ROOT"/}:$lineno: cites $id, which is rejected — cite the decision in force, or say on the same line that $id was rejected [docs/inactive-adr-refs:rejected]"
      fi
    done
    unset seen named
  done < <(grep -HnI --null -E "$ADR_REF_RE" "${files[@]}")
fi

if ((${#errors[@]})); then
  echo "Inactive ADR references check FAILED:" >&2
  printf '  - %s\n' "${errors[@]}" >&2
  exit 1
fi
echo "Inactive ADR references check passed (${#inactive[@]} inactive ADR(s), ${#files[@]} file(s) read). [docs/inactive-adr-refs:passed]"
