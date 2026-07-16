#!/usr/bin/env bash
# Jankurai audit lane: the canonical CI + local invocation for every
# tool-adoption-manifest entry rooted in the governed Jankurai audit.
#
# Mirrors `.github/workflows/jankurai.yml`'s `audit` job (steps that used
# to live inline as "Install jankurai" through "Language bad-behavior
# tests"). Sourcing this script from `scripts/ci-local.sh audit` gives
# the same evidence locally and in CI. Audit references:
# HLT-042 ci-local-parity.lib-missing,
# HLT-034 ci-bad-behavior.
#
# Jankurai is a hard dependency for this lane. ops/ci/lib.sh verifies the
# governed installation and its content-addressed receipt before execution.
#
# Usage:
#   bash ops/ci/jankurai-audit.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

LOG_DIR=".jankurai"
TARGET_DIR="target/jankurai"
AUDIT_POLICY="agent/audit-policy.toml"
mkdir -p "$LOG_DIR" "$LOG_DIR/security" "$LOG_DIR/proofbind" "$LOG_DIR/proofmark" "$LOG_DIR/rust"
mkdir -p "$TARGET_DIR/security" "$TARGET_DIR/proofmark" "$TARGET_DIR/rust"
JANKURAI_IDENTITY_LOG="$LOG_DIR/jankurai-identity.log"

force_full_smart_scan() {
    # CI jobs start with an empty target directory, but local mirrors often
    # retain smart-scan state from earlier audits. Removing it preserves the
    # canonical command string while forcing a full evidence scan.
    rm -f target/jankurai/audit-state.json
}

cleanup_jankurai_upstream_scratch() {
    rm -rf .jankurai/jankurai-src target/jankurai-src-v*
    if [ -f .jankurai/security/sbom-syft.json ] \
        && grep -Eq '/(\.jankurai/jankurai-src|target/jankurai-src-v)' .jankurai/security/sbom-syft.json
    then
        rm -f .jankurai/security/sbom-syft.json
    fi
}
trap cleanup_jankurai_upstream_scratch EXIT

# ---- 1) jankurai --version --------------------------------------------------
step_version() {
    run_governed_jankurai --version
}

# ---- 2) Governed Jankurai audit (advisory) -------------------------------
# Writes the repo score and the repair queue, exactly the
# artifacts the tool-adoption manifest names for audit-ci, proof-routing,
# contract-drift, authz-matrix, input-boundary, agent-tool-supply,
# release-readiness, and cost-budget. Reads cost-budget config from
# .jankurai/cost-budget.toml.
step_audit_advisory() {
    bash scripts/check_audit_policy_mirror.sh
    cleanup_jankurai_upstream_scratch
    force_full_smart_scan
    local -a audit_cmd=(run_governed_jankurai audit . --mode advisory --baseline .jankurai/repo-score.json --json .jankurai/repo-score.json --md .jankurai/repo-score.md --sarif .jankurai/jankurai.sarif --github-step-summary .jankurai/summary.md --repair-queue-jsonl target/jankurai/repair-queue.jsonl --policy agent/audit-policy.toml)
    "${audit_cmd[@]}"
}

# ---- 3) Fetch reviewed accepted baseline -----------------------------------
# Source baseline strictly from reviewed locations: a committed baseline
# under .jankurai/baselines/ takes priority, otherwise we pull the score
# from origin/main (the previously reviewed state). Never seed from the
# candidate audit run; that would hide score regressions
# (HLT-034 ci.ratchet.self-generated-baseline).
step_fetch_baseline() {
    if [ -f .jankurai/baselines/accepted-baseline.json ]; then
        install -m 0644 .jankurai/baselines/accepted-baseline.json "$TARGET_DIR/accepted-baseline.json"
        echo "baseline sourced from .jankurai/baselines/accepted-baseline.json"
    else
        git show origin/main:.jankurai/repo-score.json > "$TARGET_DIR/accepted-baseline.json"
        echo "baseline sourced from origin/main"
    fi
}

# ---- 4) Governed Jankurai security run (strict, pre-audit) ---------------
# Canonical CI invocation for the `security` tool-adoption entry. Runs
# with --strict in the ci profile BEFORE the final ratchet audit so
# security evidence is binding (HLT-034 ci-bad-behavior).
step_security_run() {
    cleanup_jankurai_upstream_scratch
    run_governed_jankurai security run . \
        --strict \
        --profile ci \
        --out "$TARGET_DIR/security/evidence.json"
}

# ---- 5) Governed Jankurai audit (ratchet) — tool-adoption CI evidence ----
step_audit_ratchet() {
    local rc=0
    bash scripts/check_audit_policy_mirror.sh
    cleanup_jankurai_upstream_scratch
    force_full_smart_scan
    local -a ratchet_cmd=(run_governed_jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md --policy agent/audit-policy.toml)
    "${ratchet_cmd[@]}" || rc=$?

    if [ "$rc" -eq 0 ]; then
        return 0
    fi

    if bash tools/evidence-processor/run.sh \
        jankurai-ratchet "$TARGET_DIR/repo-score.json"
    then
        printf 'jankurai ratchet accepted: no score drop, new caps, or new hard findings vs baseline\n'
        return 0
    fi

    return "$rc"
}

# ---- 6) Governed Jankurai doctor -----------------------------------------
step_doctor() {
    run_governed_jankurai doctor --fail-on critical
}

# ---- 7) Proofbind verify ---------------------------------------------------
step_proofbind() {
    local -a changed_paths=()
    local path

    while IFS= read -r -d '' path; do
        changed_paths+=(--changed "$path")
    done < <(git diff --name-only -z --diff-filter=ACMRT origin/main...HEAD --)

    run_governed_jankurai proofbind verify . "${changed_paths[@]}"
}

# ---- 8) Proofmark rust -----------------------------------------------------
step_proofmark() {
    if [ ! -e Cargo.toml ] && [ ! -e Cargo.lock ]; then
        printf '%s\n' \
            '{"status":"not_applicable","reason":"thin hub has no Rust source"}' \
            > "$TARGET_DIR/proofmark/not-applicable.json"
        return 0
    fi
    if [ ! -f Cargo.toml ] || [ ! -f Cargo.lock ]; then
        printf 'incomplete Rust dependency graph: Cargo.toml and Cargo.lock must be present together\n' >&2
        return 1
    fi
    run_governed_jankurai proofmark rust . --obligations "$TARGET_DIR/proofbind/obligations.json"
}

# ---- 9) Rust witness build -------------------------------------------------
step_rust_witness() {
    if [ ! -e Cargo.toml ] && [ ! -e Cargo.lock ]; then
        printf '%s\n' \
            '{"status":"not_applicable","reason":"thin hub has no Rust source"}' \
            > "$TARGET_DIR/rust/not-applicable.json"
        return 0
    fi
    if [ ! -f Cargo.toml ] || [ ! -f Cargo.lock ]; then
        printf 'incomplete Rust dependency graph: Cargo.toml and Cargo.lock must be present together\n' >&2
        return 1
    fi
    run_governed_jankurai rust witness build .
}

# ---- 10) Copy-code audit ---------------------------------------------------
step_copy_code() {
    run_governed_jankurai copy-code . --json "$LOG_DIR/copy-code.json" --md "$LOG_DIR/copy-code.md"
}

# ---- 11) UX QA smoke -------------------------------------------------------
step_ux_qa() {
    if [ ! -f packages/ux-qa/dist/cli.js ]; then
        printf '{"status":"not_applicable","reason":"packages/ux-qa/dist/cli.js missing; no rendered web surface in this repo"}\n' \
            > "$LOG_DIR/ux-qa.json"
        return 0
    fi

    run_governed_jankurai ux audit --config .jankurai/ux-qa.toml --out "$LOG_DIR/ux-qa.json"
}

# ---- 12) Language bad-behavior tests ---------------------------------------
# Canonical CI invocation for the ci-bad-behavior, git-bad-behavior, and
# release-bad-behavior tool-adoption entries:
#   cargo test -p jankurai --test language_bad_behavior
# Run against the upstream Jankurai source (Jankurai is not a workspace
# member here) and capture the output as the canonical evidence artifact
# target/jankurai/language-bad-behavior.log.
#
# Hard gate: the workflow YAML carries NO `continue-on-error: true` for
# this step. Soft-gate semantics (upstream-clone-failed -> exit 0) live
# here, and we ALWAYS write a machine-grep-able
# `status: upstream-{clone-failed|tests-passed|tests-failed}` line.
step_language_bad_behavior() {
    local upstream_dir=".jankurai/jankurai-src"

    cleanup_jankurai_upstream_scratch

    local cloned=0
    if ci_verify_jankurai_source \
        && git clone --depth 1 --branch "$CI_JANKURAI_TAG" "$CI_JANKURAI_GIT" "$upstream_dir"
    then
        local resolved_rev
        resolved_rev="$(git -C "$upstream_dir" rev-parse HEAD)"
        if [ "$resolved_rev" != "$CI_JANKURAI_REV" ]; then
            printf 'jankurai language test clone resolved to %s, expected %s\n' \
                "$resolved_rev" "$CI_JANKURAI_REV" >&2
            cleanup_jankurai_upstream_scratch
            return 1
        fi
        cloned=1
    fi

    if [ "${cloned}" -eq 1 ] && [ -d "$upstream_dir" ]; then
        local rc=0
        local -a language_test_cmd=(cargo test -p jankurai --test language_bad_behavior --no-fail-fast)
        ( cd "$upstream_dir" && "${language_test_cmd[@]}" ) \
            > >(tee "$TARGET_DIR/language-bad-behavior.log") 2>&1 || rc=$?
        printf 'status: %s\n' "$( [ "$rc" -eq 0 ] && echo upstream-tests-passed || echo upstream-tests-failed )" \
            >> "$TARGET_DIR/language-bad-behavior.log"
        cleanup_jankurai_upstream_scratch
        # Hard gate when the clone succeeds: test failure is a real failure.
        return "$rc"
    fi

    printf 'attempted: cargo test -p jankurai --test language_bad_behavior\nstatus: upstream-clone-failed\nsoft-gate=jankurai-language-bad-behavior-local ledger=.jankurai/ci-soft-gate-ledger.toml\n' \
        | tee "$TARGET_DIR/language-bad-behavior.log"
    cleanup_jankurai_upstream_scratch
    return 0
}

main() {
    ci_require_jankurai_logged "$JANKURAI_IDENTITY_LOG"
    cleanup_jankurai_upstream_scratch

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
