#!/usr/bin/env bash
# Run downloaded archives without invoking development toolchains.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
packages=${1:-$root/target/packages}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/no-toolchain"
for tool in cargo rustc node npm cc clang gcc; do
  printf '#!/bin/sh\necho "development toolchain invoked unexpectedly" >&2\nexit 1\n' > "$work/no-toolchain/$tool"
  chmod +x "$work/no-toolchain/$tool"
done
export PATH="$work/no-toolchain:$PATH"
prefix="$work/install with spaces"
mkdir -p "$prefix"
for archive in "$packages"/*.tar.gz; do
  directory=$(cd "$(dirname "$archive")" && pwd)
  name=${archive##*/}
  (cd "$directory"; if command -v sha256sum >/dev/null; then sha256sum -c "$name.sha256"; else shasum -a 256 -c "$name.sha256"; fi)
  tar -xzf "$archive" -C "$prefix"
done
"$prefix/bin/redlinedb" --version
"$prefix/bin/redline-testing" --version
CHECK_FFI=0 bash "$root/scripts/test-binaries.sh" "$prefix/bin"
