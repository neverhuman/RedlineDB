#!/usr/bin/env bash
set -euo pipefail
: "${TAG:?TAG is required}"
[[ $TAG =~ ^v4\.1\.0(-rc\.[0-9]+)?$ ]] || { echo 'unsupported release tag' >&2; exit 1; }
[[ $(git rev-parse "$TAG^{commit}") == $(git rev-parse HEAD) ]]
count=$(find target/packages -name '*.tar.gz' | wc -l)
[[ $count -eq 12 ]] || { echo 'expected three packages for each of four platforms' >&2; exit 1; }
(cd target/packages; for checksum in *.sha256; do sha256sum -c "$checksum"; done)
# create fails when the release already exists; immutable assets are never clobbered.
args=(--verify-tag --draft --title "RedlineDB $TAG" --notes-file docs/migration/RELEASE_NOTES.md)
[[ $TAG != *-rc.* ]] || args+=(--prerelease)
gh release create "$TAG" "${args[@]}"
gh release upload "$TAG" target/packages/*.tar.gz target/packages/*.sha256
gh release edit "$TAG" --draft=false
