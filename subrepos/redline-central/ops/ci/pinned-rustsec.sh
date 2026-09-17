#!/usr/bin/env bash
# Resolve and validate the exact RustSec database selected by the control plane.

jain_resolve_governed_rustsec() {
  local database expected_commit authority resolved actual_commit actual_tree
  database="${JAIN_PINNED_ADVISORY_DB:-}"
  expected_commit="${JAIN_PINNED_ADVISORY_COMMIT:-}"

  if [[ -n "$database" || -n "$expected_commit" ]]; then
    [[ -n "$database" && -n "$expected_commit" ]] || {
      printf 'both JAIN_PINNED_ADVISORY_DB and JAIN_PINNED_ADVISORY_COMMIT are required\n' >&2
      return 1
    }
    authority=governed_host
  else
    [[ "${JAIN_RELEASE_CI:-0}" != 1 ]] || {
      printf 'release CI did not provide governed advisory database variables\n' >&2
      return 1
    }
    database="${JAIN_RUSTSEC_ADVISORY_SOURCE:-${HOME:?HOME is required}/.cargo/advisory-db}"
    authority=local_readiness
  fi

  for command in git realpath; do
    command -v "$command" >/dev/null 2>&1 || {
      printf 'missing RustSec authority command: %s\n' "$command" >&2
      return 1
    }
  done
  [[ "$database" == /* && -d "$database" && ! -L "$database" \
    && -d "$database/.git" && ! -L "$database/.git" ]] || {
    printf 'RustSec authority must be an absolute physical Git repository: %s\n' "$database" >&2
    return 1
  }
  resolved="$(realpath -e -- "$database")" || return 1
  [[ "$resolved" == "$database" ]] || {
    printf 'RustSec authority resolved outside its exact path: %s\n' "$resolved" >&2
    return 1
  }
  [[ -z "$(find "$database" -type l -print -quit)" ]] || {
    printf 'RustSec authority contains a symbolic link\n' >&2
    return 1
  }
  [[ -z "$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null -c diff.external= \
    -C "$database" status --porcelain --untracked-files=all)" ]] || {
    printf 'RustSec authority is dirty: %s\n' "$database" >&2
    return 1
  }

  actual_commit="$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c diff.external= -C "$database" rev-parse --verify 'HEAD^{commit}')" || return 1
  if [[ -z "$expected_commit" ]]; then
    expected_commit="$actual_commit"
  fi
  [[ "$expected_commit" =~ ^[0-9a-f]{40}$ && "$actual_commit" == "$expected_commit" ]] || {
    printf 'RustSec authority commit mismatch: expected %s, got %s\n' \
      "$expected_commit" "$actual_commit" >&2
    return 1
  }
  actual_tree="$(git -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c diff.external= -C "$database" rev-parse --verify "$actual_commit^{tree}")" || return 1
  [[ "$actual_tree" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'RustSec authority tree identity is invalid\n' >&2
    return 1
  }

  export JAIN_RESOLVED_ADVISORY_DB="$database"
  export JAIN_RESOLVED_ADVISORY_COMMIT="$actual_commit"
  export JAIN_RESOLVED_ADVISORY_TREE="$actual_tree"
  export JAIN_RESOLVED_ADVISORY_AUTHORITY="$authority"
}
