#!/usr/bin/env bash
# Noninteractive GitHub binary installer. No Rust, Node, sudo or sqlite3 alias.
set -euo pipefail
die() { printf 'redlinedb install: %s\n' "$*" >&2; exit 1; }
[[ $# == 0 ]] || { printf 'Usage: VERSION=v4.1.0 PREFIX="$HOME/.local" bash install.sh\n' >&2; exit 64; }
repo=https://github.com/neverhuman/RedlineDB
prefix=${PREFIX:-$HOME/.local}
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) platform=linux-x86_64 ;;
  Linux/aarch64|Linux/arm64) platform=linux-arm64 ;;
  Darwin/x86_64) platform=macos-x86_64 ;;
  Darwin/arm64) platform=macos-arm64 ;;
  *) die "unsupported platform: $(uname -s)/$(uname -m)" ;;
esac
for tool in curl tar mktemp; do command -v "$tool" >/dev/null || die "missing prerequisite: $tool"; done
version=${VERSION:-latest}
if [[ $version == latest ]]; then
  url=$(curl --proto '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' "$repo/releases/latest")
  version=${url##*/}
fi
version=v${version#v}
[[ $version =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]] || die "invalid VERSION: $version"
asset=redlinedb-$version-$platform.tar.gz
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL "$repo/releases/download/$version/$asset" -o "$tmp/$asset"
curl --proto '=https' --tlsv1.2 -fsSL "$repo/releases/download/$version/$asset.sha256" -o "$tmp/checksum"
read -r expected filename < "$tmp/checksum"
[[ $expected =~ ^[a-fA-F0-9]{64}$ && $filename == "$asset" ]] || die 'invalid checksum file'
if command -v sha256sum >/dev/null; then
  actual=$(sha256sum "$tmp/$asset"); actual=${actual%% *}
elif command -v shasum >/dev/null; then
  actual=$(shasum -a 256 "$tmp/$asset"); actual=${actual%% *}
else
  die 'sha256sum or shasum is required'
fi
[[ $actual == "$expected" ]] || die "checksum mismatch for $asset"
tar -tzf "$tmp/$asset" > "$tmp/members"
while IFS= read -r member; do
  case "$member" in /*|../*|*/../*|*/..) die 'unsafe archive member' ;; esac
done < "$tmp/members"
mkdir "$tmp/package"
tar -xzf "$tmp/$asset" -C "$tmp/package"
[[ -x $tmp/package/bin/redlinedb && -x $tmp/package/bin/redlinedb-server ]] || die 'archive is missing CLI or server'
for dir in bin lib include share; do
  [[ ! -d $tmp/package/$dir ]] || { mkdir -p "$prefix/$dir"; cp -R "$tmp/package/$dir/." "$prefix/$dir/"; }
done
"$prefix/bin/redlinedb" --version
printf 'Installed %s in %s. Add %s/bin to PATH.\n' "$version" "$prefix" "$prefix"
