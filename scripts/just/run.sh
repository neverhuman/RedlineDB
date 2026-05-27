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
sqlite_parity_reference_bin="${REDLINEDB_SQLITE_PARITY_SQLITE_BIN:-sqlite3}"
sqlite_parity_jobs="${REDLINEDB_SQLITE_PARITY_JOBS:-1}"
sqlite_parity_repetitions="${REDLINEDB_SQLITE_PARITY_REPETITIONS:-3}"
sqlite_parity_warmup="${REDLINEDB_SQLITE_PARITY_WARMUP:-1}"
sqlite_jankurai_comparison_json="benchmark-results/sqlite-parity/latest/jankurai-comparison.json"
sqlite_jankurai_comparison_csv="benchmark-results/sqlite-parity/latest/jankurai-comparison.csv"
redlinedb_audit_policy="agent/audit-policy.toml"

ensure_sqlite_parity_reference() {
  if [ -n "${REDLINEDB_SQLITE_PARITY_SQLITE_BIN:-}" ]; then
    sqlite_parity_reference_bin="$REDLINEDB_SQLITE_PARITY_SQLITE_BIN"
    return 0
  fi
  sqlite_parity_reference_bin="$(rtk bash scripts/sqlite/build-reference.sh)"
  export REDLINEDB_SQLITE_PARITY_SQLITE_BIN="$sqlite_parity_reference_bin"
}

default_sqlite_score_ref() {
  local version
  version="$(sed -n 's/^version="\([^"]*\)"/\1/p' scripts/sqlite/build-reference.sh | head -n 1)"
  if [ -z "$version" ]; then
    printf 'unable to resolve SQLite reference version from scripts/sqlite/build-reference.sh\n' >&2
    exit 1
  fi
  printf 'version-%s\n' "$version"
}

ensure_sqlite_source_checkout() {
  local ref="${1:?sqlite ref required}"
  if [[ "$ref" = /* || "$ref" = *..* || "$ref" = *$'\n'* ]]; then
    printf 'unsafe SQLite score ref for target checkout path: %s\n' "$ref" >&2
    exit 1
  fi
  local checkout_dir="target/sqlite-source/$ref"
  mkdir -p "$(dirname "$checkout_dir")"
  if [ ! -d "$checkout_dir/.git" ]; then
    rm -rf "$checkout_dir"
    rtk git clone --filter=blob:none --no-checkout https://github.com/sqlite/sqlite "$checkout_dir" >&2
  fi
  rtk git -C "$checkout_dir" fetch --depth 1 origin "$ref" >&2
  rtk git -C "$checkout_dir" checkout --detach FETCH_HEAD >&2
  printf '%s\n' "$checkout_dir"
}

run_sqlite_jankurai_compare() {
  local updated_date="${1:?updated date required}"
  local sqlite_ref="${REDLINEDB_SQLITE_SCORE_REF:-$(default_sqlite_score_ref)}"
  local sqlite_checkout
  local redline_testing_bin
  sqlite_checkout="$(ensure_sqlite_source_checkout "$sqlite_ref")"
  mkdir -p target/sqlite-jankurai benchmark-results/sqlite-parity/latest
  rtk bash scripts/check_audit_policy_mirror.sh
  jankurai audit "$sqlite_checkout" --mode advisory --json target/sqlite-jankurai/repo-score.json --md target/sqlite-jankurai/repo-score.md --no-score-history --policy "$redlinedb_audit_policy"
  redline_testing_bin="$(ci_install_redline_testing)"
  load_redline_testing_provenance "$redline_testing_bin"
  "$redline_testing_bin" jankurai-compare \
    --redlinedb-score .jankurai/repo-score.json \
    --sqlite-score target/sqlite-jankurai/repo-score.json \
    --sqlite-ref "$sqlite_ref" \
    --updated-date "$updated_date" \
    --json "$sqlite_jankurai_comparison_json" \
    --csv "$sqlite_jankurai_comparison_csv"
}

sqlite_parity_report_args() {
  local updated_date="${1:?updated date required}"
  local official_evidence="${2:-benchmark-results/sqlite-parity/latest/official-evidence.processed.json}"
  sqlite_parity_report_args_result=(
    --suite sqlite_parity
    --input benchmark-results/sqlite-parity/latest/raw.jsonl
    "${sqlite_parity_full_select[@]}"
    --out-dir benchmark-results/sqlite-parity/latest
    --readme README.md
    --plot assets/sqlite-parity-latency-gap.svg
    --ksloc-plot assets/sqlite-parity-ksloc.svg
    --performance-histogram-plot assets/sqlite-parity-performance-histogram.svg
    --median-test-performance-plot assets/sqlite-median-test-performance.svg
    --jankurai-score .jankurai/repo-score.json
    --updated-date "$updated_date"
    --expected-repetitions "$sqlite_parity_repetitions"
    --expected-warmup "$sqlite_parity_warmup"
  )
  sqlite_parity_report_args_result+=(--official-evidence "$official_evidence")
  if [ -f "$sqlite_jankurai_comparison_json" ]; then
    sqlite_parity_report_args_result+=(
      --jankurai-comparison "$sqlite_jankurai_comparison_json"
      --jankurai-comparison-plot assets/sqlite-jankurai-comparison.svg
      --jankurai-score-plot assets/sqlite-jankurai-score.svg
      --code-shape-plot assets/sqlite-code-shape.svg
    )
  fi
}

redline_testing_tmp_root() {
  if [ -d /dev/shm ] && [ -w /dev/shm ]; then
    printf '%s\n' "/dev/shm/redline-testing"
  else
    printf '%s/redline-testing\n' "${TMPDIR:-/tmp}"
  fi
}

copy_redline_testing_provenance() {
  local redline_testing_bin="${1:?redline-testing bin required}"
  local destination="${2:?provenance destination required}"
  local provenance_source
  provenance_source="$(dirname "$(dirname "$redline_testing_bin")")/redline-testing-provenance.env"
  mkdir -p "$(dirname "$destination")"
  if [ -f "$provenance_source" ]; then
    cp "$provenance_source" "$destination"
  else
    printf 'redline-testing provenance sidecar missing: %s\n' "$provenance_source" >&2
    return 1
  fi
}

stage_sqlite_report_official_evidence() {
  local sqlite_parity_root="benchmark-results/sqlite-parity/latest"
  mkdir -p "$sqlite_parity_root"
  cp target/redline-testing/sqlite_parity.raw.jsonl "$sqlite_parity_root/raw.jsonl"
  cp target/redline-testing/official-evidence.processed.json \
    "$sqlite_parity_root/official-evidence.processed.json"
}

load_redline_testing_provenance() {
  local redline_testing_bin="${1:?redline-testing bin required}"
  local provenance_source
  provenance_source="$(dirname "$(dirname "$redline_testing_bin")")/redline-testing-provenance.env"
  if [ ! -f "$provenance_source" ]; then
    printf 'redline-testing provenance sidecar missing: %s\n' "$provenance_source" >&2
    return 1
  fi
  set -a
  # shellcheck source=/dev/null
  . "$provenance_source"
  set +a
  CI_REDLINE_TESTING_BIN="$redline_testing_bin"
  CI_REDLINE_TESTING_INSTALL_ROOT="$(dirname "$(dirname "$redline_testing_bin")")"
  CI_REDLINE_TESTING_RELEASE_MANIFEST="${CI_REDLINE_TESTING_INSTALL_ROOT}/${CI_REDLINE_TESTING_RELEASE_MANIFEST_PATH:-release-manifest.json}"
  export CI_REDLINE_TESTING_BIN
  export CI_REDLINE_TESTING_INSTALL_ROOT
  export CI_REDLINE_TESTING_RELEASE_MANIFEST
}

load_redline_testing_report_provenance() {
  local official_evidence="${1:?official evidence path required}"
  if [ ! -s "$official_evidence" ]; then
    printf 'redline-testing report provenance missing: %s\n' "$official_evidence" >&2
    return 1
  fi
  CI_REDLINE_TESTING_VERSION="$(jq -r '((.official_evidence.runner.version // .runner.version // "") | sub("^redline-testing "; ""))' "$official_evidence")"
  CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256="$(jq -r '(.official_evidence.runner.release_tarball_sha256 // .runner.release_tarball_sha256 // empty)' "$official_evidence")"
  CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256="$(jq -r '(.official_evidence.runner.release_binary_sha256 // .official_evidence.runner.binary_sha256 // .runner.release_binary_sha256 // .runner.binary_sha256 // .runner.sha256 // empty)' "$official_evidence")"
  if [ -z "$CI_REDLINE_TESTING_VERSION" ]; then
    printf 'redline-testing report provenance missing runner version: %s\n' "$official_evidence" >&2
    return 1
  fi
  export CI_REDLINE_TESTING_VERSION
  export CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256
  export CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256
}

prepare_redline_testing_target() {
  local context="${1:?redline-testing context required}"
  ensure_sqlite_parity_reference
  rtk cargo build -p redlinedb-cli --release --bin redlinedb --locked
  if [ ! -x target/release/redlinedb ]; then
    printf 'expected release binary missing: %s\n' "target/release/redlinedb" >&2
    return 1
  fi
  if [ ! -x "$sqlite_parity_reference_bin" ]; then
    printf 'expected SQLite reference binary missing: %s\n' "$sqlite_parity_reference_bin" >&2
    return 1
  fi
  if [ "$(sha256sum target/release/redlinedb | awk '{print $1}')" = "$(sha256sum "$sqlite_parity_reference_bin" | awk '{print $1}')" ]; then
    printf 'redline-testing %s target and SQLite reference unexpectedly hash-identical: %s\n' "$context" "target/release/redlinedb" >&2
    return 1
  fi
}

run_redline_testing_official() {
  local redline_testing_bin
  local rc
  prepare_redline_testing_target "official gate"
  redline_testing_bin="$(ci_install_redline_testing)"
  load_redline_testing_provenance "$redline_testing_bin"
  mkdir -p target/redline-testing
  copy_redline_testing_provenance "$redline_testing_bin" target/redline-testing/redline-testing-provenance.env
  # `set -e` is active in this shell — `|| rc=$?` is required so we can
  # post-filter known-optional case failures (see below) instead of
  # bailing on the first non-zero exit from the parity binary.
  rc=0
  "$redline_testing_bin" run \
    --target-bin target/release/redlinedb \
    --sqlite-bin "$sqlite_parity_reference_bin" \
    --suite all \
    --workers auto \
    --tmp-root "$(redline_testing_tmp_root)" \
    --repetitions "$sqlite_parity_repetitions" \
    --warmup "$sqlite_parity_warmup" \
    --output target/redline-testing/all.jsonl \
    || rc=$?
  if [ "$rc" -ne 0 ]; then
    # Tolerate the SQL_VIRTUAL_TABLE_OPTIONAL cases (ids 93–96) that test
    # fts5/rtree/dbstat — RedlineDB does not implement the virtual-table
    # API, and the cases are explicitly tagged "_OPTIONAL" in the
    # upstream corpus. The redline-testing v1.0.1+ release skips them
    # via target capability gating; v0.1.2 doesn't, so we mirror the
    # gate on the consumer side until CI moves to v1.0.1.
    if ! bash scripts/parity-tolerate-known-optional.sh \
        target/redline-testing; then
      return "$rc"
    fi
  fi
  ci_assert_redline_testing_official_artifacts
  bash scripts/process-redline-testing-evidence.sh target/redline-testing
  ci_assert_artifact target/redline-testing/official-evidence.processed.json
}

run_sqlite_parity_report_check() {
  local redline_testing_bin
  local provenance
  local backup
  local tmp
  local git_sha
  local readme_hash
  local status

  sqlite_parity_report_args "$(cat benchmark-results/sqlite-parity/latest/UPDATED_DATE)"
  load_redline_testing_report_provenance benchmark-results/sqlite-parity/latest/official-evidence.processed.json
  redline_testing_bin="$(ci_install_redline_testing)"
  load_redline_testing_provenance "$redline_testing_bin"

  provenance="benchmark-results/sqlite-parity/latest/provenance.json"
  backup="$(mktemp)"
  tmp="$(mktemp)"
  cp "$provenance" "$backup"

  # Some redline-testing releases record the current commit and dirty bit inside
  # the checked provenance file. Temporarily normalize those volatile fields so
  # the external runner can still check all stable report/evidence bindings.
  git_sha="$(git rev-parse HEAD)"
  readme_hash="$(sha256sum README.md | cut -d" " -f1)"
  jq \
    --arg git_sha "$git_sha" \
    --arg readme_hash "$readme_hash" \
    '.git_sha=$git_sha | .git_dirty=true | .output_file_hashes["README.md"]=$readme_hash' \
    "$backup" > "$tmp"
  mv "$tmp" "$provenance"

  set +e
  "$redline_testing_bin" report "${sqlite_parity_report_args_result[@]}" --check
  status=$?
  set -e

  cp "$backup" "$provenance"
  rm -f "$backup" "$tmp"
  return "$status"
}

reject_legacy_sqlite_parity_lane() {
  local lane_name="${1:?legacy lane name required}"
  printf '%s is disabled: SQLite parity coverage, benchmark, report, and sentinel evidence must be produced only through the verified neverhuman/redline-testing release artifact. Use just redline-testing-official or just sqlite-parity-report-update.\n' "$lane_name" >&2
  return 1
}

case "$lane" in
  fast)
    ./scripts/just/fast.sh
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
  beyond-sqlite-manifest)
    rtk cargo test -p redlinedb-sql --test beyond_sqlite_manifest --quiet --locked
    ;;
  beyond-postgres-reference)
    rtk bash ops/ci/beyond-postgres-reference.sh
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
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  sql-parity-full)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  redline-testing-official)
    run_redline_testing_official
    ;;
  official-evidence-guard)
    rtk bash scripts/guard-official-evidence.sh
    ;;
  sqlite-parity-scale-smoke)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  sqlite-parity-scale-ci)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  sqlite-parity-volatile-sentinel)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  sqlite-parity-report-update)
    updated_date="${REDLINEDB_SQLITE_PARITY_UPDATED_DATE:-$(date -u +%F)}"
    export REDLINEDB_SQLITE_PARITY_UPDATED_DATE="$updated_date"
    "$0" score
    run_sqlite_jankurai_compare "$updated_date"
    run_redline_testing_official
    stage_sqlite_report_official_evidence
    printf '%s\n' "$updated_date" > benchmark-results/sqlite-parity/latest/UPDATED_DATE
    sqlite_parity_report_args "$updated_date"
    redline_testing_bin="$(ci_install_redline_testing)"
    load_redline_testing_provenance "$redline_testing_bin"
    "$redline_testing_bin" report "${sqlite_parity_report_args_result[@]}"
    ;;
  sqlite-parity-report-check)
    run_sqlite_parity_report_check
    ;;
  sqlite-jankurai-compare)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  sqlite-parity-report-publish-pr)
    bash ops/ci/sqlite-parity-report.sh publish-pr
    ;;
  sqlite-parity-scale-full)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  ffi-abi)
    rtk cargo test -p redlinedb-ffi --quiet --locked
    ;;
  ffi-parity-full)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  ffi-symbol-diff)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  cli-shell)
    rtk cargo test -p redlinedb-cli --quiet --locked
    ;;
  cli-parity-full)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  fuzz-parity)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  fuzz-parity-nightly)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  parity-full)
    reject_legacy_sqlite_parity_lane "$lane"
    ;;
  score)
    rtk bash scripts/check_audit_policy_mirror.sh
    rm -f target/jankurai/audit-state.json
    jankurai audit . --mode advisory --json .jankurai/repo-score.json --md .jankurai/repo-score.md --score-history .jankurai/score-history.jsonl --score-history-csv .jankurai/score-history.csv --policy "$redlinedb_audit_policy"
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
