#!/usr/bin/env bash
set -euo pipefail

[[ "$#" == 1 ]] || {
  printf 'usage: %s <closed-native-build-tool-root>\n' "${0##*/}" >&2
  exit 2
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"

bundle_root="$(realpath -e -- "$1")"
scratch="$(mktemp -d /tmp/jain-native-authority.XXXXXX)"
cleanup() {
  rm -rf -- "$scratch"
}
trap cleanup EXIT

inventory_file="$scratch/inventory.tsv"
jain_write_native_build_tools_inventory \
  "$bundle_root" "$inventory_file" content
inventory_sha256="$(sha256sum -- "$inventory_file" | cut -d' ' -f1)"
file_count="$(wc -l <"$inventory_file")"

for tool in cmake ninja ragel yasm; do
  path="$bundle_root/bin/$tool"
  [[ -f "$path" && ! -L "$path" \
    && "$(stat -c '%a:%h' -- "$path")" == '555:1' ]] || {
    printf 'invalid native build tool: %s\n' "$tool" >&2
    exit 1
  }
  printf -v "${tool}_size" '%s' "$(stat -c %s -- "$path")"
  printf -v "${tool}_sha" '%s' "$(sha256sum -- "$path" | cut -d' ' -f1)"
  version="$("$path" --version 2>&1)"
  version="${version%%$'\n'*}"
  [[ -n "$version" && "$version" != *$'\r'* && "$version" != *$'\t'* ]] \
    || exit 1
  printf -v "${tool}_version" '%s' "$version"
done

jq -n --arg inventory "$inventory_sha256" \
  --arg root "/var/lib/jain-host-ci/native-build-tools/$inventory_sha256" \
  --arg cmake_sha "$cmake_sha" --arg cmake_version "$cmake_version" \
  --arg ninja_sha "$ninja_sha" --arg ninja_version "$ninja_version" \
  --arg ragel_sha "$ragel_sha" --arg ragel_version "$ragel_version" \
  --arg yasm_sha "$yasm_sha" --arg yasm_version "$yasm_version" \
  --argjson file_count "$file_count" \
  --argjson cmake_size "$cmake_size" --argjson ninja_size "$ninja_size" \
  --argjson ragel_size "$ragel_size" --argjson yasm_size "$yasm_size" \
  '{schema_version:"jain.native-build-tools/v1",bundle_root:$root,
    inventory_sha256:$inventory,file_count:$file_count,
    tools:{
      cmake:{path:"bin/cmake",mode:"555",size:$cmake_size,
        sha256:$cmake_sha,version:$cmake_version},
      ninja:{path:"bin/ninja",mode:"555",size:$ninja_size,
        sha256:$ninja_sha,version:$ninja_version},
      ragel:{path:"bin/ragel",mode:"555",size:$ragel_size,
        sha256:$ragel_sha,version:$ragel_version},
      yasm:{path:"bin/yasm",mode:"555",size:$yasm_size,
        sha256:$yasm_sha,version:$yasm_version}}}'
