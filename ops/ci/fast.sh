#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'fast lane: shell syntax'
mapfile -t sh_files < <(find ops scripts -type f -name '*.sh' | sort)
for f in "${sh_files[@]}"; do bash -n "$f"; done

log 'fast lane: Rust control-plane build and tests'
cargo fmt -- --check
cargo test --locked
cargo run --locked --quiet -- validate-local-jeryu --manifest repos.manifest.toml \
  --skip-remotes --skip-program-checkouts
cargo run --locked --quiet -- python-boundary

printf 'fast ok: jain-split-ops\n'
