#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/playwright-browser-runtime.sh
source "$repo_root/ops/ci/playwright-browser-runtime.sh"
fixture="$(mktemp -d /tmp/jain-playwright-browser-test.XXXXXX)"
cleanup() {
  chmod -R u+w "$fixture" 2>/dev/null || true
  rm -rf -- "$fixture"
}
trap cleanup EXIT
source_root="$fixture/source"
npm_cache="$fixture/npm-cache"
checkout="$fixture/redline-web"
destination="$fixture/destination"
mkdir -p \
  "$source_root/chromium-1223/chrome-linux64" \
  "$source_root/chromium_headless_shell-1223/chrome-headless-shell-linux64" \
  "$source_root/ffmpeg-1011" \
  "$checkout/apps/web" "$destination" "$npm_cache/_cacache/content-v2/sha512"
for directory in \
  chromium-1223 chromium_headless_shell-1223 ffmpeg-1011; do
  : >"$source_root/$directory/INSTALLATION_COMPLETE"
done
printf '#!/usr/bin/env bash\nexit 0\n' \
  >"$source_root/chromium-1223/chrome-linux64/chrome"
printf '#!/usr/bin/env bash\nexit 0\n' \
  >"$source_root/chromium_headless_shell-1223/chrome-headless-shell-linux64/chrome-headless-shell"
printf '#!/usr/bin/env bash\nexit 0\n' >"$source_root/ffmpeg-1011/ffmpeg-linux"
chmod 0755 \
  "$source_root/chromium-1223/chrome-linux64/chrome" \
  "$source_root/chromium_headless_shell-1223/chrome-headless-shell-linux64/chrome-headless-shell" \
  "$source_root/ffmpeg-1011/ffmpeg-linux"
core_package="$fixture/core-package"
mkdir -p "$core_package/package"
cat >"$core_package/package/browsers.json" <<'JSON'
{
  "browsers": [
    {
      "name": "chromium",
      "revision": "1223",
      "installByDefault": true,
      "browserVersion": "148.0.7778.96"
    },
    {
      "name": "chromium-headless-shell",
      "revision": "1223",
      "installByDefault": true,
      "browserVersion": "148.0.7778.96"
    },
    {
      "name": "ffmpeg",
      "revision": "1011",
      "installByDefault": true
    }
  ]
}
JSON
core_tar="$fixture/playwright-core.tgz"
tar -czf "$core_tar" -C "$core_package" package/browsers.json
core_hex="$(sha512sum -- "$core_tar" | cut -d' ' -f1)"
core_integrity="sha512-$(
  openssl dgst -sha512 -binary "$core_tar" | base64 -w0
)"
core_content="$npm_cache/_cacache/content-v2/sha512/${core_hex:0:2}/${core_hex:2:2}/${core_hex:4}"
mkdir -p "$(dirname "$core_content")"
cp -- "$core_tar" "$core_content"
jq -n --arg integrity "$core_integrity" '{
  name: "redline-web",
  lockfileVersion: 3,
  packages: {
    "node_modules/@playwright/test": {version: "1.60.0"},
    "node_modules/playwright": {version: "1.60.0"},
    "node_modules/playwright-core": {
      version: "1.60.0",
      resolved: "https://registry.npmjs.org/playwright-core/-/playwright-core-1.60.0.tgz",
      integrity: $integrity
    }
  }
}' >"$checkout/apps/web/package-lock.json"
authority="$destination/playwright-browser.lock.json"
"$repo_root/ops/ci/playwright-browser-authority.sh" \
  --source-browsers "$source_root" \
  --source-npm-cache "$npm_cache" \
  --lockfile "$checkout/apps/web/package-lock.json" \
  --platform linux --arch x64 --libc glibc \
  --destination "$destination" --authority "$authority"
cache_root="$destination/$(jq -er '.inventory_sha256' "$authority")"
jain_validate_playwright_browser_cache "$authority" "$cache_root" content
jain_playwright_browser_cache_matches_lock \
  "$authority" "$cache_root" "$checkout" "$npm_cache" content

expect_rejected() {
  local description="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    printf 'Playwright browser authority accepted hostile case: %s\n' \
      "$description" >&2
    exit 1
  fi
}

hostile="$fixture/hostile.json"
for mutation in \
  '.unexpected=true' \
  '.schema_version="jain.playwright-browser-cache/v0"' \
  '.platform="darwin"' \
  '.arch="arm64"' \
  '.libc="musl"' \
  '.playwright_core_version="1.59.0"' \
  '.artifacts[0].revision="1224"' \
  '.artifacts[1].executable="../escape"' \
  '.artifacts[2].executable_size=1' \
  '.directory_count=1'; do
  jq "$mutation" "$authority" >"$hostile"
  expect_rejected "$mutation" \
    jain_validate_playwright_browser_cache "$hostile" "$cache_root" content
done

jq '.browsers_json_sha256="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"' \
  "$authority" >"$hostile"
expect_rejected 'changed Playwright browsers descriptor digest' \
  jain_playwright_browser_cache_matches_lock \
  "$hostile" "$cache_root" "$checkout" "$npm_cache" content

changed_checkout="$fixture/changed-redline-web"
cp -a -- "$checkout" "$changed_checkout"
jq '.packages["node_modules/playwright-core"].version="1.59.0"' \
  "$checkout/apps/web/package-lock.json" \
  >"$changed_checkout/apps/web/package-lock.json"
expect_rejected 'changed product lock' \
  jain_playwright_browser_cache_matches_lock \
  "$authority" "$cache_root" "$changed_checkout" "$npm_cache" content

hostile_cache="$fixture/hostile-cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
printf 'tampered\n' \
  >"$hostile_cache/chromium-1223/chrome-linux64/chrome"
chmod -R a-w "$hostile_cache"
expect_rejected 'tampered executable' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod u+w "$hostile_cache"
printf 'extra\n' >"$hostile_cache/extra"
chmod 0444 "$hostile_cache/extra"
chmod u-w "$hostile_cache"
expect_rejected 'extra top-level node' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod u+w "$hostile_cache"
mv "$hostile_cache/ffmpeg-1011" "$hostile_cache/ffmpeg-1011.real"
ln -s ffmpeg-1011.real "$hostile_cache/ffmpeg-1011"
chmod u-w "$hostile_cache"
expect_rejected 'symlinked browser directory' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
ln "$hostile_cache/ffmpeg-1011/ffmpeg-linux" \
  "$hostile_cache/ffmpeg-1011/linked-copy"
chmod -R a-w "$hostile_cache"
expect_rejected 'hardlinked browser file' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
rm "$hostile_cache/chromium_headless_shell-1223/INSTALLATION_COMPLETE"
chmod -R a-w "$hostile_cache"
expect_rejected 'missing installation marker' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
mkdir "$hostile_cache/chromium-1223/empty"
chmod -R a-w "$hostile_cache"
expect_rejected 'extra empty directory' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
: >"$hostile_cache/ffmpeg-1011/DEPENDENCIES_VALIDATED"
chmod -R a-w "$hostile_cache"
expect_rejected 'mutable dependency-validation marker' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
printf 'not empty\n' \
  >"$hostile_cache/chromium-1223/INSTALLATION_COMPLETE"
chmod -R a-w "$hostile_cache"
expect_rejected 'nonempty installation marker' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

chmod -R u+w "$hostile_cache"
rm -rf -- "$hostile_cache"
cp -a -- "$cache_root" "$hostile_cache"
chmod -R u+w "$hostile_cache"
mkfifo "$hostile_cache/chromium-1223/hostile-fifo"
chmod -R a-w "$hostile_cache"
expect_rejected 'special inode' \
  jain_validate_playwright_browser_cache "$authority" "$hostile_cache" content

printf 'Playwright browser runtime authority tests passed\n'
