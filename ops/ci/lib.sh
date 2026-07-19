#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ "${CI:-}" = "true" ]; then
    # Keep CI cargo state local to the checkout so shared runner caches do not
    # become a serialization point for unrelated pipelines.
    export CARGO_HOME="$repo_root/.cargo"
    export CARGO_TARGET_DIR="$repo_root/target"
fi

ci_run() {
    if command -v rtk >/dev/null 2>&1; then
        rtk "$@"
    else
        "$@"
    fi
}

# Shared lane helpers. ROOT_DIR mirrors repo_root so lanes can locate manifests
# regardless of the caller's cwd. STRICT_TOOLS=1 turns "missing tool" warnings
# into hard failures (used in --strict CI profiles); 0 keeps bootstrap green.
ROOT_DIR="$repo_root"
STRICT_TOOLS="${REDLINE_STRICT_TOOLS:-0}"

log() { printf '[redline-ci] %s\n' "$*"; }
warn() { printf '[redline-ci][warn] %s\n' "$*" >&2; }
fail() {
    printf '[redline-ci][error] %s\n' "$*" >&2
    exit 1
}

has() { command -v "$1" >/dev/null 2>&1; }

readonly JANKURAI_VERSION="jankurai 1.6.11"
readonly JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"

# The release sandbox owns PATH. Freeze its selection before the wrapper below
# shadows the command name; require_jankurai and the wrapper both enforce the
# exact physical path, version, and digest.
JANKURAI_BIN="$(command -v jankurai 2>/dev/null || true)"
readonly JANKURAI_BIN
# The fleet control plane materializes this exact RustSec object inside the
# release sandbox. Repository lanes may select it through either the current
# variable or the compatibility name while the fleet finishes converging.
readonly RUSTSEC_DB_COMMIT="6e3286f4efa8c142fb33e5ea4342c8db6693cf34"
readonly RUSTSEC_DB_TREE="d12220aff0053a035739bec6e64aefbaafbf01a3"
readonly CARGO_DENY_RUSTSEC_DIR="advisory-db-3157b0e258782691"

verify_jankurai_identity() {
    local path="${1:?jankurai path is required}"
    local expected_version="${2:?jankurai version is required}"
    local expected_sha256="${3:?jankurai digest is required}"
    local resolved actual_sha256 actual_version

    if [ ! -f "$path" ] || [ ! -x "$path" ] || [ -L "$path" ]; then
        warn "jankurai is missing, non-executable, or symlinked: $path"
        return 1
    fi
    resolved="$(realpath -e -- "$path")" || return 1
    [ "$resolved" = "$path" ] || {
        warn "jankurai path does not resolve to itself: $path -> $resolved"
        return 1
    }
    actual_sha256="$(sha256sum -- "$path" | awk '{print $1}')" || return 1
    [ "$actual_sha256" = "$expected_sha256" ] || {
        warn "jankurai digest mismatch: $actual_sha256"
        return 1
    }
    actual_version="$("$path" --version 2>/dev/null)" || return 1
    [ "$actual_version" = "$expected_version" ] || {
        warn "jankurai version mismatch: $actual_version"
        return 1
    }
}

require_jankurai() {
    has realpath || fail "missing required tool: realpath"
    has sha256sum || fail "missing required tool: sha256sum"
    verify_jankurai_identity "$JANKURAI_BIN" "$JANKURAI_VERSION" "$JANKURAI_SHA256" \
        || fail "governed jankurai identity verification failed"
}

# Later PATH changes cannot select different bytes, and every invocation
# revalidates the identity frozen when this library was sourced.
jankurai() {
    verify_jankurai_identity "$JANKURAI_BIN" "$JANKURAI_VERSION" "$JANKURAI_SHA256" \
        || fail "governed jankurai identity verification failed"
    "$JANKURAI_BIN" "$@"
}

verify_rustsec_db_identity() {
    local path="${1:?RustSec database path is required}"
    local expected_commit="${2:?RustSec database commit is required}"
    local expected_tree="${3:?RustSec database tree is required}"
    local resolved head tree status

    [[ "$path" == /* ]] || {
        warn "RustSec database path is not absolute: $path"
        return 1
    }
    [[ "$expected_commit" =~ ^[0-9a-f]{40}$ ]] || return 1
    [[ "$expected_tree" =~ ^[0-9a-f]{40}$ ]] || return 1

    if [ ! -d "$path" ] || [ -L "$path" ]; then
        warn "RustSec database is missing or symlinked: $path"
        return 1
    fi
    resolved="$(realpath -e -- "$path")" || return 1
    [ "$resolved" = "$path" ] || return 1
    [ -d "$path/.git" ] && [ ! -L "$path/.git" ] || return 1
    [ ! -e "$path/.git/objects/info/alternates" ] || return 1
    [ -z "$(find "$path" -type l -print -quit)" ] || return 1
    head="$(git -C "$path" rev-parse HEAD 2>/dev/null)" || return 1
    [ "$head" = "$expected_commit" ] || {
        warn "RustSec database commit mismatch: $head"
        return 1
    }
    tree="$(git -C "$path" rev-parse 'HEAD^{tree}' 2>/dev/null)" || return 1
    [ "$tree" = "$expected_tree" ] || {
        warn "RustSec database tree mismatch: $tree"
        return 1
    }
    status="$(git -C "$path" status --porcelain=v1 --untracked-files=all)" || return 1
    [ -z "$status" ] || {
        warn "RustSec database is dirty"
        return 1
    }
    git -C "$path" fsck --strict --no-progress >/dev/null 2>&1
}

governed_advisory_db_path() {
    local pinned="${JAIN_PINNED_ADVISORY_DB:-}"
    local compatibility="${JAIN_ADVISORY_DB:-}"
    local selected

    if [ -n "$pinned" ] && [ -n "$compatibility" ] && [ "$pinned" != "$compatibility" ]; then
        warn "governed advisory database variables disagree"
        return 1
    fi
    selected="${pinned:-${compatibility:-${CARGO_HOME:-/home/ubuntu/.cargo}/advisory-db}}"
    [[ "$selected" == /* ]] || {
        warn "governed advisory database path is not absolute: $selected"
        return 1
    }
    if [ -n "${JAIN_PINNED_ADVISORY_COMMIT:-}" ] \
        && [ "$JAIN_PINNED_ADVISORY_COMMIT" != "$RUSTSEC_DB_COMMIT" ]; then
        warn "fleet RustSec commit disagrees with the repository pin"
        return 1
    fi
    printf '%s\n' "$selected"
}

verify_locked_cargo_closure() {
    local manifest="${1:?Cargo manifest path is required}"

    [ -f "$manifest" ] && [ ! -L "$manifest" ] || return 1
    ci_run env CARGO_NET_OFFLINE=true cargo metadata \
        --manifest-path "$manifest" --locked --offline --format-version 1 \
        >/dev/null
}

governed_cargo_deny_db_path() {
    local cargo_home="${1:?Cargo home is required}"
    local expected="$cargo_home/advisory-dbs/$CARGO_DENY_RUSTSEC_DIR"
    local selected="${JAIN_CARGO_DENY_ADVISORY_DB:-$expected}"

    [[ "$selected" == /* && "$selected" == "$expected" ]] || {
        warn "cargo-deny advisory database does not match its exact Cargo-home location"
        return 1
    }
    if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 \
        && -z "${JAIN_CARGO_DENY_ADVISORY_DB:-}" ]]; then
        warn "isolated release CI did not bind an explicit cargo-deny database"
        return 1
    fi
    printf '%s\n' "$selected"
}

verify_cargo_deny_db_binding() {
    local cargo_home="${1:?Cargo home is required}"
    local advisory_db="${2:?governed advisory database is required}"
    local expected_commit="${3:-$RUSTSEC_DB_COMMIT}"
    local expected_tree="${4:-$RUSTSEC_DB_TREE}"
    local advisory_parent="$cargo_home/advisory-dbs"
    local deny_db

    deny_db="$(governed_cargo_deny_db_path "$cargo_home")" || return 1

    [[ "$cargo_home" == /* && -d "$cargo_home" && ! -L "$cargo_home" \
        && "$(realpath -e -- "$cargo_home")" == "$cargo_home" ]] || return 1
    [[ -d "$advisory_parent" && ! -L "$advisory_parent" \
        && "$(realpath -e -- "$advisory_parent")" == "$advisory_parent" ]] || return 1
    [[ -d "$deny_db" && ! -L "$deny_db" \
        && "$(realpath -e -- "$deny_db")" == "$deny_db" ]] || return 1
    verify_rustsec_db_identity \
        "$advisory_db" "$expected_commit" "$expected_tree" || return 1
    verify_rustsec_db_identity \
        "$deny_db" "$expected_commit" "$expected_tree"
}

verify_cargo_deny_db_immutable_custody_for_owner() {
    local cargo_home="${1:?Cargo home is required}"
    local advisory_db="${2:?governed advisory database is required}"
    local expected_commit="${3:-$RUSTSEC_DB_COMMIT}"
    local expected_tree="${4:-$RUSTSEC_DB_TREE}"
    local expected_owner="${5:?expected custody owner is required}"
    local advisory_parent="$cargo_home/advisory-dbs"
    local deny_db mount_target mount_options path owner mode

    verify_cargo_deny_db_binding \
        "$cargo_home" "$advisory_db" "$expected_commit" "$expected_tree" \
        || return 1
    deny_db="$(governed_cargo_deny_db_path "$cargo_home")" || return 1
    command -v findmnt >/dev/null 2>&1 || return 1
    mount_target="$(findmnt -n -T "$advisory_parent" -o TARGET)" || return 1
    [[ "$mount_target" == "$advisory_parent" ]] || return 1
    mount_options="$(findmnt -n -T "$advisory_parent" -o VFS-OPTIONS)" || return 1
    [[ ",$mount_options," == *,ro,* ]] || return 1
    for path in "$advisory_parent" "$deny_db"; do
        owner="$(stat -Lc '%u' -- "$path")" || return 1
        mode="$(stat -Lc '%a' -- "$path")" || return 1
        [[ "$owner" == "$expected_owner" \
            && $((8#$mode & 022)) -eq 0 \
            && ! -w "$path" ]] || return 1
    done
    [[ -z "$(find "$deny_db" -writable -print -quit)" ]]
}

verify_cargo_deny_db_immutable_custody() {
    verify_cargo_deny_db_immutable_custody_for_owner "$@" 0
}

cargo_deny_db_binding_identity() {
    local cargo_home="${1:?Cargo home is required}"
    local advisory_db="${2:?governed advisory database is required}"
    local expected_commit="${3:-$RUSTSEC_DB_COMMIT}"
    local expected_tree="${4:-$RUSTSEC_DB_TREE}"
    local advisory_parent="$cargo_home/advisory-dbs"
    local deny_db

    verify_cargo_deny_db_binding \
        "$cargo_home" "$advisory_db" "$expected_commit" "$expected_tree" \
        || return 1
    deny_db="$(governed_cargo_deny_db_path "$cargo_home")" || return 1
    stat -Lc '%d:%i:%h:%u:%a' -- \
        "$cargo_home" "$advisory_parent" "$deny_db" | paste -sd ';' -
}

verify_cargo_deny_db_binding_unchanged() {
    local expected_identity="${1:?expected custody identity is required}"
    shift
    local actual_identity

    actual_identity="$(cargo_deny_db_binding_identity "$@")" || return 1
    [[ "$actual_identity" == "$expected_identity" ]]
}

run_with_cargo_deny_db_custody() {
    local cargo_home="${1:?Cargo home is required}"
    local advisory_db="${2:?governed advisory database is required}"
    local expected_commit="${3:-$RUSTSEC_DB_COMMIT}"
    local expected_tree="${4:-$RUSTSEC_DB_TREE}"
    local binding_identity command_status=0
    shift 4
    [[ "$#" -gt 0 ]] || return 1

    binding_identity="$(
        cargo_deny_db_binding_identity \
            "$cargo_home" "$advisory_db" "$expected_commit" "$expected_tree"
    )" || return 1
    if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
        verify_cargo_deny_db_immutable_custody \
            "$cargo_home" "$advisory_db" "$expected_commit" "$expected_tree" \
            || return 1
    fi

    "$@" || command_status=$?
    verify_cargo_deny_db_binding_unchanged \
        "$binding_identity" "$cargo_home" "$advisory_db" \
        "$expected_commit" "$expected_tree" || return 1
    return "$command_status"
}

missing_tool() {
    local tool="$1" reason="${2:-required for this check}"
    if [ "$STRICT_TOOLS" = "1" ]; then
        fail "missing tool: ${tool} (${reason}); install it or unset REDLINE_STRICT_TOOLS"
    fi
    warn "skipping ${tool}: not installed (${reason})"
    return 0
}

run_if_has() {
    local tool="$1" reason="$2"
    shift 2
    if ! has "$tool"; then
        missing_tool "$tool" "$reason"
        return 0
    fi
    ci_run "$@"
}

repo_has() { [ -e "$ROOT_DIR/$1" ]; }
