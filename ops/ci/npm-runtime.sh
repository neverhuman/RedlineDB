#!/usr/bin/env bash
# Closed npm CACache authority used by the root host-CI sandbox.

jain_write_npm_closure() {
  local lockfile="${1:?npm package lock is required}"
  local platform="${2:?npm platform is required}"
  local arch="${3:?npm architecture is required}"
  local libc="${4:?npm libc is required}"
  local destination="${5:?npm closure destination is required}"
  [[ -f "$lockfile" && ! -L "$lockfile" && ! -e "$destination" ]] || return 1
  jq -er --arg platform "$platform" --arg arch "$arch" --arg libc "$libc" '
    def matches($values; $wanted):
      ($values == null)
      or (
        (($values | index("!" + $wanted)) == null)
        and (
          ([$values[] | select(startswith("!") | not)] | length) == 0
          or (($values | index($wanted)) != null)
        )
      );
    [
      .packages
      | to_entries[]
      | select(
          (.key | type) == "string"
          and (.key | startswith("node_modules/"))
          and (.value.resolved | type) == "string"
          and (.value.resolved | startswith("https://registry.npmjs.org/"))
          and (.value.integrity | type) == "string"
          and (.value.integrity | test("^sha512-[A-Za-z0-9+/]+={0,2}$"))
        )
      | select(
          (.value.optional // false) == false
          or (
            matches(.value.os; $platform)
            and matches(.value.cpu; $arch)
            and matches(.value.libc; $libc)
          )
        )
      | [.key, .value.resolved, .value.integrity]
      | @tsv
    ]
    | unique
    | sort[]
  ' "$lockfile" >"$destination"
  [[ -s "$destination" ]]
}

jain_validate_npm_cache() (
  local authority="${1:?npm cache authority is required}"
  local cache_root="${2:?npm cache root is required}"
  local ownership_mode="${3:-content}"
  local expected_inventory expected_count actual_inventory actual_count
  local scratch inventory
  [[ "$ownership_mode" == root || "$ownership_mode" == content ]] || return 1
  [[ -f "$authority" && ! -L "$authority" ]] || return 1
  jq -e '
    select(type == "object")
    | select((keys | sort) == [
        "arch", "cache_root", "closure_count", "closure_sha256", "file_count",
        "format", "inventory_sha256", "libc", "npm_version",
        "package_lock_path", "package_lock_sha256", "platform",
        "schema_version"
      ])
    | select(.schema_version == "jain.npm-cache/v1")
    | select(.format == "cacache-v5")
    | select(.cache_root
        | test("^/var/lib/jain-host-ci/npm-cache/[0-9a-f]{64}$"))
    | select(.inventory_sha256 | test("^[0-9a-f]{64}$"))
    | select(.cache_root ==
        ("/var/lib/jain-host-ci/npm-cache/" + .inventory_sha256))
    | select(.file_count | type == "number" and . > 1 and floor == .)
    | select(.closure_count | type == "number" and . > 1 and floor == .)
    | select(.closure_sha256 | test("^[0-9a-f]{64}$"))
    | select(.package_lock_path == "apps/web/package-lock.json")
    | select(.package_lock_sha256 | test("^[0-9a-f]{64}$"))
    | select(.platform == "linux")
    | select(.arch == "x64")
    | select(.libc == "glibc")
    | select(.npm_version == "9.2.0")
  ' "$authority" >/dev/null || return 1
  expected_inventory="$(jq -er '.inventory_sha256' "$authority")" || return 1
  expected_count="$(jq -er '.file_count' "$authority")" || return 1
  [[ "$cache_root" == /* && "${cache_root##*/}" == "$expected_inventory" \
    && -d "$cache_root/_cacache/content-v2/sha512" \
    && -d "$cache_root/_cacache/index-v5" \
    && -z "$(find "$cache_root" -mindepth 1 -maxdepth 1 \
      ! -name _cacache -print -quit)" ]] || return 1
  scratch="$(mktemp -d /tmp/jain-npm-cache-validate.XXXXXX)" || return 1
  inventory="$scratch/inventory.tsv"
  trap 'rm -rf -- "$scratch"' EXIT
  jain_write_native_build_tools_inventory \
    "$cache_root" "$inventory" "$ownership_mode" || return 1
  actual_count="$(wc -l <"$inventory")" || return 1
  actual_inventory="$(sha256sum -- "$inventory" | cut -d' ' -f1)" || return 1
  [[ "$actual_count" == "$expected_count" \
    && "$actual_inventory" == "$expected_inventory" ]]
)

jain_npm_cache_matches_lock() (
  local authority="${1:?npm cache authority is required}"
  local cache_root="${2:?npm cache root is required}"
  local checkout="${3:?product checkout is required}"
  local ownership_mode="${4:-content}"
  local relative lockfile expected_lock scratch closure expected_closure
  local expected_count actual_count url integrity encoded hex content key
  local key_hash index row checksum record size
  relative="$(jq -er '.package_lock_path' "$authority")" || return 1
  lockfile="$checkout/$relative"
  expected_lock="$(jq -er '.package_lock_sha256' "$authority")" || return 1
  [[ -d "$checkout" && ! -L "$checkout" && -f "$lockfile" \
    && ! -L "$lockfile" && "$(stat -c '%h' -- "$lockfile")" == 1 \
    && "$(sha256sum -- "$lockfile" | cut -d' ' -f1)" == "$expected_lock" ]] \
    || return 1
  jain_validate_npm_cache "$authority" "$cache_root" "$ownership_mode" \
    || return 1
  scratch="$(mktemp -d /tmp/jain-npm-closure-validate.XXXXXX)" || return 1
  closure="$scratch/closure.tsv"
  trap 'rm -rf -- "$scratch"' EXIT
  jain_write_npm_closure "$lockfile" \
    "$(jq -er '.platform' "$authority")" \
    "$(jq -er '.arch' "$authority")" \
    "$(jq -er '.libc' "$authority")" "$closure" || return 1
  expected_closure="$(jq -er '.closure_sha256' "$authority")" || return 1
  expected_count="$(jq -er '.closure_count' "$authority")" || return 1
  actual_count="$(wc -l <"$closure")" || return 1
  [[ "$actual_count" == "$expected_count" \
    && "$(sha256sum -- "$closure" | cut -d' ' -f1)" == "$expected_closure" ]] \
    || return 1

  while IFS=$'\t' read -r package_path url integrity; do
    [[ "$package_path" == node_modules/* && "$package_path" != *..* \
      && "$url" == https://registry.npmjs.org/* \
      && "$integrity" =~ ^sha512-([A-Za-z0-9+/]+={0,2})$ ]] || return 1
    encoded="${BASH_REMATCH[1]}"
    hex="$(printf '%s' "$encoded" | base64 -d 2>/dev/null \
      | od -An -tx1 | tr -d ' \n')" || return 1
    [[ "$hex" =~ ^[0-9a-f]{128}$ ]] || return 1
    content="$cache_root/_cacache/content-v2/sha512/${hex:0:2}/${hex:2:2}/${hex:4}"
    [[ -f "$content" && ! -L "$content" \
      && "$(stat -c '%h' -- "$content")" == 1 \
      && "$(sha512sum -- "$content" | cut -d' ' -f1)" == "$hex" ]] \
      || return 1
    size="$(stat -c '%s' -- "$content")" || return 1
    key="make-fetch-happen:request-cache:$url"
    key_hash="$(printf '%s' "$key" | sha256sum | cut -d' ' -f1)" \
      || return 1
    index="$cache_root/_cacache/index-v5/${key_hash:0:2}/${key_hash:2:2}/${key_hash:4}"
    [[ -f "$index" && ! -L "$index" \
      && "$(stat -c '%h' -- "$index")" == 1 \
      && "$(wc -l <"$index")" == 1 ]] || return 1
    row="$(tail -n 1 "$index")" || return 1
    checksum="${row%%$'\t'*}"
    record="${row#*$'\t'}"
    [[ "$checksum" =~ ^[0-9a-f]{40}$ && "$record" != "$row" \
      && "$(printf '%s' "$record" | sha1sum | cut -d' ' -f1)" \
        == "$checksum" ]] || return 1
    jq -e --arg key "$key" --arg integrity "$integrity" \
      --arg url "$url" --argjson size "$size" '
        select((keys | sort) ==
          ["integrity", "key", "metadata", "size", "time"])
        | select(.key == $key and .integrity == $integrity)
        | select(.time == 0 and .size == $size)
        | select(.metadata == {
            time: 0,
            url: $url,
            reqHeaders: {},
            resHeaders: {},
            options: {compress: true}
          })
      ' <<<"$record" >/dev/null || return 1
  done <"$closure"
)

jain_validate_staged_npm_cache() (
  local authority="${1:?npm cache authority is required}"
  local source_root="${2:?sealed npm cache root is required}"
  local staged_root="${3:?staged npm cache root is required}"
  local source_ownership="${4:-root}"
  local scratch source_inventory checksum_file relative node_type mode links
  local source_count=0 staged_count=0
  [[ "$source_ownership" == root || "$source_ownership" == content ]] \
    || return 1
  jain_validate_npm_cache \
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
        [[ "$mode" == 600 && "$links" == 1 ]] || return 1
        staged_count=$((staged_count + 1))
        ;;
      *) return 1 ;;
    esac
  done < <(find "$staged_root" -xdev -mindepth 1 \
    -printf '%P\0%y\0%m\0%n\0')
  scratch="$(mktemp -d /tmp/jain-npm-stage-validate.XXXXXX)" || return 1
  trap 'rm -rf -- "$scratch"' EXIT
  source_inventory="$scratch/source.tsv"
  checksum_file="$scratch/checksums"
  jain_write_native_build_tools_inventory \
    "$source_root" "$source_inventory" "$source_ownership" || return 1
  source_count="$(wc -l <"$source_inventory")" || return 1
  awk -F '\t' '{ print $4 "  " $1 }' "$source_inventory" \
    >"$checksum_file" || return 1
  [[ "$source_count" == "$(jq -er '.file_count' "$authority")" \
    && "$staged_count" == "$source_count" ]] || return 1
  (cd "$staged_root" && sha256sum --check --strict --quiet "$checksum_file") \
    || return 1
  jain_validate_npm_cache \
    "$authority" "$source_root" "$source_ownership"
)

jain_stage_npm_cache() {
  local authority="${1:?npm cache authority is required}"
  local source_root="${2:?npm cache root is required}"
  local destination_parent="${3:?npm cache staging parent is required}"
  local source_ownership="${4:-root}"
  local inventory destination path
  inventory="$(jq -er '.inventory_sha256' "$authority")" || return 1
  destination="$destination_parent/$inventory"
  [[ "$destination_parent" == /* && -d "$destination_parent" \
    && ! -L "$destination_parent" && ! -e "$destination" ]] || return 1
  jain_validate_npm_cache \
    "$authority" "$source_root" "$source_ownership" || return 1
  /usr/bin/cp -a --reflink=never -- "$source_root" "$destination" || return 1
  jain_validate_npm_cache "$authority" "$destination" content || return 1
  while IFS= read -r -d '' path; do chmod 0700 "$path" || return 1; done \
    < <(find "$destination" -type d -print0)
  while IFS= read -r -d '' path; do chmod 0600 "$path" || return 1; done \
    < <(find "$destination" -type f -print0)
  jain_validate_staged_npm_cache \
    "$authority" "$source_root" "$destination" "$source_ownership" \
    || return 1
  printf '%s\n' "$destination"
}
