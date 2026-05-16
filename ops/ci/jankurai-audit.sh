#!/usr/bin/env bash
# Jankurai audit lane: the canonical CI + local invocation for every
# tool-adoption-manifest entry rooted in `jankurai audit`.
#
# Mirrors `.github/workflows/jankurai.yml`'s `audit` job (steps that used
# to live inline as "Install jankurai" through "Language bad-behavior
# tests"). Sourcing this script from `scripts/ci-local.sh audit` gives
# the same evidence locally and in CI. Audit references:
# HLT-038 ci.local-parity.lib-missing,
# HLT-042 ci-bad-behavior / git-bad-behavior / release-bad-behavior.
#
# Usage:
#   bash ops/ci/jankurai-audit.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

# ---- 1) Install jankurai ----------------------------------------------------
# Try the canonical git source first, then the fallback fork, then the
# crates.io publication if/when one exists. Matches the install logic
# previously inline in .github/workflows/jankurai.yml.
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
# Writes the agent-visible repo score and the repair queue, exactly the
# artifacts the tool-adoption manifest names for audit-ci, proof-routing,
# contract-drift, authz-matrix, input-boundary, agent-tool-supply,
# release-readiness, and cost-budget. Reads cost-budget config from
# agent/cost-budget.toml.
step_audit_advisory() {
    mkdir -p target/jankurai
    jankurai audit . \
        --mode advisory \
        --baseline agent/repo-score.json \
        --json agent/repo-score.json \
        --md agent/repo-score.md \
        --sarif target/jankurai/jankurai.sarif \
        --github-step-summary target/jankurai/summary.md \
        --repair-queue-jsonl target/jankurai/repair-queue.jsonl
}

# ---- 4) Fetch reviewed accepted baseline -----------------------------------
# Source baseline strictly from reviewed locations: a committed baseline
# under agent/baselines/ takes priority, otherwise we pull the score
# from origin/main (the previously reviewed state). Never seed from the
# candidate audit run; that would hide score regressions
# (HLT-034 ci.ratchet.self-generated-baseline).
step_fetch_baseline() {
    mkdir -p target/jankurai
    if [ -f agent/baselines/accepted-baseline.json ]; then
        install -m 0644 agent/baselines/accepted-baseline.json target/jankurai/accepted-baseline.json
        echo "baseline sourced from agent/baselines/accepted-baseline.json"
    else
        git show origin/main:agent/repo-score.json > target/jankurai/accepted-baseline.json
        echo "baseline sourced from origin/main"
    fi
}

# ---- 5) jankurai security run (strict, pre-audit) --------------------------
# Canonical CI invocation for the `security` tool-adoption entry. Runs
# with --strict in the ci profile BEFORE the final ratchet audit so
# security evidence is binding (HLT-042 ci-bad-behavior).
step_security_run() {
    mkdir -p target/jankurai/security
    jankurai security run . \
        --strict \
        --profile ci \
        --out target/jankurai/security/evidence.json
}

# ---- 6) jankurai audit (ratchet) — tool-adoption CI evidence ---------------
# Canonical CI invocation referenced by the tool-adoption manifest.
# Produces agent/repo-score.json + agent/repo-score.md evidence used by
# audit-ci, proof-routing, contract-drift, authz-matrix, input-boundary,
# agent-tool-supply, release-readiness, cost-budget.
step_audit_ratchet() {
    jankurai audit . \
        --mode ratchet \
        --baseline target/jankurai/accepted-baseline.json \
        --json target/jankurai/repo-score.json \
        --md target/jankurai/repo-score.md
}

# ---- 7) jankurai doctor ----------------------------------------------------
# Surfaces critical drift between local and CI environments.
step_doctor() {
    jankurai doctor --fail-on critical
}

# ---- 8) Proofbind verify ---------------------------------------------------
step_proofbind() {
    jankurai proofbind verify . --changed-from origin/main
}

# ---- 9) Proofmark rust -----------------------------------------------------
step_proofmark() {
    jankurai proofmark rust . --obligations target/jankurai/proofbind/obligations.json
}

# ---- 10) Rust witness build ------------------------------------------------
step_rust_witness() {
    jankurai rust witness build .
}

# ---- 11) UX QA smoke -------------------------------------------------------
step_ux_qa() {
    jankurai ux audit --config agent/ux-qa.toml --out target/jankurai/ux-qa.json
}

# ---- 12) Language bad-behavior tests ---------------------------------------
# Canonical CI invocation for the ci-bad-behavior, git-bad-behavior, and
# release-bad-behavior tool-adoption entries:
#   cargo test -p jankurai --test language_bad_behavior
# Run against the upstream jankurai source (jankurai is not a workspace
# member here) and capture the output as the canonical evidence artifact
# target/jankurai/language-bad-behavior.log.
#
# Hard gate (HLT-042): the previous CI step was `continue-on-error: true`.
# We instead exit 0 here ONLY when upstream is unreachable, and write a
# machine-grep-able `status: upstream-clone-failed` line to the log so the
# soft-gate semantics are explicit and the artifact is never silently
# empty. Any other failure (e.g. test failure when the clone DID succeed)
# propagates as a non-zero exit and fails the CI lane.
step_language_bad_behavior() {
    mkdir -p target/jankurai
    rm -rf target/jankurai-src

    local cloned=0
    if git clone --depth 1 "${CI_JANKURAI_GIT}.git" target/jankurai-src \
        || git clone --depth 1 https://github.com/anthropics/jankurai.git target/jankurai-src; then
        cloned=1
    fi

    if [ "${cloned}" -eq 1 ] && [ -d target/jankurai-src ]; then
        # Upstream available: run the canonical test and propagate its
        # exit status. `tee` is in a subshell so PIPESTATUS works.
        local rc=0
        ( cd target/jankurai-src && cargo test -p jankurai --test language_bad_behavior --no-fail-fast ) \
            > >(tee target/jankurai/language-bad-behavior.log) 2>&1 || rc=$?
        printf 'status: %s\n' "$( [ "$rc" -eq 0 ] && echo upstream-tests-passed || echo upstream-tests-failed )" \
            >> target/jankurai/language-bad-behavior.log
        return "$rc"
    fi

    # Upstream source unavailable in this network: record an explicit
    # `status: upstream-clone-failed` marker so the audit sees the
    # canonical evidence path AND the soft-gate reason. Exit 0 so the
    # CI step succeeds without `continue-on-error: true`.
    printf 'attempted: cargo test -p jankurai --test language_bad_behavior\nstatus: upstream-clone-failed\n' \
        | tee target/jankurai/language-bad-behavior.log
    return 0
}

main() {
    step_install_jankurai
    step_version
    step_audit_advisory
    step_fetch_baseline
    step_security_run
    step_audit_ratchet
    step_doctor
    step_proofbind
    step_proofmark
    step_rust_witness
    step_ux_qa
    step_language_bad_behavior
}

main "$@"
