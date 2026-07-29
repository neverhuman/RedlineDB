#!/usr/bin/env bash
#
# Package a release tarball.
#
# Refactored from the prior single-line `just release-local` recipe to a
# glob-driven loop so new bundled files are automatically hashed and recorded
# in release-manifest.json (which the Sigstore attestation covers via the
# `actions/attest-build-provenance` step on the GitHub release workflow).

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
test -n "$version"
tag="${GITHUB_REF_NAME:-${REDLINE_TESTING_RELEASE_TAG:-}}"
if [[ -z "$tag" ]]; then
  printf 'release package: set GITHUB_REF_NAME or REDLINE_TESTING_RELEASE_TAG explicitly\n' >&2
  exit 1
fi
cargo run --locked --quiet -p xtask -- validate-release-tag --tag "$tag"

identity_dir="target/release-package-identity"
source_identity_receipt="${identity_dir}/source-identity.json"
tag_identity_receipt="${identity_dir}/tag-identity.json"
rm -rf "$identity_dir"
mkdir -p "$identity_dir"
ops/ci/source-identity.sh snapshot "$source_identity_receipt"
ops/ci/release-tag-identity.sh "$tag" "$tag_identity_receipt"

cargo build --release --locked
ops/ci/source-identity.sh verify "$source_identity_receipt"

target_name="linux-x86_64"
package="redline-testing-${version}-${target_name}"
pkg_dir="dist/${package}"

rm -rf "${pkg_dir}"
mkdir -p "${pkg_dir}/bin"
mkdir -p "${pkg_dir}/corpus/sqlite_parity/cases"
mkdir -p "${pkg_dir}/corpus/beyond_sqlite"
mkdir -p "${pkg_dir}/metadata/beyond_sqlite"
mkdir -p "${pkg_dir}/schemas"
mkdir -p "${pkg_dir}/templates"

# Binary first (hashed separately at the top level).
cp target/release/redline-testing "${pkg_dir}/bin/redline-testing"

# Corpora — both the pinned upstream manifest and every locally-authored
# shard (hand-curated 10_* through 33_*; generated gen_*.json).
cp corpus/sqlite_parity/generated_manifest.json "${pkg_dir}/corpus/sqlite_parity/generated_manifest.json"
shopt -s nullglob
for shard in corpus/sqlite_parity/cases/*.json; do
  cp "$shard" "${pkg_dir}/corpus/sqlite_parity/cases/$(basename "$shard")"
done
cp corpus/beyond_sqlite/generated_manifest.json "${pkg_dir}/corpus/beyond_sqlite/generated_manifest.json"
cp metadata/beyond_sqlite/features.json "${pkg_dir}/metadata/beyond_sqlite/features.json"
cp schemas/*.json "${pkg_dir}/schemas/"
cp templates/*.md "${pkg_dir}/templates/"
shopt -u nullglob

ops/ci/verify-release-inventory.sh "$pkg_dir"
ops/ci/source-identity.sh verify "$source_identity_receipt"

binary_sha="$(sha256sum "${pkg_dir}/bin/redline-testing" | awk '{ print $1 }')"

# Build artifact_hashes: every file under ${pkg_dir} except release-manifest.json
# (the manifest itself, written last) and the binary (hashed in its own field).
artifact_hashes_obj="$(cd "${pkg_dir}" && find . -type f \
    ! -name 'release-manifest.json' \
    ! -path './bin/*' \
    -print0 \
  | xargs -0 sha256sum \
  | awk '{
      # Strip the leading "./".
      sub(/^\.\//, "", $2);
      printf("%s\0%s\0", $2, $1);
    }' \
  | jq -Rs '
      split("\u0000")
      | . as $a
      | reduce range(0; ($a | length) - 1; 2) as $i ({}; .[$a[$i]] = $a[$i + 1])
    ')"

commit="$(jq -er '.release_commit' "$tag_identity_receipt")"
tree="$(jq -er '.release_tree' "$tag_identity_receipt")"
tag_state="$(jq -er '.tag_state' "$tag_identity_receipt")"
source_archive_sha="$(jq -er '.source_archive_sha256' "$source_identity_receipt")"
tag_revision="${tag##*.}"

jq -n \
  --arg version "$version" \
  --arg target "$target_name" \
  --arg commit "$commit" \
  --arg tree "$tree" \
  --arg tag "$tag" \
  --arg tag_state "$tag_state" \
  --arg source_archive_sha "$source_archive_sha" \
  --argjson tag_revision "$tag_revision" \
  --arg binary_sha "$binary_sha" \
  --argjson hashes "$artifact_hashes_obj" \
  '{
    name: "redline-testing",
    version: $version,
    target: $target,
    release_commit: $commit,
    release_tree: $tree,
    release_tag: $tag,
    release_tag_state: $tag_state,
    tag_revision: $tag_revision,
    source_archive_sha256: $source_archive_sha,
    binary: "bin/redline-testing",
    binary_sha256: $binary_sha,
    tarball_sha256_source: ".sha256 sidecar",
    artifact_hashes: $hashes,
    generated_by: "scripts/release-package.sh"
  }' > "${pkg_dir}/release-manifest.json"

ops/ci/verify-release-inventory.sh "$pkg_dir"
ops/ci/source-identity.sh verify "$source_identity_receipt"

tar -C dist --sort=name --owner=0 --group=0 --numeric-owner \
  -czf "dist/${package}.tar.gz" "${package}"
ops/ci/verify-release-inventory.sh "$pkg_dir" "dist/${package}.tar.gz"
ops/ci/source-identity.sh verify "$source_identity_receipt"
sha256sum "dist/${package}.tar.gz" > "dist/${package}.tar.gz.sha256"
cp "${pkg_dir}/release-manifest.json" dist/release-manifest.json
ops/ci/source-identity.sh verify "$source_identity_receipt"

# Surface what landed.
echo "release package: dist/${package}.tar.gz"
echo "release manifest: ${pkg_dir}/release-manifest.json"
echo "artifact_hashes count: $(jq '.artifact_hashes | length' "${pkg_dir}/release-manifest.json")"
