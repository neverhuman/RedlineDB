#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

mkdir -p target/policy
rustc --edition=2021 --test tools/no-python-policy.rs \
  -o target/policy/no-python-policy-tests
target/policy/no-python-policy-tests --nocapture
