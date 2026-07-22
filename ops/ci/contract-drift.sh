#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
require_tool jq
mkdir -p target/jankurai/contract-drift
cargo test --locked ci::tests::published_contracts_match_runtime_shape
cargo run --locked --quiet -- validate-local-jeryu --manifest repos.manifest.toml --skip-remotes
cargo run --locked --quiet -- validate-manifest --manifest repos.manifest.toml --check-derived
cargo run --locked --quiet -- managed-repos --manifest repos.manifest.toml --json \
  > target/jankurai/contract-drift/managed-repositories.json
jq -e '
  .schema_version == "jain.managed-repositories/v1" and
  .repository_count == ([.repositories[].name] | unique | length) and
  .repository_count == ([.repositories[].path] | unique | length) and
  .repository_count == ([.repositories[].remote] | unique | length) and
  .repository_count == ([.family_counts[]] | add) and
  .active_repository_count == ([.repositories[] | select(.inventory_status == "active")] | length) and
  ([.repositories[].name] | index("jain-split-ops") != null) and
  ([.repositories[].name] | index("jain-smartcluster") != null) and
  ([.repositories[].name] | index("redline-central") != null) and
  ([.repositories[].name] | index("redline-split-ops") != null) and
  ([.repositories[].name] | index("jeryu-release-ops") != null) and
  ([.repositories[].name] | index("jeryu-web") != null) and
  .family_counts["jain-split"] > 0 and
  .family_counts["redline-split"] > 0 and
  .family_counts["jeryu-split"] > 0 and
  (.nested_family_gates[] | select(.family == "jeryu-split") |
    .symlink_policy == "retirement-pending" and .retirement_pending == ["jeryu-web"])
' target/jankurai/contract-drift/managed-repositories.json >/dev/null
repo_count="$(jq '.repository_count' target/jankurai/contract-drift/managed-repositories.json)"
write_receipt target/jankurai/contract-drift/receipt.json pass
printf 'contract-drift ok: jain-split-ops (%s managed repos)\n' "$repo_count"
