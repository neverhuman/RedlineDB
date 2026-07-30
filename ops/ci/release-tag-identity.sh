#!/usr/bin/env bash

set -euo pipefail

tag="${1:-}"
receipt="${2:-}"
repo_root="${3:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
forge_remote="${4:-http://127.0.0.1:8787/git/jeryu/redline-testing.git}"

if [[ -z "$tag" || -z "$receipt" ]]; then
    printf 'usage: %s TAG RECEIPT [REPO_ROOT]\n' "$0" >&2
    exit 2
fi

cd "$repo_root"

for tool in git jq; do
    command -v "$tool" >/dev/null 2>&1 || {
        printf 'release tag identity: missing tool: %s\n' "$tool" >&2
        exit 1
    }
done

if [[ "$forge_remote" != "http://127.0.0.1:8787/git/jeryu/redline-testing.git" ]]; then
    [[ "${REDLINE_TESTING_HOSTILE_TEST:-0}" == 1 && "$forge_remote" == /* ]] || {
        printf 'release tag identity: non-governed forge remote is forbidden\n' >&2
        exit 1
    }
fi

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
local_object=""
local_commit=""
if git show-ref --verify --quiet "refs/tags/$tag"; then
    local_object="$(git rev-parse --verify "refs/tags/$tag")"
    local_commit="$(git rev-parse --verify "refs/tags/$tag^{commit}")"
fi

forge_output="$(
    git ls-remote --tags "$forge_remote" \
        "refs/tags/$tag" "refs/tags/$tag^{}"
)" || {
    printf 'release tag identity: authenticated forge tag readback failed\n' >&2
    exit 1
}
forge_lines=()
if [[ -n "$forge_output" ]]; then
    mapfile -t forge_lines <<<"$forge_output"
fi
forge_object=""
forge_commit=""
for line in "${forge_lines[@]}"; do
    sha="${line%%$'\t'*}"
    ref="${line#*$'\t'}"
    [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || {
        printf 'release tag identity: forge returned a malformed tag object\n' >&2
        exit 1
    }
    case "$ref" in
        "refs/tags/$tag")
            [[ -z "$forge_object" ]] || {
                printf 'release tag identity: forge returned duplicate tag objects\n' >&2
                exit 1
            }
            forge_object="$sha"
            ;;
        "refs/tags/$tag^{}")
            [[ -z "$forge_commit" ]] || {
                printf 'release tag identity: forge returned duplicate peeled commits\n' >&2
                exit 1
            }
            forge_commit="$sha"
            ;;
        *)
            printf 'release tag identity: forge returned an unexpected ref\n' >&2
            exit 1
            ;;
    esac
done
if [[ -n "$forge_object" && -z "$forge_commit" ]]; then
    forge_commit="$forge_object"
fi

tag_state="planned"
if [[ -z "$local_object" && -z "$forge_object" ]]; then
    :
elif [[ -z "$local_object" || -z "$forge_object" ]]; then
    printf 'release tag identity: local and forge tag presence disagree for %s\n' \
        "$tag" >&2
    exit 1
elif [[ "$local_object" != "$forge_object" || "$local_commit" != "$forge_commit" ]]; then
    printf 'release tag identity: local and forge tag identities disagree for %s\n' \
        "$tag" >&2
    exit 1
elif [[ "$local_commit" != "$head_sha" ]]; then
    printf 'release tag identity: occupied tag %s resolves to %s, not HEAD %s\n' \
        "$tag" "$local_commit" "$head_sha" >&2
    exit 1
else
    tag_state="live"
fi

mkdir -p "$(dirname "$receipt")"
jq -n \
    --arg release_tag "$tag" \
    --arg tag_state "$tag_state" \
    --arg release_commit "$head_sha" \
    --arg release_tree "$tree_sha" \
    --arg forge_remote "$forge_remote" \
    --arg local_tag_object "$local_object" \
    --arg local_tag_commit "$local_commit" \
    --arg forge_tag_object "$forge_object" \
    --arg forge_tag_commit "$forge_commit" \
    '{
        schema_version: "redline.testing.release-tag-identity/v2",
        release_tag: $release_tag,
        tag_state: $tag_state,
        release_commit: $release_commit,
        release_tree: $release_tree,
        forge_remote: $forge_remote,
        local_tag_object: $local_tag_object,
        local_tag_commit: $local_tag_commit,
        forge_tag_object: $forge_tag_object,
        forge_tag_commit: $forge_tag_commit
    }' >"$receipt"

printf 'release tag identity: tag=%s state=%s commit=%s tree=%s\n' \
    "$tag" "$tag_state" "$head_sha" "$tree_sha"
