#!/usr/bin/env bash
# Full 1127-case parity corpus × 3 reps + 1 warmup × 10 workers.
# Mirrors the official CI parity gate. ~3 hours on a 16-core box.
# Project convention: full-corpus runs use 10 workers (variance/parallelism
# trade-off settled by project policy). Override via `PERF_WORKERS=N`.
#
# Usage: scripts/perf/full.sh <target-binary> <output-name>

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
source "$(dirname "$0")/lib.sh"

target_bin="${1:?usage: full.sh <target-binary> <output-name>}"
out_name="${2:?usage: full.sh <target-binary> <output-name>}"

perf_require_bins "$target_bin"

out="$PERF_ROOT/${out_name}.jsonl"
printf '==> full.sh: %s -> %s\n' "$target_bin" "$out"
# Full-corpus run uses 10 workers (project convention) + no taskset
# (workers will span cores). Disable taskset by default for this lane.
# Tolerate non-zero exit from redline-testing — the binary fails when
# any case fails, but we want the JSONL produced regardless so the
# post-filter (parity-tolerate-known-optional.sh) can apply the known-
# intentional-drift list. Real regressions still surface via the
# post-filter exit code.
set +e
PERF_WORKERS="${PERF_WORKERS:-10}" PERF_TASKSET_DISABLE=1 perf_run_jsonl \
  "$target_bin" "" 3 1 "$out" "full-${out_name}"
run_exit=$?
set -e

if [ ! -s "$out" ]; then
  printf 'perf-full: redline-testing produced no JSONL at %s (exit=%d)\n' "$out" "$run_exit" >&2
  exit "$run_exit"
fi

# Apply tolerance for the known-intentional-drift cases (fts5/rtree/dbstat
# virtual tables RedlineDB doesn't implement; DELETE/UPDATE LIMIT which
# RedlineDB supports more spec-correctly than the SQLite amalgamation).
evidence_dir="$(dirname "$out")"
cp "$out" "$evidence_dir/sqlite_parity.raw.jsonl" 2>/dev/null || true
if ! bash "$(dirname "$0")/../parity-tolerate-known-optional.sh" "$evidence_dir"; then
  printf 'perf-full: unexpected parity failures (not in tolerance list); see above\n' >&2
  exit 1
fi

printf '\n== summary ==\n'
perf_summarize_jsonl "$out"
printf 'wrote %s\n' "$out"
