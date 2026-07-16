#!/usr/bin/env bash
#
# Jankurai tool-suite lane: runs the adopted jankurai tools and writes their
# evidence artifacts under target/jankurai/**. Wired as a CI job in
# .github/workflows/ci.yml and runnable locally via `scripts/ci-local.sh
# jankurai`. Every proof step is release-blocking and uses the exact governed
# Jankurai identity; missing tools or optional proof inputs fail closed.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p target/jankurai

# Jankurai's tool-adoption contract names this stable CI baseline path. Copy
# the reviewed baseline verbatim so the scored command and uploaded evidence
# describe the same governed bytes.
cp .jankurai/baselines/accepted-baseline.json target/jankurai/accepted-baseline.json
cmp -s .jankurai/baselines/accepted-baseline.json target/jankurai/accepted-baseline.json \
    || fail "copied Jankurai baseline differs from the reviewed baseline"

status=0
require_jankurai

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
    jankurai audit . \
    --full \
    --mode ratchet \
    --baseline target/jankurai/accepted-baseline.json \
    --policy agent/audit-policy.toml \
    --no-score-history \
    --json target/jankurai/repo-score.json \
    --md target/jankurai/repo-score.md \
    --sarif target/jankurai/jankurai.sarif \
    --repair-queue-jsonl target/jankurai/repair-queue.jsonl

run_step "jankurai: copy-code" \
    jankurai copy-code . \
    --json target/jankurai/copy-code.json \
    --md target/jankurai/copy-code.md

run_step "jankurai: rust witness build" \
    jankurai rust witness build .

run_step "jankurai: security evidence (strict, ci profile)" \
    jankurai security run . \
    --strict --profile ci \
    --script ops/ci/security.sh \
    --out target/jankurai/security/evidence.json

run_step "jankurai: language bad-behavior evidence" \
    bash ops/ci/language-bad-behavior.sh

run_step "jankurai: cost budget" \
    bash ops/ci/cost-budget.sh

run_step "jankurai: release readiness" \
    bash ops/ci/release-readiness.sh

run_step "jankurai: proof routing" \
    jankurai proof . "${proofbind_changed_args[@]}" \
    --out target/jankurai/proof-routing.json \
    --md target/jankurai/proof-routing.md

run_step "jankurai: proofbind verify" \
    jankurai proofbind verify . "${proofbind_changed_args[@]}"

run_step "jankurai: proofmark rust" \
    jankurai proofmark rust . "${proofbind_changed_args[@]}" \
    --obligations target/jankurai/proofbind/obligations.json

exit "$status"
