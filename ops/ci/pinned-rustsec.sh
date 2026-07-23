#!/usr/bin/env bash

readonly REDLINE_RUSTSEC_COMMIT="6e3286f4efa8c142fb33e5ea4342c8db6693cf34"
readonly REDLINE_RUSTSEC_TREE="d12220aff0053a035739bec6e64aefbaafbf01a3"

redline_rustsec_git() {
    env -i \
        PATH=/usr/bin:/bin \
        HOME=/nonexistent \
        XDG_CONFIG_HOME=/nonexistent \
        LC_ALL=C \
        GIT_CONFIG_GLOBAL=/dev/null \
        GIT_CONFIG_SYSTEM=/dev/null \
        GIT_CONFIG_NOSYSTEM=1 \
        GIT_ATTR_NOSYSTEM=1 \
        GIT_NO_LAZY_FETCH=1 \
        GIT_TERMINAL_PROMPT=0 \
        GIT_ASKPASS=/bin/false \
        SSH_ASKPASS=/bin/false \
        git \
        -c core.attributesFile=/dev/null \
        -c core.fsmonitor=false \
        -c core.hooksPath=/dev/null \
        -c credential.helper= \
        -c diff.external= \
        -c protocol.allow=never \
        -c protocol.file.allow=always \
        "$@"
}

redline_stage_local_rustsec() {
    local stage_root="${1:?stage root is required}"
    local source database deny_home deny_database
    source="${JAIN_RUSTSEC_ADVISORY_SOURCE:-${HOME:?HOME is required}/.cargo/advisory-db}"
    database="$stage_root/advisory-db"
    deny_home="$stage_root/cargo-home"
    deny_database="$deny_home/advisory-dbs/advisory-db-3157b0e258782691"
    [[ "$stage_root" == /* && -d "$stage_root" && ! -L "$stage_root" \
        && "$source" == /* && -d "$source/.git" && ! -L "$source" ]] || {
        printf 'local RustSec staging paths are not physical absolute directories\n' >&2
        return 1
    }
    mkdir -p "$(dirname "$deny_database")"
    redline_rustsec_git clone -q --no-local --no-checkout "$source" "$database"
    redline_rustsec_git clone -q --no-local --no-checkout "$source" "$deny_database"
    redline_rustsec_git -C "$database" checkout -q --detach "$REDLINE_RUSTSEC_COMMIT"
    redline_rustsec_git -C "$deny_database" checkout -q --detach "$REDLINE_RUSTSEC_COMMIT"
    export JAIN_PINNED_ADVISORY_DB="$database"
    export JAIN_ADVISORY_DB="$database"
    export JAIN_PINNED_ADVISORY_COMMIT="$REDLINE_RUSTSEC_COMMIT"
    export REDLINE_CARGO_DENY_HOME="$deny_home"
}

redline_prepare_local_rustsec() {
    local stage_root="${1:?stage root is required}"
    local configured=0 value
    [[ "${JAIN_RELEASE_CI:-0}" != 1 ]] || return 0
    for value in JAIN_PINNED_ADVISORY_DB JAIN_ADVISORY_DB \
        JAIN_PINNED_ADVISORY_COMMIT REDLINE_CARGO_DENY_HOME; do
        [[ -z "${!value:-}" ]] || configured=$((configured + 1))
    done
    if [[ "$configured" == 0 ]]; then
        redline_stage_local_rustsec "$stage_root"
    elif [[ "$configured" != 4 ]]; then
        printf 'ordinary RustSec custody variables are incomplete\n' >&2
        return 1
    fi
}

redline_validate_pinned_rustsec() {
    local database="$1" resolved commit tree status
    local label="${2:-RustSec authority}"
    [[ "$database" == /* && -d "$database" && ! -L "$database" \
        && -d "$database/.git" && ! -L "$database/.git" \
        && ! -e "$database/.git/objects/info/alternates" ]] || {
        printf '%s must be an absolute standalone physical Git repository: %s\n' \
            "$label" "$database" >&2
        return 1
    }
    resolved="$(realpath -e -- "$database")" || return 1
    [[ "$resolved" == "$database" && -z "$(find "$database" -type l -print -quit)" ]] || {
        printf '%s contains a symbolic path\n' "$label" >&2
        return 1
    }
    if [[ "${JAIN_RELEASE_CI:-0}" == 1 ]]; then
        [[ "$(stat -c '%u:%g' -- "$database")" == 0:0 \
            && "$(stat -c '%u:%g' -- "$database/.git")" == 0:0 \
            && ! -w "$database" && ! -w "$database/.git" ]] || {
            printf '%s is not root-owned read-only release custody: %s\n' \
                "$label" "$database" >&2
            return 1
        }
    fi
    status="$(redline_rustsec_git -C "$database" \
        status --porcelain=v1 --untracked-files=all)" || return 1
    [[ -z "$status" ]] || {
        printf '%s is dirty: %s\n' "$label" "$database" >&2
        return 1
    }
    commit="$(redline_rustsec_git -C "$database" rev-parse 'HEAD^{commit}')" || return 1
    tree="$(redline_rustsec_git -C "$database" rev-parse 'HEAD^{tree}')" || return 1
    [[ "$commit" == "$REDLINE_RUSTSEC_COMMIT" && "$tree" == "$REDLINE_RUSTSEC_TREE" ]] || {
        printf '%s mismatch: expected %s/%s got %s/%s\n' \
            "$label" "$REDLINE_RUSTSEC_COMMIT" "$REDLINE_RUSTSEC_TREE" "$commit" "$tree" >&2
        return 1
    }
    redline_rustsec_git -C "$database" fsck --strict --no-progress >/dev/null || return 1
}

redline_resolve_pinned_rustsec() {
    local database
    if [[ -n "${JAIN_PINNED_ADVISORY_DB:-}" && -n "${JAIN_ADVISORY_DB:-}" \
        && "$JAIN_PINNED_ADVISORY_DB" != "$JAIN_ADVISORY_DB" ]]; then
        printf 'governed RustSec database variables disagree\n' >&2
        return 1
    fi
    database="${JAIN_PINNED_ADVISORY_DB:-${JAIN_ADVISORY_DB:-}}"
    if [[ -z "$database" ]]; then
        [[ "${JAIN_RELEASE_CI:-0}" != 1 ]] || {
            printf 'release CI did not provide a pinned RustSec database\n' >&2
            return 1
        }
        database="${JAIN_RUSTSEC_ADVISORY_SOURCE:-${HOME:?HOME is required}/.cargo/advisory-db}"
    fi
    if [[ "${JAIN_RELEASE_CI:-0}" == 1 \
        && ( -z "${JAIN_PINNED_ADVISORY_DB:-}" \
            || -z "${JAIN_ADVISORY_DB:-}" \
            || -z "${JAIN_PINNED_ADVISORY_COMMIT:-}" ) ]]; then
        printf 'release CI RustSec custody variables are incomplete\n' >&2
        return 1
    fi
    if [[ -n "${JAIN_PINNED_ADVISORY_COMMIT:-}" \
        && "$JAIN_PINNED_ADVISORY_COMMIT" != "$REDLINE_RUSTSEC_COMMIT" ]]; then
        printf 'fleet RustSec commit disagrees with repository pin\n' >&2
        return 1
    fi
    redline_validate_pinned_rustsec "$database" || return 1
    export REDLINE_PINNED_ADVISORY_DB="$database"
}

redline_resolve_cargo_deny_rustsec() {
    local cargo_home database
    cargo_home="${REDLINE_CARGO_DENY_HOME:-${CARGO_HOME:-${HOME:?HOME is required}/.cargo}}"
    [[ "$cargo_home" == /* && -d "$cargo_home" && ! -L "$cargo_home" ]] || {
        printf 'cargo-deny home must be an absolute physical directory: %s\n' "$cargo_home" >&2
        return 1
    }
    database="$cargo_home/advisory-dbs/advisory-db-3157b0e258782691"
    redline_validate_pinned_rustsec "$database" "cargo-deny RustSec authority" || return 1
    export REDLINE_CARGO_DENY_HOME="$cargo_home"
}
