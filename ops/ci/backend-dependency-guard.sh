#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

assert_graph() {
  local label="$1"
  local expected="$2"
  shift 2
  local tree
  if [[ "$label" == default ]]; then
    tree="$(cargo tree --locked -p db-shim --edges normal,build --prefix none)"
  else
    tree="$(cargo tree --locked -p db-shim --no-default-features --features "$label" \
      --edges normal,build --prefix none)"
  fi
  if ! grep -Eq "^${expected} v" <<<"$tree"; then
    printf '%s graph is missing selected adapter dependency %s\n' "$label" "$expected" >&2
    exit 1
  fi
  local forbidden
  for forbidden in "$@"; do
    if grep -Eq "^${forbidden} v" <<<"$tree"; then
      printf '%s graph unexpectedly contains adapter dependency %s\n' "$label" "$forbidden" >&2
      exit 1
    fi
  done
}

assert_graph default redlinedb-client rusqlite postgres
assert_graph backend-redline redlinedb-client rusqlite postgres
assert_graph oracle-sqlite rusqlite redlinedb-client postgres
assert_graph oracle-postgres postgres redlinedb-client rusqlite

if cargo check --locked -p db-shim --no-default-features >/dev/null 2>&1; then
  printf 'db-shim must reject a build with no selected adapter\n' >&2
  exit 1
fi
if cargo check --locked -p db-shim --no-default-features \
  --features backend-redline,oracle-sqlite >/dev/null 2>&1; then
  printf 'db-shim must reject a build with multiple selected adapters\n' >&2
  exit 1
fi

printf 'dependency guard ok: exact isolated default/redline/sqlite/postgres adapter closures\n'
