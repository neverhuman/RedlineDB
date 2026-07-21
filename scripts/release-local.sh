#!/usr/bin/env bash
# Authoritative local release dispatcher used by the protected Jeryu lifecycle.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v rtk >/dev/null 2>&1; then
    rtk() {
        "$@"
    }
fi

rtk scripts/ci-local.sh pr-ci
rtk scripts/ci-local.sh release
