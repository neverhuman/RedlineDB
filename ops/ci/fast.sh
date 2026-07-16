#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo fmt --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
DB_BACKEND=sqlite DB_DSN=:memory: DB_NAMESPACE=required \
  cargo run --locked -p db-shim --bin db-shim-parity
