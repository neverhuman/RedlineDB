#!/usr/bin/env bash
# Nightly fuzz lane: extended REDLINEDB_FUZZ_ITERS=100000 differential
# fuzz against rusqlite. Wraps `just fuzz-parity-nightly` so the same
# command runs in `.github/workflows/nightly-fuzz.yml`, locally via
# `bash ops/ci/nightly-fuzz.sh`, and from any cron driver. Audit
# references: HLT-042 ci-local-parity.lib-missing, HLT-018 perf-
# concurrency-drift (timeout is the bench cap, not the fuzz cap).
#
# Usage:
#   bash ops/ci/nightly-fuzz.sh
#
# Soft-gate rationale: see .jankurai/ci-soft-gate-ledger.toml — none. Any
# fuzz divergence above the recorded baseline rate (per
# target/proof/sqlite-full-parity/fuzz-divergence.txt) hard-fails this
# lane. Discovered regressions are written under
# crates/sql/tests/regressions/ when REDLINEDB_FUZZ_SHRINK=1 is set.

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

export REDLINEDB_FUZZ_ITERS="${REDLINEDB_FUZZ_ITERS:-100000}"
export REDLINEDB_FUZZ_SHRINK="${REDLINEDB_FUZZ_SHRINK:-1}"

cargo test -p redlinedb-bench --test fuzz_parity --release --quiet --locked
