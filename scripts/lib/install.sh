# shellcheck shell=bash
# The install of a pinned release binary (OPS-pinned-stable-rust-toolchain): the download of a release asset, its check
# against the SHA-256 pinned for it, and the install of the binary it holds into a directory on
# PATH. Sourced by scripts/install-shellcheck.sh and scripts/install-jscpd.sh, the project's own
# installers (the template brings neither), which pin the version, the assets, and their digests.
#
# Usage, from scripts/install-<tool>.sh:
#     # shellcheck source-path=SCRIPTDIR source=lib/install.sh
#     source "$(dirname "${BASH_SOURCE[0]}")/lib/install.sh"

# install_release <command> <version> <bin dir> <version line> <url> <sha256> <member> — install
# the binary <member> of the archive at <url> as <bin dir>/<command>, after checking the archive
# against <sha256>. A <command> in <bin dir> whose --version prints <version line> as a line of its
# own is kept. Each message starts with the name of the sourcing installer. A <command> found on
# PATH before <bin dir> is named as a note, since it is the one the checks would run.
install_release() {
  local command="$1" version="$2" bin_dir="$3" line="$4" url="$5" sha256="$6" member="$7"
  local me
  me="$(basename "$0" .sh)"
  if grep -qxF "$line" <<< "$("$bin_dir/$command" --version 2> /dev/null)"; then
    echo "$me: $command $version is installed in $bin_dir"
  else
    local work name="${url##*/}"
    work="$(mktemp -d)"
    # shellcheck disable=SC2064 # the path is fixed now, the trap runs on the script's exit
    trap "rm -rf '$work'" EXIT
    curl --fail --silent --show-error --location --output "$work/$name" "$url"
    echo "$sha256  $work/$name" | sha256sum --check --quiet -
    tar -xf "$work/$name" -C "$work" "$member"
    mkdir -p "$bin_dir"
    install -m 0755 "$work/$member" "$bin_dir/$command"
    echo "$me: installed $command $version in $bin_dir"
  fi

  local found
  found="$(command -v "$command" || true)"
  if [[ "$found" != "$bin_dir/$command" ]]; then
    echo "$me: note: PATH finds ${found:-no $command} before $bin_dir/$command" >&2
  fi
}
