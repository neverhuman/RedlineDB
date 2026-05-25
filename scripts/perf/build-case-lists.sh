#!/usr/bin/env bash
# Generate target/perf/quick-set.txt and target/perf/medium-set.txt
# from the current redline-testing corpus + latest ranked.csv baseline.
#
# Run once after the redline-testing pinned version changes (PR bumping
# `CI_REDLINE_TESTING_REQUESTED_VERSION` in .github/workflows/ci.yml).
#
# The generated files are committed to git so reviewers can diff drift.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
source "$(dirname "$0")/lib.sh"

if [ ! -x "$REDLINE_TESTING_BIN" ]; then
  printf 'redline-testing missing at %s\n' "$REDLINE_TESTING_BIN" >&2
  exit 2
fi

mkdir -p "$PERF_ROOT" "$PERF_CASES_DIR"
snapshot="$PERF_ROOT/corpus-snapshot.json"
ranked="${PERF_BASELINE_RANKED:-benchmark-results/sqlite-parity/latest/ranked.csv}"

if [ ! -f "$ranked" ]; then
  printf 'baseline ranked.csv missing: %s\n' "$ranked" >&2
  printf '       set PERF_BASELINE_RANKED to override\n' >&2
  exit 2
fi

printf '==> snapshotting corpus -> %s\n' "$snapshot"
"$REDLINE_TESTING_BIN" list --suite sqlite_parity --format json > "$snapshot"

printf '==> building case-lists\n'
python3 "$(dirname "$0")/build_case_lists.py" \
  --snapshot "$snapshot" \
  --ranked   "$ranked" \
  --quick-out  "$PERF_CASES_DIR/quick-set.txt" \
  --medium-out "$PERF_CASES_DIR/medium-set.txt"

quick_count="$(grep -cv '^\s*\(#\|$\)' "$PERF_CASES_DIR/quick-set.txt" || true)"
medium_count="$(grep -cv '^\s*\(#\|$\)' "$PERF_CASES_DIR/medium-set.txt" || true)"
printf 'wrote quick-set (%s cases) and medium-set (%s cases) under %s/\n' \
  "$quick_count" "$medium_count" "$PERF_CASES_DIR"
