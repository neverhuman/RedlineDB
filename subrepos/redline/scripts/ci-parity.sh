#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/ci-parity.sh [--fast] [--no-audit]

  --fast       run the quick iteration lane only
  --no-audit   accepted for compatibility; pr-ci does not run extra audit/security workflows
USAGE
}

fast=false
for arg in "$@"; do
    case "$arg" in
        --fast)
            fast=true
            ;;
        --no-audit)
            :
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
else
    bash "${ROOT}/scripts/ci-local.sh" pr-ci
fi
