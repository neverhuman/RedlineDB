#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

lane="${1:?lane name required}"

case "$lane" in
  fast)
    ./scripts/just/cache-warm.sh
    bash ops/ci/fast.sh
    ;;
  fast-check)
    ./scripts/just/fast-check.sh
    ;;
  fast-test)
    ./scripts/just/fast-test.sh
    ;;
  hygiene)
    rtk cargo fmt --check
    ./scripts/check_file_sizes.sh
    ;;
  clippy)
    rtk cargo clippy --workspace --all-targets --locked -- -D warnings
    ;;
  medium)
    rtk cargo test --workspace --quiet --locked
    rtk cargo run -p redlinedb-cli -- --help
    rtk cargo run -p redlinedb-server -- --help
    ;;
  phase8-smoke)
    rtk cargo test --workspace --quiet --locked
    rtk cargo run -p redlinedb-cli -- --help
    rtk cargo run -p redlinedb-server -- --help
    ;;
  phase9-smoke)
    rtk cargo test -p redlinedb-bench --quiet --locked
    rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/smoke.toml --out-dir target/bench/certify-smoke --seed 7 --repetitions 1 --warmup 0
    rtk cargo run -p redlinedb-bench -- cross-engine --engine both --test-dir crates/bench/compat --seed 7
    ;;
  phase9-certify)
    rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/certify-certification --seed 7 --repetitions 5 --warmup 1
    ;;
  phase9-xbabe1-gap)
    ./scripts/bench/xbabe1_sync.sh
    ./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/gap-cert.toml --out-dir target/bench/xbabe1/gap-cert --seed 7 --repetitions 3 --warmup 1
    ./scripts/bench/xbabe1_fetch.sh gap-cert
    ;;
  phase9-xbabe1-gap-strace)
    ./scripts/bench/xbabe1_sync.sh
    ./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/gap-cert.toml --out-dir target/bench/xbabe1/gap-cert-strace --seed 7 --repetitions 3 --warmup 1 --with-strace
    ./scripts/bench/xbabe1_fetch.sh gap-cert-strace
    ;;
  phase11-oltp-gap)
    rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/phase11-oltp-gap.toml --out-dir target/bench/phase11-oltp-gap --seed 7 --repetitions 3 --warmup 1
    ;;
  phase11-ephemeral-db)
    rtk cargo test -p redlinedb --test phase11_ephemeral --quiet --locked
    ;;
  phase11-sql-contracts)
    rtk cargo test -p redlinedb-sql --test phase11_temp_roots --quiet --locked
    rtk cargo test -p redlinedb-sql --test phase11_veox_queue --quiet --locked
    rtk cargo test -p redlinedb-sql --test phase11_xdoug_compat --quiet --locked
    ;;
  phase9-failpoint-matrix)
    rtk cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7
    ;;
  security)
    bash ops/ci/security.sh
    ;;
  pre-push)
    bash ops/git-hooks/pre-push
    ;;
  ci-doctor)
    bash scripts/ci-doctor.sh
    ;;
  release)
    rtk cargo build --workspace --release --locked
    ;;
  cache-warm)
    ./scripts/just/cache-warm.sh
    ;;
  fast-nextest)
    rtk cargo fmt --check
    ./scripts/check_file_sizes.sh
    rtk cargo check --workspace --locked
    rtk cargo nextest run --workspace --locked --no-fail-fast
    ;;
  kernel-cursor)
    rtk cargo test -p redlinedb-kernel --test index_raw_cursor --quiet --locked
    rtk cargo test -p redlinedb-kernel --test index_cursor_equivalence --quiet --locked
    rtk cargo test -p redlinedb-kernel --test index_tests --quiet --locked range_scan_terminates_early
    ;;
  kernel-check)
    rtk cargo check -p redlinedb-kernel --locked
    ;;
  kernel-test)
    rtk cargo test -p redlinedb-kernel --quiet --locked
    ;;
  sql-check)
    rtk cargo check -p redlinedb-sql --locked
    ;;
  sql-test)
    rtk cargo test -p redlinedb-sql --quiet --locked
    ;;
  ffi-check)
    rtk cargo check -p redlinedb-ffi --locked
    ;;
  ffi-test)
    rtk cargo test -p redlinedb-ffi --quiet --locked
    ;;
  cli-check)
    rtk cargo check -p redlinedb-cli --locked
    ;;
  cli-test)
    rtk cargo test -p redlinedb-cli --quiet --locked
    ;;
  sql-parity)
    rtk cargo test -p redlinedb-sql --test parity_coverage --test parity_scalar_funcs --test parity_agg_funcs --test differential_lab --test sqlite_full_parity --quiet --locked
    ;;
  sql-parity-full)
    rtk cargo test -p redlinedb-sql --test parity_oracle --quiet --locked
    ;;
  ffi-abi)
    rtk cargo test -p redlinedb-ffi --quiet --locked
    ;;
  ffi-parity-full)
    rtk cargo test -p redlinedb-ffi --test parity_oracle --quiet --locked
    ;;
  ffi-symbol-diff)
    rtk cargo test -p redlinedb-ffi --test symbol_diff --quiet --locked
    ;;
  cli-shell)
    rtk cargo test -p redlinedb-cli --quiet --locked
    ;;
  cli-parity-full)
    rtk cargo test -p redlinedb-cli --test dot_commands --quiet --locked
    ;;
  fuzz-parity)
    rtk cargo test -p redlinedb-bench --test fuzz_parity --quiet --locked -- --test-threads=1
    ;;
  fuzz-parity-nightly)
    REDLINEDB_FUZZ_ITERS=100000 rtk cargo test -p redlinedb-bench --test fuzz_parity --release --quiet --locked
    ;;
  parity-full)
    "$0" sql-parity-full
    "$0" ffi-parity-full
    "$0" cli-parity-full
    "$0" ffi-symbol-diff
    "$0" fuzz-parity
    ;;
  score)
    jankurai audit . --mode advisory --json agent/repo-score.json --md agent/repo-score.md --score-history agent/score-history.jsonl --score-history-csv agent/score-history.csv
    ;;
  doctor)
    jankurai doctor --fail-on high
    ;;
  rust-map)
    jankurai rust map .
    ;;
  rust-witness)
    jankurai rust witness build .
    ;;
  rust-diagnose)
    jankurai rust diagnose .
    ;;
  *)
    printf 'unknown just lane: %s\n' "$lane" >&2
    exit 1
    ;;
esac
