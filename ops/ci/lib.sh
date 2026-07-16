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

readonly JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
readonly JANKURAI_VERSION="jankurai 1.6.11"
readonly JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"
# Sourced security lanes consume this constant; standalone ShellCheck cannot see that use.
# shellcheck disable=SC2034
readonly RUSTSEC_DB_COMMIT="9f3e138091487e69144f536d36976e427a7a3307"

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

# PATH and JANKURAI_BIN from the caller cannot select different bytes.
jankurai() {
    "$JANKURAI_BIN" "$@"
}

verify_rustsec_db_identity() {
    local path="${1:?RustSec database path is required}"
    local expected_commit="${2:?RustSec database commit is required}"
    local resolved head status

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
    status="$(git -C "$path" status --porcelain=v1 --untracked-files=all)" || return 1
    [ -z "$status" ] || {
        warn "RustSec database is dirty"
        return 1
    }
    git -C "$path" fsck --strict --no-progress >/dev/null 2>&1
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

cargo_workspace_ready() {
    repo_has Cargo.toml || return 1
    has cargo || return 1
    cargo metadata --no-deps --format-version 1 >/dev/null 2>&1
}
