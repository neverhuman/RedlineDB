#!/usr/bin/env bash
# Root-only promotion policy for bounded host-CI native evidence.

jain_host_ci_verify_native_store() {
  local store="${1:?native evidence store is required}" resolved
  resolved="$(realpath -e -- "$store")" || return 1
  case "$resolved" in
    /tmp | /tmp/*) return 1 ;;
  esac
  [[ "$resolved" == "$store" && -d "$resolved" && ! -L "$resolved" \
    && "$(stat -c '%u:%g:%a' -- "$resolved")" == '0:0:700' ]] || return 1
  printf '%s\n' "$resolved"
}

jain_host_ci_verify_worker_staging() {
  local staging="${1:?worker evidence staging is required}"
  local evidence_dir="${2:?worker evidence directory is required}"
  local owner="${3:?owner is required}" repo="${4:?repository is required}"
  local head="${5:?head SHA is required}" check="${6:?check is required}"
  local worker_uid="${7:?worker UID is required}" worker_gid="${8:?worker GID is required}"
  local resolved evidence_resolved check_slug expected_parent entries file mode
  [[ "$owner" =~ ^[a-z0-9][a-z0-9-]*$ \
    && "$repo" =~ ^[a-z0-9][a-z0-9-]*$ \
    && "$head" =~ ^[0-9a-f]{40}$ \
    && "$check" =~ ^[a-z0-9][a-z0-9-]*/required$ ]] || return 1
  resolved="$(realpath -e -- "$staging")" || return 1
  evidence_resolved="$(realpath -e -- "$evidence_dir")" || return 1
  check_slug="${check//[^A-Za-z0-9_.-]/_}"
  expected_parent="$resolved/$owner/$repo/$head/$check_slug"
  [[ "$evidence_resolved" == "$expected_parent"/* \
    && "$(dirname "$evidence_resolved")" == "$expected_parent" \
    && "${evidence_resolved##*/}" =~ ^[A-Za-z0-9_.-]+$ \
    && "${evidence_resolved##*/}" != . \
    && "${evidence_resolved##*/}" != .. \
    && "$(stat -f -c '%T' -- "$resolved")" == tmpfs \
    && "$(stat -c '%u:%g:%a' -- "$resolved")" \
      == "$worker_uid:$worker_gid:700" ]] || return 1
  if find "$resolved" -xdev -mindepth 1 \
    \( -type l -o \( ! -type d ! -type f \) \) -print -quit | grep -q .; then
    return 1
  fi
  entries="$(find "$resolved" -xdev -mindepth 1 -printf . | wc -c)" || return 1
  [[ "$entries" == 21 ]] || return 1
  while IFS= read -r -d '' file; do
    [[ "$(stat -c '%u:%g:%h' -- "$file")" == "$worker_uid:$worker_gid:1" ]] \
      || return 1
    mode="$(stat -c '%a' -- "$file")" || return 1
    (( (8#$mode & 8#022) == 0 )) || return 1
  done < <(find "$resolved" -xdev -type f -print0)
  jain_verify_native_evidence_layout "$evidence_resolved" >/dev/null || return 1
  printf '%s\n' "$evidence_resolved"
}

jain_host_ci_verify_promoted_evidence() {
  local evidence_dir="${1:?promoted evidence directory is required}" file
  [[ -d "$evidence_dir" && ! -L "$evidence_dir" \
    && "$(stat -c '%u:%g:%a' -- "$evidence_dir")" == '0:0:500' ]] \
    || return 1
  while IFS= read -r file; do
    [[ "$(stat -c '%u:%g:%a:%h' -- "$evidence_dir/$file")" \
      == '0:0:400:1' ]] || return 1
  done < <(jain_native_evidence_files)
}

jain_host_ci_ensure_store_dir() {
  local parent="${1:?parent directory is required}"
  local component="${2:?directory component is required}" next
  [[ "$component" =~ ^[A-Za-z0-9_.-]+$ \
    && "$component" != . && "$component" != .. ]] || return 1
  next="$parent/$component"
  if [[ -e "$next" || -L "$next" ]]; then
    [[ -d "$next" && ! -L "$next" \
      && "$(stat -c '%u:%g:%a' -- "$next")" == '0:0:700' ]] || return 1
  else
    install -d -o root -g root -m 0700 -- "$next" || return 1
  fi
  printf '%s\n' "$next"
}

jain_host_ci_promote_native_evidence() (
  local staging="${1:?worker evidence staging is required}"
  local evidence_dir="${2:?worker evidence directory is required}"
  local receipt_sha="${3:?native receipt digest is required}"
  local store="${4:?native evidence store is required}"
  local owner="${5:?owner is required}" repo="${6:?repository is required}"
  local head="${7:?head SHA is required}" check="${8:?check is required}"
  local request_id="${9:?request ID is required}" worker_uid="${10:?worker UID is required}"
  local worker_gid="${11:?worker GID is required}" control_root="${12:?control root is required}"
  local control_commit="${13:?control commit is required}"
  local resolved_store resolved_evidence check_slug parent destination promote_tmp
  local total available required file lock lock_fd row name index
  local -a retained=()
  [[ "$(id -u)" == 0 && "$request_id" =~ ^[0-9a-f]{64}$ \
    && "$receipt_sha" =~ ^[0-9a-f]{64}$ ]] || return 1
  resolved_store="$(jain_host_ci_verify_native_store "$store")" || return 1
  resolved_evidence="$(jain_host_ci_verify_worker_staging \
    "$staging" "$evidence_dir" "$owner" "$repo" "$head" "$check" \
    "$worker_uid" "$worker_gid")" || return 1
  jain_verify_native_evidence_binding "$resolved_evidence" "$receipt_sha" \
    "$head" "$check" "$control_root" "$control_commit" || return 1
  total="$(jain_verify_native_evidence_layout "$resolved_evidence")" || return 1

  lock="$resolved_store/.promotion.lock"
  exec {lock_fd}>"$lock" || return 1
  chmod 0600 "$lock" && chown root:root "$lock" || {
    exec {lock_fd}>&-
    return 1
  }
  /usr/bin/flock -x "$lock_fd" || {
    exec {lock_fd}>&-
    return 1
  }
  available="$(df -B1 --output=avail "$resolved_store" | tail -n 1 | tr -d ' ')" \
    || return 1
  [[ "$available" =~ ^[0-9]+$ ]] || return 1
  required=$((total + JAIN_NATIVE_EVIDENCE_MIN_FREE_BYTES))
  (( available >= required )) || {
    printf 'native evidence store free-space guard failed: available=%s required=%s\n' \
      "$available" "$required" >&2
    exec {lock_fd}>&-
    return 1
  }

  check_slug="${check//[^A-Za-z0-9_.-]/_}"
  parent="$resolved_store"
  for name in "$owner" "$repo" "$check_slug"; do
    parent="$(jain_host_ci_ensure_store_dir "$parent" "$name")" || {
      exec {lock_fd}>&-
      return 1
    }
  done
  destination="$parent/$request_id"
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    exec {lock_fd}>&-
    return 1
  }
  while IFS= read -r -d '' row; do
    [[ "${row##*/}" == .promote.* && -d "$row" && ! -L "$row" \
      && "$(stat -c '%u:%g' -- "$row")" == '0:0' ]] || {
      exec {lock_fd}>&-
      return 1
    }
    rm -rf -- "$row"
  done < <(find "$parent" -mindepth 1 -maxdepth 1 -type d \
    -name '.promote.*' -print0)
  promote_tmp="$(mktemp -d "$parent/.promote.XXXXXX")" || {
    exec {lock_fd}>&-
    return 1
  }
  chmod 0700 "$promote_tmp" && chown root:root "$promote_tmp"
  while IFS= read -r file; do
    if ! install -o root -g root -m 0400 -- \
      "$resolved_evidence/$file" "$promote_tmp/$file"; then
      rm -rf -- "$promote_tmp"
      exec {lock_fd}>&-
      return 1
    fi
  done < <(jain_native_evidence_files)
  chmod 0500 "$promote_tmp"
  if ! jain_verify_native_evidence_binding "$promote_tmp" "$receipt_sha" \
      "$head" "$check" "$control_root" "$control_commit" \
    || ! jain_host_ci_verify_promoted_evidence "$promote_tmp"; then
    chmod 0700 "$promote_tmp" || true
    rm -rf -- "$promote_tmp"
    exec {lock_fd}>&-
    return 1
  fi
  mv -- "$promote_tmp" "$destination" || {
    chmod 0700 "$promote_tmp" || true
    rm -rf -- "$promote_tmp"
    exec {lock_fd}>&-
    return 1
  }

  mapfile -t retained < <(
    find "$parent" -mindepth 1 -maxdepth 1 -type d \
      -regextype posix-extended -regex ".*/[0-9a-f]{64}" \
      -printf '%T@ %f\n' | LC_ALL=C sort -nr
  )
  for ((index = JAIN_NATIVE_EVIDENCE_RETAIN_PER_CHECK; \
      index < ${#retained[@]}; index++)); do
    name="${retained[$index]#* }"
    [[ "$name" =~ ^[0-9a-f]{64}$ \
      && -d "$parent/$name" && ! -L "$parent/$name" \
      && "$(stat -c '%u:%g:%a' -- "$parent/$name")" == '0:0:500' ]] \
      || {
        exec {lock_fd}>&-
        return 1
      }
    chmod 0700 "$parent/$name"
    rm -rf -- "$parent/$name"
  done
  exec {lock_fd}>&-
  printf '%s\n' "$destination"
)

jain_host_ci_staging_is_empty() {
  local staging="${1:?worker evidence staging is required}"
  [[ -d "$staging" && ! -L "$staging" \
    && -z "$(find "$staging" -xdev -mindepth 1 -print -quit)" ]]
}
