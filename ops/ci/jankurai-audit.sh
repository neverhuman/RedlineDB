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
# Jankurai is a hard dependency for this lane. `ops/ci/lib.sh` freezes the
# release sandbox's PATH selection, then binds it by physical path, exact
# version, and exact digest. This lane never installs or fetches tool source.
#
# Usage:
#   bash ops/ci/jankurai-audit.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

LOG_DIR="target/jankurai"
AUDIT_POLICY="agent/audit-policy.toml"
mkdir -p "$LOG_DIR" "$LOG_DIR/security" "$LOG_DIR/proofbind" "$LOG_DIR/proofmark" "$LOG_DIR/rust"
JANKURAI_VERIFY_LOG="$LOG_DIR/governed-jankurai.log"

force_full_smart_scan() {
    # CI jobs start with an empty target directory, but local mirrors often
    # retain smart-scan state from earlier audits. Removing it preserves the
    # canonical command string while forcing a full evidence scan.
    rm -f target/jankurai/audit-state.json
}

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
    bash scripts/check_audit_policy_mirror.sh
    force_full_smart_scan
    jankurai audit . \
        --mode advisory \
        --baseline .jankurai/baselines/main.repo-score.json \
        --json "$LOG_DIR/repo-score.advisory.json" \
        --md "$LOG_DIR/repo-score.advisory.md" \
        --sarif "$LOG_DIR/jankurai.sarif" \
        --github-step-summary "$LOG_DIR/summary.md" \
        --repair-queue-jsonl "$LOG_DIR/repair-queue.jsonl" \
        --no-score-history \
        --policy "$AUDIT_POLICY"
}

# ---- 3) Fetch reviewed accepted baseline -----------------------------------
# Source baseline strictly from reviewed locations: a committed baseline
# under .jankurai/baselines/ takes priority, otherwise we pull the score
# from origin/main (the previously reviewed state). Never seed from the
# candidate audit run; that would hide score regressions
# (HLT-034 ci.ratchet.self-generated-baseline).
step_fetch_baseline() {
    [ -f .jankurai/baselines/main.repo-score.json ] || {
        printf 'missing reviewed Jankurai baseline: .jankurai/baselines/main.repo-score.json\n' >&2
        return 1
    }
    install -m 0644 .jankurai/baselines/main.repo-score.json "$LOG_DIR/accepted-baseline.json"
    echo "baseline sourced from .jankurai/baselines/main.repo-score.json"
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
    bash scripts/check_audit_policy_mirror.sh
    force_full_smart_scan
    jankurai audit . \
        --mode ratchet \
        --baseline "$LOG_DIR/accepted-baseline.json" \
        --json "$LOG_DIR/repo-score.json" \
        --md "$LOG_DIR/repo-score.md" \
        --repair-queue-jsonl "$LOG_DIR/repair-queue.jsonl" \
        --no-score-history \
        --policy "$AUDIT_POLICY" || rc=$?

    if [ "$rc" -eq 0 ]; then
        return 0
    fi

    if cargo run --quiet --locked -p redlinedb-bench --bin score_policy -- \
        audit-acceptance "$LOG_DIR/repo-score.json"
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
    local -a changed_paths=()
    local path

    while IFS= read -r -d '' path; do
        changed_paths+=(--changed "$path")
    done < <(git diff --name-only -z --diff-filter=ACMRT origin/main...HEAD --)

    jankurai proofbind verify . "${changed_paths[@]}"
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

# ---- 12) Governed binary hostile probes ------------------------------------
step_governed_jankurai_probes() {
    bash ops/ci/governed-jankurai-test.sh \
        | tee "$LOG_DIR/governed-jankurai-test.log"
}

main() {
    ci_install_jankurai_logged "$JANKURAI_VERIFY_LOG"
    step_governed_jankurai_probes

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
}

main "$@"
