#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

: "${REDLINE_CORPUS_DSN:?family-release requires REDLINE_CORPUS_DSN}"
: "${POSTGRES_CORPUS_DSN:?family-release requires POSTGRES_CORPUS_DSN}"

if [[ "$REDLINE_CORPUS_DSN" == :memory: || "$REDLINE_CORPUS_DSN" == file:* ]]; then
  printf 'family-release requires an external Redline service DSN\n' >&2
  exit 1
fi
if [[ "$POSTGRES_CORPUS_DSN" != postgres://* && "$POSTGRES_CORPUS_DSN" != postgresql://* ]]; then
  printf 'family-release requires an explicit Postgres service DSN\n' >&2
  exit 1
fi

DB_DSN="$REDLINE_CORPUS_DSN" DB_NAMESPACE=family_release_redline \
  cargo run --locked -p db-shim --no-default-features \
    --features backend-redline --bin db-shim-parity
DB_DSN="$POSTGRES_CORPUS_DSN" DB_NAMESPACE=family_release_postgres \
  cargo run --locked -p db-shim --no-default-features \
    --features oracle-postgres --bin db-shim-parity

printf 'family-release corpus ok: genuine Redline and Postgres services\n'
