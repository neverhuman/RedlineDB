#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

default_tree="$(cargo tree --locked -p db-shim --edges normal --prefix none)"
if grep -Eq '^(rusqlite|libsqlite3-sys) v' <<<"$default_tree"; then
  printf 'default db-shim dependency graph must be Redline-only\n' >&2
  printf '%s\n' "$default_tree" >&2
  exit 1
fi

parity_tree="$(cargo tree --locked -p db-shim --edges normal --prefix none \
  --features sqlite-parity)"
for dependency in rusqlite libsqlite3-sys; do
  if ! grep -Eq "^${dependency} v" <<<"$parity_tree"; then
    printf 'sqlite-parity graph is missing %s\n' "$dependency" >&2
    exit 1
  fi
done

printf 'dependency guard ok: default Redline-only; SQLite explicit via sqlite-parity\n'
