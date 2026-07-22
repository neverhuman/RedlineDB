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
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
package="redline-testing-${version}-linux-x86_64"

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
  if [ "$clone" = "$sandbox_a/source" ]; then
    mkdir -p "$clone/target"
    for hostile_epoch in 1 2; do
      hostile_log="$clone/target/hostile-epoch-${hostile_epoch}.log"
      if (
        cd "$clone"
        env CARGO_NET_OFFLINE=true SOURCE_DATE_EPOCH="$hostile_epoch" \
          scripts/release-package.sh >"$hostile_log" 2>&1
      ); then
        printf 'divergent numeric SOURCE_DATE_EPOCH was accepted: %s\n' "$hostile_epoch" >&2
        exit 1
      fi
      grep -F -q 'SOURCE_DATE_EPOCH must equal release commit epoch' "$hostile_log"
      [ ! -e "$clone/dist/${package}.tar.gz" ]
      [ ! -e "$clone/dist/${package}.tar.gz.sha256" ]
    done
  fi
  (
    cd "$clone"
    if [ "$clone" = "$sandbox_a/source" ]; then
      env CARGO_NET_OFFLINE=true SOURCE_DATE_EPOCH="$(git show -s --format=%ct HEAD)" \
        scripts/release-package.sh
    else
      env -u SOURCE_DATE_EPOCH CARGO_NET_OFFLINE=true scripts/release-package.sh
    fi
  )
done

package_root="$sandbox_a/source/dist/${package}"
package_runner="$package_root/bin/redline-testing"
for marker_spec in \
  "contracts/compatibility-v1.toml:file" \
  "corpus/sqlite_parity:dir" \
  "release-manifest.json:file"; do
  marker="${marker_spec%:*}"
  marker_type="${marker_spec##*:}"
  marker_path="$package_root/$marker"
  held_path="$marker_path.hostile-held"
  mv -- "$marker_path" "$held_path"
  for hostile_shape in missing wrong-type; do
    if [ "$hostile_shape" = wrong-type ]; then
      if [ "$marker_type" = file ]; then
        mkdir -p "$marker_path"
      else
        printf 'not a directory\n' >"$marker_path"
      fi
    fi
    hostile_log="$sandbox_a/package-layout-${marker//\//-}-${hostile_shape}.log"
    if "$package_runner" major-gate \
      --baseline contracts/compatibility-v1.reviewed.toml \
      --candidate contracts/compatibility-v1.toml \
      >"$hostile_log" 2>&1; then
      printf 'partial packaged layout reached an enclosing source: %s %s\n' \
        "$marker" "$hostile_shape" >&2
      exit 1
    fi
    grep -F -q 'incomplete physical redline-testing layout' "$hostile_log"
    if [ "$hostile_shape" = wrong-type ]; then
      rm -rf -- "$marker_path"
    fi
  done
  mv -- "$held_path" "$marker_path"
done

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
