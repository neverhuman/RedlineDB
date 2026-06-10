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

# Pinned auditor: never the stale ~/.local/bin shadow.
JANKURAI_BIN="${JANKURAI_BIN:-$HOME/.cargo/bin/jankurai}"

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

jankurai_bin() {
  if [[ -x "$JANKURAI_BIN" ]]; then
    printf '%s' "$JANKURAI_BIN"
    return 0
  fi
  if command -v jankurai >/dev/null 2>&1; then
    command -v jankurai
    return 0
  fi
  return 1
}

ensure_artifacts() {
  mkdir -p "$ARTIFACT_DIR"
}
