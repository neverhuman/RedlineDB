#!/usr/bin/env bash
# PR-gate perf check against the curated medium-set (~300 cases) with
# 3 measured reps + 1 warmup × 2 workers. ~20-30 minutes.
#
# Usage: scripts/perf/medium.sh <target-binary> <output-name>

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
source "$(dirname "$0")/lib.sh"

target_bin="${1:?usage: medium.sh <target-binary> <output-name>}"
out_name="${2:?usage: medium.sh <target-binary> <output-name>}"

perf_require_bins "$target_bin"

case_list="$PERF_CASES_DIR/medium-set.txt"
if [ ! -f "$case_list" ]; then
  printf 'medium-set.txt missing at %s — run scripts/perf/build-case-lists.sh\n' "$case_list" >&2
  exit 2
fi

out="$PERF_ROOT/${out_name}.jsonl"
printf '==> medium.sh: %s -> %s\n' "$target_bin" "$out"
PERF_WORKERS="${PERF_WORKERS:-2}" perf_run_jsonl \
  "$target_bin" "$case_list" 3 1 "$out" "medium-${out_name}"

printf '\n== summary ==\n'
perf_summarize_jsonl "$out"
printf 'wrote %s\n' "$out"
