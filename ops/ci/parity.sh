#!/usr/bin/env bash
# Official redline-testing dispatcher with fail-closed legacy local parity
# stage names.
#
# Usage:
#   CI_PARITY_STAGE=redline-testing-official bash ops/ci/parity.sh
#   CI_PARITY_STAGE=all bash ops/ci/parity.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if [ -z "${REDLINEDB_BENCH_GIT_SHA:-}" ]; then
    export REDLINEDB_BENCH_GIT_SHA="$(git rev-parse HEAD)"
fi

run_just_lane() {
    bash scripts/just/run.sh "$1"
}

reject_local_parity_stage() {
    printf '%s is disabled: SQLite parity coverage, benchmark, report, sentinel, and proof evidence must be produced only through the pinned neverhuman/redline-testing release artifact.\n' "$1" >&2
    return 1
}

run_stage() {
    case "$1" in
        redline-testing-official)
            run_just_lane redline-testing-official
            ;;
        sql-parity-all-tests)
            reject_local_parity_stage "$1"
            ;;
        sql-parity-full)
            reject_local_parity_stage "$1"
            ;;
        ffi-parity-full)
            reject_local_parity_stage "$1"
            ;;
        cli-parity-full)
            reject_local_parity_stage "$1"
            ;;
        fuzz-parity)
            reject_local_parity_stage "$1"
            ;;
        fuzz-parity-nightly)
            reject_local_parity_stage "$1"
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
        run_stage redline-testing-official
        ;;
    *)
        run_stage "$stage"
        ;;
esac
