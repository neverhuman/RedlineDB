#!/usr/bin/env bash
# shellcheck disable=SC2034 # Constants are consumed by scripts that source this library.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
readonly JANKURAI_VERSION="1.6.11"
readonly JANKURAI_REV="3c804453e6c7a6e0e4028d95cc3bccea467277ef"
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
  local binary="${JANKURAI_BIN:-}" resolved
  if [[ -z "$binary" ]]; then
    binary="$(command -v jankurai || true)"
  fi
  [[ "$binary" == /* ]] || {
    printf 'governed Jankurai path must be absolute\n' >&2
    return 1
  }
  [[ -f "$binary" && ! -L "$binary" && -x "$binary" ]] || {
    printf 'governed Jankurai must be an executable regular non-symlink: %s\n' \
      "$binary" >&2
    return 1
  }
  resolved="$(realpath -e -- "$binary")"
  [[ "$resolved" == "$binary" ]] || {
    printf 'governed Jankurai resolved outside its exact path\n' >&2
    return 1
  }
  [[ "$(stat -c '%h' -- "$binary")" == 1 ]] || {
    printf 'governed Jankurai must have exactly one filesystem link\n' >&2
    return 1
  }
  [[ "$($binary --version)" == "jankurai $JANKURAI_VERSION" ]] || {
    printf 'expected jankurai %s\n' "$JANKURAI_VERSION" >&2
    return 1
  }
  local binary_dir
  binary_dir="$(dirname "$binary")"
  JANKURAI_BIN="$binary"
  export JANKURAI_BIN
  export PATH="$binary_dir:$PATH"
}
