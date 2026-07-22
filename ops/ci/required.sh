#!/usr/bin/env bash
# Required lane for the Rust jain-split-ops control plane. Python is not a
# control-plane runtime dependency; parity-only Python lives in bounded test
# surfaces outside this operational lane.
set -euo pipefail

bash ops/ci/typed-required-non-test.sh pre-rust-tests
cargo test --locked
bash ops/ci/typed-required-non-test.sh post-rust-tests

printf 'required ok: jain-split-ops\n'
