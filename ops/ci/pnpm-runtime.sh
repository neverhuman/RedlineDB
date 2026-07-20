#!/usr/bin/env bash
# Closed offline pnpm-store authority used by the root host-CI sandbox.

jain_validate_pnpm_store() (
  local authority="${1:?pnpm store authority is required}"
  local store_root="${2:?pnpm store root is required}"
  local ownership_mode="${3:-content}"
  local expected_inventory expected_count actual_inventory actual_count
  local scratch inventory
  [[ "$ownership_mode" == root || "$ownership_mode" == content ]] || return 1
  [[ -f "$authority" && ! -L "$authority" ]] || return 1
  jq -e '
    select(type == "object")
    | select((keys | sort) ==
        ["file_count", "format", "inventory_sha256", "lockfile_path",
         "lockfile_sha256", "pnpm_version", "schema_version", "store_root"])
    | select(.schema_version == "jain.pnpm-store/v1")
    | select(.format == "v10")
    | select(.store_root
        | test("^/var/lib/jain-host-ci/pnpm-store/[0-9a-f]{64}$"))
    | select(.inventory_sha256 | test("^[0-9a-f]{64}$"))
    | select(.store_root ==
        ("/var/lib/jain-host-ci/pnpm-store/" + .inventory_sha256))
    | select(.file_count | type == "number" and . > 1 and floor == .)
    | select(.lockfile_path == "apps/web/pnpm-lock.yaml")
    | select(.lockfile_sha256 | test("^[0-9a-f]{64}$"))
    | select(.pnpm_version == "10.18.3")' "$authority" >/dev/null || return 1
  expected_inventory="$(jq -er '.inventory_sha256' "$authority")" || return 1
  expected_count="$(jq -er '.file_count' "$authority")" || return 1
  [[ "$store_root" == /* && "${store_root##*/}" == "$expected_inventory" \
    && -d "$store_root/v10/files" && -d "$store_root/v10/index" \
    && ! -e "$store_root/v10/projects" ]] || return 1
  scratch="$(mktemp -d /tmp/jain-pnpm-store-validate.XXXXXX)" || return 1
  inventory="$scratch/inventory.tsv"
  trap 'rm -rf -- "$scratch"' EXIT
  jain_write_native_build_tools_inventory \
    "$store_root" "$inventory" "$ownership_mode" || return 1
  actual_count="$(wc -l <"$inventory")" || return 1
  actual_inventory="$(sha256sum -- "$inventory" | cut -d' ' -f1)" || return 1
  [[ "$actual_count" == "$expected_count" \
    && "$actual_inventory" == "$expected_inventory" ]]
)

jain_pnpm_lockfile_matches() {
  local authority="${1:?pnpm store authority is required}"
  local checkout="${2:?product checkout is required}"
  local relative lockfile expected
  relative="$(jq -er '.lockfile_path' "$authority")" || return 1
  lockfile="$checkout/$relative"
  expected="$(jq -er '.lockfile_sha256' "$authority")" || return 1
  [[ -d "$checkout" && ! -L "$checkout" && -f "$lockfile" \
    && ! -L "$lockfile" && "$(stat -c '%h' -- "$lockfile")" == 1 \
    && "$(sha256sum -- "$lockfile" | cut -d' ' -f1)" == "$expected" ]]
}

jain_validate_staged_pnpm_store() (
  local authority="${1:?pnpm store authority is required}"
  local source_root="${2:?sealed pnpm store root is required}"
  local staged_root="${3:?staged pnpm store root is required}"
  local source_ownership="${4:-root}"
  local scratch source_inventory checksum_file relative node_type mode links
  local expected_mode source_count=0 staged_count=0
  [[ "$source_ownership" == root || "$source_ownership" == content ]] \
    || return 1
  jain_validate_pnpm_store \
    "$authority" "$source_root" "$source_ownership" || return 1
  [[ "$staged_root" == /* && -d "$staged_root" && ! -L "$staged_root" \
    && "$(realpath -e -- "$staged_root")" == "$staged_root" \
    && "$(stat -c '%F:%a' -- "$staged_root")" == 'directory:700' ]] \
    || return 1

  while IFS= read -r -d '' relative; do
    IFS= read -r -d '' node_type || return 1
    IFS= read -r -d '' mode || return 1
    IFS= read -r -d '' links || return 1
    jain_native_relative_path_is_canonical "$relative" || return 1
    case "$node_type" in
      d) [[ "$mode" == 700 ]] || return 1 ;;
      f)
        expected_mode=600
        [[ "$relative" != *-exec ]] || expected_mode=700
        [[ "$mode" == "$expected_mode" && "$links" == 1 ]] || return 1
        staged_count=$((staged_count + 1))
        ;;
      *) return 1 ;;
    esac
  done < <(find "$staged_root" -xdev -mindepth 1 \
    -printf '%P\0%y\0%m\0%n\0')

  scratch="$(mktemp -d /tmp/jain-pnpm-stage-validate.XXXXXX)" || return 1
  trap 'rm -rf -- "$scratch"' EXIT
  source_inventory="$scratch/source.tsv"
  checksum_file="$scratch/checksums"
  jain_write_native_build_tools_inventory \
    "$source_root" "$source_inventory" "$source_ownership" || return 1
  source_count="$(wc -l <"$source_inventory")" || return 1
  awk -F '\t' '{ print $4 "  " $1 }' "$source_inventory" \
    >"$checksum_file" || return 1
  [[ "$source_count" == "$(jq -er '.file_count' "$authority")" \
    && "$staged_count" == "$source_count" ]] \
    || return 1
  (cd "$staged_root" && sha256sum --check --strict --quiet "$checksum_file") \
    || return 1
  jain_validate_pnpm_store \
    "$authority" "$source_root" "$source_ownership"
)

jain_stage_pnpm_store() {
  local authority="${1:?pnpm store authority is required}"
  local source_root="${2:?pnpm store root is required}"
  local destination_parent="${3:?pnpm staging parent is required}"
  local source_ownership="${4:-root}"
  local inventory destination path
  [[ "$source_ownership" == root || "$source_ownership" == content ]] \
    || return 1
  inventory="$(jq -er '.inventory_sha256' "$authority")" || return 1
  destination="$destination_parent/$inventory"
  [[ "$destination_parent" == /* && -d "$destination_parent" \
    && ! -L "$destination_parent" && ! -e "$destination" ]] || return 1
  jain_validate_pnpm_store \
    "$authority" "$source_root" "$source_ownership" || return 1
  /usr/bin/cp -a --reflink=never -- "$source_root" "$destination" || return 1
  jain_validate_pnpm_store "$authority" "$destination" content || return 1
  jain_validate_pnpm_store \
    "$authority" "$source_root" "$source_ownership" || return 1
  while IFS= read -r -d '' path; do chmod 0700 "$path" || return 1; done \
    < <(find "$destination" -type d -print0)
  while IFS= read -r -d '' path; do chmod 0600 "$path" || return 1; done \
    < <(find "$destination" -type f -print0)
  while IFS= read -r -d '' path; do chmod 0700 "$path" || return 1; done \
    < <(find "$destination" -type f -name '*-exec' -print0)
  jain_validate_staged_pnpm_store \
    "$authority" "$source_root" "$destination" "$source_ownership" \
    || return 1
  printf '%s\n' "$destination"
}
