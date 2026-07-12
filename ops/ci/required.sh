#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
./redlinectl test-receipt target/jankurai/coverage/rust-tests.json
./redlinectl validate
