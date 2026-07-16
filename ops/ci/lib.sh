#!/usr/bin/env bash
# Shared helpers and tool pins for every redline-web CI lane. GitHub Actions and
# local runs both source this module, so the commands are identical (ci-local
# parity). Every ops/ci/<lane>.sh sources this file via common.sh.
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WEB_DIR="${ROOT_DIR}/apps/web"
API_DIR="${ROOT_DIR}/apps/api"
ARTIFACT_DIR="${ROOT_DIR}/target/jankurai"

# When set to 1, missing tools are a hard failure instead of a skip. CI sets
# this on the runners that have the full toolchain installed.
STRICT_TOOLS="${REDLINE_STRICT_TOOLS:-0}"

# Governed auditor: caller environment and PATH never select release evidence.
readonly JAIN_GOVERNED_JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
readonly JAIN_GOVERNED_JANKURAI_VERSION="jankurai 1.6.11"
readonly JAIN_GOVERNED_JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"

# Tool version pins (documented for ci-doctor / supply-chain parity).
NODE_PIN="${REDLINE_NODE_PIN:-22}"
RUST_PIN="${REDLINE_RUST_PIN:-stable}"

log() {
  printf '[redline-ci] %s\n' "$*"
}

warn() {
  printf '[redline-ci][warn] %s\n' "$*" >&2
}

fail() {
  printf '[redline-ci][error] %s\n' "$*" >&2
  exit 1
}

has() {
  command -v "$1" >/dev/null 2>&1
}

missing_tool() {
  local tool="$1"
  local reason="${2:-required for this check}"
  if [[ "$STRICT_TOOLS" == "1" ]]; then
    fail "missing tool: ${tool} (${reason}); install it or set REDLINE_STRICT_TOOLS=0 for bootstrap"
  fi
  warn "skipping ${tool}: not installed (${reason})"
  return 0
}

run_if_has() {
  local tool="$1"
  local reason="$2"
  shift 2
  if ! has "$tool"; then
    missing_tool "$tool" "$reason"
    return 0
  fi
  "$@"
}

repo_has() {
  [[ -e "${ROOT_DIR}/$1" ]]
}

cargo_workspace_ready() {
  repo_has Cargo.toml || return 1
  has cargo || return 1
  (cd "$ROOT_DIR" && cargo metadata --no-deps --format-version 1 >/dev/null 2>&1)
}

jain_sha256() {
  sha256sum -- "${1:?file is required}" | awk '{print $1}'
}

jain_verify_exact_executable() {
  local label="${1:?label is required}" path="${2:?path is required}"
  local expected_digest="${3:?digest is required}" resolved
  [[ -f "$path" && -x "$path" && ! -L "$path" ]] || {
    printf '%s must be an executable regular non-symlink: %s\n' "$label" "$path" >&2
    return 1
  }
  resolved="$(realpath -e -- "$path")" || return 1
  [[ "$resolved" == "$path" ]] || {
    printf '%s resolved outside its exact path: %s\n' "$label" "$resolved" >&2
    return 1
  }
  [[ "$(jain_sha256 "$path")" == "$expected_digest" ]] || {
    printf '%s digest mismatch: %s\n' "$label" "$path" >&2
    return 1
  }
}

jain_verify_governed_jankurai() {
  local path="${1:?path is required}" expected_version="${2:?version is required}"
  local expected_digest="${3:?digest is required}" actual
  jain_verify_exact_executable governed-Jankurai "$path" "$expected_digest" || return 1
  actual="$("$path" --version 2>/dev/null)" || return 1
  [[ "$actual" == "$expected_version" ]] || {
    printf 'governed Jankurai version mismatch: %s\n' "${actual:-missing}" >&2
    return 1
  }
}

jankurai_bin() {
  jain_verify_governed_jankurai \
    "$JAIN_GOVERNED_JANKURAI_BIN" \
    "$JAIN_GOVERNED_JANKURAI_VERSION" \
    "$JAIN_GOVERNED_JANKURAI_SHA256" || return 1
  printf '%s' "$JAIN_GOVERNED_JANKURAI_BIN"
}

ensure_artifacts() {
  mkdir -p "$ARTIFACT_DIR"
}

json_array() {
  if [[ "$#" -eq 0 ]]; then
    printf '[]'
    return
  fi
  printf '%s\n' "$@" | jq -R . | jq -s .
}
