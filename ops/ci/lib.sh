#!/usr/bin/env bash
# shellcheck disable=SC2034 # Constants are consumed by scripts that source this library.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
readonly JANKURAI_VERSION="1.6.11"
readonly JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"
readonly JANKURAI_TAG="v1.6.11-deadlang-precision"
readonly JANKURAI_REV="dface7397fe24d46b0b1885ddd5782c34edbff49"
readonly JANKURAI_GIT="http://127.0.0.1:8787/git/jeryu/jankurai.git"
readonly CARGO_AUDIT_VERSION="0.22.1"
readonly CARGO_DENY_VERSION="0.19.8"
readonly ZIZMOR_VERSION="1.25.2"
readonly ACTIONLINT_VERSION="1.7.8"
readonly GITLEAKS_VERSION="8.21.2"
readonly SYFT_VERSION="1.40.0"

require_tool() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'required tool is unavailable: %s\n' "$1" >&2
    return 1
  }
}

require_jankurai() {
  [[ -f "$JANKURAI_BIN" && ! -L "$JANKURAI_BIN" && -x "$JANKURAI_BIN" ]] || {
    printf 'governed Jankurai must be an executable regular non-symlink: %s\n' \
      "$JANKURAI_BIN" >&2
    return 1
  }
  [[ "$(realpath -e -- "$JANKURAI_BIN")" == "$JANKURAI_BIN" ]] || {
    printf 'governed Jankurai resolved outside its exact path\n' >&2
    return 1
  }
  [[ "$($JANKURAI_BIN --version)" == "jankurai $JANKURAI_VERSION" ]] || {
    printf 'governed Jankurai version mismatch: expected %s\n' "$JANKURAI_VERSION" >&2
    return 1
  }
  [[ "$(sha256sum -- "$JANKURAI_BIN" | awk '{print $1}')" == "$JANKURAI_SHA256" ]] || {
    printf 'governed Jankurai digest mismatch\n' >&2
    return 1
  }
  export JANKURAI_BIN
}
