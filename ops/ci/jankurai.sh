#!/usr/bin/env bash
#
# Jankurai tool-suite lane: runs the adopted jankurai tools and writes their
# evidence artifacts under target/jankurai/**. Wired as a CI job in
# .github/workflows/ci.yml and runnable locally via `scripts/ci-local.sh
# jankurai`. The exact governed 1.6.11 identity is a hard prerequisite;
# supplementary analysis failures are recorded only after that verification.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p target/jankurai

status=0
require_governed_jankurai

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

# Best-effort: logs but never reddens the lane (needs a base ref or optional
# schema that may be unavailable on some runners).
run_step_soft() {
    local name="$1"
    shift
    if [ "$STRICT_TOOLS" = "1" ]; then
        run_step "$name" "$@"
        return
    fi
    log "$name"
    if ! ci_run "$@"; then
        warn "$name failed (non-fatal supplementary evidence lane)"
    fi
}

# audit-ci / contract-drift / authz-matrix / input-boundary / agent-tool-supply
run_step "jankurai: audit" \
    bash ops/ci/run-jankurai.sh audit . \
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
    bash ops/ci/run-jankurai.sh copy-code . \
    --json target/jankurai/copy-code.json \
    --md target/jankurai/copy-code.md

run_step "jankurai: rust witness build" \
    bash ops/ci/run-jankurai.sh rust witness build .

run_step "jankurai: security evidence (strict, ci profile)" \
    bash ops/ci/run-jankurai.sh security run . \
    --strict --profile ci \
    --script ops/ci/security.sh \
    --out target/jankurai/security/evidence.json

run_step "jankurai: language bad-behavior evidence" \
    bash ops/ci/language-bad-behavior.sh

run_step "jankurai: cost budget" \
    bash ops/ci/cost-budget.sh

run_step "jankurai: release readiness" \
    bash ops/ci/release-readiness.sh

run_step_soft "jankurai: proof routing" \
    bash ops/ci/run-jankurai.sh proof . "${proofbind_changed_args[@]}" \
    --out target/jankurai/proof-routing.json \
    --md target/jankurai/proof-routing.md

run_step_soft "jankurai: proofbind verify" \
    bash ops/ci/run-jankurai.sh proofbind verify . "${proofbind_changed_args[@]}"

run_step_soft "jankurai: proofmark rust" \
    bash ops/ci/run-jankurai.sh proofmark rust . "${proofbind_changed_args[@]}" \
    --obligations target/jankurai/proofbind/obligations.json

exit "$status"
