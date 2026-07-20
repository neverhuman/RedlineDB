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

# cargo-audit invokes Git internally to inspect FETCH_HEAD metadata. The pinned
# database is deliberately root-owned in release CI, so give that subprocess
# one exact safe.directory without carrying any caller Git configuration into
# the tool. The wrapper's physical path/commit/clean checks above remain the
# authority; this only permits read access to those already-validated bytes.
git_config_variables=("${!GIT_CONFIG_KEY_@}" "${!GIT_CONFIG_VALUE_@}")
for variable in "${git_config_variables[@]}"; do
  [[ -z "$variable" ]] || unset "$variable"
done
exec /usr/bin/env \
  -u GIT_CONFIG -u GIT_CONFIG_PARAMETERS -u GIT_CONFIG_SYSTEM \
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
  GIT_CONFIG_COUNT=4 \
  GIT_CONFIG_KEY_0=safe.directory \
  GIT_CONFIG_VALUE_0="$JAIN_PINNED_ADVISORY_DB" \
  GIT_CONFIG_KEY_1=core.fsmonitor GIT_CONFIG_VALUE_1=false \
  GIT_CONFIG_KEY_2=core.hooksPath GIT_CONFIG_VALUE_2=/dev/null \
  GIT_CONFIG_KEY_3=protocol.allow GIT_CONFIG_VALUE_3=never \
  GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 \
  "$JAIN_REAL_CARGO_AUDIT" "${args[@]}" \
    --db "$JAIN_PINNED_ADVISORY_DB" --no-fetch
