#!/usr/bin/env bash
# scripts/perf/lib.sh — shared environment for A/B measurement against
# the redline-testing parity harness.
#
# Sourced by: quick.sh, medium.sh, full.sh, profile-one.sh,
# build-case-lists.sh, and any new perf scripts.
#
# Conventions match scripts/just/run.sh + scripts/perf/pgo.sh + the
# CI parity gate (ops/ci/lib.sh::ci_resolve_redline_testing_release).
#
# Environment overrides:
#   REDLINE_TESTING_BIN     path to the redline-testing binary
#   SQLITE_REF_BIN          path to the sqlite3 reference binary
#   PERF_ROOT               where JSONL outputs land (default: target/perf)
#   PERF_WORKERS            override workers (default 1 for low variance)
#   PERF_TASKSET_CPUS       CPU list passed to taskset (default 2-5)
#   PERF_TASKSET_DISABLE    set non-empty to skip CPU pinning
#   CI_REDLINE_TESTING_BIN  CI-resolved binary path (takes precedence)

set -euo pipefail

PERF_ROOT="${PERF_ROOT:-target/perf}"
# Case-lists live in the source tree so reviewers can audit drift.
# Output JSONLs and per-run scratch live under PERF_ROOT (target/perf).
PERF_CASES_DIR="${PERF_CASES_DIR:-bench/perf/cases}"

REDLINE_TESTING_BIN_DEFAULT="/home/ubuntu/redlineDB/target/ci/redline-testing/0.1.3-redline-testing-0.1.3-linux-x86_64/bin/redline-testing"
REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-$REDLINE_TESTING_BIN_DEFAULT}"

SQLITE_REF_BIN_DEFAULT="/home/ubuntu/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-$SQLITE_REF_BIN_DEFAULT}"

# CI overrides everything (ops/ci/lib.sh::ci_resolve_redline_testing_release
# sets CI_REDLINE_TESTING_BIN after SHA-256-verifying a pinned release).
if [ -n "${CI_REDLINE_TESTING_BIN:-}" ]; then
  REDLINE_TESTING_BIN="$CI_REDLINE_TESTING_BIN"
fi

perf_require_bins() {
  local target_bin="$1"
  if [ ! -x "$target_bin" ]; then
    printf 'perf: target binary not executable: %s\n' "$target_bin" >&2
    exit 2
  fi
  if [ ! -x "$REDLINE_TESTING_BIN" ]; then
    printf 'perf: redline-testing missing: %s\n' "$REDLINE_TESTING_BIN" >&2
    printf '       set REDLINE_TESTING_BIN or install via ops/ci/lib.sh\n' >&2
    exit 2
  fi
  if [ ! -x "$SQLITE_REF_BIN" ]; then
    printf 'perf: sqlite3 reference missing: %s\n' "$SQLITE_REF_BIN" >&2
    printf '       set SQLITE_REF_BIN or run scripts/sqlite/build-reference.sh\n' >&2
    exit 2
  fi
  # Refuse to time sqlite3 vs itself — guards against accidental misuse.
  local target_sha sqlite_sha
  target_sha="$(sha256sum "$target_bin" | awk '{print $1}')"
  sqlite_sha="$(sha256sum "$SQLITE_REF_BIN" | awk '{print $1}')"
  if [ "$target_sha" = "$sqlite_sha" ]; then
    printf 'perf: target binary sha256 equals sqlite3 reference — refusing\n' >&2
    exit 2
  fi
}

perf_tmp_root() {
  local tag="${1:-default}"
  if [ -d /dev/shm ] && [ -w /dev/shm ]; then
    printf '/dev/shm/redline-testing-perf-%s\n' "$tag"
  else
    printf '%s/redline-testing-perf-%s\n' "${TMPDIR:-/tmp}" "$tag"
  fi
}

perf_quiet_system() {
  # Best-effort variance reduction; never fail if we lack permission.
  if [ -w /proc/sys/vm/drop_caches ]; then
    sync
    printf 3 > /proc/sys/vm/drop_caches 2>/dev/null || true
  fi
  if command -v cpupower >/dev/null 2>&1 && [ "$(id -u)" = 0 ]; then
    cpupower frequency-set -g performance >/dev/null 2>&1 || true
  fi
}

# Run the parity workload with variance-controlled defaults.
#
# `redline-testing run` itself does not accept a case-list filter (only
# `report` and `list` do), so for subset runs we use the local custom
# replay driver at scripts/perf/run_subset.py which replays cases from
# the corpus snapshot and emits JSONL matching the official schema.
# For full-corpus runs (case-list empty) we use `redline-testing run`
# directly.
#
# Usage: perf_run_jsonl <target-bin> <case-list-file-or-empty> <reps> <warmup> <output.jsonl> <tmp-tag>
perf_run_jsonl() {
  local target_bin="$1" case_list="$2" reps="$3" warmup="$4" out="$5" tag="$6"
  local tmp
  tmp="$(perf_tmp_root "$tag")"
  mkdir -p "$(dirname "$out")" "$tmp"
  perf_quiet_system

  local taskset_cmd=()
  if [ -z "${PERF_TASKSET_DISABLE:-}" ] && command -v taskset >/dev/null 2>&1; then
    taskset_cmd=("taskset" "-c" "${PERF_TASKSET_CPUS:-2-5}")
  fi

  if [ -n "$case_list" ] && [ -f "$case_list" ]; then
    # Subset run via the custom replay driver.
    local snapshot="$PERF_ROOT/corpus-snapshot.json"
    if [ ! -f "$snapshot" ]; then
      printf 'corpus snapshot missing at %s — run scripts/perf/build-case-lists.sh\n' "$snapshot" >&2
      exit 2
    fi
    REDLINEDB_DEFAULT_DURABILITY=normal \
    "${taskset_cmd[@]}" \
      python3 "$(dirname "${BASH_SOURCE[0]}")/run_subset.py" \
        --case-list   "$case_list" \
        --target-bin  "$target_bin" \
        --sqlite-bin  "$SQLITE_REF_BIN" \
        --output      "$out" \
        --repetitions "$reps" \
        --warmup      "$warmup" \
        --snapshot    "$snapshot" \
        --tmp-root    "$tmp" \
        --workers     "${PERF_WORKERS:-1}"
  else
    # Full-corpus run via the official harness.
    REDLINEDB_DEFAULT_DURABILITY=normal \
    "${taskset_cmd[@]}" \
      "$REDLINE_TESTING_BIN" run \
        --target-bin   "$target_bin" \
        --sqlite-bin   "$SQLITE_REF_BIN" \
        --suite        sqlite_parity \
        --workers      "${PERF_WORKERS:-1}" \
        --tmp-root     "$tmp" \
        --repetitions  "$reps" \
        --warmup       "$warmup" \
        --output       "$out"
  fi
}

# Print a compact summary of a JSONL file. Used by the runner scripts so
# the user sees results inline without having to run diff.py separately.
perf_summarize_jsonl() {
  local jsonl="$1"
  python3 - "$jsonl" <<'PYEOF'
import json
import statistics
import sys
from collections import defaultdict

path = sys.argv[1]
rows = []
with open(path) as f:
    for line in f:
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            continue

measured = [
    r for r in rows
    if r.get("status") == "passed"
    and isinstance(r.get("sample_role"), str)
    and r["sample_role"].startswith("measured")
    and isinstance(r.get("latency_ratio"), (int, float))
    and r["latency_ratio"] > 0
]
ratios = [r["latency_ratio"] for r in measured]
case_ids = {r["case_id"] for r in measured}
print(f"  cases measured: {len(case_ids)}")
print(f"  samples:        {len(measured)}")
if ratios:
    print(f"  ratio median:   {statistics.median(ratios):.3f}")
    if len(ratios) >= 10:
        deciles = statistics.quantiles(ratios, n=10)
        print(f"  ratio p90:      {deciles[-1]:.3f}")
    faster = sum(1 for r in ratios if r < 1.0)
    print(f"  cases faster than sqlite: {faster}/{len(ratios)}")
PYEOF
}
