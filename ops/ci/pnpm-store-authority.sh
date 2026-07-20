#!/usr/bin/env bash
set -euo pipefail

[[ "$#" == 3 ]] || {
  printf 'usage: %s <closed-store-root> <pnpm-lock.yaml> <sealed-pnpm>\n' \
    "${0##*/}" >&2
  exit 2
}
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/native-runtime.sh
source "$repo_root/ops/ci/native-runtime.sh"
store_root="$(realpath -e -- "$1")"
lockfile="$(realpath -e -- "$2")"
pnpm="$(realpath -e -- "$3")"
scratch="$(mktemp -d /tmp/jain-pnpm-authority.XXXXXX)"
trap 'rm -rf -- "$scratch"' EXIT
inventory="$scratch/inventory.tsv"
jain_write_native_build_tools_inventory "$store_root" "$inventory" content
inventory_sha256="$(sha256sum -- "$inventory" | cut -d' ' -f1)"
file_count="$(wc -l <"$inventory")"
pnpm_version="$(/usr/bin/env -i PATH="$(dirname "$pnpm"):/usr/bin:/bin" \
  LC_ALL=C HOME=/nonexistent "$pnpm" --version)"
[[ "$pnpm_version" == 10.18.3 \
  && -d "$store_root/v10/files" && -d "$store_root/v10/index" \
  && ! -e "$store_root/v10/projects" ]] || exit 1
jq -n --arg inventory "$inventory_sha256" \
  --arg root "/var/lib/jain-host-ci/pnpm-store/$inventory_sha256" \
  --arg lock_sha "$(sha256sum -- "$lockfile" | cut -d' ' -f1)" \
  --arg pnpm_version "$pnpm_version" --argjson file_count "$file_count" \
  '{schema_version:"jain.pnpm-store/v1",store_root:$root,
    inventory_sha256:$inventory,file_count:$file_count,format:"v10",
    lockfile_path:"apps/web/pnpm-lock.yaml",lockfile_sha256:$lock_sha,
    pnpm_version:$pnpm_version}'
