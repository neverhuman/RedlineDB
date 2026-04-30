set shell := ["bash", "-euo", "pipefail", "-c"]

default: fast

fast:
  rtk cargo fmt --check
  ./scripts/check_file_sizes.sh
  rtk cargo check --workspace --locked
  rtk cargo test --workspace --quiet --locked

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
  rtk cargo run -p redlinedb-bench -- compat --engine both --test-dir crates/bench/compat --seed 7

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
  rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/phase11-oltp-gap.toml --out-dir target/bench/phase11-oltp-gap --seed 7 --repetitions 3 --warmup 1

# Wave 6 Lane B: strace-instrumented certification (Linux-only). Wraps
# each per-engine bench child with `strace -c` so the manifest captures
# aggregate syscall counts.
phase9-certify-with-strace:
  rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/certify-strace --seed 7 --repetitions 5 --warmup 1 --with-strace

phase9-failpoint-matrix:
  rtk cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7

security:
  rtk cargo audit
  rtk cargo deny check
  rtk gitleaks detect --source .

security-local:
  rtk cargo audit
  rtk cargo deny check
  rtk gitleaks detect --source .

release:
  rtk cargo build --workspace --release --locked
