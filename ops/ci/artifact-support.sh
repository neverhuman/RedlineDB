#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for artifact evidence\n' >&2
  exit 1
}

cargo build --release --locked --workspace --all-targets
target_directory="$(cargo metadata --locked --format-version 1 --no-deps \
  | jq -er '.target_directory | select(type == "string" and length > 0)')"
[[ -d "$target_directory" && ! -L "$target_directory" ]] || {
  printf 'Cargo target directory must be a physical directory: %s\n' "$target_directory" >&2
  exit 1
}
for binary in redlinedb-client-smoke; do
  [[ -f "$target_directory/release/$binary" \
    && ! -L "$target_directory/release/$binary" \
    && -x "$target_directory/release/$binary" ]] || {
    printf 'missing physical release binary: %s\n' "$target_directory/release/$binary" >&2
    exit 1
  }
done
version="$(tr -d '\n' < VERSION)"
[[ "$version" == "4.1.0" ]]
rm -rf -- target/artifact-support/package
mkdir -p target/artifact-support/package
install -m 0755 "$target_directory/release/redlinedb-client-smoke" \
  target/artifact-support/package/redlinedb-client-smoke
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
