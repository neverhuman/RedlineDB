#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
require_tool jq
require_tool cmp
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
jq -e 'select(.properties.schema_version.const == "jain.release-flow/v1")' \
  schemas/release-flow.schema.json >/dev/null
for output in release-flow.json release-flow-repeat.json; do
  ATOMICSOUL_PUSH=0 cargo run --locked --quiet -- release-flow \
    --manifest "$REPO_ROOT/repos.manifest.toml" \
    --evidence-root "$REPO_ROOT/docs/release-evidence/8.0.1" \
    >"target/jankurai/contract-drift/$output"
done
cmp target/jankurai/contract-drift/release-flow.json \
  target/jankurai/contract-drift/release-flow-repeat.json
jq -e '
  .schema_version == "jain.release-flow/v1" and
  .release == "8.0.1" and
  .mode == "read-only" and
  .external_state_changed == false and
  (.receipt_integrity_sha256 | test("^[0-9a-f]{64}$")) and
  ([.gates[].id] == [
    "candidate_invariants", "authority_manifest", "control_plane",
    "repository_state", "redline_dependency", "forge_readback",
    "cloud_release_spec"
  ]) and
  (.next_action.argv_without_secrets | type == "array")
' target/jankurai/contract-drift/release-flow.json >/dev/null
write_receipt target/jankurai/contract-drift/receipt.json pass
printf 'contract-drift ok: jain-split-ops (%s managed repos)\n' "$repo_count"
