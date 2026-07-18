#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'contract-drift lane: validate manifest and local-Jeryu policy contracts'
require_tool jq
for schema in schemas/program-*.schema.json; do
  jq -e 'type == "object"' "$schema" >/dev/null
done
mkdir -p target/jankurai/contract-drift
cargo run --locked --quiet -- validate-local-jeryu --manifest repos.manifest.toml \
  --skip-remotes --skip-program-checkouts
# The standalone exact-SHA control-plane checkout has no sibling product repositories. Program
# lifecycle evidence remains pending; the ordinary family validator must perform that census.
cargo run --locked --quiet -- validate-manifest --manifest repos.manifest.toml --check-derived
cargo run --locked --quiet -- program-release validate-all --authority-dir authority
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
