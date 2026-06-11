#!/usr/bin/env bash
# Local CI lane dispatcher.
# Usage: bash scripts/ci-local.sh [lane] [extra args...]
#   lane defaults to "pr-ci"
set -Eeuo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lane="${1:-pr-ci}"
exec bash "ops/ci/${lane}.sh" "${@:2}"
