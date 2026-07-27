#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s --source-cache ROOT --lockfile FILE --platform linux --arch x64 --libc glibc --destination DIR --authority FILE\n' \
    "${0##*/}" >&2
  exit 2
}

source_cache=""
lockfile=""
platform=""
arch=""
libc=""
destination=""
authority=""
while (($#)); do
  case "$1" in
    --source-cache) source_cache="${2-}"; shift 2 ;;
    --lockfile) lockfile="${2-}"; shift 2 ;;
    --platform) platform="${2-}"; shift 2 ;;
    --arch) arch="${2-}"; shift 2 ;;
    --libc) libc="${2-}"; shift 2 ;;
    --destination) destination="${2-}"; shift 2 ;;
    --authority) authority="${2-}"; shift 2 ;;
    *) usage ;;
  esac
done
[[ -n "$source_cache" && -n "$lockfile" && "$platform" == linux \
  && "$arch" == x64 && "$libc" == glibc && -n "$destination" \
  && -n "$authority" ]] || usage

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/npm-runtime.sh
source "$repo_root/ops/ci/npm-runtime.sh"
source_cache="$(realpath -e -- "$source_cache")"
lockfile="$(realpath -e -- "$lockfile")"
destination="$(realpath -e -- "$destination")"
[[ -d "$source_cache/_cacache/content-v2/sha512" \
  && -f "$lockfile" && ! -L "$lockfile" \
  && -d "$destination" && ! -L "$destination" \
  && -z "$(find "$destination" -mindepth 1 -print -quit)" \
  && "$authority" == "$destination"/* && ! -e "$authority" ]] || exit 1

scratch="$(mktemp -d "$destination/.npm-cache-build.XXXXXX")"
cleanup() {
  chmod -R u+w "$scratch" 2>/dev/null || true
  rm -rf -- "$scratch"
}
trap cleanup EXIT
closed="$scratch/closed"
closure="$scratch/closure.tsv"
inventory="$scratch/inventory.tsv"
mkdir -m 0755 -p "$closed/_cacache/content-v2/sha512" \
  "$closed/_cacache/index-v5"
jain_write_npm_closure \
  "$lockfile" "$platform" "$arch" "$libc" "$closure"

while IFS=$'\t' read -r package_path url integrity; do
  [[ "$package_path" == node_modules/* && "$package_path" != *..* \
    && "$url" == https://registry.npmjs.org/* \
    && "$integrity" =~ ^sha512-([A-Za-z0-9+/]+={0,2})$ ]] || exit 1
  encoded="${BASH_REMATCH[1]}"
  hex="$(printf '%s' "$encoded" | base64 -d 2>/dev/null \
    | od -An -tx1 | tr -d ' \n')"
  [[ "$hex" =~ ^[0-9a-f]{128}$ ]] || exit 1
  source_content="$source_cache/_cacache/content-v2/sha512/${hex:0:2}/${hex:2:2}/${hex:4}"
  [[ -f "$source_content" && ! -L "$source_content" \
    && "$(stat -c '%h' -- "$source_content")" == 1 \
    && "$(sha512sum -- "$source_content" | cut -d' ' -f1)" == "$hex" ]] \
    || exit 1
  content="$closed/_cacache/content-v2/sha512/${hex:0:2}/${hex:2:2}/${hex:4}"
  mkdir -m 0755 -p "$(dirname "$content")"
  if [[ -e "$content" ]]; then
    cmp -s -- "$source_content" "$content" || exit 1
  else
    cp --reflink=never -- "$source_content" "$content"
  fi
  size="$(stat -c '%s' -- "$content")"
  key="make-fetch-happen:request-cache:$url"
  key_hash="$(printf '%s' "$key" | sha256sum | cut -d' ' -f1)"
  index="$closed/_cacache/index-v5/${key_hash:0:2}/${key_hash:2:2}/${key_hash:4}"
  mkdir -m 0755 -p "$(dirname "$index")"
  record="$(jq -cn --arg key "$key" --arg integrity "$integrity" \
    --arg url "$url" --argjson size "$size" '{
      key: $key,
      integrity: $integrity,
      time: 0,
      size: $size,
      metadata: {
        time: 0,
        url: $url,
        reqHeaders: {},
        resHeaders: {},
        options: {compress: true}
      }
    }')"
  checksum="$(printf '%s' "$record" | sha1sum | cut -d' ' -f1)"
  if [[ -e "$index" ]]; then
    [[ "$(tail -n 1 "$index")" == "$checksum"$'\t'"$record" ]] || exit 1
  else
    printf '\n%s\t%s' "$checksum" "$record" >"$index"
  fi
done <"$closure"

chmod -R a-w "$closed"
jain_write_native_build_tools_inventory "$closed" "$inventory" content
inventory_sha256="$(sha256sum -- "$inventory" | cut -d' ' -f1)"
file_count="$(wc -l <"$inventory")"
closure_sha256="$(sha256sum -- "$closure" | cut -d' ' -f1)"
closure_count="$(wc -l <"$closure")"
final="$destination/$inventory_sha256"
[[ ! -e "$final" ]]
chmod u+w "$closed"
mv -- "$closed" "$final"
chmod u-w "$final"
jq -n --arg inventory "$inventory_sha256" \
  --arg root "/var/lib/jain-host-ci/npm-cache/$inventory_sha256" \
  --arg closure_sha "$closure_sha256" \
  --arg lock_sha "$(sha256sum -- "$lockfile" | cut -d' ' -f1)" \
  --arg platform "$platform" --arg arch "$arch" --arg libc "$libc" \
  --argjson file_count "$file_count" \
  --argjson closure_count "$closure_count" '{
    schema_version: "jain.npm-cache/v1",
    cache_root: $root,
    inventory_sha256: $inventory,
    file_count: $file_count,
    format: "cacache-v5",
    package_lock_path: "apps/web/package-lock.json",
    package_lock_sha256: $lock_sha,
    closure_sha256: $closure_sha,
    closure_count: $closure_count,
    platform: $platform,
    arch: $arch,
    libc: $libc,
    npm_version: "9.2.0"
  }' >"$authority"
jain_validate_npm_cache "$authority" "$final" content
checkout="${lockfile%/apps/web/package-lock.json}"
jain_npm_cache_matches_lock "$authority" "$final" "$checkout" content
