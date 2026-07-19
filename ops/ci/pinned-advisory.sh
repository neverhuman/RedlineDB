#!/usr/bin/env bash
# Hermetic RustSec advisory database materialization for detached release CI.

# 2026-07-12 RustSec main. Advancing this pin requires a reviewed control-plane
# change and a green security lane; host CI never fetches or resets a user DB.
JAIN_PINNED_RUSTSEC_COMMIT="6e3286f4efa8c142fb33e5ea4342c8db6693cf34"
JAIN_CARGO_DENY_RUSTSEC_DIR="advisory-db-3157b0e258782691"

# Inspect only the fetched object database before any checkout can create a
# filesystem node. Recursive ls-tree omits directory entries; the only admitted
# leaf modes are regular files, executables, and pinned submodule commits.
jain_git_object_tree_is_symlink_free() {
  local repository="${1:?Git repository is required}"
  local revision="${2:?Git revision is required}"
  local inventory
  inventory="$(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 \
    /usr/bin/git -c protocol.allow=never -c core.fsmonitor=false \
    -c core.hooksPath=/dev/null -c core.untrackedCache=false \
    -c core.alternateRefsCommand=false -c diff.external= \
    -C "$repository" ls-tree -r --full-tree --format='%(objectmode)' \
    "$revision")" || return 1
  awk '
        NF == 0 { next }
        $1 == "120000" { prohibited = 1; next }
        $1 !~ /^(100644|100755|160000)$/ { prohibited = 1 }
        END { exit prohibited }
      ' <<<"$inventory"
}

jain_materialize_pinned_advisory_db() {
  local source="${1:?advisory database source is required}"
  local destination="${2:?advisory database destination is required}"
  local expected="${3:?advisory database commit is required}"
  local source_uid="${4:-}" source_gid="${5:-}"
  local actual dirty pack
  local -a source_prefix=() source_git safe_git

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
  if [[ "$(id -u)" == 0 ]]; then
    [[ "$source_uid" =~ ^[0-9]+$ && "$source_uid" != 0 \
      && "$source_gid" =~ ^[0-9]+$ && "$source_gid" != 0 \
      && "$(stat -c '%u:%g' -- "$source")" == "$source_uid:$source_gid" \
      && "$(stat -c '%u:%g' -- "$source/.git")" == "$source_uid:$source_gid" ]] \
      || {
        printf 'root RustSec staging requires the exact non-root source owner\n' >&2
        return 1
      }
    source_prefix=(/usr/bin/setpriv --reuid="$source_uid" \
      --regid="$source_gid" --clear-groups --no-new-privs)
  else
    source_uid="$(id -u)"
    source_gid="$(id -g)"
  fi

  # Source Git config is caller-controlled. Every source-object operation runs
  # as a non-root identity, with global/system config removed, lazy promisor
  # fetches disabled, and every transport forbidden. Root consumes only the
  # resulting object pack, never the source repository's local config.
  source_git=("${source_prefix[@]}" /usr/bin/env -i \
    PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 \
    /usr/bin/git -c "safe.directory=$source" \
    -c protocol.allow=never -c core.fsmonitor=false \
    -c core.hooksPath=/dev/null -c core.untrackedCache=false \
    -c core.alternateRefsCommand=false -c diff.external=)
  safe_git=(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 \
    /usr/bin/git -c protocol.allow=never -c core.fsmonitor=false \
    -c core.hooksPath=/dev/null -c core.untrackedCache=false \
    -c core.alternateRefsCommand=false -c diff.external=)
  "${source_git[@]}" -C "$source" cat-file -e \
    "$expected^{commit}" 2>/dev/null || {
    printf 'pinned RustSec commit %s is unavailable in %s\n' "$expected" "$source" >&2
    return 1
  }
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    printf 'isolated RustSec destination already exists: %s\n' "$destination" >&2
    return 1
  }

  mkdir -p "$(dirname "$destination")"
  "${safe_git[@]}" init --quiet --template= "$destination" || return 1
  pack="$destination/.git/pinned.pack"
  printf '%s\n' "$expected" \
    | "${source_git[@]}" -C "$source" pack-objects --stdout --revs \
      >"$pack" || return 1
  "${safe_git[@]}" -C "$destination" index-pack --stdin \
    <"$pack" >/dev/null || return 1
  rm -f -- "$pack"
  jain_git_object_tree_is_symlink_free "$destination" "$expected" || {
    printf 'pinned RustSec object tree contains a prohibited mode\n' >&2
    return 1
  }
  "${safe_git[@]}" -C "$destination" checkout --quiet --detach \
    "$expected" || return 1
  actual="$("${safe_git[@]}" -C "$destination" rev-parse HEAD)" || return 1
  [[ "$actual" == "$expected" ]] || {
    printf 'isolated RustSec commit mismatch: expected %s, got %s\n' \
      "$expected" "$actual" >&2
    return 1
  }
  dirty="$("${safe_git[@]}" -C "$destination" \
    status --porcelain --untracked-files=all)" || return 1
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
  local deny_db="$deny_db_root/$JAIN_CARGO_DENY_RUSTSEC_DIR"
  local advisory_commit deny_commit
  local -a advisory_git deny_git

  case "$tool_dir:$advisory_db:$cargo_home:$script_root" in
    /*:/*:/*:/*) ;;
    *)
      printf 'pinned RustSec tool paths must be absolute\n' >&2
      return 1
      ;;
  esac
  [[ -d "$advisory_db/.git" && ! -L "$advisory_db" \
    && ! -L "$advisory_db/.git" ]] || {
    printf 'isolated RustSec database is unavailable: %s\n' "$advisory_db" >&2
    return 1
  }
  [[ -x "$script_root/ops/ci/pinned-cargo-audit.sh" ]] || return 1
  [[ -x "$script_root/ops/ci/pinned-cargo-deny.sh" ]] || return 1

  [[ -d "$deny_db/.git" && ! -L "$deny_db_root" && ! -L "$deny_db" \
    && ! -L "$deny_db/.git" ]] || {
    printf 'physical cargo-deny RustSec database is unavailable: %s\n' \
      "$deny_db" >&2
    return 1
  }
  advisory_git=(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 /usr/bin/git \
    -c "safe.directory=$advisory_db" -c protocol.allow=never \
    -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c core.untrackedCache=false -c core.alternateRefsCommand=false \
    -c diff.external=)
  deny_git=(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_NO_LAZY_FETCH=1 \
    GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 /usr/bin/git \
    -c "safe.directory=$deny_db" -c protocol.allow=never \
    -c core.fsmonitor=false -c core.hooksPath=/dev/null \
    -c core.untrackedCache=false -c core.alternateRefsCommand=false \
    -c diff.external=)
  advisory_commit="$("${advisory_git[@]}" -C "$advisory_db" \
    rev-parse 'HEAD^{commit}')" || return 1
  deny_commit="$("${deny_git[@]}" -C "$deny_db" \
    rev-parse 'HEAD^{commit}')" || return 1
  [[ "$advisory_commit" == "$deny_commit" ]] || {
    printf 'cargo-audit and cargo-deny RustSec snapshots differ\n' >&2
    return 1
  }

  mkdir -p "$tool_dir"
  [[ ! -e "$tool_dir/cargo-audit" && ! -L "$tool_dir/cargo-audit" \
    && ! -e "$tool_dir/cargo-deny" && ! -L "$tool_dir/cargo-deny" ]] || {
    printf 'pinned RustSec tool destination is not empty: %s\n' "$tool_dir" >&2
    return 1
  }
  install -m 0555 -- \
    "$script_root/ops/ci/pinned-cargo-audit.sh" "$tool_dir/cargo-audit"
  install -m 0555 -- \
    "$script_root/ops/ci/pinned-cargo-deny.sh" "$tool_dir/cargo-deny"
  [[ ! -L "$tool_dir/cargo-audit" && ! -L "$tool_dir/cargo-deny" \
    && "$(sha256sum -- "$tool_dir/cargo-audit" | cut -d' ' -f1)" \
      == "$(sha256sum -- "$script_root/ops/ci/pinned-cargo-audit.sh" | cut -d' ' -f1)" \
    && "$(sha256sum -- "$tool_dir/cargo-deny" | cut -d' ' -f1)" \
      == "$(sha256sum -- "$script_root/ops/ci/pinned-cargo-deny.sh" | cut -d' ' -f1)" ]] \
    || return 1
}
