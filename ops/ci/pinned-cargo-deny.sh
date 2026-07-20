#!/usr/bin/env bash
set -euo pipefail

: "${JAIN_REAL_CARGO_DENY:?real cargo-deny path is required}"
: "${JAIN_PINNED_ADVISORY_DB:?isolated advisory database path is required}"
: "${JAIN_ADVISORY_DB:?canonical advisory database path is required}"
: "${JAIN_CARGO_DENY_ADVISORY_DB:?physical cargo-deny database path is required}"
: "${JAIN_PINNED_ADVISORY_COMMIT:?pinned advisory database commit is required}"

advisory_git=(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_NO_LAZY_FETCH=1 \
  GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 /usr/bin/git \
  -c "safe.directory=$JAIN_PINNED_ADVISORY_DB" -c protocol.allow=never \
  -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c core.untrackedCache=false -c core.alternateRefsCommand=false \
  -c diff.external=)
deny_git=(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C HOME=/nonexistent \
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_NO_LAZY_FETCH=1 \
  GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 /usr/bin/git \
  -c "safe.directory=$JAIN_CARGO_DENY_ADVISORY_DB" -c protocol.allow=never \
  -c core.fsmonitor=false -c core.hooksPath=/dev/null \
  -c core.untrackedCache=false -c core.alternateRefsCommand=false \
  -c diff.external=)

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

actual="$("${advisory_git[@]}" -C "$JAIN_PINNED_ADVISORY_DB" rev-parse HEAD)"
[[ "$actual" == "$JAIN_PINNED_ADVISORY_COMMIT" ]] || {
  printf 'cargo-deny RustSec commit mismatch: expected %s, got %s\n' \
    "$JAIN_PINNED_ADVISORY_COMMIT" "$actual" >&2
  exit 1
}
[[ -z "$("${advisory_git[@]}" -C "$JAIN_PINNED_ADVISORY_DB" \
  status --porcelain --untracked-files=all)" ]] || {
  printf 'cargo-deny isolated RustSec database is dirty: %s\n' \
    "$JAIN_PINNED_ADVISORY_DB" >&2
  exit 1
}
deny_actual="$("${deny_git[@]}" -C "$JAIN_CARGO_DENY_ADVISORY_DB" \
  rev-parse HEAD)"
[[ "$deny_actual" == "$JAIN_PINNED_ADVISORY_COMMIT" ]] || {
  printf 'cargo-deny physical RustSec commit mismatch: expected %s, got %s\n' \
    "$JAIN_PINNED_ADVISORY_COMMIT" "$deny_actual" >&2
  exit 1
}
[[ -z "$("${deny_git[@]}" -C "$JAIN_CARGO_DENY_ADVISORY_DB" \
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

# cargo-deny invokes Git internally to inspect FETCH_HEAD metadata in its
# hashed advisory database. That exact child is deliberately root-owned in
# release CI, so admit only the already-validated physical path and strip all
# caller Git configuration before entering the real tool.
git_config_variables=("${!GIT_CONFIG_KEY_@}" "${!GIT_CONFIG_VALUE_@}")
for variable in "${git_config_variables[@]}"; do
  [[ -z "$variable" ]] || unset "$variable"
done
deny_environment=(
  /usr/bin/env
  -u GIT_CONFIG -u GIT_CONFIG_PARAMETERS -u GIT_CONFIG_SYSTEM
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
  GIT_CONFIG_COUNT=4
  GIT_CONFIG_KEY_0=safe.directory
  GIT_CONFIG_VALUE_0="$JAIN_CARGO_DENY_ADVISORY_DB"
  GIT_CONFIG_KEY_1=core.fsmonitor GIT_CONFIG_VALUE_1=false
  GIT_CONFIG_KEY_2=core.hooksPath GIT_CONFIG_VALUE_2=/dev/null
  GIT_CONFIG_KEY_3=protocol.allow GIT_CONFIG_VALUE_3=never
  GIT_NO_LAZY_FETCH=1 GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0
)
for ((index = 0; index < ${#args[@]}; index++)); do
  if [[ "${args[$index]}" == check ]]; then
    exec "${deny_environment[@]}" "$JAIN_REAL_CARGO_DENY" \
      "${prefix[@]}" "${args[@]:0:$((index + 1))}" --disable-fetch \
      "${args[@]:$((index + 1))}"
  fi
done
exec "${deny_environment[@]}" "$JAIN_REAL_CARGO_DENY" \
  "${prefix[@]}" "${args[@]}"
