#!/usr/bin/env bash
# scripts/perf/lib.sh — shared environment for A/B measurement against
# the redline-testing parity harness.
#
# Sourced by: full.sh and any new full-corpus perf scripts.
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

REDLINE_CORE_ROOT="${REDLINE_CORE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REDLINE_SPLIT_ROOT="${REDLINE_SPLIT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
REDLINE_TESTING_BIN_DEFAULT="${REDLINE_SPLIT_ROOT}/redline-testing/target/release/redline-testing"
REDLINE_TESTING_BIN="${REDLINE_TESTING_BIN:-$REDLINE_TESTING_BIN_DEFAULT}"

SQLITE_REF_BIN_DEFAULT="${SQLITE_REF_BIN_DEFAULT:-${REDLINE_SPLIT_ROOT}/sqlite-reference/bin/sqlite3}"
SQLITE_REF_BIN="${SQLITE_REF_BIN:-$SQLITE_REF_BIN_DEFAULT}"

# CI overrides everything (ops/ci/lib.sh::ci_resolve_redline_testing_release
# sets CI_REDLINE_TESTING_BIN after SHA-256-verifying a pinned release).
if [ -n "${CI_REDLINE_TESTING_BIN:-}" ]; then
  REDLINE_TESTING_BIN="$CI_REDLINE_TESTING_BIN"
fi

perf_evidence() {
  cargo run --quiet --locked --manifest-path "$REDLINE_CORE_ROOT/Cargo.toml" \
    -p redlinedb-bench --bin perf_evidence -- "$@"
}

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
  perf_evidence assert-distinct-binaries "$target_bin" "$SQLITE_REF_BIN"
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

# Run the complete parity workload through the verified external runner with
# variance-controlled defaults.
#
# Usage: perf_run_jsonl <target-bin> <reps> <warmup> <output.jsonl> <tmp-tag>
perf_run_jsonl() {
  local target_bin="$1" reps="$2" warmup="$3" out="$4" tag="$5"
  local tmp
  tmp="$(perf_tmp_root "$tag")"
  mkdir -p "$(dirname "$out")" "$tmp"
  perf_quiet_system

  local taskset_cmd=()
  if [ -z "${PERF_TASKSET_DISABLE:-}" ] && command -v taskset >/dev/null 2>&1; then
    taskset_cmd=("taskset" "-c" "${PERF_TASKSET_CPUS:-2-5}")
  fi

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
}

# Print a compact summary of a JSONL file. Used by the runner scripts so
# the user sees results inline.
perf_summarize_jsonl() {
  local jsonl="$1"
  perf_evidence summarize-jsonl "$jsonl"
}
