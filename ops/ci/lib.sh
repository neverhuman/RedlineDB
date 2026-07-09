#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export REPO_ROOT

log() {
  printf '[ci] %s\n' "$*"
}

require_tool() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || {
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  }
}

write_receipt() {
  local path="$1"
  local status="$2"
  mkdir -p "$(dirname "$path")"
  printf '{"schema":"jain-split-ops.receipt/v1","status":"%s"}\n' "$status" > "$path"
}

