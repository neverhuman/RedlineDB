#!/usr/bin/env bash
# Security-sensitive helpers sourced by split-host-ci.sh.

jeryu_token() {
  if [ -n "${STATUS_TOKEN:-}" ]; then
    printf '%s' "$STATUS_TOKEN"
    return
  fi
  if [ -n "${JERYU_MERGE_TOKEN:-}" ]; then
    printf '%s' "$JERYU_MERGE_TOKEN"
    return
  fi
  local token_file="${JERYU_MERGE_TOKEN_FILE:-$HOME/.jeryu/secrets/merge-token}"
  [ -r "$token_file" ] && tr -d '\n' < "$token_file"
}

jeryu_curl() {
  local token rc
  token="$(jeryu_token)"
  [ -n "$token" ] || return 1
  printf 'Authorization: Bearer %s\n' "$token" |
    env -u JERYU_MERGE_TOKEN curl --header @- "$@"
  rc=$?
  token=""
  return "$rc"
}

seed_pinned_advisory_db() {
  local source="$1" destination="$2" expected="$3"
  local source_head source_tree

  [[ "$expected" =~ ^[0-9a-f]{40}$ ]] || {
    printf '[split-host-ci] invalid RustSec commit pin\n' >&2
    return 1
  }
  [ -d "$source/.git" ] && [ ! -L "$source" ] && [ ! -L "$source/.git" ] || {
    printf '[split-host-ci] RustSec source is not a standalone Git checkout: %s\n' "$source" >&2
    return 1
  }
  [ -z "$(find "$source" -type l -print -quit 2>/dev/null)" ] || {
    printf '[split-host-ci] RustSec source contains a symlink: %s\n' "$source" >&2
    return 1
  }
  source_head="$(git -C "$source" rev-parse --verify 'HEAD^{commit}')" || return 1
  [ "$source_head" = "$expected" ] || {
    printf '[split-host-ci] RustSec source is not at the reviewed pin\n' >&2
    return 1
  }
  [ -z "$(git -C "$source" status --porcelain=v1 --untracked-files=all)" ] || {
    printf '[split-host-ci] RustSec source is dirty\n' >&2
    return 1
  }
  source_tree="$(git -C "$source" rev-parse --verify 'HEAD^{tree}')" || return 1
  [ ! -e "$destination" ] || return 1

  git clone --quiet --no-local --no-checkout "$source" "$destination" || return 1
  git -C "$destination" checkout --quiet --detach --force "$expected" || return 1
  git -C "$destination" remote remove origin || return 1
  [ "$(git -C "$destination" rev-parse --verify 'HEAD^{commit}')" = "$expected" ] || return 1
  [ "$(git -C "$destination" rev-parse --verify 'HEAD^{tree}')" = "$source_tree" ] || return 1
  [ -z "$(git -C "$destination" status --porcelain=v1 --untracked-files=all)" ] || return 1
  [ -z "$(find "$destination" -type l -print -quit 2>/dev/null)" ] || return 1

  # Close the source-read race: it must still be exact and clean after clone.
  [ "$(git -C "$source" rev-parse --verify 'HEAD^{commit}')" = "$expected" ] || return 1
  [ -z "$(git -C "$source" status --porcelain=v1 --untracked-files=all)" ] || return 1
}

