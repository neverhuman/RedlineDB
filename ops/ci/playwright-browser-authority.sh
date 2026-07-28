#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s --source-browsers ROOT --source-npm-cache ROOT --lockfile FILE --platform linux --arch x64 --libc glibc --destination DIR --authority FILE\n' \
    "${0##*/}" >&2
  exit 2
}

source_browsers=""
source_npm_cache=""
lockfile=""
platform=""
arch=""
libc=""
destination=""
authority=""
while (($#)); do
  case "$1" in
    --source-browsers) source_browsers="${2-}"; shift 2 ;;
    --source-npm-cache) source_npm_cache="${2-}"; shift 2 ;;
    --lockfile) lockfile="${2-}"; shift 2 ;;
    --platform) platform="${2-}"; shift 2 ;;
    --arch) arch="${2-}"; shift 2 ;;
    --libc) libc="${2-}"; shift 2 ;;
    --destination) destination="${2-}"; shift 2 ;;
    --authority) authority="${2-}"; shift 2 ;;
    *) usage ;;
  esac
done
[[ -n "$source_browsers" && -n "$source_npm_cache" && -n "$lockfile" \
  && "$platform" == linux \
  && "$arch" == x64 && "$libc" == glibc && -n "$destination" \
  && -n "$authority" ]] || usage

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/playwright-browser-runtime.sh
source "$repo_root/ops/ci/playwright-browser-runtime.sh"
source_browsers="$(realpath -e -- "$source_browsers")"
source_npm_cache="$(realpath -e -- "$source_npm_cache")"
lockfile="$(realpath -e -- "$lockfile")"
destination="$(realpath -e -- "$destination")"
[[ -d "$source_browsers" && ! -L "$source_browsers" \
  && -d "$source_npm_cache/_cacache/content-v2/sha512" \
  && -f "$lockfile" && ! -L "$lockfile" \
  && -d "$destination" && ! -L "$destination" \
  && -z "$(find "$destination" -mindepth 1 -print -quit)" \
  && "$authority" == "$destination"/* && ! -e "$authority" ]] || exit 1

playwright_core_version=1.60.0
playwright_core_integrity="$(
  jq -er '.packages["node_modules/playwright-core"].integrity' "$lockfile"
)"
[[ "$playwright_core_integrity" =~ ^sha512-[A-Za-z0-9+/]+={0,2}$ ]]
jq -e --arg version "$playwright_core_version" '
  select(.lockfileVersion == 3)
  | select(.packages["node_modules/@playwright/test"].version == $version)
  | select(.packages["node_modules/playwright"].version == $version)
  | select(.packages["node_modules/playwright-core"].version == $version)
  | select(.packages["node_modules/playwright-core"].resolved
      == "https://registry.npmjs.org/playwright-core/-/playwright-core-1.60.0.tgz")
' "$lockfile" >/dev/null

cache_dirs=(
  chromium-1223
  chromium_headless_shell-1223
  ffmpeg-1011
)
executables=(
  chrome-linux64/chrome
  chrome-headless-shell-linux64/chrome-headless-shell
  ffmpeg-linux
)
scratch="$(mktemp -d "$destination/.playwright-browser-build.XXXXXX")"
cleanup() {
  chmod -R u+w "$scratch" 2>/dev/null || true
  rm -rf -- "$scratch"
}
trap cleanup EXIT
closed="$scratch/closed"
inventory="$scratch/inventory.tsv"
browsers_json="$scratch/browsers.json"
encoded="${playwright_core_integrity#sha512-}"
hex="$(printf '%s' "$encoded" | base64 -d 2>/dev/null \
  | od -An -tx1 | tr -d ' \n')"
[[ "$hex" =~ ^[0-9a-f]{128}$ ]]
playwright_core_tar="$source_npm_cache/_cacache/content-v2/sha512/${hex:0:2}/${hex:2:2}/${hex:4}"
[[ -f "$playwright_core_tar" && ! -L "$playwright_core_tar" \
  && "$(stat -c '%h' -- "$playwright_core_tar")" == 1 \
  && "$(sha512sum -- "$playwright_core_tar" | cut -d' ' -f1)" == "$hex" ]]
tar -xOf "$playwright_core_tar" package/browsers.json >"$browsers_json"
browsers_json_sha256="$(sha256sum -- "$browsers_json" | cut -d' ' -f1)"
jq -e '
  [.browsers[]
    | select(.name == "chromium"
        or .name == "chromium-headless-shell"
        or .name == "ffmpeg")
    | {name, revision, browser_version: (.browserVersion // "")}
  ] == [
    {
      name: "chromium",
      revision: "1223",
      browser_version: "148.0.7778.96"
    },
    {
      name: "chromium-headless-shell",
      revision: "1223",
      browser_version: "148.0.7778.96"
    },
    {name: "ffmpeg", revision: "1011", browser_version: ""}
  ]
' "$browsers_json" >/dev/null
mkdir -m 0755 "$closed"
for index in "${!cache_dirs[@]}"; do
  relative="${cache_dirs[$index]}"
  source_dir="$source_browsers/$relative"
  [[ -d "$source_dir" && ! -L "$source_dir" \
    && -f "$source_dir/INSTALLATION_COMPLETE" \
    && ! -L "$source_dir/INSTALLATION_COMPLETE" \
    && -f "$source_dir/${executables[$index]}" \
    && ! -L "$source_dir/${executables[$index]}" \
    && -z "$(find "$source_dir" -xdev ! -type d ! -type f -print -quit)" \
    && -z "$(find "$source_dir" -xdev -type f -links +1 -print -quit)" ]] \
    || exit 1
  cp -a --reflink=never -- "$source_dir" "$closed/$relative"
  rm -f -- "$closed/$relative/DEPENDENCIES_VALIDATED"
done
[[ -z "$(find "$closed" -mindepth 1 -type d -empty -print -quit)" ]]
find "$closed" -xdev -type d -exec chmod 0555 {} +
while IFS= read -r -d '' path; do
  if [[ -x "$path" ]]; then
    chmod 0555 "$path"
  else
    chmod 0444 "$path"
  fi
done < <(find "$closed" -xdev -type f -print0)
jain_write_native_build_tools_inventory "$closed" "$inventory" content
inventory_sha256="$(sha256sum -- "$inventory" | cut -d' ' -f1)"
file_count="$(wc -l <"$inventory")"
directory_count="$(find "$closed" -mindepth 1 -type d | wc -l)"
total_bytes="$(awk -F '\t' '{ total += $3 } END { print total + 0 }' "$inventory")"
chromium_executable="$closed/chromium-1223/chrome-linux64/chrome"
headless_executable="$closed/chromium_headless_shell-1223/chrome-headless-shell-linux64/chrome-headless-shell"
ffmpeg_executable="$closed/ffmpeg-1011/ffmpeg-linux"
chromium_sha256="$(sha256sum -- "$chromium_executable" | cut -d' ' -f1)"
headless_sha256="$(sha256sum -- "$headless_executable" | cut -d' ' -f1)"
ffmpeg_sha256="$(sha256sum -- "$ffmpeg_executable" | cut -d' ' -f1)"
chromium_size="$(stat -c '%s' -- "$chromium_executable")"
headless_size="$(stat -c '%s' -- "$headless_executable")"
ffmpeg_size="$(stat -c '%s' -- "$ffmpeg_executable")"
final="$destination/$inventory_sha256"
[[ ! -e "$final" ]]
chmod u+w "$closed"
mv -- "$closed" "$final"
chmod u-w "$final"
jq -n \
  --arg inventory "$inventory_sha256" \
  --arg root "/var/lib/jain-host-ci/playwright-browsers/$inventory_sha256" \
  --arg lock_sha "$(sha256sum -- "$lockfile" | cut -d' ' -f1)" \
  --arg platform "$platform" --arg arch "$arch" --arg libc "$libc" \
  --arg version "$playwright_core_version" \
  --arg core_integrity "$playwright_core_integrity" \
  --arg browsers_json_sha "$browsers_json_sha256" \
  --arg chromium_sha "$chromium_sha256" \
  --arg headless_sha "$headless_sha256" \
  --arg ffmpeg_sha "$ffmpeg_sha256" \
  --argjson file_count "$file_count" \
  --argjson directory_count "$directory_count" \
  --argjson total_bytes "$total_bytes" \
  --argjson chromium_size "$chromium_size" \
  --argjson headless_size "$headless_size" \
  --argjson ffmpeg_size "$ffmpeg_size" '{
    schema_version: "jain.playwright-browser-cache/v1",
    format: "playwright-browser-cache-v1",
    cache_root: $root,
    inventory_sha256: $inventory,
    file_count: $file_count,
    directory_count: $directory_count,
    total_bytes: $total_bytes,
    package_lock_path: "apps/web/package-lock.json",
    package_lock_sha256: $lock_sha,
    platform: $platform,
    arch: $arch,
    libc: $libc,
    playwright_core_version: $version,
    playwright_core_integrity: $core_integrity,
    browsers_json_sha256: $browsers_json_sha,
    artifacts: [
      {
        name: "chromium",
        revision: "1223",
        browser_version: "148.0.7778.96",
        cache_dir: "chromium-1223",
        executable: "chrome-linux64/chrome",
        executable_sha256: $chromium_sha,
        executable_size: $chromium_size
      },
      {
        name: "chromium-headless-shell",
        revision: "1223",
        browser_version: "148.0.7778.96",
        cache_dir: "chromium_headless_shell-1223",
        executable: "chrome-headless-shell-linux64/chrome-headless-shell",
        executable_sha256: $headless_sha,
        executable_size: $headless_size
      },
      {
        name: "ffmpeg",
        revision: "1011",
        browser_version: "",
        cache_dir: "ffmpeg-1011",
        executable: "ffmpeg-linux",
        executable_sha256: $ffmpeg_sha,
        executable_size: $ffmpeg_size
      }
    ]
  }' >"$authority"
jain_validate_playwright_browser_cache "$authority" "$final" content
checkout="${lockfile%/apps/web/package-lock.json}"
jain_playwright_browser_cache_matches_lock \
  "$authority" "$final" "$checkout" "$source_npm_cache" content
