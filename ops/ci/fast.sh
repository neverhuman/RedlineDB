#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo fmt --check
bash ops/ci/backend-dependency-guard.sh
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
for feature in oracle-sqlite oracle-postgres; do
  cargo check --locked -p db-shim --all-targets --no-default-features --features "$feature"
  cargo test --locked -p db-shim --all-targets --no-default-features --features "$feature"
  cargo clippy --locked -p db-shim --all-targets --no-default-features --features "$feature" -- -D warnings
done

DB_DSN=:memory: DB_NAMESPACE=required_sqlite \
  cargo run --locked -p db-shim --no-default-features --features oracle-sqlite --bin db-shim-parity

if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
  if [[ -z "${REDLINE_CORPUS_DSN:-}" ]]; then
    printf 'JAIN_RELEASE_CI=1 requires REDLINE_CORPUS_DSN for genuine Redline corpus proof\n' >&2
    exit 1
  fi
  if [[ -z "${POSTGRES_CORPUS_DSN:-}" ]]; then
    printf 'JAIN_RELEASE_CI=1 requires POSTGRES_CORPUS_DSN for genuine Postgres corpus proof\n' >&2
    exit 1
  fi
  DB_DSN="$REDLINE_CORPUS_DSN" DB_NAMESPACE=required_redline \
    cargo run --locked -p db-shim --no-default-features --features backend-redline --bin db-shim-parity
  DB_DSN="$POSTGRES_CORPUS_DSN" DB_NAMESPACE=required_postgres \
    cargo run --locked -p db-shim --no-default-features --features oracle-postgres --bin db-shim-parity
else
  printf 'live Redline/Postgres corpus deferred: arm with JAIN_RELEASE_CI=1 and explicit DSNs\n'
fi
