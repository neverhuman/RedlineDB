#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for artifact evidence\n' >&2
  exit 1
}

cargo build --release --locked --workspace --all-targets
version="$(tr -d '\n' < VERSION)"
[[ "$version" == "4.1.0" ]]
mkdir -p target/artifact-support/package
install -m 0755 target/release/redlinedb-client-smoke \
  target/artifact-support/package/redlinedb-client-smoke
install -m 0755 target/release/db-shim-parity \
  target/artifact-support/package/db-shim-parity
install -m 0644 README.md docs/release.md docker/Dockerfile docker/docker-compose.yml \
  target/artifact-support/package/
tar -C target/artifact-support/package -czf \
  "target/artifact-support/redline-central-v${version}.tar.gz" .
artifact="target/artifact-support/redline-central-v${version}.tar.gz"
jq -n \
  --arg repo "redline-central" \
  --arg version "$version" \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  --arg lock_sha256 "$(sha256sum Cargo.lock | awk '{print $1}')" \
  --arg artifact_sha256 "$(sha256sum "$artifact" | awk '{print $1}')" \
  '{schema_version:"redline-central.artifact-support/v1",repo:$repo,version:$version,commit:$commit,tree:$tree,lock_sha256:$lock_sha256,artifact_sha256:$artifact_sha256,status:"pass"}' \
  > target/artifact-support/evidence.json
printf 'artifact support ok: redline-central v%s\n' "$version"
