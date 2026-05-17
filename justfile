set shell := ["bash", "-euo", "pipefail", "-c"]
export RUSTC_WRAPPER := "sccache"

default: fast

fast:
  rtk cargo fmt --check
  ./scripts/check_file_sizes.sh
  rtk cargo check --workspace --locked
  rtk cargo nextest run --workspace --locked --no-fail-fast

hygiene:
  rtk cargo fmt --check
  ./scripts/check_file_sizes.sh

clippy:
  rtk cargo clippy --workspace --all-targets --locked -- -D warnings

medium:
  rtk cargo test --workspace --quiet --locked
  rtk cargo run -p redlinedb-cli -- --help
  rtk cargo run -p redlinedb-server -- --help

phase8-smoke:
  rtk cargo test --workspace --quiet --locked
  rtk cargo run -p redlinedb-cli -- --help
  rtk cargo run -p redlinedb-server -- --help

phase9-smoke:
  rtk cargo test -p redlinedb-bench --quiet --locked
  rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/smoke.toml --out-dir target/bench/certify-smoke --seed 7 --repetitions 1 --warmup 0
  rtk cargo run -p redlinedb-bench -- cross-engine --engine both --test-dir crates/bench/compat --seed 7

phase9-certify:
  rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/certify-certification --seed 7 --repetitions 5 --warmup 1

phase9-xbabe1-gap:
  ./scripts/bench/xbabe1_sync.sh
  ./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/gap-cert.toml --out-dir target/bench/xbabe1/gap-cert --seed 7 --repetitions 3 --warmup 1
  ./scripts/bench/xbabe1_fetch.sh gap-cert

phase9-xbabe1-gap-strace:
  ./scripts/bench/xbabe1_sync.sh
  ./scripts/bench/xbabe1_run.sh rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/gap-cert.toml --out-dir target/bench/xbabe1/gap-cert-strace --seed 7 --repetitions 3 --warmup 1 --with-strace
  ./scripts/bench/xbabe1_fetch.sh gap-cert-strace

phase11-oltp-gap:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/phase11-oltp-gap.toml --out-dir target/bench/phase11-oltp-gap --seed 7 --repetitions 3 --warmup 1

phase11-ephemeral-db:
  rtk cargo test -p redlinedb --test phase11_ephemeral --quiet --locked

phase11-sql-contracts:
  rtk cargo test -p redlinedb-sql --test phase11_temp_roots --quiet --locked
  rtk cargo test -p redlinedb-sql --test phase11_veox_queue --quiet --locked
  rtk cargo test -p redlinedb-sql --test phase11_xdoug_compat --quiet --locked

dick-head-choas:
  just dick-head-choas-smoke
  just dick-head-choas-xbabe1
  just dick-head-choas-xbabe1-extreme

dick-head-choas-smoke:
  rtk cargo test -p redlinedb-bench --test lane_bh chaos_suite_workloads_smoke --locked
  rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/dick-head-choas.toml --out-dir target/bench/dick-head-choas-smoke --seed 7 --repetitions 1 --warmup 0
  python3 scripts/bench/export_benchmark_results.py

dick-head-choas-xbabe1:
  ./scripts/bench/dick_head_choas_xbabe1.sh bounded

dick-head-choas-xbabe1-extreme:
  ./scripts/bench/dick_head_choas_xbabe1.sh extreme

connection-limit-256:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/connection-limit-256.toml --out-dir target/bench/xbabe1/connection-limit-256 --seed 7 --repetitions 3 --warmup 1

connection-fixed-high:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/connection-fixed-high.toml --out-dir target/bench/xbabe1/connection-fixed-high --seed 7 --repetitions 3 --warmup 1

queue-mixed-highload:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/queue-mixed-highload.toml --out-dir target/bench/xbabe1/queue-mixed-highload --seed 7 --repetitions 3 --warmup 1

phase10-cert-smoke:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-smoke.toml --out-dir target/bench/phase10-cert-smoke --seed 7 --repetitions 1 --warmup 0

phase10-cert-cert:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-cert.toml --out-dir target/bench/xbabe1/phase10-cert --seed 7 --repetitions 5 --warmup 1

phase10-cert-compare:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-compare.toml --out-dir target/bench/xbabe1/phase10-cert-compare --seed 7 --repetitions 5 --warmup 1

phase10-cert-stress:
  rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-stress.toml --out-dir target/bench/xbabe1/phase10-cert-stress --seed 7 --repetitions 5 --warmup 1

benchmark-export-existing:
  python3 scripts/bench/export_benchmark_results.py

benchmark-xbabe1-all:
  just phase9-smoke
  just phase9-certification
  just phase9-xbabe1-gap
  just phase9-xbabe1-gap-strace
  just phase9-xbabe1-certification
  just phase9-xbabe1-certify-with-strace
  just phase10-cert-smoke
  just phase10-cert-cert
  just phase10-cert-compare
  just phase10-cert-stress
  just phase11-oltp-gap
  just dick-head-choas
  just connection-limit-256
  just connection-fixed-high
  just queue-mixed-highload
  just phase9-recovery-matrix
  just phase9-failpoint-matrix

# Wave 6 Lane B: strace-instrumented certification (Linux-only). Wraps
# each per-engine bench child with `strace -c` so the manifest captures
# aggregate syscall counts.
phase9-certify-with-strace:
  rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/certify-strace --seed 7 --repetitions 5 --warmup 1 --with-strace

phase9-failpoint-matrix:
  rtk cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7

security:
  bash ops/ci/security.sh

pre-push:
  bash ops/git-hooks/pre-push

ci-doctor:
  bash scripts/ci-doctor.sh

release:
  rtk cargo build --workspace --release --locked

# Targeted iteration recipes for agent loops. Narrow lanes avoid the
# workspace-wide check + test that dominates `just fast` on cold caches.
# Audit reference: HLT-018 perf-concurrency-drift.
cache-warm:
  rtk cargo build --workspace --tests --locked

fast-nextest:
  rtk cargo fmt --check
  ./scripts/check_file_sizes.sh
  rtk cargo check --workspace --locked
  rtk cargo nextest run --workspace --locked --no-fail-fast

crate-check crate:
  rtk cargo check -p {{crate}} --locked

crate-test crate:
  rtk cargo test -p {{crate}} --locked --quiet

# D6 FFI symbol-diff lane (sqlite-parity closure plan, layer 2).
# Refreshes the libsqlite3 reference list (no-op when unchanged), builds
# the FFI cdylib, runs the symbol-diff integration test. The test fails
# until WS-B1-B5 export the in-scope sqlite3_* symbols (see
# crates/ffi/tests/symbol_allowlist.toml for legitimate exclusions).
ffi-symbol-diff:
  bash scripts/parity/dump-sqlite-symbols.sh
  rtk cargo build -p redlinedb-ffi --locked
  rtk cargo test -p redlinedb-ffi --test symbol_diff --quiet --locked -- --ignored

# D7 random-SQL fuzz parity lane (sqlite-parity closure plan, layer 2).
# Runs REDLINEDB_FUZZ_ITERS (default 1000) seedable iterations; each
# iteration runs the generated SQL through both rusqlite and RedlineDB
# and asserts (rows-normalized, error-class) equal. The gate is rate-
# monotone: divergence rate must not exceed the previously recorded
# baseline (see target/proof/sqlite-full-parity/fuzz-divergence.txt).
fuzz-parity:
  rtk cargo test -p redlinedb-bench --test fuzz_parity --quiet --locked -- --test-threads=1

# Extended differential fuzz (nightly CI only) — 100k iterations.
fuzz-parity-nightly:
  REDLINEDB_FUZZ_ITERS=100000 rtk cargo test -p redlinedb-bench --test fuzz_parity --release --quiet --locked

# jankurai scaffold Justfile
score:
	jankurai audit . --mode advisory --json agent/repo-score.json --md agent/repo-score.md --score-history agent/score-history.jsonl --score-history-csv agent/score-history.csv
doctor:
	jankurai doctor --fail-on high
rust-map:
	jankurai rust map .
rust-witness:
	jankurai rust witness build .
rust-diagnose:
	jankurai rust diagnose .
check: fast score security rust-map rust-witness rust-diagnose
