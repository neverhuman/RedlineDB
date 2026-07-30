#!/usr/bin/env bash

set -euo pipefail

mode="${1:-}"
receipt="${2:-}"
repo_root="${3:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

if [[ "$mode" != "snapshot" && "$mode" != "verify" ]] || [[ -z "$receipt" ]]; then
    printf 'usage: %s {snapshot|verify} RECEIPT [REPO_ROOT]\n' "$0" >&2
    exit 2
fi

cd "$repo_root"

for tool in git jq sha256sum; do
    command -v "$tool" >/dev/null 2>&1 || {
        printf 'source identity: missing tool: %s\n' "$tool" >&2
        exit 1
    }
done

status="$(git status --porcelain=v1 --untracked-files=all)"
if [[ -n "$status" ]]; then
    printf 'source identity: repository is not clean\n%s\n' "$status" >&2
    exit 1
fi

head_sha="$(git rev-parse --verify HEAD)"
tree_sha="$(git rev-parse --verify 'HEAD^{tree}')"
archive_sha="$(git archive --format=tar --prefix=source/ "$head_sha" | sha256sum | awk '{print $1}')"
tracked_file_count=0
while IFS= read -r -d '' _tracked_path; do
    ((tracked_file_count += 1))
done < <(git ls-files -z)

case "$head_sha:$tree_sha:$archive_sha" in
    *[!0-9a-f:]*)
        printf 'source identity: git emitted a non-canonical identity\n' >&2
        exit 1
        ;;
esac
[[ ${#head_sha} -eq 40 && ${#tree_sha} -eq 40 && ${#archive_sha} -eq 64 ]] || {
    printf 'source identity: git emitted an identity with the wrong length\n' >&2
    exit 1
}

if [[ "$mode" == "snapshot" ]]; then
    mkdir -p "$(dirname "$receipt")"
    jq -n \
        --arg head_sha "$head_sha" \
        --arg tree_sha "$tree_sha" \
        --arg source_archive_sha256 "$archive_sha" \
        --argjson tracked_file_count "$tracked_file_count" \
        '{
            schema_version: "redline.testing.source-identity/v1",
            clean: true,
            head_sha: $head_sha,
            tree_sha: $tree_sha,
            source_archive_sha256: $source_archive_sha256,
            tracked_file_count: $tracked_file_count
        }' >"$receipt"
else
    jq -e \
        --arg head_sha "$head_sha" \
        --arg tree_sha "$tree_sha" \
        --arg source_archive_sha256 "$archive_sha" \
        --argjson tracked_file_count "$tracked_file_count" \
        '
            .schema_version == "redline.testing.source-identity/v1"
            and .clean == true
            and .head_sha == $head_sha
            and .tree_sha == $tree_sha
            and .source_archive_sha256 == $source_archive_sha256
            and .tracked_file_count == $tracked_file_count
        ' "$receipt" >/dev/null || {
            printf 'source identity: repository identity changed after the snapshot\n' >&2
            exit 1
        }
fi

printf 'source identity: %s head=%s tree=%s archive=%s\n' \
    "$mode" "$head_sha" "$tree_sha" "$archive_sha"
