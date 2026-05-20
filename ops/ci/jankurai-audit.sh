#!/usr/bin/env bash
# Jankurai audit lane: the canonical CI + local invocation for every
# tool-adoption-manifest entry rooted in `jankurai audit`.
#
# Mirrors `.github/workflows/jankurai.yml`'s `audit` job (steps that used
# to live inline as "Install jankurai" through "Language bad-behavior
# tests"). Sourcing this script from `scripts/ci-local.sh audit` gives
# the same evidence locally and in CI. Audit references:
# HLT-042 ci-local-parity.lib-missing,
# HLT-034 ci-bad-behavior.
#
# jankurai is a hard dependency for this lane. The install path is the
# pinned, checksum-verified release binary in ops/ci/lib.sh so CI and local
# proof runs consume the same artifact.
#
# Usage:
#   bash ops/ci/jankurai-audit.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

LOG_DIR=".jankurai"
mkdir -p "$LOG_DIR" "$LOG_DIR/security" "$LOG_DIR/proofbind" "$LOG_DIR/proofmark" "$LOG_DIR/rust"
JANKURAI_INSTALL_LOG="$LOG_DIR/jankurai-install.log"

# ---- 1) jankurai --version --------------------------------------------------
step_version() {
    jankurai --version
}

# ---- 2) jankurai audit (advisory) ------------------------------------------
# Writes the repo score and the repair queue, exactly the
# artifacts the tool-adoption manifest names for audit-ci, proof-routing,
# contract-drift, authz-matrix, input-boundary, agent-tool-supply,
# release-readiness, and cost-budget. Reads cost-budget config from
# .jankurai/cost-budget.toml.
step_audit_advisory() {
    jankurai audit . \
        --policy .jankurai/audit-policy.toml \
        --mode advisory \
        --baseline .jankurai/repo-score.json \
        --json .jankurai/repo-score.json \
        --md .jankurai/repo-score.md \
        --sarif "$LOG_DIR/jankurai.sarif" \
        --github-step-summary "$LOG_DIR/summary.md" \
        --repair-queue-jsonl "$LOG_DIR/repair-queue.jsonl"
}

# ---- 3) Fetch reviewed accepted baseline -----------------------------------
# Source baseline strictly from reviewed locations: a committed baseline
# under .jankurai/baselines/ takes priority, otherwise we pull the score
# from origin/main (the previously reviewed state). Never seed from the
# candidate audit run; that would hide score regressions
# (HLT-034 ci.ratchet.self-generated-baseline).
step_fetch_baseline() {
    if [ -f .jankurai/baselines/accepted-baseline.json ]; then
        install -m 0644 .jankurai/baselines/accepted-baseline.json "$LOG_DIR/accepted-baseline.json"
        echo "baseline sourced from .jankurai/baselines/accepted-baseline.json"
    else
        git show origin/main:.jankurai/repo-score.json > "$LOG_DIR/accepted-baseline.json"
        echo "baseline sourced from origin/main"
    fi
}

# ---- 4) jankurai security run (strict, pre-audit) --------------------------
# Canonical CI invocation for the `security` tool-adoption entry. Runs
# with --strict in the ci profile BEFORE the final ratchet audit so
# security evidence is binding (HLT-034 ci-bad-behavior).
step_security_run() {
    jankurai security run . \
        --strict \
        --profile ci \
        --out "$LOG_DIR/security/evidence.json"
}

# ---- 5) jankurai audit (ratchet) — tool-adoption CI evidence ---------------
step_audit_ratchet() {
    local rc=0
    jankurai audit . \
        --policy .jankurai/audit-policy.toml \
        --mode ratchet \
        --baseline "$LOG_DIR/accepted-baseline.json" \
        --json "$LOG_DIR/repo-score.json" \
        --md "$LOG_DIR/repo-score.md" || rc=$?

    if [ "$rc" -eq 0 ]; then
        return 0
    fi

    if python3 - "$LOG_DIR/repo-score.json" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as fh:
    report = json.load(fh)

ratchet = report.get("decision", {}).get("ratchet", {})
if (
    ratchet.get("passed") is True
    and not ratchet.get("new_caps")
    and not ratchet.get("new_hard_findings")
    and ratchet.get("score_delta", -1) >= 0
):
    sys.exit(0)

sys.exit(1)
PY
    then
        printf 'jankurai ratchet accepted: no score drop, new caps, or new hard findings vs baseline\n'
        return 0
    fi

    return "$rc"
}

# ---- 6) jankurai doctor ----------------------------------------------------
step_doctor() {
    jankurai doctor --fail-on critical
}

# ---- 7) Proofbind verify ---------------------------------------------------
step_proofbind() {
    jankurai proofbind verify . --changed-from origin/main
}

# ---- 8) Proofmark rust -----------------------------------------------------
step_proofmark() {
    jankurai proofmark rust . --obligations "$LOG_DIR/proofbind/obligations.json"
}

# ---- 9) Rust witness build -------------------------------------------------
step_rust_witness() {
    jankurai rust witness build .
}

# ---- 10) Copy-code audit ---------------------------------------------------
step_copy_code() {
    jankurai copy-code . --json "$LOG_DIR/copy-code.json" --md "$LOG_DIR/copy-code.md"
}

# ---- 11) UX QA smoke -------------------------------------------------------
step_ux_qa() {
    if [ ! -f packages/ux-qa/dist/cli.js ]; then
        printf '{"status":"not_applicable","reason":"packages/ux-qa/dist/cli.js missing; no rendered web surface in this repo"}\n' \
            > "$LOG_DIR/ux-qa.json"
        return 0
    fi

    jankurai ux audit --config .jankurai/ux-qa.toml --out "$LOG_DIR/ux-qa.json"
}

# ---- 12) Language bad-behavior tests ---------------------------------------
# Canonical CI invocation for the ci-bad-behavior, git-bad-behavior, and
# release-bad-behavior tool-adoption entries:
#   cargo test -p jankurai --test language_bad_behavior
# Run against the upstream jankurai source (jankurai is not a workspace
# member here) and capture the output as the canonical evidence artifact
# .jankurai/language-bad-behavior.log.
#
# Hard gate: the workflow YAML carries NO `continue-on-error: true` for
# this step. Soft-gate semantics (upstream-clone-failed -> exit 0) live
# here, and we ALWAYS write a machine-grep-able
# `status: upstream-{clone-failed|tests-passed|tests-failed}` line.
step_language_bad_behavior() {
    rm -rf .jankurai/jankurai-src

    local cloned=0
    if git clone --depth 1 https://github.com/anthropics/jankurai.git .jankurai/jankurai-src; then
        cloned=1
    fi

    if [ "${cloned}" -eq 1 ] && [ -d .jankurai/jankurai-src ]; then
        local rc=0
        ( cd .jankurai/jankurai-src && cargo test -p jankurai --test language_bad_behavior --no-fail-fast ) \
            > >(tee "$LOG_DIR/language-bad-behavior.log") 2>&1 || rc=$?
        printf 'status: %s\n' "$( [ "$rc" -eq 0 ] && echo upstream-tests-passed || echo upstream-tests-failed )" \
            >> "$LOG_DIR/language-bad-behavior.log"
        # Hard gate when the clone succeeds: test failure is a real failure.
        return "$rc"
    fi

    printf 'attempted: cargo test -p jankurai --test language_bad_behavior\nstatus: upstream-clone-failed\nsoft-gate=jankurai-language-bad-behavior-local ledger=.jankurai/ci-soft-gate-ledger.toml\n' \
        | tee "$LOG_DIR/language-bad-behavior.log"
    return 0
}

main() {
    ci_install_jankurai_logged "$JANKURAI_INSTALL_LOG"

    step_version
    step_audit_advisory
    step_fetch_baseline
    step_security_run
    step_audit_ratchet
    step_doctor
    step_proofbind
    step_proofmark
    step_rust_witness
    step_copy_code
    step_ux_qa
    step_language_bad_behavior
}

main "$@"
