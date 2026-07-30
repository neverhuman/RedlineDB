#!/usr/bin/env bash
#
# Jankurai tool-suite lane: runs the adopted jankurai tools and writes their
# evidence artifacts under target/jankurai/**. Wired as a CI job in
# .github/workflows/ci.yml and runnable locally via `scripts/ci-local.sh
# jankurai`. Strict mode requires closed, exact-head Proofbind and Proofmark
# evidence; advisory developer runs still produce the same semantic artifacts.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p \
    target/jankurai/proofbind \
    target/jankurai/proofmark \
    target/jankurai/proof-receipts/final/routed

JANKURAI="${JANKURAI_BIN:-jankurai}"
status=0

if ! has "$JANKURAI"; then
    missing_tool "$JANKURAI" "jankurai tool suite"
    log "jankurai: binary unavailable; skipping lane"
    exit 0
fi

jankurai_exact() {
    GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.abbrev GIT_CONFIG_VALUE_0=40 \
        ci_run "$JANKURAI" "$@"
}

base_ref="${JANKURAI_BASE_REF:-origin/main}"
proofbind_changed_file="target/jankurai/proofbind-existing-paths"
proofbind_changed_args=()

if ! git diff --diff-filter=d --name-only -z "${base_ref}...HEAD" >"$proofbind_changed_file"; then
    fail "cannot resolve proof paths from ${base_ref}...HEAD"
fi
if [ ! -s "$proofbind_changed_file" ]; then
    # Detached release CI intentionally runs the exact reviewed main commit, so
    # origin/main...HEAD has no delta. Verify the reviewed commit's own paths in
    # that case instead of treating a clean release snapshot as an error.
    if ! git diff-tree --no-commit-id --name-only -r -z --diff-filter=d HEAD \
        >"$proofbind_changed_file"; then
        fail "cannot resolve proof paths from reviewed commit HEAD"
    fi
fi
while IFS= read -r -d '' changed_path; do
    proofbind_changed_args+=(--changed "$changed_path")
done <"$proofbind_changed_file"
if [ "${#proofbind_changed_args[@]}" -eq 0 ]; then
    fail "proofbind has no existing changed paths to verify"
fi

run_step() {
    local name="$1"
    shift
    log "$name"
    if ! ci_run "$@"; then
        warn "$name failed"
        status=1
    fi
}

# audit-ci / contract-drift / authz-matrix / input-boundary / agent-tool-supply
run_step "jankurai: audit" \
    "$JANKURAI" audit . \
    --full \
    --mode ratchet \
    --baseline .jankurai/baselines/accepted-baseline.json \
    --policy agent/audit-policy.toml \
    --no-score-history \
    --json target/jankurai/repo-score.json \
    --md target/jankurai/repo-score.md \
    --sarif target/jankurai/jankurai.sarif \
    --repair-queue-jsonl target/jankurai/repair-queue.jsonl

run_step "jankurai: copy-code" \
    "$JANKURAI" copy-code . \
    --json target/jankurai/copy-code.json \
    --md target/jankurai/copy-code.md

run_step "jankurai: rust witness build" \
    "$JANKURAI" rust witness build .

run_step "jankurai: security evidence (strict, ci profile)" \
    "$JANKURAI" security run . \
    --strict --profile ci \
    --script ops/ci/security.sh \
    --out target/jankurai/security/evidence.json

run_step "jankurai: language bad-behavior evidence" \
    bash ops/ci/language-bad-behavior.sh

run_step "jankurai: cost budget" \
    bash ops/ci/cost-budget.sh

run_step "jankurai: release readiness" \
    bash ops/ci/release-readiness.sh

log "jankurai: proof routing"
jankurai_exact proof . "${proofbind_changed_args[@]}" \
    --out target/jankurai/proof-routing.json \
    --md target/jankurai/proof-routing.md

log "jankurai: initial Proofbind obligations"
jankurai_exact proofbind verify . "${proofbind_changed_args[@]}" \
    --mode advisory \
    --out target/jankurai/proofbind/surface-witness-initial.json \
    --obligations-out target/jankurai/proofbind/obligations-initial.json \
    --md target/jankurai/proofbind/proofbind-initial.md

ci_run bash ops/ci/jankurai-evidence-proof.sh

proofmark_mode=advisory
if [ "$STRICT_TOOLS" = "1" ]; then
    proofmark_mode=required
fi

log "jankurai: ${proofmark_mode} Proofmark"
jankurai_exact proofmark rust . "${proofbind_changed_args[@]}" \
    --mode "$proofmark_mode" \
    --obligations target/jankurai/proofbind/obligations-initial.json \
    --coverage target/jankurai/evidence-contract/lcov.info \
    --mutation target/jankurai/evidence-contract/mutation.json \
    --negative-proof HLT-023-INPUT-BOUNDARY-GAP \
    --negative-proof HLT-024-AGENT-TOOL-SUPPLY-GAP \
    --out target/jankurai/proofmark/proofmark-receipt.json \
    --proof-receipt target/jankurai/proof-receipts/final/proofmark-rust.json \
    --md target/jankurai/proofmark/proofmark.md

head_sha="$(git rev-parse --verify 'HEAD^{commit}')"
ci_run ops/ci/validate-jankurai-evidence.sh proofmark \
    target/jankurai/proof-receipts/final/proofmark-rust.json "$head_sha"

if [ "$STRICT_TOOLS" = "1" ]; then
    log "jankurai: execute every routed command"
    rm -rf -- target/jankurai/proof-receipts/final/routed
    mkdir -p target/jankurai/proof-receipts/final/routed
    REDLINE_TESTING_PROOF_CHILD=1 jankurai_exact prove . \
        --plan target/jankurai/proof-routing.json \
        --out-dir target/jankurai/proof-receipts/final/routed \
        --evidence-index target/jankurai/evidence-index.json
    # shellcheck disable=SC2016
    ci_run jq -e --arg head "$head_sha" '
        .git_head == $head
        and (.changed_paths | length) > 0
        and (.commands | length) > 0
        and (.receipts | length) == (.commands | length)
        and (.failed_receipts | type == "array" and length == 0)
        and (.risk_notes | type == "array" and length == 0)
        and (.human_approval_requirements | type == "array" and length == 0)
    ' target/jankurai/evidence-index.json >/dev/null

    log "jankurai: required Proofbind readback"
    jankurai_exact proofbind verify . "${proofbind_changed_args[@]}" \
        --mode required \
        --proof-receipts target/jankurai/proof-receipts/final \
        --out target/jankurai/proofbind/surface-witness.json \
        --obligations-out target/jankurai/proofbind/obligations.json \
        --md target/jankurai/proofbind/proofbind.md
    ci_run ops/ci/validate-jankurai-evidence.sh proofbind \
        target/jankurai/proofbind/obligations.json "$head_sha"
else
    log "jankurai: required routed receipts deferred outside strict mode"
fi

exit "$status"
