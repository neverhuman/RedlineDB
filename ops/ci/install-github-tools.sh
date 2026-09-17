#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
dest="$root/target/ci/tools"
mkdir -p "$dest"
tag=v1.6.11-deadlang-precision-split.3
asset=jankurai-${tag#v}-x86_64-unknown-linux-gnu.tar.gz
archive_sha=a192cb302ba6e4fc58657c6f26c5d7a9a49d76302ba8c09dab463fe3fe95a66e
binary_sha=9e6b8857a26f6004d4c74e510e13b06d880f2e2ae0c89502698889ed690c5d6c
if [[ ! -f $dest/jankurai ]] || [[ $(sha256sum "$dest/jankurai" | cut -d' ' -f1) != "$binary_sha" ]]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  curl --proto '=https' --tlsv1.2 -fsSL "https://github.com/neverhuman/jankurai/releases/download/$tag/$asset" -o "$tmp/$asset"
  printf '%s  %s\n' "$archive_sha" "$tmp/$asset" | sha256sum -c -
  tar -xzf "$tmp/$asset" -C "$tmp"
  install -m 755 "$tmp/${asset%.tar.gz}/jankurai" "$dest/jankurai"
fi
printf '%s  %s\n' "$binary_sha" "$dest/jankurai" | sha256sum -c -
[[ $("$dest/jankurai" --version) == 'jankurai 1.6.11' ]]
[[ -z ${GITHUB_PATH:-} ]] || printf '%s\n' "$dest" >> "$GITHUB_PATH"
