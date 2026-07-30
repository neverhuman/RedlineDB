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

if [[ "${REDLINE_TESTING_RELEASE_INTERNAL:-0}" != 1 ]]; then
  for forbidden in \
    CARGO_BUILD_RUSTC CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER \
    RUSTC RUSTC_LINKER RUSTC_WRAPPER RUSTFLAGS; do
    if [[ -v "$forbidden" ]]; then
      printf 'release package: caller build override is forbidden: %s\n' \
        "$forbidden" >&2
      exit 1
    fi
  done

  source_receipt="$repo_root/target/release-package-source.json"
  ops/ci/source-identity.sh snapshot "$source_receipt"
  release_head="$(git rev-parse --verify HEAD)"
  release_sandbox="$(mktemp -d "$repo_root/target/release-sandbox.XXXXXX")"
  cleanup_release_sandbox() {
    sandbox_real="$(realpath -e -- "$release_sandbox" 2>/dev/null || true)"
    case "$sandbox_real" in
      "$repo_root"/target/release-sandbox.??????)
        [[ "$sandbox_real" == "$release_sandbox" && ! -L "$sandbox_real" ]] \
          && rm -rf -- "$sandbox_real"
        ;;
    esac
  }
  trap cleanup_release_sandbox EXIT

  projection="$release_sandbox/source"
  ops/ci/release-source-projection.sh "$repo_root" "$projection" "$release_head"

  cargo_registry_source="$(realpath -e -- /var/lib/jain-host-ci/cargo-registry)"
  [[ -d "$cargo_registry_source" && ! -L "$cargo_registry_source" \
      && "$(stat -c '%u:%g:%a' "$cargo_registry_source")" == "0:0:555" ]] || {
    printf 'release package: governed Cargo registry source is unavailable\n' >&2
    exit 1
  }
  cargo_stage_tool="/home/ubuntu/jain-split/target/release-tools/splitctl-9d1b57223daa162d52882b3bde0ead7e9601fc27-db4f2db3d8b4bcd0b1fd07dafefe39eabdadc2993acd4c9becf595d00020a80a"
  [[ -x "$cargo_stage_tool" && ! -L "$cargo_stage_tool" ]] || {
    printf 'release package: protected Cargo staging tool is unavailable\n' >&2
    exit 1
  }
  cargo_stage_tool_sha="$(sha256sum "$cargo_stage_tool" | awk '{print $1}')"
  cargo_path="$(realpath -e -- "$(command -v cargo)")"
  rustc_path="$(realpath -e -- "$(command -v rustc)")"
  cargo_sha="$(sha256sum "$cargo_path" | awk '{print $1}')"
  rustc_sha="$(sha256sum "$rustc_path" | awk '{print $1}')"
  cargo_version_sha="$(cargo --version --verbose | sha256sum | awk '{print $1}')"
  rustc_version_sha="$(rustc -vV | sha256sum | awk '{print $1}')"
  cargo_home="$release_sandbox/cargo-home"
  cargo_target="$release_sandbox/cargo-target"
  build_home="$release_sandbox/home"
  build_tmp="$release_sandbox/tmp"
  mkdir -m 0700 "$cargo_home" "$cargo_target" "$build_home" "$build_tmp"
  cargo_config="$cargo_home/config.toml"
  printf '%s\n' \
    '[net]' \
    'offline = true' >"$cargo_config"
  cargo_config_sha="$(sha256sum "$cargo_config" | awk '{print $1}')"
  cargo_stage_receipt="$cargo_home/registry/stage-receipt.json"
  "$cargo_stage_tool" cargo-cache-stage \
    --lock "$projection/Cargo.lock" \
    --source "$cargo_registry_source" \
    --destination "$cargo_home/registry" \
    --receipt "$cargo_stage_receipt" \
    --expected-source-uid 0 --expected-source-gid 0
  jq -e '
    . as $receipt
    | .schema_version == "jain.locked-cargo-cache/v2"
    and .lock_count == 1
    and (.lock_sha256s | length) == 1
    and (.package_count | type == "number" and . > 0)
    and (.packages | type == "array" and length == $receipt.package_count)
  ' "$cargo_stage_receipt" >/dev/null || {
    printf 'release package: Cargo staging receipt is invalid\n' >&2
    exit 1
  }
  cargo_stage_receipt_sha="$(
    sha256sum "$cargo_stage_receipt" | awk '{print $1}'
  )"
  cargo_lock_sha="$(sha256sum "$projection/Cargo.lock" | awk '{print $1}')"
  build_path="/home/ubuntu/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
  environment_sha="$(
    printf '%s\n' \
      "CARGO_HOME=$cargo_home" \
      "CARGO_NET_OFFLINE=true" \
      "CARGO_TARGET_DIR=$cargo_target" \
      "HOME=$build_home" \
      "PATH=$build_path" \
      "RUSTUP_HOME=/home/ubuntu/.rustup" \
      "SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD)" \
      "TMPDIR=$build_tmp" \
      "CARGO_REGISTRY_STAGE=$cargo_stage_receipt_sha" \
      | sha256sum | awk '{print $1}'
  )"

  env -i \
    CARGO_HOME="$cargo_home" \
    CARGO_NET_OFFLINE=true \
    CARGO_TARGET_DIR="$cargo_target" \
    HOME="$build_home" \
    PATH="$build_path" \
    RUSTUP_HOME=/home/ubuntu/.rustup \
    SOURCE_DATE_EPOCH="$(git show -s --format=%ct HEAD)" \
    TMPDIR="$build_tmp" \
    REDLINE_TESTING_RELEASE_INTERNAL=1 \
    REDLINE_TESTING_RELEASE_TAG="$tag" \
    REDLINE_TESTING_BUILD_CARGO_PATH="$cargo_path" \
    REDLINE_TESTING_BUILD_CARGO_SHA256="$cargo_sha" \
    REDLINE_TESTING_BUILD_CARGO_VERSION_SHA256="$cargo_version_sha" \
    REDLINE_TESTING_BUILD_RUSTC_PATH="$rustc_path" \
    REDLINE_TESTING_BUILD_RUSTC_SHA256="$rustc_sha" \
    REDLINE_TESTING_BUILD_RUSTC_VERSION_SHA256="$rustc_version_sha" \
    REDLINE_TESTING_BUILD_CARGO_CONFIG_SHA256="$cargo_config_sha" \
    REDLINE_TESTING_BUILD_CARGO_LOCK_SHA256="$cargo_lock_sha" \
    REDLINE_TESTING_BUILD_CARGO_REGISTRY_SOURCE="$cargo_registry_source" \
    REDLINE_TESTING_BUILD_CARGO_REGISTRY_RECEIPT_SHA256="$cargo_stage_receipt_sha" \
    REDLINE_TESTING_BUILD_CARGO_STAGE_TOOL_SHA256="$cargo_stage_tool_sha" \
    REDLINE_TESTING_BUILD_ENVIRONMENT_SHA256="$environment_sha" \
    bash "$projection/scripts/release-package.sh"

  target_name="linux-x86_64"
  package="redline-testing-${version}-${target_name}"
  mkdir -p "$repo_root/dist"
  rm -rf -- "$repo_root/dist/$package"
  rm -f -- "$repo_root/dist/${package}.tar.gz" \
    "$repo_root/dist/${package}.tar.gz.sha256" \
    "$repo_root/dist/release-manifest.json"
  cp -a -- "$projection/dist/$package" "$repo_root/dist/$package"
  cp -- "$projection/dist/${package}.tar.gz" \
    "$repo_root/dist/${package}.tar.gz"
  cp -- "$projection/dist/${package}.tar.gz.sha256" \
    "$repo_root/dist/${package}.tar.gz.sha256"
  cp -- "$projection/dist/release-manifest.json" \
    "$repo_root/dist/release-manifest.json"
  ops/ci/validate-release-manifest.sh schemas/release-manifest.schema.json \
    "$repo_root/dist/$package/release-manifest.json"
  ops/ci/verify-release-inventory.sh "$repo_root/dist/$package" \
    "$repo_root/dist/${package}.tar.gz" \
    "$repo_root/dist/${package}.tar.gz.sha256"
  cmp -s "$repo_root/dist/$package/release-manifest.json" \
    "$repo_root/dist/release-manifest.json" || {
      printf 'release package: exported manifest differs from package manifest\n' >&2
      exit 1
    }
  ops/ci/source-identity.sh verify "$source_receipt"
  printf 'release package: dist/%s.tar.gz\n' "$package"
  printf 'release manifest: dist/%s/release-manifest.json\n' "$package"
  exit 0
fi

identity_dir="${CARGO_TARGET_DIR:?}/release-package-identity"
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
cp "${CARGO_TARGET_DIR:?}/release/redline-testing" "${pkg_dir}/bin/redline-testing"

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
artifact_hash_records="${identity_dir}/artifact-hashes.jsonl"
: >"$artifact_hash_records"
while IFS= read -r -d '' artifact; do
  relative="${artifact#"${pkg_dir}/"}"
  if [[ ! "$relative" =~ ^[A-Za-z0-9._/-]+$ \
      || "$relative" == /* || "$relative" == *//* \
      || "/$relative/" == *"/../"* || "/$relative/" == *"/./"* ]]; then
    printf 'release package: ambiguous artifact path is forbidden: %q\n' \
      "$relative" >&2
    exit 1
  fi
  artifact_sha="$(sha256sum "$artifact" | awk '{print $1}')"
  jq -nc --arg key "$relative" --arg value "$artifact_sha" \
    '{key:$key,value:$value}' >>"$artifact_hash_records"
done < <(
  find "$pkg_dir" -type f \
    ! -name 'release-manifest.json' \
    ! -path '*/bin/*' -print0 | LC_ALL=C sort -z
)
artifact_hashes_obj="$(jq -s 'sort_by(.key) | from_entries' "$artifact_hash_records")"

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
  --arg cargo_path "${REDLINE_TESTING_BUILD_CARGO_PATH:?}" \
  --arg cargo_sha "${REDLINE_TESTING_BUILD_CARGO_SHA256:?}" \
  --arg cargo_version_sha "${REDLINE_TESTING_BUILD_CARGO_VERSION_SHA256:?}" \
  --arg rustc_path "${REDLINE_TESTING_BUILD_RUSTC_PATH:?}" \
  --arg rustc_sha "${REDLINE_TESTING_BUILD_RUSTC_SHA256:?}" \
  --arg rustc_version_sha "${REDLINE_TESTING_BUILD_RUSTC_VERSION_SHA256:?}" \
  --arg cargo_config_sha "${REDLINE_TESTING_BUILD_CARGO_CONFIG_SHA256:?}" \
  --arg cargo_lock_sha "${REDLINE_TESTING_BUILD_CARGO_LOCK_SHA256:?}" \
  --arg cargo_registry_source "${REDLINE_TESTING_BUILD_CARGO_REGISTRY_SOURCE:?}" \
  --arg cargo_registry_receipt_sha "${REDLINE_TESTING_BUILD_CARGO_REGISTRY_RECEIPT_SHA256:?}" \
  --arg cargo_stage_tool_sha "${REDLINE_TESTING_BUILD_CARGO_STAGE_TOOL_SHA256:?}" \
  --arg environment_sha "${REDLINE_TESTING_BUILD_ENVIRONMENT_SHA256:?}" \
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
    build_inputs: {
      source_mode: "sanitized-no-local",
      cargo_path: $cargo_path,
      cargo_sha256: $cargo_sha,
      cargo_version_sha256: $cargo_version_sha,
      rustc_path: $rustc_path,
      rustc_sha256: $rustc_sha,
      rustc_version_sha256: $rustc_version_sha,
      cargo_config_sha256: $cargo_config_sha,
      cargo_lock_sha256: $cargo_lock_sha,
      cargo_registry_source: $cargo_registry_source,
      cargo_registry_receipt_sha256: $cargo_registry_receipt_sha,
      cargo_stage_tool_sha256: $cargo_stage_tool_sha,
      environment_sha256: $environment_sha
    },
    generated_by: "scripts/release-package.sh"
  }' > "${pkg_dir}/release-manifest.json"

ops/ci/validate-release-manifest.sh schemas/release-manifest.schema.json \
  "${pkg_dir}/release-manifest.json"
ops/ci/verify-release-inventory.sh "$pkg_dir"
ops/ci/source-identity.sh verify "$source_identity_receipt"

source_epoch="$(git show -s --format=%ct HEAD)"
tar -C dist --sort=name --mtime="@${source_epoch}" \
  --owner=0 --group=0 --numeric-owner \
  --mode='u+rwX,go+rX,go-w' --format=ustar \
  -cf - "${package}" | gzip -n >"dist/${package}.tar.gz"
ops/ci/verify-release-inventory.sh "$pkg_dir" "dist/${package}.tar.gz" \
  "dist/${package}.tar.gz.sha256"
ops/ci/source-identity.sh verify "$source_identity_receipt"
cp "${pkg_dir}/release-manifest.json" dist/release-manifest.json
ops/ci/source-identity.sh verify "$source_identity_receipt"

# Surface what landed.
echo "release package: dist/${package}.tar.gz"
echo "release manifest: ${pkg_dir}/release-manifest.json"
echo "artifact_hashes count: $(jq '.artifact_hashes | length' "${pkg_dir}/release-manifest.json")"
