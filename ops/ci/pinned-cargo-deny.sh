#!/usr/bin/env bash
set -euo pipefail

: "${JAIN_REAL_CARGO_DENY:?real cargo-deny path is required}"
: "${JAIN_PINNED_ADVISORY_DB:?isolated advisory database path is required}"
: "${JAIN_PINNED_ADVISORY_COMMIT:?pinned advisory database commit is required}"

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
