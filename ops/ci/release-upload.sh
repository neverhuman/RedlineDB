#!/usr/bin/env bash
set -euo pipefail

: "${TAG:?TAG is required}"
: "${PKG:?PKG is required; run ops/ci/release-build.sh first}"

SOURCE_DIR="${SOURCE_DIR:-.}"
cd "$SOURCE_DIR"

case "$TAG" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *)
    printf 'release tag must look like vX.Y.Z, got %s\n' "$TAG" >&2
    exit 1
    ;;
esac

archive="${PKG}.tar.gz"
checksum="${archive}.sha256"
for path in "$archive" "$checksum"; do
  if [ ! -s "$path" ]; then
    printf 'release asset candidate is missing or empty: %s\n' "$path" >&2
    exit 1
  fi
done

if ! git rev-parse --verify --quiet "refs/tags/${TAG}" >/dev/null; then
  printf 'release tag %s is not present in the checked-out source\n' "$TAG" >&2
  exit 1
fi

if ! gh release view "$TAG" >/dev/null 2>&1; then
  gh release create "$TAG" --verify-tag --title "redlinedb ${TAG}" --generate-notes
fi

for asset in "$archive" "$checksum"; do
  if gh release view "$TAG" --json assets \
      --jq ".assets[].name" | grep -Fx -- "$asset" >/dev/null; then
    printf 'release asset already exists and will not be overwritten: %s\n' "$asset" >&2
    exit 1
  fi
done

gh release upload "$TAG" "$archive" "$checksum"
