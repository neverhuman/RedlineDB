#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo fmt --check
bash ops/ci/redline-only-dependency-guard.sh
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked -p db-shim --all-targets --features sqlite-parity
cargo clippy --locked -p db-shim --all-targets --features sqlite-parity -- -D warnings
DB_BACKEND=sqlite DB_DSN=:memory: DB_NAMESPACE=required \
  cargo run --locked -p db-shim --features sqlite-parity --bin db-shim-parity
