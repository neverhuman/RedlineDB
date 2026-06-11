#!/usr/bin/env bash
# Shared helper library sourced by all ops/ci/*.sh scripts.
# Usage: source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
set -Eeuo pipefail

REQUIRED_GITLEAKS_VERSION="8.21.2"

# Typed error codes — agent-friendly exception pattern.
# Each code maps to a common fix described in docs/testing.md.
readonly ERR_MISSING_TOOL=1     # required tool not on PATH
readonly ERR_CONTRACT_MISMATCH=2  # install.sh URL is malformed or missing the ${VERSION} variable
readonly ERR_POINTER_SYNC=3     # README/FAMILY.md/family.json are out of sync
readonly ERR_ENGINE_LEAKED=4    # Rust/engine code found in the hub repo
readonly ERR_SECRET_DETECTED=5  # gitleaks found a secret

# Agent-friendly typed error handler.
# Usage: die <ERROR_CODE> <message>
# Prints: ERROR[code]: message, then points to docs/testing.md.
die() {
    local code="${1:-ERR}" msg="${2:-unknown error}"
    printf 'ERROR[%s]: %s\n' "$code" "$msg" >&2
    printf 'repair: see docs/testing.md#agent-repair-hints for common fixes\n' >&2
    exit "${code//[!0-9]/1}"
}

require_tool() {
    local tool="$1"
    if ! command -v "$tool" >/dev/null 2>&1; then
        die "$ERR_MISSING_TOOL" "required tool not found: $tool"
    fi
}

log_step() {
    echo "==> $*"
}

log_ok() {
    echo "    ok: $*"
}

# Resolve the repo root regardless of where the script is called from.
repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || pwd
}

# Check that $1 contains $2; fail with a message if not.
assert_contains() {
    local file="$1" pattern="$2"
    grep -q "$pattern" "$file" || {
        echo "ERROR: $file does not contain expected pattern: $pattern" >&2
        return 1
    }
}
