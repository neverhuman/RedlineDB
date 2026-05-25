#!/usr/bin/env bash
# Full 1127-case parity corpus × 3 reps + 1 warmup × auto workers.
# Mirrors the official CI parity gate. ~3 hours on a 16-core box.
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
# Full-corpus run uses --workers auto + no taskset (workers will span
# all cores). Disable taskset by default for this lane.
PERF_WORKERS="${PERF_WORKERS:-auto}" PERF_TASKSET_DISABLE=1 perf_run_jsonl \
  "$target_bin" "" 3 1 "$out" "full-${out_name}"

printf '\n== summary ==\n'
perf_summarize_jsonl "$out"
printf 'wrote %s\n' "$out"
