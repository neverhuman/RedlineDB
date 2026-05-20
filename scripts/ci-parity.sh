#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/ci-parity.sh [--fast] [--no-audit]

  --fast       run the fast CI lane only
  --no-audit   skip the jankurai audit/security lanes
USAGE
}

fast=false
audit=true
for arg in "$@"; do
    case "$arg" in
        --fast)
            fast=true
            ;;
        --no-audit)
            audit=false
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'ci-parity: unknown option %q\n\n' "$arg" >&2
            usage
            exit 64
            ;;
    esac
done

if [ "$fast" = true ]; then
    bash "${ROOT}/scripts/ci-local.sh" fast
    if [ "$audit" = true ]; then
        bash "${ROOT}/scripts/ci-local.sh" security
        bash "${ROOT}/scripts/ci-local.sh" audit
    fi
else
    if [ "$audit" = true ]; then
        bash "${ROOT}/scripts/ci-local.sh" all
    else
        bash "${ROOT}/scripts/ci-local.sh" fast
    fi
fi
