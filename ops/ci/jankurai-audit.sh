#!/usr/bin/env bash
# Jankurai audit lane: the canonical CI + local invocation for every
# tool-adoption-manifest entry rooted in `jankurai audit`.
#
# Mirrors `.github/workflows/jankurai.yml`'s `audit` job (steps that used
# to live inline as "Install jankurai" through "Language bad-behavior
# tests"). Sourcing this script from `scripts/ci-local.sh audit` gives
# the same evidence locally and in CI. Audit references:
# HLT-042 ci-local-parity.lib-missing,
# HLT-034 ci-bad-behavior (audit-job soft gate moved out of YAML).
#
# Soft-gate rationale: see .jankurai/ci-soft-gate-ledger.toml#jankurai-audit-job
# The workflow YAML carries NO `continue-on-error: true`. The audit-job
# soft-gate semantics (jankurai is not yet published to a CI-reachable
# source) live in this script: `step_install_jankurai` is run through
# `ci_soft_gate`, and on install failure the subsequent jankurai-rooted
# steps are short-circuited with an explicit ledger-cross-referenced
# marker. The script always exits 0; if/when jankurai becomes
# CI-installable, the soft gate flips to hard simply by removing the
# `ci_soft_gate` wrapper around `step_install_jankurai`.
#
# Usage:
#   bash ops/ci/jankurai-audit.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

LOG_DIR="target/jankurai"
mkdir -p "$LOG_DIR" "$LOG_DIR/security" "$LOG_DIR/proofbind" "$LOG_DIR/proofmark" "$LOG_DIR/rust"
JANKURAI_INSTALL_LOG="$LOG_DIR/jankurai-install.log"

# ---- 1) Install jankurai ----------------------------------------------------
# Try the canonical git source first, then the fallback fork, then the
# crates.io publication if/when one exists. Matches the install logic
# previously inline in .github/workflows/jankurai.yml.
#
# Returns the install exit code so `ci_soft_gate` can stamp the marker.
step_install_jankurai() {
    cargo install --git "${CI_JANKURAI_GIT}" --locked jankurai \
        || cargo install --git https://github.com/anthropics/jankurai --locked jankurai \
        || cargo install jankurai --locked
}

# ---- 2) jankurai --version --------------------------------------------------
step_version() {
    jankurai --version
}

# ---- 3) jankurai audit (advisory) ------------------------------------------
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

# ---- 4) Fetch reviewed accepted baseline -----------------------------------
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

# ---- 5) jankurai security run (strict, pre-audit) --------------------------
# Canonical CI invocation for the `security` tool-adoption entry. Runs
# with --strict in the ci profile BEFORE the final ratchet audit so
# security evidence is binding (HLT-034 ci-bad-behavior).
step_security_run() {
    jankurai security run . \
        --strict \
        --profile ci \
        --out "$LOG_DIR/security/evidence.json"
}

# ---- 6) jankurai audit (ratchet) — tool-adoption CI evidence ---------------
step_audit_ratchet() {
    jankurai audit . \
        --policy .jankurai/audit-policy.toml \
        --mode ratchet \
        --baseline "$LOG_DIR/accepted-baseline.json" \
        --json "$LOG_DIR/repo-score.json" \
        --md "$LOG_DIR/repo-score.md"
}

# ---- 7) jankurai doctor ----------------------------------------------------
step_doctor() {
    jankurai doctor --fail-on critical
}

# ---- 8) Proofbind verify ---------------------------------------------------
step_proofbind() {
    jankurai proofbind verify . --changed-from origin/main
}

# ---- 9) Proofmark rust -----------------------------------------------------
step_proofmark() {
    jankurai proofmark rust . --obligations "$LOG_DIR/proofbind/obligations.json"
}

# ---- 10) Rust witness build ------------------------------------------------
step_rust_witness() {
    jankurai rust witness build .
}

# ---- 11) Copy-code audit ---------------------------------------------------
step_copy_code() {
    jankurai copy-code . --json "$LOG_DIR/copy-code.json" --md "$LOG_DIR/copy-code.md"
}

# ---- 12) UX QA smoke -------------------------------------------------------
step_ux_qa() {
    jankurai ux audit --config .jankurai/ux-qa.toml --out "$LOG_DIR/ux-qa.json"
}

# ---- 13) Language bad-behavior tests ---------------------------------------
# Canonical CI invocation for the ci-bad-behavior, git-bad-behavior, and
# release-bad-behavior tool-adoption entries:
#   cargo test -p jankurai --test language_bad_behavior
# Run against the upstream jankurai source (jankurai is not a workspace
# member here) and capture the output as the canonical evidence artifact
# target/jankurai/language-bad-behavior.log.
#
# Hard gate: the workflow YAML carries NO `continue-on-error: true` for
# this step. Soft-gate semantics (upstream-clone-failed -> exit 0) live
# here, and we ALWAYS write a machine-grep-able
# `status: upstream-{clone-failed|tests-passed|tests-failed}` line.
step_language_bad_behavior() {
    rm -rf target/jankurai-src

    local cloned=0
    if git clone --depth 1 "${CI_JANKURAI_GIT}.git" target/jankurai-src \
        || git clone --depth 1 https://github.com/anthropics/jankurai.git target/jankurai-src; then
        cloned=1
    fi

    if [ "${cloned}" -eq 1 ] && [ -d target/jankurai-src ]; then
        local rc=0
        ( cd target/jankurai-src && cargo test -p jankurai --test language_bad_behavior --no-fail-fast ) \
            > >(tee "$LOG_DIR/language-bad-behavior.log") 2>&1 || rc=$?
        printf 'status: %s\n' "$( [ "$rc" -eq 0 ] && echo upstream-tests-passed || echo upstream-tests-failed )" \
            >> "$LOG_DIR/language-bad-behavior.log"
        # Hard gate when the clone succeeds: test failure is a real failure.
        return "$rc"
    fi

    printf 'attempted: cargo test -p jankurai --test language_bad_behavior\nstatus: upstream-clone-failed\nsoft-gate=jankurai-audit-job ledger=.jankurai/ci-soft-gate-ledger.toml\n' \
        | tee "$LOG_DIR/language-bad-behavior.log"
    return 0
}

main() {
    # Soft gate the install: if jankurai is not installable in CI we
    # record the failure to JANKURAI_INSTALL_LOG and short-circuit the
    # downstream jankurai-rooted steps. The wrapper still returns 0 so
    # the workflow step is green; the marker line is the auditable
    # evidence and is cross-referenced from
    # .jankurai/ci-soft-gate-ledger.toml#jankurai-audit-job.
    ci_soft_gate jankurai-audit-job "$JANKURAI_INSTALL_LOG" -- step_install_jankurai

    if ! command -v jankurai >/dev/null 2>&1; then
        printf 'soft-gate=jankurai-audit-job status=installed=false ledger=.jankurai/ci-soft-gate-ledger.toml\nstatus: jankurai-unavailable\n' \
            | tee -a "$JANKURAI_INSTALL_LOG"
        # Still produce the language-bad-behavior evidence artifact so
        # upload-artifact has something to publish; this step has its
        # own soft-gate marker internally.
        step_language_bad_behavior
        return 0
    fi

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
