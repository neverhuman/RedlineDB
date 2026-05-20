#!/usr/bin/env bash
# Local CI dispatcher: gives developers the same lanes CI runs.
#
# Routes to the canonical lane scripts under ops/ci/<lane>.sh, so a
# successful local run is byte-for-byte the same evidence CI produces.
# Audit reference: HLT-042 ci-local-parity.lib-missing.
#
# Usage:
#   scripts/ci-local.sh fast               # cargo fmt+check+test, file-size guard
#   scripts/ci-local.sh security           # cargo audit + cargo deny + gitleaks
#   scripts/ci-local.sh audit              # full jankurai audit lane
#   scripts/ci-local.sh dependency-review  # local dependency-review mirror
#   scripts/ci-local.sh pr-gate            # PR freshness + staged jankurai gate
#   scripts/ci-local.sh jankurai-tools     # local mirror for jankurai-tools.yml matrix
#   scripts/ci-local.sh all                # full local PR CI mirror

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/ci-local.sh {fast|security|audit|dependency-review|jankurai-tools|pr-gate|all}

  fast                run ops/ci/fast.sh                (fmt + size + check + test)
  security            run ops/ci/security.sh            (cargo audit + deny + gitleaks)
  audit               run ops/ci/jankurai-audit.sh      (full jankurai audit lane)
  dependency-review   run ops/ci/dependency-review.sh   (cargo deny advisories/bans/licenses/sources)
  jankurai-tools      run every jankurai-tools matrix lane plus input-boundary cross-check
  pr-gate             run PR freshness + jankurai staged-gate against origin/main
  all                 run the full local PR CI mirror
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
    dependency-review)
        bash "$ROOT/ops/ci/dependency-review.sh"
        ;;
    jankurai-tools)
        for tool in \
            audit-ci \
            proof-routing \
            security \
            contract-drift \
            authz-matrix \
            input-boundary \
            agent-tool-supply \
            release-readiness \
            cost-budget
        do
            bash "$ROOT/ops/ci/jankurai-tools.sh" "$tool"
            if [ "$tool" = "input-boundary" ]; then
                cargo test -p redlinedb-ffi --test exec_input_boundary --locked --no-run
            fi
        done
        ;;
    pr-gate)
        git -C "$ROOT" fetch origin main --quiet
        if ! git -C "$ROOT" merge-base --is-ancestor origin/main HEAD; then
            echo "PR branch is behind origin/main. Rebase or merge main before committing."
            exit 1
        fi
        LOG_DIR=.jankurai/staged-gate-local \
            BASE_REF=origin/main \
            bash "$ROOT/ops/ci/jankurai-staged-gate.sh"
        ;;
    all)
        bash "$ROOT/ops/ci/fast.sh"
        bash "$ROOT/ops/ci/security.sh"
        bash "$ROOT/ops/ci/dependency-review.sh"
        bash "$ROOT/ops/ci/jankurai-audit.sh"
        "$0" jankurai-tools
        "$0" pr-gate
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
