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
# Soft-gate rationale: see agent/ci-soft-gate-ledger.toml (none — every
# command in this lane is hard-gated by design).

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

run_preflight() {
    cargo fmt --check
    bash scripts/check_file_sizes.sh
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
        sql-contracts)
            cargo test -p redlinedb-sql --test phase11_temp_roots --quiet --locked
            cargo test -p redlinedb-sql --test phase11_veox_queue --quiet --locked
            cargo test -p redlinedb-sql --test phase11_xdoug_compat --quiet --locked
            ;;
        sql-parity)
            cargo test -p redlinedb-sql --test parity_coverage --test parity_scalar_funcs --test parity_agg_funcs --test differential_lab --test sqlite_full_parity --quiet --locked
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
    core|kernel|sql-unit|sql-contracts|sql-parity|bench)
        run_test_stage "$stage"
        ;;
    tests)
        run_test_stage core
        run_test_stage kernel
        run_test_stage sql-unit
        run_test_stage sql-contracts
        run_test_stage sql-parity
        run_test_stage bench
        ;;
    all)
        run_preflight
        run_test_stage core
        run_test_stage kernel
        run_test_stage sql-unit
        run_test_stage sql-contracts
        run_test_stage sql-parity
        run_test_stage bench
        ;;
    *)
        printf 'unknown fast stage: %s\n' "$stage" >&2
        exit 1
        ;;
esac
