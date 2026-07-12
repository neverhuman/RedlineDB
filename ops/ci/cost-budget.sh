#!/usr/bin/env bash
#
# Cost-budget lane: validates the zero-spend manifest and emits a receipt.
# redline-testing has no paid or unbounded runtime surface; this lane proves
# the budgets, quota caps, and stop conditions stay at zero.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p target/jankurai

log "cost-budget: validating zero-spend manifest"
cargo run --locked --quiet -p xtask -- cost-budget

log "cost-budget: complete"
