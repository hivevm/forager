#!/usr/bin/env bash
#
# Check: the Dev Container binds the GitHub CLI's login from a host directory of its own
# (SEC-gh-login-outlives-rebuilds, SEC-gh-login-stays-on-host). Forager's own check: the
# template's scripts/check-devcontainer.sh stays as the release ships it.
#
# It reads .devcontainer/devcontainer.json and fails where:
#
#   devcontainer/gh-login:host
#      a mount binds a host path ending in '.config/gh', the host's own configuration, whose
#      token may sit in a keyring the container cannot read;
#   devcontainer/gh-login:source
#      a mount binds the container's /home/vscode/.config/gh from a host path that does not end
#      in '.config/gh-devcontainer', the directory of the container's login.
#
# A tree without .devcontainer/devcontainer.json has no Dev Container, and nothing binds: the check
# passes and says so. Full-line '//' comments are dropped before reading. A mount is a string
# '"source=…,target=…"' on one line, as the Dev Container writes its mounts; a mount written as an
# object is not read. The file is small and hand-written, and no JSON parser is guaranteed on
# PATH.
#
# Pure bash + grep. Usage:
#     scripts/check-gh-login.sh [--root <dir>]
# The root defaults to this repository; scripts/test-check-gh-login.sh passes fixtures.
# Exit code 0 when the check passes, 1 otherwise, and 2 on an argument it does not read
# or a root that is no directory.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# shellcheck source-path=SCRIPTDIR source=lib/check.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/check.sh"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/check-gh-login.sh [--root <dir>]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
rest_args "${BASH_SOURCE[0]}" "$usage" 0 "${ARGS[@]}"
CONFIG=".devcontainer/devcontainer.json"

if [[ ! -f "$ROOT/$CONFIG" ]]; then
  echo "GitHub CLI login check passed: no $CONFIG, so nothing binds. [devcontainer/gh-login:unbound]"
  exit 0
fi

count=0
while IFS= read -r mount; do
  count=$((count + 1))
  source="$(grep -oE 'source=[^,"]*' <<< "$mount")"
  target="$(grep -oE 'target=[^,"]*' <<< "$mount")"
  if [[ "$source" =~ \.config/gh/?$ ]]; then
    add_error "$CONFIG: binds the host's own '.config/gh' — the container keeps its login in '~/.config/gh-devcontainer' on the host (SEC-gh-login-outlives-rebuilds) [devcontainer/gh-login:host]"
  elif [[ "$target" =~ ^target=/home/vscode/\.config/gh/?$ \
    && ! "$source" =~ \.config/gh-devcontainer/?$ ]]; then
    add_error "$CONFIG: binds /home/vscode/.config/gh from '${source#source=}' — it comes from '~/.config/gh-devcontainer' on the host (SEC-gh-login-outlives-rebuilds) [devcontainer/gh-login:source]"
  fi
done < <(grep -vE '^[[:space:]]*//' "$ROOT/$CONFIG" | grep -oE '"source=[^"]*,[[:space:]]*target=[^"]*"')

if ((${#errors[@]})); then
  echo "GitHub CLI login check FAILED:" >&2
  printf '  - %s\n' "${errors[@]}" >&2
  exit 1
fi
echo "GitHub CLI login check passed ($count mount(s), none of the host's own gh configuration). [devcontainer/gh-login:passed]"
