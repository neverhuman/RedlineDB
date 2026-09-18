#!/usr/bin/env bash
# Fast lane dispatcher for the canonical local + CI pre-merge gate.
#
# The `preflight` stage mirrors the shared fast-lane setup checks.
# The `tests` stage runs the test shards in the same order used by the
# local aggregate lane. CI fans the shards out into separate jobs so each
# one stays under the GitHub timeout, while local `just fast` / `ci-local`
# still exercise the same command set end to end.
#
# Usage:
#   CI_FAST_STAGE=preflight bash ops/ci/fast.sh
#   CI_FAST_STAGE=core bash ops/ci/fast.sh
#   bash ops/ci/fast.sh                # run preflight + all test shards
#
# Every command in this lane is hard-gated by design.

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

if [ -z "${REDLINEDB_BENCH_GIT_SHA:-}" ]; then
    export REDLINEDB_BENCH_GIT_SHA="$(git rev-parse HEAD)"
fi

if [ "${1:-}" = "sqlite-parity-report-publish-pr" ]; then
    bash ops/ci/sqlite-parity-report.sh publish-pr
    exit 0
fi

run_preflight() {
    cargo fmt --check
    bash scripts/check_file_sizes.sh
    bash scripts/parity/lint-sqlite-parity-ledger.sh
    cargo build --locked -p redlinedb-cli --bin redlinedb
    local smoke_directory smoke_binary
    smoke_directory=$(mktemp -d)
    smoke_binary="$PWD/target/debug/redlinedb"
    (cd "$smoke_directory"; test "$("$smoke_binary" -batch :memory: 'SELECT 1;')" = 1)
    rm -rf "$smoke_directory"
    cargo check --workspace --locked
}

run_test_stage() {
    case "$1" in
        core)
            cargo nextest run \
                -p redlinedb \
                -p redlinedb-tokio \
                -p redlinedb-sqlx \
                -p redlinedb-server \
                -p redlinedb-cli \
                -p redlinedb-ffi \
                --locked --no-fail-fast
            ;;
        kernel)
            cargo test -p redlinedb-kernel --quiet --locked
            ;;
        sql-unit)
            cargo test -p redlinedb-sql --lib --quiet --locked
            ;;
        sqlite-oracle)
            REDLINEDB_SQLITE_REFERENCE_PROFILE=extended bash scripts/sqlite/build-reference.sh
            python3 scripts/sqlite/test_reference.py
            REDLINEDB_SQLITE_REFERENCE_PROFILE=ordinary bash scripts/sqlite/build-reference.sh
            ;;
        sqlite-evidence)
            # Exercise this commit's comparator, not an older published runner.
            local workspace_root reference_bin
            workspace_root=$(git rev-parse --show-toplevel)
            reference_bin=$(REDLINEDB_SQLITE_REFERENCE_PROFILE=extended bash scripts/sqlite/build-reference.sh)
            cargo build -p redlinedb-cli --locked
            (
                cd subrepos/redline-testing
                cargo test --locked
                cargo run --locked -- run --suite sqlite_parity \
                    --target-bin "$workspace_root/target/debug/redlinedb" \
                    --sqlite-bin "$reference_bin" --workers 4 \
                    --repetitions 1 --warmup 0 \
                    --tmp-root "$workspace_root/target/compatibility-ci/tmp" \
                    --output "$workspace_root/target/compatibility-ci/strict.raw.jsonl"
            )
            ;;
        sql-contracts)
            # Keep the local aggregate identical to the four required CI shards.
            local failed=0
            for shard in 1 2 3 4; do
                run_test_stage "sql-integration-${shard}" || failed=1
            done
            return "$failed"
            ;;
        sql-integration-[1-4])
            cargo nextest run -p redlinedb-sql --tests --locked \
                --partition "hash:${1##*-}/4" --no-fail-fast
            ;;
        bench)
            cargo test -p redlinedb-bench --quiet --locked
            ;;
        *)
            printf 'unknown fast test stage: %s\n' "$1" >&2
            return 1
            ;;
    esac
}

stage="${CI_FAST_STAGE:-all}"
case "$stage" in
    preflight)
        run_preflight
        ;;
    core|kernel|sql-unit|sqlite-oracle|sqlite-evidence|sql-contracts|sql-integration-[1-4]|bench)
        run_test_stage "$stage"
        ;;
    tests)
        run_test_stage sqlite-oracle
        run_test_stage sqlite-evidence
        run_test_stage core
        run_test_stage kernel
        run_test_stage sql-unit
        run_test_stage sql-contracts
        run_test_stage bench
        ;;
    all)
        run_preflight
        run_test_stage sqlite-oracle
        run_test_stage sqlite-evidence
        run_test_stage core
        run_test_stage kernel
        run_test_stage sql-unit
        run_test_stage sql-contracts
        run_test_stage bench
        ;;
    *)
        printf 'unknown fast stage: %s\n' "$stage" >&2
        exit 1
        ;;
esac
