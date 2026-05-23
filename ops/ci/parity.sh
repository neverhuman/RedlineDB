#!/usr/bin/env bash
# Exhaustive SQLite parity and beyond-SQLite CI dispatcher.
#
# Usage:
#   CI_PARITY_STAGE=sql-parity-all-tests bash ops/ci/parity.sh
#   CI_PARITY_STAGE=all bash ops/ci/parity.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if [ -z "${REDLINEDB_BENCH_GIT_SHA:-}" ]; then
    export REDLINEDB_BENCH_GIT_SHA="$(git rev-parse HEAD)"
fi

run_sql_parity_all_tests() {
    local test_path
    local test_name
    local -a cargo_args=()

    while IFS= read -r test_path; do
        test_name="$(basename "$test_path" .rs)"
        cargo_args+=(--test "$test_name")
    done < <(find crates/sql/tests -maxdepth 1 -type f -name 'parity_*.rs' | sort)

    cargo_args+=(--test sqlite_full_parity)
    cargo test -p redlinedb-sql "${cargo_args[@]}" --quiet --locked
}

run_just_lane() {
    bash scripts/just/run.sh "$1"
}

run_stage() {
    case "$1" in
        sql-parity-all-tests)
            run_sql_parity_all_tests
            ;;
        sql-parity-full)
            run_just_lane sql-parity-full
            ;;
        sqlite-parity-scale-ci)
            run_just_lane sqlite-parity-scale-ci
            ;;
        sqlite-parity-report-check)
            run_just_lane sqlite-parity-report-check
            ;;
        sqlite-parity-volatile-sentinel)
            run_just_lane sqlite-parity-volatile-sentinel
            ;;
        sqlite-parity-scale-full)
            run_just_lane sqlite-parity-scale-full
            ;;
        ffi-parity-full)
            run_just_lane ffi-parity-full
            ;;
        cli-parity-full)
            run_just_lane cli-parity-full
            ;;
        fuzz-parity)
            run_just_lane fuzz-parity
            ;;
        fuzz-parity-nightly)
            run_just_lane fuzz-parity-nightly
            ;;
        beyond-sqlite-manifest)
            run_just_lane beyond-sqlite-manifest
            ;;
        *)
            printf 'unknown parity stage: %s\n' "$1" >&2
            return 1
            ;;
    esac
}

stage="${CI_PARITY_STAGE:-all}"
case "$stage" in
    all)
        run_stage sql-parity-all-tests
        run_stage sql-parity-full
        run_stage sqlite-parity-scale-ci
        run_stage sqlite-parity-report-check
        run_stage sqlite-parity-volatile-sentinel
        run_stage sqlite-parity-scale-full
        run_stage ffi-parity-full
        run_stage cli-parity-full
        run_stage fuzz-parity
        run_stage fuzz-parity-nightly
        run_stage beyond-sqlite-manifest
        ;;
    *)
        run_stage "$stage"
        ;;
esac
