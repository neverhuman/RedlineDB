#!/usr/bin/env bash

set -euo pipefail

tag="${1:-}"
receipt="${2:-}"
repo_root="${3:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

if [[ -z "$tag" || -z "$receipt" ]]; then
    printf 'usage: %s TAG RECEIPT [REPO_ROOT]\n' "$0" >&2
    exit 2
fi

cd "$repo_root"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
expected_prefix="redline-testing-v${version}-jain."
if [[ "$tag" != "$expected_prefix"* ]]; then
    printf 'release tag identity: tag must match %s<positive canonical decimal>\n' \
        "$expected_prefix" >&2
    exit 1
fi
revision="${tag#"$expected_prefix"}"
if [[ ! "$revision" =~ ^[1-9][0-9]*$ ]]; then
    printf 'release tag identity: tag must use a positive canonical decimal revision\n' >&2
    exit 1
fi

head_sha="$(git rev-parse --verify HEAD)"
tree_sha="$(git rev-parse --verify 'HEAD^{tree}')"
tag_state="planned"
tag_commit=""
if git show-ref --verify --quiet "refs/tags/$tag"; then
    tag_commit="$(git rev-parse --verify "refs/tags/$tag^{commit}")"
    if [[ "$tag_commit" != "$head_sha" ]]; then
        printf 'release tag identity: occupied tag %s resolves to %s, not HEAD %s\n' \
            "$tag" "$tag_commit" "$head_sha" >&2
        exit 1
    fi
    tag_state="live"
fi

mkdir -p "$(dirname "$receipt")"
jq -n \
    --arg release_tag "$tag" \
    --arg tag_state "$tag_state" \
    --arg release_commit "$head_sha" \
    --arg release_tree "$tree_sha" \
    '{
        schema_version: "redline.testing.release-tag-identity/v1",
        release_tag: $release_tag,
        tag_state: $tag_state,
        release_commit: $release_commit,
        release_tree: $release_tree
    }' >"$receipt"

printf 'release tag identity: tag=%s state=%s commit=%s tree=%s\n' \
    "$tag" "$tag_state" "$head_sha" "$tree_sha"
