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
printf 'live Redline/Postgres corpus is a separate explicit family-release lane\n'
