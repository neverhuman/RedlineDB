#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# shellcheck source=ops/ci/lib.sh
. "$repo_root/ops/ci/lib.sh"

if ! command -v rtk >/dev/null 2>&1; then
  rtk() {
    "$@"
  }
fi

if [ -z "${REDLINEDB_BENCH_GIT_SHA:-}" ]; then
  export REDLINEDB_BENCH_GIT_SHA="$(git rev-parse HEAD)"
fi

lane="${1:?lane name required}"
sqlite_parity_full_select=(
  --priorities P0,P1,P2,P3,P4
  --profiles memory,tempfile,catalog,external_app,side_effect
  --include-quarantine
)
sqlite_parity_full_compare=("${sqlite_parity_full_select[@]}" --deny-skips)
sqlite_parity_reference_bin="${REDLINEDB_SQLITE_PARITY_SQLITE_BIN:-sqlite3}"

ensure_sqlite_parity_reference() {
  if [ -n "${REDLINEDB_SQLITE_PARITY_SQLITE_BIN:-}" ]; then
    sqlite_parity_reference_bin="$REDLINEDB_SQLITE_PARITY_SQLITE_BIN"
    return 0
  fi
  sqlite_parity_reference_bin="$(rtk bash scripts/sqlite/build-reference.sh)"
  export REDLINEDB_SQLITE_PARITY_SQLITE_BIN="$sqlite_parity_reference_bin"
}

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
  release-binary-smoke)
    ci_verify_redlinedb_release_smoke
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
    set +e
    rtk cargo test -p redlinedb-sql --test parity_oracle --quiet --locked
    test_status=$?
    rtk bash scripts/parity/write-sqlite-full-parity-receipts.sh
    receipt_status=$?
    set -e
    if [[ "$test_status" -ne 0 ]]; then
      exit "$test_status"
    fi
    exit "$receipt_status"
    ;;
  sqlite-parity-scale-smoke)
    rtk cargo run -p redlinedb-bench --release --bin sqlite_parity -- run --sqlite-bin "$sqlite_parity_reference_bin" --engine-name sqlite3 --profiles memory --priorities P0 --jobs auto --out target/sqlite-parity/sqlite-scale-smoke.jsonl
    ;;
  sqlite-parity-scale-ci)
    ensure_sqlite_parity_reference
    rtk cargo build -p redlinedb-cli --release --bin redlinedb --locked
    mkdir -p benchmark-results/sqlite-parity/latest assets
    mkdir -p target/sqlite-parity
    raw_tmp="target/sqlite-parity/full-corpus-ci.raw.jsonl"
    rm -f "$raw_tmp"
    updated_date="${REDLINEDB_SQLITE_PARITY_UPDATED_DATE:-$(date -u +%F)}"
    rtk cargo run -p redlinedb-bench --release --bin sqlite_parity -- compare --reference-bin "$sqlite_parity_reference_bin" --target-bin target/release/redlinedb "${sqlite_parity_full_compare[@]}" --repetitions "${REDLINEDB_SQLITE_PARITY_REPETITIONS:-1}" --warmup "${REDLINEDB_SQLITE_PARITY_WARMUP:-0}" --jobs auto --out "$raw_tmp"
    mv "$raw_tmp" benchmark-results/sqlite-parity/latest/raw.jsonl
    printf '%s\n' "$updated_date" > benchmark-results/sqlite-parity/latest/UPDATED_DATE
    rtk cargo run -p redlinedb-bench --bin sqlite_parity -- report --input benchmark-results/sqlite-parity/latest/raw.jsonl "${sqlite_parity_full_select[@]}" --out-dir benchmark-results/sqlite-parity/latest --readme README.md --plot assets/sqlite-parity-latency-gap.svg --ksloc-plot assets/sqlite-parity-ksloc.svg --jankurai-score .jankurai/repo-score.json --updated-date "$updated_date"
    ;;
  sqlite-parity-volatile-sentinel)
    rtk cargo build -p redlinedb-cli --release --bin redlinedb --locked
    rm -f target/sqlite-parity/volatile-fastpath-sentinel.jsonl
    rtk cargo run -p redlinedb-bench --release --bin sqlite_parity -- compare --reference-bin sqlite3 --target-bin target/release/redlinedb --case-list crates/bench/sqlite_parity/volatile-fastpath-sentinel.txt --repetitions "${REDLINEDB_VOLATILE_SENTINEL_REPETITIONS:-3}" --warmup "${REDLINEDB_VOLATILE_SENTINEL_WARMUP:-1}" --jobs auto --out target/sqlite-parity/volatile-fastpath-sentinel.jsonl
    rtk cargo run -p redlinedb-bench --release --bin sqlite_parity -- sentinel --input target/sqlite-parity/volatile-fastpath-sentinel.jsonl --ceiling-ns 00003=250000000 --ceiling-ns 00274=200000000 --ceiling-ns 00807=500000000 --ceiling-ns 00949=750000000 ${REDLINEDB_VOLATILE_SENTINEL_ENFORCE:+--enforce}
    ;;
  sqlite-parity-report-update)
    "$0" score
    REDLINEDB_SQLITE_PARITY_REPETITIONS="${REDLINEDB_SQLITE_PARITY_REPETITIONS:-3}" \
      REDLINEDB_SQLITE_PARITY_WARMUP="${REDLINEDB_SQLITE_PARITY_WARMUP:-1}" \
      "$0" sqlite-parity-scale-ci
    ;;
  sqlite-parity-report-check)
    rtk cargo run -p redlinedb-bench --bin sqlite_parity -- report --input benchmark-results/sqlite-parity/latest/raw.jsonl "${sqlite_parity_full_select[@]}" --out-dir benchmark-results/sqlite-parity/latest --readme README.md --plot assets/sqlite-parity-latency-gap.svg --ksloc-plot assets/sqlite-parity-ksloc.svg --jankurai-score .jankurai/repo-score.json --updated-date "$(cat benchmark-results/sqlite-parity/latest/UPDATED_DATE)" --check
    ;;
  sqlite-parity-report-publish-pr)
    bash ops/ci/sqlite-parity-report.sh publish-pr
    ;;
  sqlite-parity-scale-full)
    ensure_sqlite_parity_reference
    rtk cargo run -p redlinedb-bench --release --bin sqlite_parity -- run --sqlite-bin "$sqlite_parity_reference_bin" --engine-name sqlite3 "${sqlite_parity_full_compare[@]}" --jobs auto --out target/sqlite-parity/sqlite-scale-full.jsonl
    ;;
  ffi-abi)
    rtk cargo test -p redlinedb-ffi --quiet --locked
    ;;
  ffi-parity-full)
    "$0" ffi-abi
    "$0" ffi-symbol-diff
    ;;
  ffi-symbol-diff)
    rtk bash scripts/parity/dump-sqlite-symbols.sh
    rtk cargo build -p redlinedb-ffi --quiet --locked
    rtk cargo test -p redlinedb-ffi --test symbol_diff --quiet --locked -- --ignored
    ;;
  cli-shell)
    rtk cargo test -p redlinedb-cli --quiet --locked
    ;;
  cli-parity-full)
    if ! command -v sqlite3 >/dev/null 2>&1; then
      printf 'sqlite3 CLI is required for cli-parity-full\n' >&2
      exit 127
    fi
    printf 'sqlite3_version='
    sqlite3 --version
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
    jankurai audit . --policy .jankurai/audit-policy.toml --mode advisory --json .jankurai/repo-score.json --md .jankurai/repo-score.md --score-history .jankurai/score-history.jsonl --score-history-csv .jankurai/score-history.csv
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
