#!/usr/bin/env bash
# shellcheck disable=SC2034 # Constants are consumed by scripts that source this library.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly repo_root
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
  local candidate="${JANKURAI_BIN:-}" resolved
  if [[ -z "$candidate" ]]; then
    candidate="$(command -v jankurai || true)"
  fi
  [[ "$candidate" == /* ]] || {
    printf 'governed Jankurai path must be absolute\n' >&2
    return 1
  }
  [[ -f "$candidate" && ! -L "$candidate" && -x "$candidate" ]] || {
    printf 'governed Jankurai must be an executable regular non-symlink: %s\n' \
      "$candidate" >&2
    return 1
  }
  resolved="$(realpath -e -- "$candidate")"
  [[ "$resolved" == "$candidate" ]] || {
    printf 'governed Jankurai resolved outside its exact path\n' >&2
    return 1
  }
  [[ "$(stat -c '%h' -- "$candidate")" == 1 ]] || {
    printf 'governed Jankurai must have exactly one filesystem link\n' >&2
    return 1
  }
  [[ "$("$candidate" --version)" == "jankurai $JANKURAI_VERSION" ]] || {
    printf 'governed Jankurai version mismatch: expected %s\n' "$JANKURAI_VERSION" >&2
    return 1
  }
  [[ "$(sha256sum -- "$candidate" | awk '{print $1}')" == "$JANKURAI_SHA256" ]] || {
    printf 'governed Jankurai digest mismatch\n' >&2
    return 1
  }
  JANKURAI_BIN="$candidate"
  export JANKURAI_BIN
}
