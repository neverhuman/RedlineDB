#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
# shellcheck source=ops/ci/npm-runtime.sh
source "$repo_root/ops/ci/npm-runtime.sh"
tmp="$(mktemp -d /tmp/jain-npm-runtime-test.XXXXXX)"
cleanup() {
  chmod -R u+w "$tmp" 2>/dev/null || true
  rm -rf -- "$tmp"
}
trap cleanup EXIT
mkdir -p "$tmp/product/apps/web" "$tmp/source/_cacache/content-v2/sha512" \
  "$tmp/tarballs" "$tmp/stage"

for package in fixture-a fixture-b; do
  package_root="$tmp/package-$package/package"
  mkdir -p "$package_root"
  jq -n --arg name "$package" \
    '{name:$name,version:"1.0.0",main:"index.js"}' \
    >"$package_root/package.json"
  printf 'module.exports = "%s";\n' "$package" >"$package_root/index.js"
  tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
    -czf "$tmp/tarballs/$package.tgz" -C "${package_root%/package}" package
done

integrity_a="sha512-$(sha512sum "$tmp/tarballs/fixture-a.tgz" \
  | cut -d' ' -f1 | xxd -r -p | base64 -w0)"
integrity_b="sha512-$(sha512sum "$tmp/tarballs/fixture-b.tgz" \
  | cut -d' ' -f1 | xxd -r -p | base64 -w0)"
url_a="https://registry.npmjs.org/fixture-a/-/fixture-a-1.0.0.tgz"
url_b="https://registry.npmjs.org/fixture-b/-/fixture-b-1.0.0.tgz"
jq -n --arg ia "$integrity_a" --arg ib "$integrity_b" \
  --arg ua "$url_a" --arg ub "$url_b" '{
    name:"npm-runtime-fixture",version:"1.0.0",lockfileVersion:3,requires:true,
    packages:{
      "":{name:"npm-runtime-fixture",version:"1.0.0",
        dependencies:{"fixture-a":"1.0.0","fixture-b":"1.0.0"}},
      "node_modules/fixture-a":{version:"1.0.0",resolved:$ua,integrity:$ia},
      "node_modules/fixture-b":{version:"1.0.0",resolved:$ub,integrity:$ib}
    }
  }' >"$tmp/product/apps/web/package-lock.json"
cp "$tmp/product/apps/web/package-lock.json" "$tmp/product/apps/web/package.json"
jq '{name,version,dependencies:.packages[""].dependencies}' \
  "$tmp/product/apps/web/package-lock.json" >"$tmp/product/apps/web/package.json"

for package in fixture-a fixture-b; do
  tarball="$tmp/tarballs/$package.tgz"
  hex="$(sha512sum "$tarball" | cut -d' ' -f1)"
  content="$tmp/source/_cacache/content-v2/sha512/${hex:0:2}/${hex:2:2}/${hex:4}"
  mkdir -p "$(dirname "$content")"
  cp "$tarball" "$content"
done

authority_dir="$tmp/authority"
mkdir "$authority_dir"
bash "$repo_root/ops/ci/npm-cache-authority.sh" \
  --source-cache "$tmp/source" \
  --lockfile "$tmp/product/apps/web/package-lock.json" \
  --platform linux --arch x64 --libc glibc \
  --destination "$authority_dir" \
  --authority "$authority_dir/npm-cache.lock.json"
authority="$authority_dir/npm-cache.lock.json"
cache="$authority_dir/$(jq -er '.inventory_sha256' "$authority")"
jain_validate_npm_cache "$authority" "$cache" content
jain_npm_cache_matches_lock "$authority" "$cache" "$tmp/product" content
staged="$(jain_stage_npm_cache "$authority" "$cache" "$tmp/stage" content)"
jain_validate_staged_npm_cache "$authority" "$cache" "$staged" content

npm_environment=(
  env -i PATH=/usr/bin:/bin LC_ALL=C HOME="$tmp/home"
  NPM_CONFIG_CACHE="$staged" NPM_CONFIG_OFFLINE=true
  NPM_CONFIG_UPDATE_NOTIFIER=false
  NPM_CONFIG_USERCONFIG="$repo_root/ops/ci/npmrc.empty"
  NPM_CONFIG_GLOBALCONFIG=/dev/null
)
npm_command=(
  npm ci --prefix "$tmp/product/apps/web" --ignore-scripts
  --no-audit --no-fund
)
if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
  "${npm_environment[@]}" "${npm_command[@]}" >/dev/null
else
  sudo -n unshare -n --setuid "$(id -u)" --setgid "$(id -g)" \
    "${npm_environment[@]}" "${npm_command[@]}" >/dev/null
fi
[[ -f "$tmp/product/apps/web/node_modules/fixture-a/index.js" \
  && -f "$tmp/product/apps/web/node_modules/fixture-b/index.js" ]]

cp "$tmp/product/apps/web/package-lock.json" "$tmp/changed-lock.json"
jq '.packages["node_modules/fixture-a"].version = "1.0.1"' \
  "$tmp/changed-lock.json" >"$tmp/changed-lock.next"
mv "$tmp/changed-lock.next" "$tmp/product/apps/web/package-lock.json"
if jain_npm_cache_matches_lock \
  "$authority" "$cache" "$tmp/product" content 2>/dev/null; then
  printf 'npm cache accepted a changed package lock\n' >&2
  exit 1
fi
cp "$tmp/changed-lock.json" "$tmp/product/apps/web/package-lock.json"

jq '.unexpected = true' "$authority" >"$tmp/unknown-authority.json"
if jain_validate_npm_cache \
  "$tmp/unknown-authority.json" "$cache" content 2>/dev/null; then
  printf 'npm cache accepted an unknown authority field\n' >&2
  exit 1
fi
jq '.platform = "darwin"' "$authority" >"$tmp/wrong-platform.json"
if jain_validate_npm_cache \
  "$tmp/wrong-platform.json" "$cache" content 2>/dev/null; then
  printf 'npm cache accepted the wrong platform\n' >&2
  exit 1
fi
jq '.schema_version = "jain.npm-cache/v2"' \
  "$authority" >"$tmp/wrong-schema.json"
if jain_validate_npm_cache \
  "$tmp/wrong-schema.json" "$cache" content 2>/dev/null; then
  printf 'npm cache accepted the wrong authority schema\n' >&2
  exit 1
fi
jq '.arch = "arm64"' "$authority" >"$tmp/wrong-arch.json"
if jain_validate_npm_cache \
  "$tmp/wrong-arch.json" "$cache" content 2>/dev/null; then
  printf 'npm cache accepted the wrong architecture\n' >&2
  exit 1
fi
jq '.libc = "musl"' "$authority" >"$tmp/wrong-libc.json"
if jain_validate_npm_cache \
  "$tmp/wrong-libc.json" "$cache" content 2>/dev/null; then
  printf 'npm cache accepted the wrong libc\n' >&2
  exit 1
fi
jq '.closure_count += 1' "$authority" >"$tmp/wrong-closure-count.json"
if jain_npm_cache_matches_lock \
  "$tmp/wrong-closure-count.json" "$cache" \
  "$tmp/product" content 2>/dev/null; then
  printf 'npm cache accepted the wrong closure object count\n' >&2
  exit 1
fi
jq '.closure_sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"' \
  "$authority" >"$tmp/wrong-closure-digest.json"
if jain_npm_cache_matches_lock \
  "$tmp/wrong-closure-digest.json" "$cache" \
  "$tmp/product" content 2>/dev/null; then
  printf 'npm cache accepted the wrong closure digest\n' >&2
  exit 1
fi

inventory="$(jq -er '.inventory_sha256' "$authority")"
hostile_parent="$tmp/hostile"
hostile="$hostile_parent/$inventory"
mkdir "$hostile_parent"
cp -a "$cache" "$hostile"
chmod -R u+w "$hostile"
victim="$(find "$hostile/_cacache/content-v2/sha512" -type f | head -n 1)"
printf 'tampered\n' >>"$victim"
chmod -R a-w "$hostile"
if jain_validate_npm_cache "$authority" "$hostile" content 2>/dev/null; then
  printf 'npm cache accepted tampered content\n' >&2
  exit 1
fi

missing_parent="$tmp/missing"
missing="$missing_parent/$inventory"
mkdir "$missing_parent"
cp -a "$cache" "$missing"
chmod -R u+w "$missing"
rm -- "$(find "$missing/_cacache/index-v5" -type f | head -n 1)"
chmod -R a-w "$missing"
if jain_validate_npm_cache "$authority" "$missing" content 2>/dev/null; then
  printf 'npm cache accepted missing index content\n' >&2
  exit 1
fi

extra_parent="$tmp/extra"
extra="$extra_parent/$inventory"
mkdir "$extra_parent"
cp -a "$cache" "$extra"
chmod -R u+w "$extra"
printf 'extra\n' >"$extra/_cacache/extra"
chmod -R a-w "$extra"
if jain_validate_npm_cache "$authority" "$extra" content 2>/dev/null; then
  printf 'npm cache accepted extra content\n' >&2
  exit 1
fi

symlink_parent="$tmp/symlink"
symlink_cache="$symlink_parent/$inventory"
mkdir "$symlink_parent"
cp -a "$cache" "$symlink_cache"
chmod -R u+w "$symlink_cache"
victim="$(find "$symlink_cache/_cacache/index-v5" -type f | head -n 1)"
rm -- "$victim"
ln -s /dev/null "$victim"
chmod -R a-w "$symlink_cache"
if jain_validate_npm_cache \
  "$authority" "$symlink_cache" content 2>/dev/null; then
  printf 'npm cache accepted a symlink\n' >&2
  exit 1
fi

hardlink_parent="$tmp/hardlink"
hardlink_cache="$hardlink_parent/$inventory"
mkdir "$hardlink_parent"
cp -a "$cache" "$hardlink_cache"
chmod -R u+w "$hardlink_cache"
mapfile -t hardlink_files < <(
  find "$hardlink_cache/_cacache/index-v5" -type f | sort | head -n 2
)
rm -- "${hardlink_files[1]}"
ln "${hardlink_files[0]}" "${hardlink_files[1]}"
chmod -R a-w "$hardlink_cache"
if jain_validate_npm_cache \
  "$authority" "$hardlink_cache" content 2>/dev/null; then
  printf 'npm cache accepted a hardlink\n' >&2
  exit 1
fi

fifo_parent="$tmp/fifo"
fifo_cache="$fifo_parent/$inventory"
mkdir "$fifo_parent"
cp -a "$cache" "$fifo_cache"
chmod -R u+w "$fifo_cache"
victim="$(find "$fifo_cache/_cacache/index-v5" -type f | head -n 1)"
rm -- "$victim"
mkfifo "$victim"
chmod -R a-w "$fifo_cache"
if jain_validate_npm_cache \
  "$authority" "$fifo_cache" content 2>/dev/null; then
  printf 'npm cache accepted a FIFO\n' >&2
  exit 1
fi

staged_hostile="$tmp/staged-hostile"
cp -a "$staged" "$staged_hostile"
victim="$(find "$staged_hostile/_cacache/content-v2/sha512" \
  -type f | head -n 1)"
printf 'staged-tamper\n' >>"$victim"
if jain_validate_staged_npm_cache \
  "$authority" "$cache" "$staged_hostile" content 2>/dev/null; then
  printf 'npm cache accepted a tampered staged copy\n' >&2
  exit 1
fi

semantic_parent="$tmp/semantic"
semantic_work="$semantic_parent/work"
mkdir "$semantic_parent"
cp -a "$cache" "$semantic_work"
chmod -R u+w "$semantic_work"
victim="$(find "$semantic_work/_cacache/content-v2/sha512" -type f | head -n 1)"
printf 'semantic-tamper\n' >>"$victim"
chmod -R a-w "$semantic_work"
semantic_inventory="$tmp/semantic-inventory.tsv"
jain_write_native_build_tools_inventory \
  "$semantic_work" "$semantic_inventory" content
semantic_digest="$(sha256sum "$semantic_inventory" | cut -d' ' -f1)"
chmod u+w "$semantic_work"
semantic_cache="$semantic_parent/$semantic_digest"
mv "$semantic_work" "$semantic_cache"
chmod u-w "$semantic_cache"
jq --arg digest "$semantic_digest" \
  --arg root "/var/lib/jain-host-ci/npm-cache/$semantic_digest" \
  '.inventory_sha256 = $digest | .cache_root = $root' \
  "$authority" >"$tmp/semantic-authority.json"
jain_validate_npm_cache \
  "$tmp/semantic-authority.json" "$semantic_cache" content
if jain_npm_cache_matches_lock \
  "$tmp/semantic-authority.json" "$semantic_cache" \
  "$tmp/product" content 2>/dev/null; then
  printf 'npm closure accepted content with a rebound wrong SHA-512\n' >&2
  exit 1
fi

index_parent="$tmp/index-semantic"
index_work="$index_parent/work"
mkdir "$index_parent"
cp -a "$cache" "$index_work"
chmod -R u+w "$index_work"
victim="$(find "$index_work/_cacache/index-v5" -type f | head -n 1)"
row="$(tail -n 1 "$victim")"
record="${row#*$'\t'}"
record="$(jq -c '.metadata.url = "https://registry.npmjs.org/hostile.tgz"' \
  <<<"$record")"
checksum="$(printf '%s' "$record" | sha1sum | cut -d' ' -f1)"
printf '\n%s\t%s' "$checksum" "$record" >"$victim"
chmod -R a-w "$index_work"
index_inventory="$tmp/index-inventory.tsv"
jain_write_native_build_tools_inventory \
  "$index_work" "$index_inventory" content
index_digest="$(sha256sum "$index_inventory" | cut -d' ' -f1)"
chmod u+w "$index_work"
index_cache="$index_parent/$index_digest"
mv "$index_work" "$index_cache"
chmod u-w "$index_cache"
jq --arg digest "$index_digest" \
  --arg root "/var/lib/jain-host-ci/npm-cache/$index_digest" \
  '.inventory_sha256 = $digest | .cache_root = $root' \
  "$authority" >"$tmp/index-authority.json"
jain_validate_npm_cache "$tmp/index-authority.json" "$index_cache" content
if jain_npm_cache_matches_lock \
  "$tmp/index-authority.json" "$index_cache" \
  "$tmp/product" content 2>/dev/null; then
  printf 'npm closure accepted a rebound URL-index mismatch\n' >&2
  exit 1
fi

printf 'npm runtime authority ok\n'
