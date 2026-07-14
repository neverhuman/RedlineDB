#!/usr/bin/env bash
# Hermetic RustSec advisory database materialization for detached release CI.

# 2026-07-12 RustSec main. Advancing this pin requires a reviewed control-plane
# change and a green security lane; host CI never fetches or resets a user DB.
JAIN_PINNED_RUSTSEC_COMMIT="6e3286f4efa8c142fb33e5ea4342c8db6693cf34"
JAIN_CARGO_DENY_RUSTSEC_DIR="advisory-db-3157b0e258782691"

jain_materialize_pinned_advisory_db() {
  local source="${1:?advisory database source is required}"
  local destination="${2:?advisory database destination is required}"
  local expected="${3:?advisory database commit is required}"
  local actual dirty

  case "$source:$destination" in
    /*:/*) ;;
    *)
      printf 'advisory database paths must be absolute: source=%s destination=%s\n' \
        "$source" "$destination" >&2
      return 1
      ;;
  esac
  [[ "$expected" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'invalid pinned RustSec commit: %s\n' "$expected" >&2
    return 1
  }
  [[ -d "$source/.git" || -f "$source/.git" ]] || {
    printf 'local RustSec source is not a Git worktree: %s\n' "$source" >&2
    return 1
  }
  git -C "$source" cat-file -e "$expected^{commit}" 2>/dev/null || {
    printf 'pinned RustSec commit %s is unavailable in %s\n' "$expected" "$source" >&2
    return 1
  }
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    printf 'isolated RustSec destination already exists: %s\n' "$destination" >&2
    return 1
  }

  mkdir -p "$(dirname "$destination")"
  git clone --quiet --no-checkout --local -- "$source" "$destination" || return 1
  git -C "$destination" checkout --quiet --detach "$expected" || return 1
  actual="$(git -C "$destination" rev-parse HEAD)" || return 1
  [[ "$actual" == "$expected" ]] || {
    printf 'isolated RustSec commit mismatch: expected %s, got %s\n' \
      "$expected" "$actual" >&2
    return 1
  }
  dirty="$(git -C "$destination" status --porcelain --untracked-files=all)" || return 1
  [[ -z "$dirty" ]] || {
    printf 'isolated RustSec database is dirty: %s\n' "$destination" >&2
    return 1
  }
}

jain_install_pinned_rustsec_tools() {
  local tool_dir="${1:?tool override directory is required}"
  local advisory_db="${2:?isolated advisory database is required}"
  local cargo_home="${3:?Cargo home is required}"
  local script_root="${4:?control-plane root is required}"
  local deny_db_root="$cargo_home/advisory-dbs"

  case "$tool_dir:$advisory_db:$cargo_home:$script_root" in
    /*:/*:/*:/*) ;;
    *)
      printf 'pinned RustSec tool paths must be absolute\n' >&2
      return 1
      ;;
  esac
  [[ -d "$advisory_db/.git" || -f "$advisory_db/.git" ]] || {
    printf 'isolated RustSec database is unavailable: %s\n' "$advisory_db" >&2
    return 1
  }
  [[ -x "$script_root/ops/ci/pinned-cargo-audit.sh" ]] || return 1
  [[ -x "$script_root/ops/ci/pinned-cargo-deny.sh" ]] || return 1

  mkdir -p "$tool_dir" "$deny_db_root"
  ln -s "$script_root/ops/ci/pinned-cargo-audit.sh" "$tool_dir/cargo-audit"
  ln -s "$script_root/ops/ci/pinned-cargo-deny.sh" "$tool_dir/cargo-deny"
  ln -s "$advisory_db" "$deny_db_root/$JAIN_CARGO_DENY_RUSTSEC_DIR"
}
