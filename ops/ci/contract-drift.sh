#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
cargo run --locked --quiet -- contract-drift \
  --manifest repos.manifest.toml \
  --output-dir target/jankurai/contract-drift
