#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
require_tool jq
mkdir -p target/jankurai/contract-drift
cargo run --locked --quiet -- validate-local-jeryu --manifest repos.manifest.toml --skip-remotes
cargo run --locked --quiet -- validate-manifest --manifest repos.manifest.toml --check-derived
cargo run --locked --quiet -- managed-repos --manifest repos.manifest.toml --json \
  > target/jankurai/contract-drift/managed-repositories.json
jq -e '
  .schema_version == "jain.managed-repositories/v1" and
  .repository_count == 34 and
  ([.repositories[].name] | index("jain-split-ops") != null) and
  ([.repositories[].name] | index("jain-smartcluster") != null) and
  ([.repositories[].name] | index("redline-central") != null) and
  ([.repositories[].name] | index("redline-split-ops") != null)
' target/jankurai/contract-drift/managed-repositories.json >/dev/null
repo_count="$(jq '.repository_count' target/jankurai/contract-drift/managed-repositories.json)"
write_receipt target/jankurai/contract-drift/receipt.json pass
printf 'contract-drift ok: jain-split-ops (%s managed repos)\n' "$repo_count"
