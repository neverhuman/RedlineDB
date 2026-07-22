#!/usr/bin/env bash
#
# Package a release tarball.
#
# Refactored from the prior single-line `just release-local` recipe to a
# glob-driven loop so new bundled files are automatically hashed and recorded
# in release-manifest.json. Local Jeryu evidence binds that manifest and the
# immutable tag; no external release or attestation service is involved.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
test -n "$version"
tag="${REDLINE_TESTING_RELEASE_TAG:-redline-testing-v${version}-jain.1}"
cargo run --locked --quiet -p xtask -- validate-release-tag --tag "$tag"
# Release bytes must not depend on the absolute checkout path or caller flags.
# The logical prefix is stable across every no-local release sandbox.
unset CARGO_ENCODED_RUSTFLAGS
export RUSTFLAGS="--remap-path-prefix=${repo_root}=/redline-testing"
cargo build --release --locked

target_name="linux-x86_64"
package="redline-testing-${version}-${target_name}"
pkg_dir="dist/${package}"

rm -rf "${pkg_dir}"
mkdir -p "${pkg_dir}/bin"
mkdir -p "${pkg_dir}/corpus/sqlite_parity/cases"
mkdir -p "${pkg_dir}/corpus/beyond_sqlite"
mkdir -p "${pkg_dir}/contracts"
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
cp contracts/*.toml "${pkg_dir}/contracts/"
cp metadata/beyond_sqlite/features.json "${pkg_dir}/metadata/beyond_sqlite/features.json"
cp schemas/*.json "${pkg_dir}/schemas/"
cp templates/*.md "${pkg_dir}/templates/"
shopt -u nullglob

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

commit="$(git rev-parse HEAD 2>/dev/null || printf unknown)"
tag_revision="${tag##*.}"
source_date_epoch="${SOURCE_DATE_EPOCH:-$(git show -s --format=%ct HEAD)}"
case "$source_date_epoch" in
  ''|*[!0-9]*)
    printf 'SOURCE_DATE_EPOCH must be an unsigned integer: %s\n' "$source_date_epoch" >&2
    exit 2
    ;;
esac

jq -n \
  --arg version "$version" \
  --arg target "$target_name" \
  --arg commit "$commit" \
  --arg tag "$tag" \
  --argjson tag_revision "$tag_revision" \
  --arg binary_sha "$binary_sha" \
  --argjson hashes "$artifact_hashes_obj" \
  '{
    name: "redline-testing",
    version: $version,
    target: $target,
    release_commit: $commit,
    release_tag: $tag,
    tag_revision: $tag_revision,
    binary: "bin/redline-testing",
    binary_sha256: $binary_sha,
    tarball_sha256_source: ".sha256 sidecar",
    artifact_hashes: $hashes,
    generated_by: "scripts/release-package.sh"
  }' > "${pkg_dir}/release-manifest.json"

LC_ALL=C tar -C dist --sort=name --format=gnu \
  --owner=0 --group=0 --numeric-owner --mtime="@${source_date_epoch}" \
  -cf - "${package}" \
  | gzip -n > "dist/${package}.tar.gz"
sha256sum "dist/${package}.tar.gz" > "dist/${package}.tar.gz.sha256"
cp "${pkg_dir}/release-manifest.json" dist/release-manifest.json

# Surface what landed.
echo "release package: dist/${package}.tar.gz"
echo "release manifest: ${pkg_dir}/release-manifest.json"
echo "artifact_hashes count: $(jq '.artifact_hashes | length' "${pkg_dir}/release-manifest.json")"
