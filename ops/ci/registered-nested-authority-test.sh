#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

test_root="$(mktemp -d target/registered-nested-authority.XXXXXX)"
trap 'rm -rf -- "$test_root"' EXIT

# Change a valid outer projection identity without touching the child authority.
# Native validation must reach the recursive comparator and reject the drift.
sed 's/current_tag = "jeryu-v5.0.0-split.0"/current_tag = "jeryu-v5.0.0-split.1"/' \
  repos.manifest.toml >"$test_root/repos.manifest.toml"
if cargo run --locked --quiet -- validate-local-jeryu \
  --manifest "$test_root/repos.manifest.toml" --skip-remotes \
  >"$test_root/stdout" 2>"$test_root/stderr"; then
  printf 'native validation accepted a drifted registered child authority\n' >&2
  exit 1
fi
grep -F \
  'nested_families.jeryu.repository[jeryu].current_tag differs from child authority' \
  "$test_root/stderr" >/dev/null

printf 'registered nested authority hostile test ok\n'
