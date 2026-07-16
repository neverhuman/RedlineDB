#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo llvm-cov --version >/dev/null 2>&1 || {
  printf 'cargo-llvm-cov is required for release coverage\n' >&2
  exit 1
}
mkdir -p target/jankurai/coverage
cargo llvm-cov --workspace --all-targets --locked --no-default-features --features oracle-sqlite --lcov \
  --output-path target/jankurai/coverage/lcov.info
[[ -s target/jankurai/coverage/lcov.info ]]
printf 'coverage ok: target/jankurai/coverage/lcov.info\n'
