#!/usr/bin/env bash
set -euo pipefail

: "${JAIN_REAL_CARGO_AUDIT:?real cargo-audit path is required}"
: "${JAIN_PINNED_ADVISORY_DB:?isolated advisory database path is required}"
: "${JAIN_ADVISORY_DB:?canonical advisory database path is required}"
: "${JAIN_PINNED_ADVISORY_COMMIT:?pinned advisory database commit is required}"

advisory_git=(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_NO_LAZY_FETCH=1 \
  GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 /usr/bin/git \
  -c "safe.directory=$JAIN_PINNED_ADVISORY_DB" -c protocol.allow=never \
  -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c core.untrackedCache=false -c core.alternateRefsCommand=false \
  -c diff.external=)

[[ "$JAIN_ADVISORY_DB" == "$JAIN_PINNED_ADVISORY_DB" \
  && "$JAIN_PINNED_ADVISORY_DB" = /* \
  && "$(realpath -e -- "$JAIN_PINNED_ADVISORY_DB")" \
    == "$JAIN_PINNED_ADVISORY_DB" \
  && ! -L "$JAIN_PINNED_ADVISORY_DB" \
  && -d "$JAIN_PINNED_ADVISORY_DB/.git" \
  && ! -L "$JAIN_PINNED_ADVISORY_DB/.git" ]] || {
  printf 'cargo-audit RustSec authority is not one canonical physical snapshot\n' >&2
  exit 1
}

actual="$("${advisory_git[@]}" -C "$JAIN_PINNED_ADVISORY_DB" rev-parse HEAD)"
[[ "$actual" == "$JAIN_PINNED_ADVISORY_COMMIT" ]] || {
  printf 'cargo-audit RustSec commit mismatch: expected %s, got %s\n' \
    "$JAIN_PINNED_ADVISORY_COMMIT" "$actual" >&2
  exit 1
}
[[ -z "$("${advisory_git[@]}" -C "$JAIN_PINNED_ADVISORY_DB" \
  status --porcelain --untracked-files=all)" ]] || {
  printf 'cargo-audit isolated RustSec database is dirty: %s\n' \
    "$JAIN_PINNED_ADVISORY_DB" >&2
  exit 1
}

args=()
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    -d | --db)
      [[ "$#" -ge 2 ]] || {
        printf 'cargo-audit %s requires a path\n' "$1" >&2
        exit 2
      }
      shift 2
      ;;
    --db=*) shift ;;
    -n | --no-fetch) shift ;;
    *) args+=("$1"); shift ;;
  esac
done

exec "$JAIN_REAL_CARGO_AUDIT" "${args[@]}" \
  --db "$JAIN_PINNED_ADVISORY_DB" --no-fetch
