#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/pnpm-runtime.sh
source "$repo_root/ops/ci/pnpm-runtime.sh"
tmp="$(mktemp -d /tmp/jain-pnpm-runtime-test.XXXXXX)"
cleanup() { chmod -R u+w "$tmp" 2>/dev/null || true; rm -rf -- "$tmp"; }
trap cleanup EXIT
stage="$tmp/store-stage"
mkdir -p "$stage/v10/files/aa" "$stage/v10/index/bb" \
  "$tmp/product/apps/web" "$tmp/bin"
printf 'package bytes\n' >"$stage/v10/files/aa/content"
printf 'executable package bytes\n' >"$stage/v10/files/aa/content-exec"
printf 'index bytes\n' >"$stage/v10/index/bb/package@1.0.0.json"
printf 'lockfileVersion: 9.0\n' >"$tmp/product/apps/web/pnpm-lock.yaml"
printf '%s\n' '#!/bin/sh' 'printf "10.18.3\n"' >"$tmp/bin/pnpm"
chmod 0555 "$tmp/bin/pnpm"
find "$stage" -type d -exec chmod 0555 {} +
find "$stage" -type f -exec chmod 0444 {} +
authority="$tmp/pnpm-store.lock.json"
"$repo_root/ops/ci/pnpm-store-authority.sh" "$stage" \
  "$tmp/product/apps/web/pnpm-lock.yaml" "$tmp/bin/pnpm" >"$authority"
inventory="$(jq -er '.inventory_sha256' "$authority")"
root="$tmp/$inventory"
mv -- "$stage" "$root"
jain_validate_pnpm_store "$authority" "$root" content
jain_pnpm_lockfile_matches "$authority" "$tmp/product"
mkdir "$tmp/staged"
staged="$(jain_stage_pnpm_store \
  "$authority" "$root" "$tmp/staged" content)"
[[ "$staged" == "$tmp/staged/$inventory" \
  && "$(stat -c %a "$staged")" == 700 \
  && "$(stat -c %a "$staged/v10/index/bb/package@1.0.0.json")" == 600 \
  && "$(stat -c %a "$staged/v10/files/aa/content")" == 600 \
  && "$(stat -c %a "$staged/v10/files/aa/content-exec")" == 700 \
  && -z "$(find "$staged" -type l -print -quit)" \
  && -z "$(find "$staged" ! -type d ! -type f -print -quit)" \
  && -z "$(find "$staged" -type f -links +1 -print -quit)" ]] || exit 1
jain_validate_staged_pnpm_store "$authority" "$root" "$staged" content

index_file="$root/v10/index/bb/package@1.0.0.json"
chmod 0644 "$index_file"
printf 'tampered\n' >"$index_file"
chmod 0444 "$index_file"
if jain_validate_pnpm_store "$authority" "$root" content 2>/dev/null; then
  printf 'pnpm store validator accepted tampered bytes\n' >&2; exit 1
fi
chmod 0644 "$index_file"
printf 'index bytes\n' >"$index_file"
chmod 0444 "$index_file"
chmod 0755 "$root/v10/files/aa"
ln -s content "$root/v10/files/aa/linked"
chmod 0555 "$root/v10/files/aa"
if jain_validate_pnpm_store "$authority" "$root" content 2>/dev/null; then
  printf 'pnpm store validator accepted a symlink\n' >&2; exit 1
fi
chmod 0755 "$root/v10/files/aa"
rm -- "$root/v10/files/aa/linked"
ln "$root/v10/files/aa/content" "$root/v10/files/aa/hardlink"
chmod 0555 "$root/v10/files/aa"
if jain_validate_pnpm_store "$authority" "$root" content 2>/dev/null; then
  printf 'pnpm store validator accepted a hardlink\n' >&2; exit 1
fi
chmod 0755 "$root/v10/files/aa"
rm -- "$root/v10/files/aa/hardlink"
chmod 0555 "$root/v10/files/aa"
chmod 0644 "$tmp/product/apps/web/pnpm-lock.yaml"
printf 'changed\n' >>"$tmp/product/apps/web/pnpm-lock.yaml"
if jain_pnpm_lockfile_matches "$authority" "$tmp/product" 2>/dev/null; then
  printf 'pnpm authority accepted a changed lockfile\n' >&2; exit 1
fi
jq '.unreviewed = true' "$authority" >"$tmp/unknown.json"
if jain_validate_pnpm_store "$tmp/unknown.json" "$root" content 2>/dev/null; then
  printf 'pnpm authority accepted an unknown field\n' >&2; exit 1
fi
jq '.pnpm_version = "11.9.0"' "$authority" >"$tmp/wrong-version.json"
if jain_validate_pnpm_store \
  "$tmp/wrong-version.json" "$root" content 2>/dev/null; then
  printf 'pnpm authority accepted the wrong product version\n' >&2; exit 1
fi
printf 'pnpm runtime authority ok\n'
