#!/usr/bin/env bash
# Shared validators for immutable Python wheelhouse authority and the bounded
# worker-to-auditor evidence handoff. This file is sourced only from a
# digest-bound installed broker or the exact reviewed control-plane checkout.

JAIN_HOST_CI_WHEELHOUSE_MAX_FILES=1024
JAIN_HOST_CI_WHEELHOUSE_MAX_FILE_BYTES=536870912
JAIN_HOST_CI_WHEELHOUSE_MAX_TOTAL_BYTES=8589934592
JAIN_HOST_CI_AUDIT_INPUT_MAX_FILE_BYTES=33554432
JAIN_HOST_CI_AUDIT_INPUT_MAX_TOTAL_BYTES=50331648
JAIN_HOST_CI_AUDIT_INPUT_PATHS=(
  target/jankurai/coverage/coverage-audit.json
  target/jankurai/coverage/rust-lcov.info
  target/jankurai/security/evidence.json
  target/llvm-cov/lcov.info
  target/security/evidence.json
)

jain_host_ci_audit_input_path_allowed() {
  local candidate="${1:?audit input path is required}" allowed
  for allowed in "${JAIN_HOST_CI_AUDIT_INPUT_PATHS[@]}"; do
    [[ "$candidate" == "$allowed" ]] && return 0
  done
  return 1
}

jain_host_ci_python_wheelhouse_inventory() {
  local root="${1:?wheelhouse root is required}"
  local expected_uid="${2:-0}" expected_gid="${3:-0}"
  local canonical path name before after digest size total=0
  local -a paths=() lines=()
  [[ "$root" == /* && -d "$root" && ! -L "$root" ]] || return 1
  canonical="$(realpath -e -- "$root")" || return 1
  [[ "$canonical" == "$root" \
    && "$(stat -c '%u:%g:%a' -- "$root")" \
      == "$expected_uid:$expected_gid:555" ]] || return 1
  mapfile -d '' -t paths < <(
    find "$root" -mindepth 1 -maxdepth 1 -print0 | LC_ALL=C sort -z
  )
  (( ${#paths[@]} > 0 \
    && ${#paths[@]} <= JAIN_HOST_CI_WHEELHOUSE_MAX_FILES )) || return 1
  for path in "${paths[@]}"; do
    name="${path##*/}"
    [[ "$name" =~ ^[A-Za-z0-9][A-Za-z0-9_.+-]*\.whl$ \
      && -f "$path" && ! -L "$path" \
      && "$(stat -c '%u:%g:%a:%h' -- "$path")" \
        == "$expected_uid:$expected_gid:444:1" ]] || return 1
    before="$(stat -c '%d:%i:%u:%g:%a:%h:%s:%Y:%Z' -- "$path")" \
      || return 1
    size="$(stat -c '%s' -- "$path")" || return 1
    [[ "$size" =~ ^[0-9]+$ \
      && "$size" -le "$JAIN_HOST_CI_WHEELHOUSE_MAX_FILE_BYTES" ]] || return 1
    digest="$(sha256sum -- "$path" | cut -d' ' -f1)" || return 1
    after="$(stat -c '%d:%i:%u:%g:%a:%h:%s:%Y:%Z' -- "$path")" \
      || return 1
    [[ "$before" == "$after" && "$digest" =~ ^[0-9a-f]{64}$ ]] || return 1
    total=$((total + size))
    (( total <= JAIN_HOST_CI_WHEELHOUSE_MAX_TOTAL_BYTES )) || return 1
    lines+=("$name"$'\t'"$size"$'\t'"$digest"$'\n')
  done
  digest="$(
    printf '%s' "${lines[@]}" | sha256sum | cut -d' ' -f1
  )" || return 1
  [[ "$digest" =~ ^[0-9a-f]{64}$ ]] || return 1
  printf '%s\t%s\t%s\n' "$digest" "${#paths[@]}" "$total"
}

jain_host_ci_verify_python_wheelhouse() {
  local root="${1:?wheelhouse root is required}"
  local expected_digest="${2:?wheelhouse digest is required}"
  local expected_uid="${3:-0}" expected_gid="${4:-0}" inventory
  inventory="$(
    jain_host_ci_python_wheelhouse_inventory \
      "$root" "$expected_uid" "$expected_gid"
  )" || return 1
  [[ "${inventory%%$'\t'*}" == "$expected_digest" ]]
}

jain_host_ci_stage_audit_inputs() {
  local source_root="${1:?source root is required}"
  local staging_root="${2:?staging root is required}"
  local request_id="${3:?request id is required}"
  local control_commit="${4:?control commit is required}"
  local owner="${5:?owner is required}" repository="${6:?repository is required}"
  local head_sha="${7:?head SHA is required}"
  local required_check="${8:?required check is required}"
  local relative source destination before after digest size source_meta total=0
  local entries="$staging_root/.entries.jsonl"
  [[ -d "$source_root" && ! -L "$source_root" \
    && -d "$staging_root" && ! -L "$staging_root" \
    && -z "$(find "$staging_root" -mindepth 1 -print -quit)" \
    && "$request_id" =~ ^[0-9a-f]{64}$ \
    && "$control_commit" =~ ^[0-9a-f]{40}$ \
    && "$head_sha" =~ ^[0-9a-f]{40}$ ]] || return 1
  mkdir -m 0700 "$staging_root/files" || return 1
  : >"$entries" || return 1
  chmod 0600 "$entries" || return 1
  for relative in "${JAIN_HOST_CI_AUDIT_INPUT_PATHS[@]}"; do
    source="$source_root/$relative"
    if [[ ! -e "$source" && ! -L "$source" ]]; then
      continue
    fi
    source_meta="$(stat -c '%u:%g:%a:%h' -- "$source")" || return 1
    [[ -f "$source" && ! -L "$source" \
      && "$source_meta" =~ ^$(id -u):$(id -g):(600|644):1$ ]] || return 1
    before="$(stat -c '%d:%i:%u:%g:%a:%h:%s:%Y:%Z' -- "$source")" \
      || return 1
    size="$(stat -c '%s' -- "$source")" || return 1
    [[ "$size" =~ ^[0-9]+$ \
      && "$size" -le "$JAIN_HOST_CI_AUDIT_INPUT_MAX_FILE_BYTES" ]] || return 1
    digest="$(sha256sum -- "$source" | cut -d' ' -f1)" || return 1
    after="$(stat -c '%d:%i:%u:%g:%a:%h:%s:%Y:%Z' -- "$source")" \
      || return 1
    [[ "$before" == "$after" && "$digest" =~ ^[0-9a-f]{64}$ ]] || return 1
    total=$((total + size))
    (( total <= JAIN_HOST_CI_AUDIT_INPUT_MAX_TOTAL_BYTES )) || return 1
    destination="$staging_root/files/$relative"
    mkdir -p -m 0700 "$(dirname "$destination")" || return 1
    install -m 0600 -- "$source" "$destination" || return 1
    [[ "$(stat -c '%u:%g:%a:%h:%s' -- "$destination")" \
        == "$(id -u):$(id -g):600:1:$size" \
      && "$(sha256sum -- "$destination" | cut -d' ' -f1)" == "$digest" ]] \
      || return 1
    jq -nc --arg path "$relative" --arg sha "$digest" \
      --argjson size "$size" \
      '{path:$path,size:$size,sha256:$sha}' >>"$entries" || return 1
  done
  find "$staging_root" -xdev -type d -exec chmod 0700 {} + || return 1
  jq -s --arg request_id "$request_id" --arg control "$control_commit" \
    --arg owner "$owner" --arg repository "$repository" \
    --arg head "$head_sha" --arg check "$required_check" \
    '{schema_version:"jain.host-ci-audit-input/v1",
      request_id:$request_id,control_plane_commit:$control,
      owner:$owner,repository:$repository,head_sha:$head,
      required_check:$check,files:.}' \
    "$entries" >"$staging_root/receipt.tmp" || return 1
  chmod 0600 "$staging_root/receipt.tmp" || return 1
  mv -- "$staging_root/receipt.tmp" "$staging_root/receipt.json" || return 1
  rm -- "$entries" || return 1
}

jain_host_ci_validate_audit_inputs() {
  local staging_root="${1:?staging root is required}"
  local request_id="${2:?request id is required}"
  local control_commit="${3:?control commit is required}"
  local owner="${4:?owner is required}" repository="${5:?repository is required}"
  local head_sha="${6:?head SHA is required}"
  local required_check="${7:?required check is required}"
  local expected_uid="${8:?expected uid is required}"
  local expected_gid="${9:?expected gid is required}"
  local receipt="$staging_root/receipt.json"
  local relative path parent before after digest size total=0 receipt_sha
  local -a declared=() actual=() declared_dirs=() actual_dirs=()
  [[ -d "$staging_root" && ! -L "$staging_root" \
    && -f "$receipt" && ! -L "$receipt" \
    && "$(stat -c '%u:%g:%a:%h' -- "$receipt")" \
      == "$expected_uid:$expected_gid:600:1" \
    && -z "$(find "$staging_root" -xdev ! -type d ! -type f -print -quit)" ]] \
    || return 1
  while IFS= read -r -d '' path; do
    [[ "$(stat -c '%u:%g:%a' -- "$path")" \
      == "$expected_uid:$expected_gid:700" ]] || return 1
  done < <(find "$staging_root" -xdev -type d -print0)
  jq -e --arg request_id "$request_id" --arg control "$control_commit" \
    --arg owner "$owner" --arg repository "$repository" \
    --arg head "$head_sha" --arg check "$required_check" '
    select((keys | sort) == ([
      "control_plane_commit","files","head_sha","owner","repository",
      "request_id","required_check","schema_version"
    ] | sort))
    | select(.schema_version == "jain.host-ci-audit-input/v1")
    | select(.request_id == $request_id and .control_plane_commit == $control)
    | select(.owner == $owner and .repository == $repository)
    | select(.head_sha == $head and .required_check == $check)
    | select((.files | type) == "array" and (.files | length) <= 5)
    | select(.files == (.files | sort_by(.path)))
    | select((.files | map(.path) | unique | length) == (.files | length))
    | select(all(.files[];
        (keys | sort) == (["path","sha256","size"] | sort)
        and (.path | type) == "string"
        and (.sha256 | test("^[0-9a-f]{64}$"))
        and (.size | type) == "number" and .size >= 0
        and .size <= 33554432))
  ' "$receipt" >/dev/null || return 1
  mapfile -t declared < <(jq -er '.files[].path' "$receipt")
  mapfile -t actual < <(
    find "$staging_root/files" -xdev -type f -printf '%P\n' | LC_ALL=C sort
  )
  [[ "${declared[*]}" == "${actual[*]}" ]] || return 1
  mapfile -t declared_dirs < <(
    {
      printf '.\nfiles\n'
      for relative in "${declared[@]}"; do
        parent="files/${relative%/*}"
        while [[ "$parent" != "files" ]]; do
          printf '%s\n' "$parent"
          parent="${parent%/*}"
        done
      done
    } | LC_ALL=C sort -u
  )
  mapfile -t actual_dirs < <(
    {
      printf '.\n'
      find "$staging_root" -xdev -mindepth 1 -type d -printf '%P\n'
    } | LC_ALL=C sort
  )
  [[ "${declared_dirs[*]}" == "${actual_dirs[*]}" ]] || return 1
  for relative in "${declared[@]}"; do
    jain_host_ci_audit_input_path_allowed "$relative" || return 1
    path="$staging_root/files/$relative"
    [[ -f "$path" && ! -L "$path" \
      && "$(stat -c '%u:%g:%a:%h' -- "$path")" \
        == "$expected_uid:$expected_gid:600:1" ]] || return 1
    before="$(stat -c '%d:%i:%u:%g:%a:%h:%s:%Y:%Z' -- "$path")" \
      || return 1
    size="$(stat -c '%s' -- "$path")" || return 1
    digest="$(sha256sum -- "$path" | cut -d' ' -f1)" || return 1
    after="$(stat -c '%d:%i:%u:%g:%a:%h:%s:%Y:%Z' -- "$path")" \
      || return 1
    [[ "$before" == "$after" \
      && "$(jq -er --arg path "$relative" \
        '.files[] | select(.path == $path) | .size' "$receipt")" == "$size" \
      && "$(jq -er --arg path "$relative" \
        '.files[] | select(.path == $path) | .sha256' "$receipt")" \
        == "$digest" ]] || return 1
    total=$((total + size))
    (( total <= JAIN_HOST_CI_AUDIT_INPUT_MAX_TOTAL_BYTES )) || return 1
  done
  receipt_sha="$(sha256sum -- "$receipt" | cut -d' ' -f1)" || return 1
  [[ "$receipt_sha" =~ ^[0-9a-f]{64}$ ]] || return 1
  printf '%s\t%s\n' "$receipt_sha" "${#declared[@]}"
}

jain_host_ci_install_audit_inputs() {
  local staging_root="${1:?staging root is required}"
  local audit_root="${2:?audit root is required}"
  local canonical relative source destination parent
  [[ -d "$audit_root" && ! -L "$audit_root" ]] || return 1
  canonical="$(realpath -e -- "$audit_root")" || return 1
  [[ "$canonical" == "$audit_root" ]] || return 1
  while IFS= read -r relative; do
    jain_host_ci_audit_input_path_allowed "$relative" || return 1
    source="$staging_root/files/$relative"
    destination="$audit_root/$relative"
    parent="$(dirname "$destination")"
    mkdir -p -m 0755 "$parent" || return 1
    parent="$(realpath -e -- "$parent")" || return 1
    case "$parent/" in
      "$audit_root/"*) ;;
      *) return 1 ;;
    esac
    install -o root -g root -m 0444 -- "$source" "$destination" || return 1
  done < <(jq -er '.files[].path' "$staging_root/receipt.json")
  [[ -z "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c diff.external= -C "$audit_root" \
    status --porcelain=v1 --untracked-files=all)" ]]
}
