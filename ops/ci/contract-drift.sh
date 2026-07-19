#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
require_tool jq
mkdir -p target/jankurai/contract-drift
cargo run --locked --quiet -- distributed-evidence-index \
  --release 9.0.0-distributed.1 \
  --root docs/release-evidence/9.0.0-distributed.1 \
  --historical-root-if-present docs/release-evidence/9.0.0-alpha.6 \
  --historical-root-if-present docs/release-evidence/9.0.0-appliance.1 \
  > target/jankurai/contract-drift/distributed-evidence-index.json
jq -e '
  .schema_version == "jain.distributed-evidence-index/v1" and
  .release == "9.0.0-distributed.1" and
  .status == "pass" and
  .file_count >= 1 and
  (.files | all(.sha256 | test("^[0-9a-f]{64}$")))
' target/jankurai/contract-drift/distributed-evidence-index.json >/dev/null
for schema in \
  schemas/distributed-release.v1.schema.json \
  schemas/soak-status.v1.schema.json \
  schemas/accelerated-qualification.v1.schema.json; do
  jq -e '
    .type == "object" and
    .additionalProperties == false and
    .properties.release.const == "9.0.0-distributed.1"
  ' "$schema" >/dev/null
done
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
