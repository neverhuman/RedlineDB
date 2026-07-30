#!/usr/bin/env bash

set -euo pipefail

source_root="${1:-}"
destination="${2:-}"
expected_head="${3:-}"

if [[ -z "$source_root" || -z "$destination" \
    || ! "$expected_head" =~ ^[0-9a-f]{40}$ ]]; then
    printf 'usage: %s SOURCE_ROOT DESTINATION EXPECTED_HEAD\n' "$0" >&2
    exit 2
fi
if [[ -e "$destination" || -L "$destination" ]]; then
    printf 'release projection: destination already exists\n' >&2
    exit 1
fi

source_root="$(realpath -e -- "$source_root")"
[[ -d "$source_root/.git" && ! -L "$source_root" ]] || {
    printf 'release projection: source is not a physical Git checkout\n' >&2
    exit 1
}
[[ "$(git -C "$source_root" rev-parse --verify HEAD)" == "$expected_head" ]] || {
    printf 'release projection: source HEAD differs from the requested commit\n' >&2
    exit 1
}
status="$(git -C "$source_root" status --porcelain=v1 --untracked-files=all)"
[[ -z "$status" ]] || {
    printf 'release projection: source has tracked or untracked changes\n' >&2
    exit 1
}

git clone --no-local --no-checkout "$source_root" "$destination" >/dev/null
git -C "$destination" checkout --detach "$expected_head" >/dev/null
git -C "$destination" remote remove origin

[[ "$(git -C "$destination" rev-parse --verify HEAD)" == "$expected_head" \
    && "$(git -C "$destination" rev-parse --verify 'HEAD^{tree}')" \
        == "$(git -C "$source_root" rev-parse --verify 'HEAD^{tree}')" \
    && -z "$(git -C "$destination" status --porcelain=v1 --untracked-files=all)" \
    && -z "$(git -C "$destination" status --porcelain=v1 --ignored \
        --untracked-files=all)" ]] || {
    printf 'release projection: detached source projection is not exact and closed\n' >&2
    exit 1
}

printf 'release projection: exact no-local source=%s head=%s\n' \
    "$destination" "$expected_head"
