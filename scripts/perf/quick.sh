#!/usr/bin/env bash
# Sub-5-minute A/B perf check against the curated quick-set (~40 cases)
# with 5 measured reps + 2 warmup. Use for tight iteration on a single
# code change.
#
# Usage: scripts/perf/quick.sh <target-binary> <output-name>
#   target-binary  the redlinedb binary under test
#   output-name    label written to target/perf/<output-name>.jsonl
#
# Example: scripts/perf/quick.sh target/release/redlinedb after-ahash

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
source "$(dirname "$0")/lib.sh"

target_bin="${1:?usage: quick.sh <target-binary> <output-name>}"
out_name="${2:?usage: quick.sh <target-binary> <output-name>}"

perf_require_bins "$target_bin"

case_list="$PERF_CASES_DIR/quick-set.txt"
if [ ! -f "$case_list" ]; then
  printf 'quick-set.txt missing at %s — run scripts/perf/build-case-lists.sh\n' "$case_list" >&2
  exit 2
fi

out="$PERF_ROOT/${out_name}.jsonl"
printf '==> quick.sh: %s -> %s\n' "$target_bin" "$out"
perf_run_jsonl "$target_bin" "$case_list" 5 2 "$out" "quick-${out_name}"

printf '\n== summary ==\n'
perf_summarize_jsonl "$out"
printf 'wrote %s\n' "$out"
