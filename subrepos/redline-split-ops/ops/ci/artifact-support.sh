#!/usr/bin/env bash
set -euo pipefail

# shellcheck disable=SC1091,SC2154
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
# shellcheck disable=SC2154
cd "$repo_root"

mkdir -p target/artifact-support
archive="target/artifact-support/redline-split-ops-source.tar"
git archive --format=tar --output "$archive" HEAD

head_sha="$(git rev-parse --verify 'HEAD^{commit}')"
archive_sha="$(sha256sum "$archive" | awk '{print $1}')"
lock_sha="$(sha256sum Cargo.lock | awk '{print $1}')"
manifest_sha="$(sha256sum repos.manifest.toml | awk '{print $1}')"

jq -n \
  --arg head_sha "$head_sha" \
  --arg archive "$archive" \
  --arg archive_sha256 "$archive_sha" \
  --arg cargo_lock_sha256 "$lock_sha" \
  --arg manifest_sha256 "$manifest_sha" \
  '{
    schema_version:"redline.artifact-support/v1",
    status:"pass",
    repository:"redline-split-ops",
    release_version:"8.0.0",
    release_status:"candidate",
    formal_ga:false,
    head_sha:$head_sha,
    source_archive:$archive,
    source_archive_sha256:$archive_sha256,
    cargo_lock_sha256:$cargo_lock_sha256,
    manifest_sha256:$manifest_sha256
  }' >target/artifact-support/receipt.json
sha256sum target/artifact-support/receipt.json \
  >target/artifact-support/receipt.json.sha256

printf 'Redline artifact support ok: source archive SHA-256 %s\n' "$archive_sha"
