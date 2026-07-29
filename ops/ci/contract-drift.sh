#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"

if ! has jq; then
    fail "missing tool: jq (release contract validation)"
fi
if ! has sha256sum; then
    fail "missing tool: sha256sum (release artifact verification)"
fi

expected_version="1.0.1"
package_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
if [ "$package_version" != "$expected_version" ]; then
    fail "package version drift: expected $expected_version, got ${package_version:-missing}"
fi

schema_count=0
while IFS= read -r schema; do
    [ -n "$schema" ] || continue
    ci_run jq -e '
        type == "object"
        and ."$schema" == "https://json-schema.org/draft/2020-12/schema"
        and (.type == "object" or (.oneOf | type == "array"))
    ' "$schema" >/dev/null
    schema_count=$((schema_count + 1))
done < <(git ls-files 'schemas/*.json')
if [ "$schema_count" -eq 0 ]; then
    fail "no tracked JSON schemas found"
fi

ci_run scripts/release-package.sh

target_name="linux-x86_64"
package="redline-testing-${expected_version}-${target_name}"
package_dir="dist/${package}"
manifest="${package_dir}/release-manifest.json"
tarball="dist/${package}.tar.gz"
sidecar="${tarball}.sha256"
for artifact in "$package_dir" "$manifest" "$tarball" "$sidecar"; do
    if [ ! -e "$artifact" ]; then
        fail "release artifact missing: $artifact"
    fi
done

ci_run jq -e \
    --arg version "$expected_version" \
    --arg target "$target_name" \
    '
        .name == "redline-testing"
        and .version == $version
        and .target == $target
        and (.release_commit | test("^[0-9a-f]{40}$"))
        and (.release_tag | test("^redline-testing-v1\\.0\\.1-jain\\.[1-9][0-9]*$"))
        and (.tag_revision | type == "number" and . >= 1 and floor == .)
        and .binary == "bin/redline-testing"
        and (.binary_sha256 | test("^[0-9a-f]{64}$"))
        and .tarball_sha256_source == ".sha256 sidecar"
        and (.artifact_hashes | type == "object" and length > 0)
        and ([.artifact_hashes[] | test("^[0-9a-f]{64}$")] | all)
        and .generated_by == "scripts/release-package.sh"
    ' "$manifest" >/dev/null

binary_path="${package_dir}/$(jq -er '.binary' "$manifest")"
binary_expected="$(jq -er '.binary_sha256' "$manifest")"
binary_actual="$(sha256sum "$binary_path" | awk '{print $1}')"
if [ "$binary_actual" != "$binary_expected" ]; then
    fail "release binary digest mismatch"
fi

declared_count="$(jq -er '.artifact_hashes | length' "$manifest")"
actual_count="$(find "$package_dir" -type f \
    ! -name release-manifest.json \
    ! -path '*/bin/*' | wc -l)"
if [ "$actual_count" -ne "$declared_count" ]; then
    fail "release inventory mismatch: declared=$declared_count actual=$actual_count"
fi

while IFS=$'\t' read -r relative expected_sha; do
    case "$relative" in
        ""|/*|*".."*|*$'\n'*|*$'\r'*)
            fail "unsafe release manifest path: $relative"
            ;;
    esac
    artifact="${package_dir}/${relative}"
    if [ ! -f "$artifact" ] || [ -L "$artifact" ]; then
        fail "release manifest member is not a physical regular file: $relative"
    fi
    actual_sha="$(sha256sum "$artifact" | awk '{print $1}')"
    if [ "$actual_sha" != "$expected_sha" ]; then
        fail "release manifest digest mismatch: $relative"
    fi
done < <(jq -r '.artifact_hashes | to_entries[] | [.key, .value] | @tsv' "$manifest")

ci_run sha256sum -c "$sidecar"
ci_run cargo test --locked --test release_manifest_integrity \
    release_manifest_enumerates_every_bundled_file -- --exact

receipt_dir="target/jankurai/contract-drift"
mkdir -p "$receipt_dir"
manifest_sha="$(sha256sum "$manifest" | awk '{print $1}')"
tarball_sha="$(sha256sum "$tarball" | awk '{print $1}')"
ci_run jq -n \
    --arg head_sha "$(git rev-parse HEAD)" \
    --arg package_version "$expected_version" \
    --arg manifest_sha256 "$manifest_sha" \
    --arg tarball_sha256 "$tarball_sha" \
    --argjson schema_count "$schema_count" \
    --argjson artifact_count "$declared_count" \
    '{
        schema_version: "redline.testing.contract-drift/v1",
        status: "pass",
        head_sha: $head_sha,
        package_version: $package_version,
        schema_count: $schema_count,
        artifact_count: $artifact_count,
        manifest_sha256: $manifest_sha256,
        tarball_sha256: $tarball_sha256
    }' >"$receipt_dir/receipt.json"

log "contract-drift: pass version=${expected_version} schemas=${schema_count} artifacts=${declared_count}"
