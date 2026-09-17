#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for contract validation\n' >&2
  exit 1
}

[[ "$(tr -d '\n' < VERSION)" == "4.1.0" ]]
cargo metadata --locked --no-deps --format-version 1 | jq -e '
  [.packages[] | select(.name == "redlinedb-client" or .name == "db-shim") | .version]
    | length == 2 and all(. == "4.1.0")
' >/dev/null
cargo test --locked --workspace --all-targets
grep -Fq 'EXPOSE 6033' docker/Dockerfile
grep -Fq '6033:6033' docker/docker-compose.yml
grep -Fq 'redline-central-v4.1.0-jain.4' docs/release.md
printf 'contract drift ok: native 4.1.0 identity and central-service packaging\n'
