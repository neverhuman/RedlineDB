#!/usr/bin/env bash
# Closed Playwright Chromium authority used by the root host-CI sandbox.

jain_playwright_core_browsers_json_matches() (
  local authority="${1:?Playwright browser authority is required}"
  local npm_cache_root="${2:?validated npm cache root is required}"
  local integrity encoded hex content scratch browsers_json expected_sha
  integrity="$(jq -er '.playwright_core_integrity' "$authority")" || return 1
  [[ "$integrity" =~ ^sha512-([A-Za-z0-9+/]+={0,2})$ ]] || return 1
  encoded="${BASH_REMATCH[1]}"
  hex="$(printf '%s' "$encoded" | base64 -d 2>/dev/null \
    | od -An -tx1 | tr -d ' \n')" || return 1
  [[ "$hex" =~ ^[0-9a-f]{128}$ ]] || return 1
  content="$npm_cache_root/_cacache/content-v2/sha512/${hex:0:2}/${hex:2:2}/${hex:4}"
  [[ -f "$content" && ! -L "$content" \
    && "$(stat -c '%h' -- "$content")" == 1 \
    && "$(sha512sum -- "$content" | cut -d' ' -f1)" == "$hex" ]] \
    || return 1
  scratch="$(mktemp -d /tmp/jain-playwright-core-descriptor.XXXXXX)" \
    || return 1
  browsers_json="$scratch/browsers.json"
  trap 'rm -rf -- "$scratch"' EXIT
  tar -xOf "$content" package/browsers.json >"$browsers_json" || return 1
  expected_sha="$(jq -er '.browsers_json_sha256' "$authority")" || return 1
  [[ "$(sha256sum -- "$browsers_json" | cut -d' ' -f1)" == "$expected_sha" ]] \
    || return 1
  jq -e '
    [.browsers[]
      | select(.name == "chromium"
          or .name == "chromium-headless-shell"
          or .name == "ffmpeg")
      | {
          name,
          revision,
          browser_version: (.browserVersion // "")
        }
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
      {
        name: "ffmpeg",
        revision: "1011",
        browser_version: ""
      }
    ]
  ' "$browsers_json" >/dev/null
)

jain_validate_playwright_browser_cache() (
  local authority="${1:?Playwright browser authority is required}"
  local cache_root="${2:?Playwright browser cache root is required}"
  local ownership_mode="${3:-content}"
  local expected_inventory expected_count expected_directories expected_bytes
  local actual_inventory actual_count actual_directories actual_bytes scratch
  local inventory expected_nodes actual_nodes relative executable marker
  local expected_executable_sha expected_executable_size
  [[ "$ownership_mode" == root || "$ownership_mode" == content ]] || return 1
  [[ -f "$authority" && ! -L "$authority" ]] || return 1
  jq -e '
    select(type == "object")
    | select((keys | sort) == [
        "arch", "artifacts", "browsers_json_sha256", "cache_root",
        "directory_count", "file_count", "format", "inventory_sha256",
        "libc", "package_lock_path", "package_lock_sha256", "platform",
        "playwright_core_integrity", "playwright_core_version",
        "schema_version", "total_bytes"
      ])
    | select(.schema_version == "jain.playwright-browser-cache/v1")
    | select(.format == "playwright-browser-cache-v1")
    | select(.cache_root
        | test("^/var/lib/jain-host-ci/playwright-browsers/[0-9a-f]{64}$"))
    | select(.inventory_sha256 | test("^[0-9a-f]{64}$"))
    | select(.cache_root ==
        ("/var/lib/jain-host-ci/playwright-browsers/" + .inventory_sha256))
    | select(.file_count | type == "number" and . > 3 and floor == .)
    | select(.directory_count | type == "number" and . > 2 and floor == .)
    | select(.total_bytes | type == "number" and . > 0 and floor == .)
    | select(.package_lock_path == "apps/web/package-lock.json")
    | select(.package_lock_sha256 | test("^[0-9a-f]{64}$"))
    | select(.platform == "linux")
    | select(.arch == "x64")
    | select(.libc == "glibc")
    | select(.playwright_core_version == "1.60.0")
    | select(.playwright_core_integrity
        | test("^sha512-[A-Za-z0-9+/]+={0,2}$"))
    | select(.browsers_json_sha256
        | test("^[0-9a-f]{64}$"))
    | select([.artifacts[] | del(.executable_sha256, .executable_size)] == [
        {
          name: "chromium",
          revision: "1223",
          browser_version: "148.0.7778.96",
          cache_dir: "chromium-1223",
          executable: "chrome-linux64/chrome"
        },
        {
          name: "chromium-headless-shell",
          revision: "1223",
          browser_version: "148.0.7778.96",
          cache_dir: "chromium_headless_shell-1223",
          executable: "chrome-headless-shell-linux64/chrome-headless-shell"
        },
        {
          name: "ffmpeg",
          revision: "1011",
          browser_version: "",
          cache_dir: "ffmpeg-1011",
          executable: "ffmpeg-linux"
        }
      ])
    | select(all(.artifacts[];
        (.executable_sha256 | test("^[0-9a-f]{64}$"))
        and (.executable_size | type == "number"
          and . > 0 and floor == .)))
  ' "$authority" >/dev/null || return 1
  expected_inventory="$(jq -er '.inventory_sha256' "$authority")" || return 1
  expected_count="$(jq -er '.file_count' "$authority")" || return 1
  expected_directories="$(jq -er '.directory_count' "$authority")" || return 1
  expected_bytes="$(jq -er '.total_bytes' "$authority")" || return 1
  [[ "$cache_root" == /* && "${cache_root##*/}" == "$expected_inventory" \
    && -d "$cache_root" && ! -L "$cache_root" \
    && "$(realpath -e -- "$cache_root")" == "$cache_root" ]] || return 1
  expected_nodes="$(
    jq -r '.artifacts[].cache_dir' "$authority" | LC_ALL=C sort
  )" || return 1
  actual_nodes="$(
    find "$cache_root" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort
  )" || return 1
  [[ "$actual_nodes" == "$expected_nodes" ]] || return 1
  actual_directories="$(
    find "$cache_root" -mindepth 1 -type d | wc -l
  )" || return 1
  [[ "$actual_directories" == "$expected_directories" \
    && -z "$(find "$cache_root" -mindepth 1 -type d -empty -print -quit)" \
    && -z "$(find "$cache_root" -name DEPENDENCIES_VALIDATED -print -quit)" ]] \
    || return 1
  while IFS=$'\t' read -r relative executable; do
    marker="$cache_root/$relative/INSTALLATION_COMPLETE"
    executable="$cache_root/$relative/$executable"
    expected_executable_sha="$(
      jq -er --arg cache_dir "$relative" '
        .artifacts[] | select(.cache_dir == $cache_dir) | .executable_sha256
      ' "$authority"
    )" || return 1
    expected_executable_size="$(
      jq -er --arg cache_dir "$relative" '
        .artifacts[] | select(.cache_dir == $cache_dir) | .executable_size
      ' "$authority"
    )" || return 1
    [[ -f "$marker" && ! -L "$marker" \
      && "$(stat -c '%a:%h:%s' -- "$marker")" == '444:1:0' \
      && -f "$executable" && ! -L "$executable" \
      && "$(stat -c '%a:%h:%s' -- "$executable")" \
        == "555:1:$expected_executable_size" \
      && "$(sha256sum -- "$executable" | cut -d' ' -f1)" \
        == "$expected_executable_sha" ]] || return 1
  done < <(jq -r '.artifacts[] | [.cache_dir,.executable] | @tsv' "$authority")
  scratch="$(mktemp -d /tmp/jain-playwright-browser-validate.XXXXXX)" \
    || return 1
  inventory="$scratch/inventory.tsv"
  trap 'rm -rf -- "$scratch"' EXIT
  jain_write_native_build_tools_inventory \
    "$cache_root" "$inventory" "$ownership_mode" || return 1
  actual_count="$(wc -l <"$inventory")" || return 1
  actual_bytes="$(awk -F '\t' '{ total += $3 } END { print total + 0 }' \
    "$inventory")" || return 1
  actual_inventory="$(sha256sum -- "$inventory" | cut -d' ' -f1)" || return 1
  [[ "$actual_count" == "$expected_count" \
    && "$actual_bytes" == "$expected_bytes" \
    && "$actual_inventory" == "$expected_inventory" ]]
)

jain_playwright_browser_cache_matches_lock() (
  local authority="${1:?Playwright browser authority is required}"
  local cache_root="${2:?Playwright browser cache root is required}"
  local checkout="${3:?product checkout is required}"
  local npm_cache_root="${4:?validated npm cache root is required}"
  local ownership_mode="${5:-content}"
  local relative lockfile expected_lock expected_version expected_integrity
  relative="$(jq -er '.package_lock_path' "$authority")" || return 1
  lockfile="$checkout/$relative"
  expected_lock="$(jq -er '.package_lock_sha256' "$authority")" || return 1
  expected_version="$(jq -er '.playwright_core_version' "$authority")" || return 1
  expected_integrity="$(jq -er '.playwright_core_integrity' "$authority")" \
    || return 1
  [[ -d "$checkout" && ! -L "$checkout" && -f "$lockfile" \
    && ! -L "$lockfile" && "$(stat -c '%h' -- "$lockfile")" == 1 \
    && "$(sha256sum -- "$lockfile" | cut -d' ' -f1)" == "$expected_lock" ]] \
    || return 1
  jq -e --arg version "$expected_version" \
    --arg integrity "$expected_integrity" '
    select(.lockfileVersion == 3)
    | select(.packages["node_modules/@playwright/test"].version == $version)
    | select(.packages["node_modules/playwright"].version == $version)
    | select(.packages["node_modules/playwright-core"].version == $version)
    | select(.packages["node_modules/playwright-core"].integrity == $integrity)
  ' "$lockfile" >/dev/null || return 1
  jain_playwright_core_browsers_json_matches \
    "$authority" "$npm_cache_root" || return 1
  jain_validate_playwright_browser_cache \
    "$authority" "$cache_root" "$ownership_mode"
)
