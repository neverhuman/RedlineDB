#!/usr/bin/env bash
set -euo pipefail

require_cmd() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || {
    printf 'missing required command: %s\n' "$name" >&2
    exit 1
  }
}

readonly JANKURAI_VERSION="1.6.11"
readonly JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"

# Freeze the release sandbox's PATH selection before defining the wrapper.
JANKURAI_BIN="$(command -v jankurai 2>/dev/null || true)"
readonly JANKURAI_BIN

verify_jankurai_identity() {
  local path="${1:?Jankurai path is required}"
  local expected_version="${2:?Jankurai version is required}"
  local expected_sha256="${3:?Jankurai digest is required}"
  local resolved actual_version actual_sha256

  [[ -f "$path" && ! -L "$path" && -x "$path" ]] || {
    printf 'governed Jankurai must be an executable regular non-symlink: %s\n' \
      "$path" >&2
    return 1
  }
  resolved="$(realpath -e -- "$path")" || return 1
  [[ "$resolved" == "$path" ]] || {
    printf 'governed Jankurai path is not physical: %s -> %s\n' \
      "$path" "$resolved" >&2
    return 1
  }
  actual_version="$("$path" --version 2>/dev/null)" || return 1
  [[ "$actual_version" == "jankurai $expected_version" ]] || {
    printf 'governed Jankurai version mismatch: %s\n' \
      "${actual_version:-missing}" >&2
    return 1
  }
  actual_sha256="$(sha256sum -- "$path" | awk '{print $1}')" || return 1
  [[ "$actual_sha256" == "$expected_sha256" ]] || {
    printf 'governed Jankurai digest mismatch: %s\n' "$actual_sha256" >&2
    return 1
  }
}

require_governed_jankurai() {
  require_cmd realpath
  require_cmd sha256sum
  verify_jankurai_identity "$JANKURAI_BIN" "$JANKURAI_VERSION" "$JANKURAI_SHA256"
}

jankurai() {
  require_governed_jankurai
  "$JANKURAI_BIN" "$@"
}
