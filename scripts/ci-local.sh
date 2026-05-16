#!/usr/bin/env bash
# Local CI dispatcher: gives developers the same lanes CI runs.
#
# Routes to the canonical lane scripts under ops/ci/<lane>.sh, so a
# successful local run is byte-for-byte the same evidence CI produces.
# Audit reference: HLT-038 ci.local-parity.lib-missing.
#
# Usage:
#   scripts/ci-local.sh fast       # cargo fmt+check+test, file-size guard
#   scripts/ci-local.sh security   # cargo audit + cargo deny + gitleaks
#   scripts/ci-local.sh audit      # full jankurai audit lane
#   scripts/ci-local.sh all        # fast, then security, then audit

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/ci-local.sh {fast|security|audit|all}

  fast       run ops/ci/fast.sh           (fmt + size + check + test)
  security   run ops/ci/security.sh       (cargo audit + deny + gitleaks)
  audit      run ops/ci/jankurai-audit.sh (full jankurai audit lane)
  all        run fast, then security, then audit
USAGE
}

if [ "$#" -ne 1 ]; then
    usage
    exit 64
fi

case "$1" in
    fast)
        bash "$ROOT/ops/ci/fast.sh"
        ;;
    security)
        bash "$ROOT/ops/ci/security.sh"
        ;;
    audit)
        bash "$ROOT/ops/ci/jankurai-audit.sh"
        ;;
    all)
        bash "$ROOT/ops/ci/fast.sh"
        bash "$ROOT/ops/ci/security.sh"
        bash "$ROOT/ops/ci/jankurai-audit.sh"
        ;;
    -h|--help|help)
        usage
        ;;
    *)
        printf 'ci-local: unknown lane %q\n\n' "$1" >&2
        usage
        exit 64
        ;;
esac
