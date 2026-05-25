#!/usr/bin/env bash
# Local CI dispatcher: gives developers the same lanes CI runs.
#
# Routes to the canonical lane scripts under ops/ci/<lane>.sh, so a
# successful local run exercises the same command surface as CI.
# Audit reference: HLT-042 ci-local-parity.lib-missing.
#
# Usage:
#   scripts/ci-local.sh pr-ci              # exact local mirror of .github/workflows/ci.yml
#   scripts/ci-local.sh fast               # quick iteration lane
#   scripts/ci-local.sh security           # cargo audit + cargo deny + gitleaks
#   scripts/ci-local.sh audit              # full jankurai audit lane
#   scripts/ci-local.sh dependency-review  # local dependency-review mirror
#   scripts/ci-local.sh sqlite-parity-report # local SQLite parity report update
#   scripts/ci-local.sh pr-gate            # PR freshness + staged jankurai gate
#   scripts/ci-local.sh jankurai-tools     # local mirror for jankurai-tools.yml matrix
#   scripts/ci-local.sh all                # local mirror of all PR workflows

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/ci-local.sh {pr-ci|fast|security|audit|dependency-review|sqlite-parity-report|jankurai-tools|pr-gate|all}

  pr-ci              run the exact local mirror of .github/workflows/ci.yml
  fast                run scripts/just/fast.sh          (quick iteration lane)
  security            run ops/ci/security.sh            (cargo audit + deny + gitleaks)
  audit               run ops/ci/jankurai-audit.sh      (full jankurai audit lane)
  dependency-review   run ops/ci/dependency-review.sh   (cargo deny advisories/bans/licenses/sources)
  sqlite-parity-report run ops/ci/sqlite-parity-report.sh update
  jankurai-tools      run every jankurai-tools matrix lane plus input-boundary cross-check
  pr-gate             run PR freshness + jankurai staged-gate against origin/main
  all                 run local mirrors for all PR workflows
USAGE
}

if [ "$#" -ne 1 ]; then
    usage
    exit 64
fi

run_ci_yml_pr_mirror() {
    local stage

    printf 'ci-local pr-ci: preflight\n' >&2
    CI_FAST_STAGE=preflight bash "$ROOT/ops/ci/fast.sh"

    for stage in \
        core \
        kernel \
        sql-unit \
        sql-contracts \
        bench
    do
        printf 'ci-local pr-ci: tests/%s\n' "$stage" >&2
        CI_FAST_STAGE="$stage" bash "$ROOT/ops/ci/fast.sh"
    done

    for stage in \
        redline-testing-official
    do
        printf 'ci-local pr-ci: parity/%s\n' "$stage" >&2
        CI_PARITY_STAGE="$stage" bash "$ROOT/ops/ci/parity.sh"
    done

    printf 'ci-local pr-ci: official-evidence-guard\n' >&2
    bash "$ROOT/scripts/guard-official-evidence.sh"

    printf 'ci-local pr-ci: metrics-readme (verify)\n' >&2
    bash "$ROOT/scripts/generate-metrics-readme.sh" --check
}

case "$1" in
    pr-ci)
        run_ci_yml_pr_mirror
        ;;
    fast)
        bash "$ROOT/scripts/just/fast.sh"
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
    sqlite-parity-report)
        bash "$ROOT/ops/ci/sqlite-parity-report.sh" update
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
        bash "$ROOT/scripts/ci-local.sh" pr-ci
        bash "$ROOT/ops/ci/security.sh"
        bash "$ROOT/ops/ci/dependency-review.sh"
        bash "$ROOT/ops/ci/jankurai-audit.sh"
        bash "$ROOT/scripts/ci-local.sh" jankurai-tools
        bash "$ROOT/scripts/ci-local.sh" pr-gate
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
