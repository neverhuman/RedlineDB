#!/usr/bin/env bash

# Capture a command's NUL-delimited output in deterministic byte order. The
# final file appears only when every producer in the pipeline succeeds.
jain_capture_sorted_nul() {
  local destination="${1:-}" destination_parent parent_real basename staging rc
  shift || return 2
  [[ "$destination" = /* && "$#" -gt 0 \
    && "$destination" != *$'\n'* && "$destination" != *$'\r'* \
    && ! -e "$destination" && ! -L "$destination" ]] || return 2
  destination_parent="${destination%/*}"
  basename="${destination##*/}"
  [[ "$basename" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]] || return 2
  parent_real="$(realpath -e -- "$destination_parent")" || return 2
  [[ "$destination_parent" == "$parent_real" \
    && "$destination" == "$parent_real/$basename" \
    && -d "$parent_real" && ! -L "$parent_real" \
    && "$(stat -c '%u' -- "$parent_real")" == "$(id -u)" ]] || return 2
  staging="$(mktemp "$parent_real/.${basename}.XXXXXX")" || return 1
  chmod 0600 "$staging" || {
    rm -f -- "$staging"
    return 1
  }
  if (set -o pipefail; "$@" | LC_ALL=C /usr/bin/sort -z >"$staging"); then
    :
  else
    rc=$?
    rm -f -- "$staging"
    return "$rc"
  fi
  [[ -f "$staging" && ! -L "$staging" \
    && "$(stat -c '%u:%a:%h' -- "$staging")" == "$(id -u):600:1" ]] || {
    rm -f -- "$staging"
    return 1
  }
  mv -T -- "$staging" "$destination" || {
    rm -f -- "$staging"
    return 1
  }
  if [[ -f "$destination" && ! -L "$destination" \
    && "$(stat -c '%u:%a:%h' -- "$destination")" \
      == "$(id -u):600:1" ]]; then
    return 0
  fi
  rm -f -- "$destination"
  return 1
}

jain_cargo_lock_source_record() {
  local repository="${1:-}" commit="${2:-}" checkout="${3:-}" lock_list="${4:-}"
  local -a relative_locks=()
  local lock_path lock_sha256s
  [[ "$repository" =~ ^[a-z0-9][a-z0-9-]*$ \
    && "$commit" =~ ^[0-9a-f]{40}$ \
    && "$checkout" = /* && -d "$checkout" && ! -L "$checkout" \
    && "$lock_list" = /* && -f "$lock_list" && ! -L "$lock_list" \
    && "$(stat -c '%u:%a:%h' -- "$lock_list")" \
      == "$(id -u):600:1" ]] || return 2
  mapfile -d '' -t relative_locks <"$lock_list"
  lock_sha256s="$({
    for lock_path in "${relative_locks[@]}"; do
      [[ -n "$lock_path" && "$lock_path" != /* \
        && "$lock_path" != ../* && "$lock_path" != */../* \
        && -f "$checkout/$lock_path" && ! -L "$checkout/$lock_path" ]] \
        || return 1
      sha256sum -- "$checkout/$lock_path" | cut -d' ' -f1
    done
  } | jq -Rsc 'split("\n")[:-1] | sort')" || return 1
  jq -nc --arg repository "$repository" --arg commit "$commit" \
    --argjson lock_count "${#relative_locks[@]}" \
    --argjson lock_sha256s "$lock_sha256s" \
    '{repository:$repository,commit:$commit,lock_count:$lock_count,
      lock_sha256s:$lock_sha256s}'
}

jain_render_cargo_lock_source_closure() {
  local records="${1:-}" destination="${2:-}" rendered
  [[ "$records" = /* && -f "$records" && ! -L "$records" \
    && "$destination" = /* && ! -e "$destination" && ! -L "$destination" ]] \
    || return 2
  rendered="$(jq -s -c '
    select(length > 0)
    | select((map(.repository) | unique | length) == (. | length))
    | select(all(.[];
        (.repository | test("^[a-z0-9][a-z0-9-]*$"))
        and (.commit | test("^[0-9a-f]{40}$"))
        and (.lock_count | type) == "number" and .lock_count >= 0
        and (.lock_sha256s | type) == "array"
        and .lock_count == (.lock_sha256s | length)
        and all(.lock_sha256s[]; test("^[0-9a-f]{64}$"))))
    | {schema_version:"jain.cargo-lock-source-closure/v1",
       sources:sort_by(.repository)}
    | .source_count=(.sources | length)
    | .lock_count=([.sources[].lock_count] | add)
    | .lock_sha256s=([.sources[].lock_sha256s[]] | sort)
  ' "$records")" || return 1
  (umask 077; printf '%s\n' "$rendered" >"$destination") || return 1
  [[ -f "$destination" && ! -L "$destination" \
    && "$(stat -c '%u:%a:%h' -- "$destination")" \
      == "$(id -u):600:1" ]]
}
