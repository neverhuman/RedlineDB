#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workspace_root="$(cd "$repo_root/../.." && pwd)"
head="$(git -C "$repo_root" rev-parse HEAD)"
tree="$(git -C "$repo_root" rev-parse 'HEAD^{tree}')"
status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=no)"
[ -z "$status" ] || {
  printf 'cross-root release proof requires an exact committed tree\n' >&2
  exit 1
}

sandbox_a=""
sandbox_b=""
cleanup() {
  for sandbox in "$sandbox_a" "$sandbox_b"; do
    [ -n "$sandbox" ] || continue
    case "$sandbox" in
      "$repo_root"/target/cross-root-release-a.*|"$workspace_root"/target/cross-root-release-b.*)
        chmod -R u+w -- "$sandbox" 2>/dev/null || true
        rm -rf -- "$sandbox"
        ;;
      *)
        printf 'refusing unsafe cross-root cleanup target: %s\n' "$sandbox" >&2
        ;;
    esac
  done
}
trap cleanup EXIT HUP INT TERM
sandbox_a="$(mktemp -d "$repo_root/target/cross-root-release-a.XXXXXX")"
sandbox_b="$(mktemp -d "$workspace_root/target/cross-root-release-b.XXXXXX")"

for clone in "$sandbox_a/source" "$sandbox_b/source"; do
  git clone -q --no-local "$repo_root" "$clone"
  git -C "$clone" checkout -q --detach "$head"
  [ "$(git -C "$clone" rev-parse HEAD)" = "$head" ]
  [ "$(git -C "$clone" rev-parse 'HEAD^{tree}')" = "$tree" ]
  [ -z "$(git -C "$clone" status --porcelain=v1 --untracked-files=all)" ]
  [ -d "$clone/.git" ] && [ ! -e "$clone/.git/objects/info/alternates" ]
  (
    cd "$clone"
    export CARGO_NET_OFFLINE=true
    source_date_epoch="$(git show -s --format=%ct HEAD)"
    export SOURCE_DATE_EPOCH="$source_date_epoch"
    scripts/release-package.sh
  )
done

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
package="redline-testing-${version}-linux-x86_64"
artifacts=(
  "dist/${package}/bin/redline-testing"
  "dist/release-manifest.json"
  "dist/${package}.tar.gz"
  "dist/${package}.tar.gz.sha256"
)
for relative in "${artifacts[@]}"; do
  left="$sandbox_a/source/$relative"
  right="$sandbox_b/source/$relative"
  cmp -s -- "$left" "$right" || {
    printf 'cross-root release artifact differs: %s\n' "$relative" >&2
    sha256sum -- "$left" "$right" >&2
    exit 1
  }
  sha256sum -- "$left"
done

for clone in "$sandbox_a/source" "$sandbox_b/source"; do
  binary="$clone/dist/${package}/bin/redline-testing"
  if grep -F -a -q -- "$clone" "$binary"; then
    printf 'release binary embeds its absolute build root: %s\n' "$clone" >&2
    exit 1
  fi
done

printf 'cross-root release proof passed: head=%s tree=%s\n' "$head" "$tree"
