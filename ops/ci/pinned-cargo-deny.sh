#!/usr/bin/env bash
set -euo pipefail

: "${JAIN_REAL_CARGO_DENY:?real cargo-deny path is required}"
: "${JAIN_PINNED_ADVISORY_DB:?isolated advisory database path is required}"
: "${JAIN_ADVISORY_DB:?canonical advisory database path is required}"
: "${JAIN_CARGO_DENY_ADVISORY_DB:?physical cargo-deny database path is required}"
: "${JAIN_PINNED_ADVISORY_COMMIT:?pinned advisory database commit is required}"

[[ "$JAIN_ADVISORY_DB" == "$JAIN_PINNED_ADVISORY_DB" \
  && "$JAIN_PINNED_ADVISORY_DB" = /* \
  && "$(realpath -e -- "$JAIN_PINNED_ADVISORY_DB")" \
    == "$JAIN_PINNED_ADVISORY_DB" \
  && "$(realpath -e -- "$JAIN_CARGO_DENY_ADVISORY_DB")" \
    == "$JAIN_CARGO_DENY_ADVISORY_DB" \
  && ! -L "$JAIN_PINNED_ADVISORY_DB" \
  && ! -L "$JAIN_CARGO_DENY_ADVISORY_DB" \
  && -d "$JAIN_PINNED_ADVISORY_DB/.git" \
  && -d "$JAIN_CARGO_DENY_ADVISORY_DB/.git" \
  && ! -L "$JAIN_PINNED_ADVISORY_DB/.git" \
  && ! -L "$JAIN_CARGO_DENY_ADVISORY_DB/.git" ]] || {
  printf 'cargo-deny RustSec inputs are not canonical physical snapshots\n' >&2
  exit 1
}

actual="$(git -C "$JAIN_PINNED_ADVISORY_DB" rev-parse HEAD)"
[[ "$actual" == "$JAIN_PINNED_ADVISORY_COMMIT" ]] || {
  printf 'cargo-deny RustSec commit mismatch: expected %s, got %s\n' \
    "$JAIN_PINNED_ADVISORY_COMMIT" "$actual" >&2
  exit 1
}
[[ -z "$(git -C "$JAIN_PINNED_ADVISORY_DB" status --porcelain --untracked-files=all)" ]] || {
  printf 'cargo-deny isolated RustSec database is dirty: %s\n' \
    "$JAIN_PINNED_ADVISORY_DB" >&2
  exit 1
}
deny_actual="$(git -C "$JAIN_CARGO_DENY_ADVISORY_DB" rev-parse HEAD)"
[[ "$deny_actual" == "$JAIN_PINNED_ADVISORY_COMMIT" ]] || {
  printf 'cargo-deny physical RustSec commit mismatch: expected %s, got %s\n' \
    "$JAIN_PINNED_ADVISORY_COMMIT" "$deny_actual" >&2
  exit 1
}
[[ -z "$(git -C "$JAIN_CARGO_DENY_ADVISORY_DB" \
  status --porcelain --untracked-files=all)" ]] || {
  printf 'cargo-deny physical RustSec database is dirty: %s\n' \
    "$JAIN_CARGO_DENY_ADVISORY_DB" >&2
  exit 1
}
if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
  [[ "$(stat -c '%u' -- "$JAIN_PINNED_ADVISORY_DB")" == 0 \
    && "$(stat -c '%u' -- "$JAIN_CARGO_DENY_ADVISORY_DB")" == 0 \
    && ! -w "$JAIN_PINNED_ADVISORY_DB" \
    && ! -w "$JAIN_CARGO_DENY_ADVISORY_DB" ]] || {
    printf 'cargo-deny RustSec databases are not immutable root authority\n' >&2
    exit 1
  }
fi

args=()
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --disable-fetch) shift ;;
    *) args+=("$1"); shift ;;
  esac
done
prefix=()
if [[ "${args[0]:-}" == deny ]]; then
  prefix=(deny)
  args=("${args[@]:1}")
fi
for ((index = 0; index < ${#args[@]}; index++)); do
  if [[ "${args[$index]}" == check ]]; then
    exec "$JAIN_REAL_CARGO_DENY" "${prefix[@]}" "${args[@]:0:$((index + 1))}" \
      --disable-fetch "${args[@]:$((index + 1))}"
  fi
done
exec "$JAIN_REAL_CARGO_DENY" "${prefix[@]}" "${args[@]}"
