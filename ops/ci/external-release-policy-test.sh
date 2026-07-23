#!/usr/bin/env bash
# The Jain release path is the local Jeryu forge. A GitHub tag/release upload
# workflow would create a second publisher and must remain absent.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
workflow_root="$repo_root/.github/workflows"

[[ ! -e "$workflow_root/release-build.yml" \
    && ! -L "$workflow_root/release-build.yml" ]] || {
    printf 'retired external release workflow reappeared: %s\n' \
        "$workflow_root/release-build.yml" >&2
    exit 1
}

if rg -n -i \
    'gh[[:space:]]+release|release-upload|upload[[:space:]]+to[[:space:]]+release|/releases/(upload|download)' \
    "$workflow_root"; then
    printf 'external release upload command is forbidden in tracked workflows\n' >&2
    exit 1
fi

while IFS= read -r -d '' workflow; do
    if rg -q '^[[:space:]]+(tags|release):' "$workflow" \
        && rg -q 'contents:[[:space:]]+write' "$workflow"; then
        printf 'externally writable tag/release workflow is forbidden: %s\n' \
            "${workflow#"$repo_root/"}" >&2
        exit 1
    fi
done < <(find "$workflow_root" -maxdepth 1 -type f \
    \( -name '*.yml' -o -name '*.yaml' \) -print0)

printf 'external release policy passed: local Jeryu is the sole publisher\n'
