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
